//! Cache persistant des images calculées (vignettes, aperçus standard, 1:1).
//!
//! Un fichier JPEG par entrée, rangé par niveau ; aucune base de données. La clé dépend du
//! fichier source (chemin, taille, date de modification) et des paramètres de calcul : si
//! l'un change, l'ancienne entrée n'est simplement plus trouvée. Voir `doc/en/cache.md`.
//!
//! Entrées rangées par dossier d'origine (23/10/2026) : `<niveau>/<dossier>/<clé>`, où
//! `<dossier>` est un hash du dossier de la photo. Vider le cache d'un dossier
//! ([`Cache::clear_folder`]) n'a donc besoin ni des réglages ni de la taille de l'écran
//! qui entrent dans les clés.
//!
//! Cache sur le disque des photos (24/10/2026, issue n° 55) : un dossier [`DISK_DIR`] à la
//! racine d'un disque (ou de n'importe quel dossier) y garde le cache de toutes les photos
//! en dessous, rangé par chemin *relatif* à cette racine ; la clé ne contient que le nom du
//! fichier (le dossier est dans le rangement). Branché sur un autre ordinateur, monté
//! ailleurs, le disque garde son cache.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bitmap::Bitmap;
use crate::jpeg::{self, Chroma};

/// Version du format du cache. L'incrémenter rend toutes les anciennes entrées invisibles.
/// v2 (23/10/2026) : entrées rangées par dossier d'origine. v3 (24/10/2026) : clé sans le
/// chemin du dossier (cache sur le disque des photos), source de chaque dossier notée.
/// v4 (25/10/2026) : date de modification prise au quart d'heure près ([`ZONE_STEP_NS`]).
const FORMAT_VERSION: &str = "v4";

/// Formats précédents, supprimés en fond à l'ouverture (invisibles, ils ne feraient
/// qu'occuper le disque).
const OLD_FORMATS: [&str; 3] = ["v1", "v2", "v3"];

/// Pas des fuseaux horaires (15 min, en nanosecondes). La clé ne garde de la date de
/// modification que le reste de sa division par ce pas : un disque FAT32 note ses dates en
/// heure locale, et un changement d'heure ou de fuseau les décale toutes d'un multiple de
/// ce pas (issue n° 74 : tout le cache d'une clé USB recalculé au passage à l'heure
/// d'hiver). Une vraie modification change la taille, ou la date d'autre chose qu'un
/// multiple exact de ce pas. Constante du calendrier, pas un réglage.
const ZONE_STEP_NS: u128 = 15 * 60 * 1_000_000_000;

/// Dossier caché qui garde, à la racine d'un disque, le cache des photos de ce disque
/// ([`Cache::use_disk`]).
pub const DISK_DIR: &str = ".tri-cache";

/// Fichier de chaque dossier d'entrées : le dossier de photos d'origine (chemin absolu
/// dans le cache central, relatif à la racine sur un disque), pour déplacer les entrées
/// entre les deux.
const SOURCE: &str = ".source";

/// Change à chaque [`Cache::use_disk`] / [`Cache::release_disk`] : les caches ouverts
/// oublient où ranger chaque dossier.
static DISK_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Longueur (caractères hexadécimaux) du nom du dossier d'une entrée.
const FOLDER_ID_LEN: usize = 16;

/// Qualité JPEG des entrées par défaut (voir [`Cache::with_quality`]).
pub const DEFAULT_JPEG_QUALITY: u8 = 90;

/// Niveau de détail d'une entrée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Level {
    /// Vignette de la grille.
    Thumbnail,
    /// Aperçu plein écran tiré du JPEG embarqué par le boîtier (RAW seulement) : immédiat,
    /// mais reflète les réglages du boîtier, pas les données RAW.
    Embedded,
    /// Aperçu plein écran, à la taille configurée (rendu RAW, ou image plate décodée).
    Standard,
    /// JPEG embarqué du RAW à sa taille d'origine (1:1 du JPEG boîtier), à la demande.
    EmbeddedFull,
    /// Rendu complet (zoom 100 %).
    Full,
    /// Analyse de l'exposition ([`crate::exposure::Exposure`]) : histogrammes, saturés /
    /// bouchés, masque. Entrée binaire, pas une image (voir [`Cache::get_bytes`]).
    Exposure,
    /// Score de netteté ([`crate::sharpness::Score`]). Entrée binaire.
    Sharpness,
    /// Empreinte de regroupement ([`crate::similarity::Signature`]). Entrée binaire.
    Similarity,
    /// Image développée par un logiciel externe ([`crate::develop`]), à la taille de
    /// l'aperçu standard : le JPEG du logiciel, rangé tel quel. Un niveau par logiciel :
    /// une image développée dans deux logiciels a deux rendus.
    Developed(crate::develop::Engine),
}

impl Level {
    /// Tous les niveaux.
    pub const ALL: [Level; 10] = [
        Level::Thumbnail,
        Level::Embedded,
        Level::Standard,
        Level::EmbeddedFull,
        Level::Full,
        Level::Exposure,
        Level::Sharpness,
        Level::Similarity,
        Level::Developed(crate::develop::Engine::Darktable),
        Level::Developed(crate::develop::Engine::RapidRaw),
    ];

    /// Vrai pour les niveaux qui sont des images (JPEG) ; faux pour les analyses
    /// (binaire).
    pub fn is_image(self) -> bool {
        !matches!(self, Level::Exposure | Level::Sharpness | Level::Similarity)
    }

    fn dir_name(self) -> &'static str {
        match self {
            Self::Thumbnail => "thumbnail",
            Self::Embedded => "embedded",
            Self::EmbeddedFull => "embedded-full",
            Self::Standard => "standard",
            Self::Full => "full",
            Self::Exposure => "exposure",
            Self::Sharpness => "sharpness",
            Self::Similarity => "similarity",
            // darktable garde le nom d'avant les autres logiciels : son cache reste valable.
            Self::Developed(crate::develop::Engine::Darktable) => "developed",
            Self::Developed(crate::develop::Engine::RapidRaw) => "developed-rapidraw",
        }
    }
}

/// Identifie une entrée du cache. S'obtient avec [`Key::new`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    level: Level,
    /// Dossier d'origine (absolu) : il choisit le rangement, pas la clé.
    folder: PathBuf,
    hash: String,
}

/// Taille et date de modification (ns) des fichiers sources, relevées une fois puis
/// retenues ; `None` : fichier absent.
type Stamps = HashMap<PathBuf, Option<(u64, u128)>>;

fn stamps() -> &'static Mutex<Stamps> {
    static STAMPS: std::sync::OnceLock<Mutex<Stamps>> = std::sync::OnceLock::new();
    STAMPS.get_or_init(Default::default)
}

/// Taille et date de modification (ns) de `path`, ou `None` s'il n'existe pas : relevées
/// une fois, puis retenues (retour du 29/10/2026 : sur un NAS, chaque relevé attendait
/// derrière les RAW lus par le moteur, et l'interface en faisait des milliers ; 37 s
/// figée). Un fichier changé ailleurs est à oublier ([`forget_stamps`]) : la surveillance
/// du dossier le fait, et l'ouverture d'un dossier oublie les siens.
pub fn stamp(path: &Path) -> Option<(u64, u128)> {
    lookup_stamp(path).unwrap_or(None)
}

/// Comme [`stamp`], sans jamais attendre le disque dans un fil où les relevés sont
/// différés ([`defer_stamps`]) : `None` tant que le relevé n'est pas connu (il est
/// demandé en fond, puis [`on_stamps_read`] est prévenu) ; sinon `Some` du relevé.
pub fn lookup_stamp(path: &Path) -> Option<Option<(u64, u128)>> {
    if let Some(known) = stamps().lock().unwrap_or_else(|e| e.into_inner()).get(path) {
        return Some(*known);
    }
    if DEFERRED.with(Cell::get) {
        read_in_background(path);
        return None;
    }
    Some(read_stamp(path))
}

thread_local! {
    /// Relevés différés dans ce fil ([`defer_stamps`]).
    static DEFERRED: Cell<bool> = const { Cell::new(false) };
}

/// Diffère (`on`) les relevés manquants dans ce fil : le fil de l'interface ne doit
/// jamais attendre un NAS (retour du 29/10/2026 : encore 0,4 à 3 s par image, pour les
/// premiers relevés de chaque vignette).
pub fn defer_stamps(on: bool) {
    DEFERRED.with(|d| d.set(on));
}

/// Fonction appelée quand des relevés différés sont arrivés (redessiner l'interface).
pub fn on_stamps_read(notify: impl Fn() + Send + Sync + 'static) {
    let _ = NOTIFY.set(Box::new(notify));
}

static NOTIFY: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

/// Relevé demandé au fil des relevés différés (une fois par fichier en attente).
fn read_in_background(path: &Path) {
    use std::sync::OnceLock;
    use std::sync::mpsc::{self, Sender};
    static PENDING: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    static QUEUE: OnceLock<Mutex<Sender<PathBuf>>> = OnceLock::new();
    let pending = PENDING.get_or_init(Default::default);
    if !pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(path.to_path_buf())
    {
        return;
    }
    let queue = QUEUE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let _ = std::thread::Builder::new()
            .name("tri-stamps".into())
            .spawn(move || {
                while let Ok(path) = rx.recv() {
                    read_stamp(&path);
                    pending
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&path);
                    if let Some(notify) = NOTIFY.get() {
                        notify();
                    }
                }
            });
        Mutex::new(tx)
    });
    let _ = queue
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .send(path.to_path_buf());
}

/// Relève `path` sur le disque et le retient.
fn read_stamp(path: &Path) -> Option<(u64, u128)> {
    let read = fs::metadata(path).ok().map(|meta| {
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        (meta.len(), modified)
    });
    stamps()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(path.to_path_buf(), read);
    read
}

/// Oublie les relevés de `paths` ([`stamp`]) : fichiers changés, ajoutés ou retirés.
pub fn forget_stamps<'a>(paths: impl IntoIterator<Item = &'a Path>) {
    let mut known = stamps().lock().unwrap_or_else(|e| e.into_inner());
    for path in paths {
        known.remove(path);
    }
}

/// Oublie les relevés des fichiers sous `dir` (dossier ouvert ou rouvert).
pub fn forget_stamps_under(dir: &Path) {
    stamps()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|path, _| !path.starts_with(dir));
}

impl Key {
    /// Clé pour l'image `source` au niveau `level`, calculée avec les paramètres `params`
    /// (description libre et stable, ex. `"half=1;side=2560"`).
    ///
    /// D'après la taille et la date de modification du fichier ([`stamp`], relevées une
    /// fois) : erreur si `source` n'existe pas. Le chemin est rendu absolu, pour qu'un même
    /// fichier ait la même clé quel que soit le répertoire courant.
    pub fn new(source: &Path, level: Level, params: &str) -> io::Result<Key> {
        let path = std::path::absolute(source)?;
        let (len, modified) = match lookup_stamp(&path) {
            Some(Some(stamp)) => stamp,
            Some(None) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{} introuvable", path.display()),
                ));
            }
            // Fil de l'interface : relevé demandé en fond, clé à redemander.
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!("{} pas encore relevé", path.display()),
                ));
            }
        };
        Ok(Key::from_parts(
            path.as_os_str(),
            len,
            modified,
            level,
            params,
        ))
    }

    fn from_parts(
        path: &std::ffi::OsStr,
        size: u64,
        modified_ns: u128,
        level: Level,
        params: &str,
    ) -> Key {
        let mut h = blake3::Hasher::new();
        // Chaque champ est préfixé par sa longueur : pas d'ambiguïté entre (« ab », « c ») et
        // (« a », « bc »).
        let mut field = |bytes: &[u8]| {
            h.update(&(bytes.len() as u64).to_le_bytes());
            h.update(bytes);
        };
        field(FORMAT_VERSION.as_bytes());
        // Le nom seul : le dossier est dans le rangement (cache déplaçable sur un disque).
        let path = Path::new(path);
        field(path.file_name().unwrap_or_default().as_encoded_bytes());
        field(&size.to_le_bytes());
        field(&(modified_ns % ZONE_STEP_NS).to_le_bytes());
        field(level.dir_name().as_bytes());
        field(params.as_bytes());
        Key {
            level,
            folder: path.parent().unwrap_or(Path::new("")).to_path_buf(),
            hash: h.finalize().to_hex().to_string(),
        }
    }

    /// Niveau de l'entrée.
    pub fn level(&self) -> Level {
        self.level
    }
}

/// Où garder le cache d'un dossier ouvert dont le cache est encore chez l'utilisateur
/// (`[cache] location`, issue n° 55).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Location {
    /// Cache de l'utilisateur ([`default_dir`]) : photos qui ne bougent pas.
    #[default]
    User,
    /// À la racine du disque amovible du dossier (disque de transfert) ; ailleurs, chez
    /// l'utilisateur.
    Disk,
    /// Dans le dossier lui-même (dossier qu'on déplace).
    Folder,
}

/// Racine où ranger le cache de `folder` (absolu) selon `location` ; `removable` : racines
/// des disques amovibles. `None` : chez l'utilisateur.
pub fn wanted_root(location: Location, folder: &Path, removable: &[PathBuf]) -> Option<PathBuf> {
    match location {
        Location::User => None,
        Location::Folder => Some(folder.to_path_buf()),
        Location::Disk => removable
            .iter()
            .filter(|root| folder.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned(),
    }
}

/// Génération et racine du cache sur disque de chaque dossier (voir [`Cache::disk_of`]).
type DiskMemo = (u64, HashMap<PathBuf, Option<PathBuf>>);

/// Cache sur disque, enraciné dans un dossier. Ne garde aucun état en mémoire : plusieurs
/// threads (ou processus) peuvent utiliser le même dossier en même temps.
#[derive(Clone, Debug)]
pub struct Cache {
    root: PathBuf,
    quality: u8,
    /// Racine du cache sur disque de chaque dossier déjà vu (`None` : cache central), et la
    /// génération ([`DISK_GENERATION`]) où elle a été lue.
    disks: Arc<Mutex<DiskMemo>>,
}

impl Cache {
    /// Ouvre (et crée si besoin) un cache dans `root`, qualité JPEG par défaut.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Cache> {
        let cache = Cache {
            root: root.into(),
            quality: DEFAULT_JPEG_QUALITY,
            disks: Arc::default(),
        };
        for level in Level::ALL {
            fs::create_dir_all(cache.level_dir(level))?;
        }
        let old: Vec<PathBuf> = OLD_FORMATS
            .iter()
            .map(|v| cache.root.join(v))
            .filter(|dir| dir.is_dir())
            .collect();
        if !old.is_empty() {
            // Peut prendre quelques secondes pour un gros cache : sans bloquer.
            std::thread::spawn(move || {
                for dir in old {
                    let _ = fs::remove_dir_all(dir);
                }
            });
        }
        Ok(cache)
    }

    /// Ouvre le cache à son emplacement par défaut (voir [`default_dir`]).
    pub fn open_default() -> io::Result<Cache> {
        let dir = default_dir().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, crate::t!("cache-no-user-dir"))
        })?;
        Cache::open(dir)
    }

    /// Même cache, avec une autre qualité JPEG (1 à 100, ramenée dans ces bornes) pour les
    /// entrées écrites désormais.
    ///
    /// La qualité n'entre pas dans la clé : les entrées déjà écrites restent valides (elles
    /// représentent la même image), il suffit de vider le cache pour les réécrire.
    pub fn with_quality(mut self, quality: u8) -> Cache {
        self.quality = quality.clamp(1, 100);
        self
    }

    /// Qualité JPEG des entrées écrites.
    pub fn quality(&self) -> u8 {
        self.quality
    }

    /// Dossier racine du cache.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn level_dir(&self, level: Level) -> PathBuf {
        self.root.join(FORMAT_VERSION).join(level.dir_name())
    }

    /// Dossier des entrées des images de `folder` (absolu) au niveau `level`, et sa source
    /// (voir [`SOURCE`]). Un sous-dossier par dossier d'origine : vidage par dossier, et
    /// pas des dizaines de milliers de fichiers dans un même dossier.
    fn shard(&self, level: Level, folder: &Path) -> (PathBuf, String) {
        match self.disk_of(folder) {
            Some(disk) => {
                let relative = relative_name(folder.strip_prefix(&disk).unwrap_or(Path::new("")));
                let dir = disk_level_dir(&disk, level).join(folder_id(relative.as_ref()));
                (dir, relative)
            }
            None => (
                self.level_dir(level).join(folder_id(folder)),
                folder.to_string_lossy().into_owned(),
            ),
        }
    }

    /// Racine du cache sur disque qui garde les images de `folder` (absolu) : le plus proche
    /// dossier parent (ou `folder` lui-même) qui contient [`DISK_DIR`] ; `None` : cache
    /// central. Gardé en mémoire par dossier.
    pub fn disk_of(&self, folder: &Path) -> Option<PathBuf> {
        let generation = DISK_GENERATION.load(Ordering::Relaxed);
        let mut disks = self.disks.lock().unwrap_or_else(|e| e.into_inner());
        if disks.0 != generation {
            *disks = (generation, HashMap::new());
        }
        disks
            .1
            .entry(folder.to_path_buf())
            .or_insert_with(|| {
                folder
                    .ancestors()
                    .find(|a| a.join(DISK_DIR).is_dir())
                    .map(Path::to_path_buf)
            })
            .clone()
    }

    /// Garde désormais dans `root` (racine d'un disque amovible, ou un dossier qu'on veut
    /// pouvoir déplacer) le cache des photos qui sont dessous : crée [`DISK_DIR`] et y
    /// déplace les entrées déjà calculées (cache central, ou cache sur disque plus haut).
    /// Renvoie ce qui a été déplacé.
    pub fn use_disk(&self, root: &Path) -> io::Result<Usage> {
        let root = std::path::absolute(root)?;
        let holder = self.disk_of(&root);
        if holder.as_deref() == Some(root.as_path()) {
            return Ok(Usage::default());
        }
        let mut shards = self.shards_under(&self.root, None, &root);
        if let Some(holder) = &holder {
            shards.extend(self.shards_under(&holder.join(DISK_DIR), Some(holder), &root));
        }
        fs::create_dir_all(root.join(DISK_DIR).join(FORMAT_VERSION))?;
        DISK_GENERATION.fetch_add(1, Ordering::Relaxed);
        self.rehome(shards, Path::to_path_buf)
    }

    /// Applique `location` à `folder` (dossier ouvert) si son cache est encore chez
    /// l'utilisateur ; un choix déjà fait (un [`DISK_DIR`] au-dessus) est respecté.
    /// Renvoie la racine choisie et ce qui y a été déplacé ; `None` : rien à faire.
    pub fn apply(
        &self,
        location: Location,
        folder: &Path,
        removable: &[PathBuf],
    ) -> io::Result<Option<(PathBuf, Usage)>> {
        let folder = std::path::absolute(folder)?;
        if self.disk_of(&folder).is_some() {
            return Ok(None);
        }
        match wanted_root(location, &folder, removable) {
            Some(root) => Ok(Some((root.clone(), self.use_disk(&root)?))),
            None => Ok(None),
        }
    }

    /// Ne garde plus le cache dans `root` ([`Cache::use_disk`]) : ses entrées vont où ses
    /// photos les attendent désormais (cache sur disque plus haut, sinon cache central),
    /// puis [`DISK_DIR`] est supprimé. Renvoie ce qui a été déplacé.
    pub fn release_disk(&self, root: &Path) -> io::Result<Usage> {
        let root = std::path::absolute(root)?;
        let marker = root.join(DISK_DIR);
        if !marker.is_dir() {
            return Ok(Usage::default());
        }
        // Mis de côté d'abord : le dossier ne compte plus comme cache pendant le
        // déplacement.
        let aside = root.join(format!("{DISK_DIR}-{}", std::process::id()));
        fs::rename(&marker, &aside)?;
        DISK_GENERATION.fetch_add(1, Ordering::Relaxed);
        let shards = self.shards_under(&aside, Some(&root), &root);
        let moved = self.rehome(shards, Path::to_path_buf)?;
        fs::remove_dir_all(&aside)?;
        Ok(moved)
    }

    /// Après le déplacement (ou le renommage) du dossier `from` en `to` : ses entrées et
    /// celles de ses sous-dossiers suivent (sinon, recalculées). Un cache gardé dans le
    /// dossier lui-même est déjà parti avec lui.
    pub fn relocate(&self, from: &Path, to: &Path) -> io::Result<Usage> {
        let (from, to) = (std::path::absolute(from)?, std::path::absolute(to)?);
        let mut shards = self.shards_under(&self.root, None, &from);
        if let Some(holder) = self.disk_of(&from)
            && !holder.starts_with(&from)
        {
            shards.extend(self.shards_under(&holder.join(DISK_DIR), Some(&holder), &from));
        }
        self.rehome(shards, |folder| {
            to.join(folder.strip_prefix(&from).unwrap_or(Path::new("")))
        })
    }

    /// Dossiers d'entrées du rangement `store` (le cache central, ou le [`DISK_DIR`] d'un
    /// disque dont `base` est la racine) qui gardent les photos de `under` ou d'un de ses
    /// sous-dossiers : (niveau, dossier d'entrées, dossier de photos).
    fn shards_under(
        &self,
        store: &Path,
        base: Option<&Path>,
        under: &Path,
    ) -> Vec<(Level, PathBuf, PathBuf)> {
        let mut found = Vec::new();
        for level in Level::ALL {
            let dir = store.join(FORMAT_VERSION).join(level.dir_name());
            let Ok(shards) = fs::read_dir(dir) else {
                continue;
            };
            for shard in shards.flatten() {
                let Ok(source) = fs::read_to_string(shard.path().join(SOURCE)) else {
                    continue;
                };
                let source = source.trim_end();
                let folder = match base {
                    Some(base) if source.is_empty() => base.to_path_buf(),
                    // Écrit avec des « / » (`relative_name`) : recollé composant par
                    // composant, sinon un chemin Windows mélangerait « \ » et « / ».
                    Some(base) => source.split('/').fold(base.to_path_buf(), |p, c| p.join(c)),
                    None => PathBuf::from(source),
                };
                if folder.starts_with(under) {
                    found.push((level, shard.path(), folder));
                }
            }
        }
        found
    }

    /// Range à nouveau des dossiers d'entrées ([`Cache::shards_under`]) là où leurs photos,
    /// au nouveau chemin donné par `place`, les attendent.
    fn rehome(
        &self,
        shards: Vec<(Level, PathBuf, PathBuf)>,
        place: impl Fn(&Path) -> PathBuf,
    ) -> io::Result<Usage> {
        let mut moved = Usage::default();
        for (level, shard, folder) in shards {
            let (to, source) = self.shard(level, &place(&folder));
            if to != shard {
                moved.add(move_entries(&shard, &to, &source)?);
            }
        }
        Ok(moved)
    }

    fn entry_path(&self, key: &Key) -> PathBuf {
        self.shard(key.level, &key.folder).0.join(format!(
            "{}.{}",
            key.hash,
            if key.level.is_image() { "jpg" } else { "bin" }
        ))
    }

    /// Vrai si l'entrée existe (sans la décoder).
    pub fn contains(&self, key: &Key) -> bool {
        self.entry_path(key).is_file()
    }

    /// Lit une entrée. `None` si elle n'existe pas. Une entrée illisible ou corrompue est
    /// supprimée et donne `None` : elle sera recalculée.
    ///
    /// Pour un 1:1, une lecture réussie met à jour la date de modification du fichier, qui
    /// sert de date de dernier usage pour [`Cache::evict`].
    pub fn get(&self, key: &Key) -> Option<Bitmap> {
        let path = self.entry_path(key);
        let bytes = fs::read(&path).ok()?;
        let Ok(bitmap) = jpeg::decode(&bytes, 0) else {
            let _ = fs::remove_file(&path);
            return None;
        };
        touch(key.level, &path);
        Some(bitmap)
    }

    /// Écrit une entrée (JPEG). L'écriture est atomique : fichier temporaire puis
    /// renommage, donc un lecteur ne voit jamais d'entrée à moitié écrite, même si le
    /// programme est interrompu.
    pub fn put(&self, key: &Key, bitmap: &Bitmap) -> io::Result<()> {
        // Couleur à pleine résolution pour le 1:1, 4:2:0 ailleurs (plus compact).
        let chroma = match key.level {
            Level::Full | Level::EmbeddedFull => Chroma::Full,
            _ => Chroma::Half,
        };
        let bytes = jpeg::encode(bitmap, self.quality, chroma).map_err(io::Error::other)?;
        self.write_entry(key, &bytes)
    }

    /// Lit une entrée binaire (analyse), telle qu'écrite par [`Cache::put_bytes`]. `None`
    /// si elle n'existe pas.
    pub fn get_bytes(&self, key: &Key) -> Option<Vec<u8>> {
        let path = self.entry_path(key);
        let bytes = fs::read(&path).ok()?;
        touch(key.level, &path);
        Some(bytes)
    }

    /// Écrit une entrée binaire (analyse), atomiquement comme [`Cache::put`].
    pub fn put_bytes(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        self.write_entry(key, bytes)
    }

    /// Écriture atomique d'une entrée : fichier temporaire, puis renommage.
    fn write_entry(&self, key: &Key, bytes: &[u8]) -> io::Result<()> {
        let path = self.entry_path(key);
        let dir = path
            .parent()
            .expect("une entrée a toujours un dossier parent");
        fs::create_dir_all(dir)?;
        let source = dir.join(SOURCE);
        if !source.exists() {
            fs::write(&source, self.shard(key.level, &key.folder).1)?;
        }
        let tmp = temp_path(&path);
        let result = (|| {
            let mut file = File::create(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&tmp, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    /// Supprime une entrée (sans erreur si elle n'existe pas).
    pub fn remove(&self, key: &Key) -> io::Result<()> {
        match fs::remove_file(self.entry_path(key)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Vide un niveau, ou tout le cache si `level` vaut `None`.
    pub fn clear(&self, level: Option<Level>) -> io::Result<()> {
        let levels = level.map_or(Level::ALL.to_vec(), |l| vec![l]);
        for level in levels {
            let dir = self.level_dir(level);
            match fs::remove_dir_all(&dir) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => fs::create_dir_all(&dir)?,
            }
        }
        Ok(())
    }

    /// Supprime les entrées des images du dossier `folder` (pas de ses sous-dossiers), à
    /// un niveau ou à tous si `level` vaut `None`, quels que soient les réglages avec
    /// lesquels elles ont été calculées. Renvoie l'espace libéré.
    pub fn clear_folder(&self, folder: &Path, level: Option<Level>) -> io::Result<Usage> {
        let folder = std::path::absolute(folder)?;
        let levels = level.map_or(Level::ALL.to_vec(), |l| vec![l]);
        let mut freed = Usage::default();
        for level in levels {
            let dir = self.shard(level, &folder).0;
            let entries = match fs::read_dir(&dir) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                result => result?,
            };
            for file in entries {
                let file = file?;
                if file
                    .path()
                    .extension()
                    .is_some_and(|e| e == "jpg" || e == "bin")
                {
                    freed.bytes += file.metadata()?.len();
                    freed.entries += 1;
                }
            }
            match fs::remove_dir_all(&dir) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        Ok(freed)
    }

    /// Taille totale (octets) et nombre d'entrées d'un niveau.
    pub fn usage(&self, level: Level) -> io::Result<Usage> {
        let entries = self.entries(level)?;
        Ok(Usage {
            bytes: entries.iter().map(|e| e.bytes).sum(),
            entries: entries.len(),
        })
    }

    /// Supprime les entrées les moins récemment utilisées d'un niveau jusqu'à ce qu'il pèse
    /// au plus `max_bytes`. Renvoie l'espace libéré.
    pub fn evict(&self, level: Level, max_bytes: u64) -> io::Result<Usage> {
        let mut entries = self.entries(level)?;
        let mut total: u64 = entries.iter().map(|e| e.bytes).sum();
        entries.sort_by_key(|e| e.used);
        let mut freed = Usage::default();
        for entry in entries {
            if total <= max_bytes {
                break;
            }
            match fs::remove_file(&entry.path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
            total -= entry.bytes;
            freed.bytes += entry.bytes;
            freed.entries += 1;
        }
        Ok(freed)
    }

    fn entries(&self, level: Level) -> io::Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for shard in fs::read_dir(self.level_dir(level))? {
            let shard = shard?;
            if !shard.file_type()?.is_dir() {
                continue;
            }
            for file in fs::read_dir(shard.path())? {
                let file = file?;
                let path = file.path();
                if path.extension().is_none_or(|e| e != "jpg" && e != "bin") {
                    continue; // fichiers temporaires en cours d'écriture
                }
                let meta = file.metadata()?;
                entries.push(Entry {
                    path,
                    bytes: meta.len(),
                    used: meta.modified().unwrap_or(UNIX_EPOCH),
                });
            }
        }
        Ok(entries)
    }
}

/// Occupation d'un niveau du cache, ou espace libéré par [`Cache::evict`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Octets.
    pub bytes: u64,
    /// Nombre d'entrées.
    pub entries: usize,
}

impl Usage {
    fn add(&mut self, other: Usage) {
        self.bytes += other.bytes;
        self.entries += other.entries;
    }
}

/// Dossier d'un niveau dans le cache du disque `root`.
fn disk_level_dir(root: &Path, level: Level) -> PathBuf {
    root.join(DISK_DIR)
        .join(FORMAT_VERSION)
        .join(level.dir_name())
}

/// Chemin relatif écrit avec des « / » quel que soit le système : le même dossier d'un
/// disque a le même rangement sous macOS, Linux et Windows.
fn relative_name(relative: &Path) -> String {
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Déplace les entrées du dossier `from` dans `to` (créé, sa source notée), puis supprime
/// `from`. Vers un autre disque, copie puis suppression ; une entrée déjà présente dans
/// `to` (calculée pendant le déplacement) est gardée.
fn move_entries(from: &Path, to: &Path, source: &str) -> io::Result<Usage> {
    fs::create_dir_all(to)?;
    fs::write(to.join(SOURCE), source)?;
    let mut moved = Usage::default();
    for file in fs::read_dir(from)?.flatten() {
        let path = file.path();
        if path.extension().is_none_or(|e| e != "jpg" && e != "bin") {
            continue;
        }
        let target = to.join(file.file_name());
        if !target.exists() {
            let bytes = file.metadata()?.len();
            if fs::rename(&path, &target).is_err() {
                fs::copy(&path, &target)?;
            }
            moved.bytes += bytes;
            moved.entries += 1;
        }
    }
    fs::remove_dir_all(from)?;
    Ok(moved)
}

struct Entry {
    path: PathBuf,
    bytes: u64,
    used: SystemTime,
}

/// Nom du sous-dossier des entrées des images du dossier `folder` (chemin absolu) : début
/// d'un hash de son chemin.
/// Date de dernier usage d'une entrée lue, pour l'éviction ([`Cache::evict`]) : les 1:1
/// seulement, seuls à être évincés. Ailleurs, une écriture par lecture ralentissait tout
/// sur un disque amovible (issue n° 74 : clé USB, écritures vidées aussitôt sous Linux).
fn touch(level: Level, path: &Path) {
    if matches!(level, Level::Full | Level::EmbeddedFull)
        && let Ok(file) = File::options().write(true).open(path)
    {
        let _ = file.set_modified(SystemTime::now());
    }
}

fn folder_id(folder: &Path) -> String {
    let mut h = blake3::Hasher::new();
    h.update(folder.as_os_str().as_encoded_bytes());
    h.finalize().to_hex()[..FOLDER_ID_LEN].to_string()
}

/// Nom de fichier temporaire unique (processus + compteur), dans le même dossier que `path`
/// pour que le renommage final reste atomique.
fn temp_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{}-{n}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// Emplacement par défaut du cache, selon le système :
/// - Linux et autres Unix : `$XDG_CACHE_HOME/tri` (si absolu), sinon `~/.cache/tri` ;
/// - macOS : `~/Library/Caches/tri` ;
/// - Windows : `%LOCALAPPDATA%\tri`.
///
/// `None` si les variables d'environnement nécessaires sont absentes.
pub fn default_dir() -> Option<PathBuf> {
    default_dir_for(std::env::consts::OS, |name| std::env::var_os(name))
}

/// Logique de [`default_dir`], avec l'OS et l'environnement injectés (pour les tests).
fn default_dir_for(os: &str, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let base = match os {
        "windows" => absolute("LOCALAPPDATA")?,
        "macos" => absolute("HOME")?.join("Library").join("Caches"),
        _ => absolute("XDG_CACHE_HOME").or_else(|| Some(absolute("HOME")?.join(".cache")))?,
    };
    Some(base.join("tri"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Les tests qui s'en servent comparent des chemins Unix : pas sous Windows.
    #[cfg(not(windows))]
    fn env<'a>(vars: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn default_dir_per_os() {
        let home = [("HOME", "/home/u")];
        assert_eq!(
            default_dir_for("linux", env(&home)),
            Some("/home/u/.cache/tri".into())
        );
        let xdg = [("HOME", "/home/u"), ("XDG_CACHE_HOME", "/data/cache")];
        assert_eq!(
            default_dir_for("linux", env(&xdg)),
            Some("/data/cache/tri".into())
        );
        assert_eq!(
            default_dir_for("macos", env(&home)),
            Some("/home/u/Library/Caches/tri".into())
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn default_dir_ignores_relative_or_missing_vars() {
        // XDG relatif : ignoré (spécification XDG), on retombe sur HOME.
        let rel = [("HOME", "/home/u"), ("XDG_CACHE_HOME", "cache")];
        assert_eq!(
            default_dir_for("linux", env(&rel)),
            Some("/home/u/.cache/tri".into())
        );
        assert_eq!(default_dir_for("linux", env(&[])), None);
        assert_eq!(default_dir_for("windows", env(&[("HOME", "/h")])), None);
    }

    #[test]
    fn a_stamp_is_read_once_until_forgotten() {
        // Retour du 29/10/2026 (NAS) : un relevé par fichier, retenu, jamais un accès
        // réseau à chaque image de l'interface.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.RAF");
        assert_eq!(stamp(&file), None, "absent, retenu aussi");
        fs::write(&file, b"raw").unwrap();
        assert_eq!(stamp(&file), None, "pas relu de lui-même");
        forget_stamps([file.as_path()]);
        assert_eq!(stamp(&file).map(|(len, _)| len), Some(3));
        let before = Key::new(&file, Level::Thumbnail, "").unwrap();
        fs::write(&file, b"raw, edited").unwrap();
        assert_eq!(Key::new(&file, Level::Thumbnail, "").unwrap(), before);
        forget_stamps_under(dir.path());
        assert_ne!(
            Key::new(&file, Level::Thumbnail, "").unwrap(),
            before,
            "fichier changé : nouvelle clé une fois oublié"
        );
    }

    #[test]
    fn a_deferred_stamp_is_read_in_the_background() {
        // Le fil de l'interface n'attend jamais le disque : relevé demandé en fond.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("b.RAF");
        fs::write(&file, b"raw").unwrap();
        defer_stamps(true);
        let first = lookup_stamp(&file);
        defer_stamps(false);
        assert_eq!(first, None, "pas encore relevé");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        defer_stamps(true);
        while lookup_stamp(&file).is_none() {
            assert!(std::time::Instant::now() < deadline, "relevé en fond");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        defer_stamps(false);
        assert_eq!(stamp(&file).map(|(len, _)| len), Some(3));
    }

    #[test]
    fn key_depends_on_every_part() {
        let base = || Key::from_parts("a.RAF".as_ref(), 10, 20, Level::Standard, "p");
        let variants = [
            Key::from_parts("b.RAF".as_ref(), 10, 20, Level::Standard, "p"),
            Key::from_parts("a.RAF".as_ref(), 11, 20, Level::Standard, "p"),
            Key::from_parts("a.RAF".as_ref(), 10, 21, Level::Standard, "p"),
            Key::from_parts("a.RAF".as_ref(), 10, 20, Level::Full, "p"),
            Key::from_parts("a.RAF".as_ref(), 10, 20, Level::Standard, "q"),
        ];
        assert_eq!(base(), base(), "clé déterministe");
        for v in variants {
            assert_ne!(v.hash, base().hash);
        }
    }

    #[test]
    fn key_fields_are_unambiguous() {
        let a = Key::from_parts("ab".as_ref(), 1, 1, Level::Full, "c");
        let b = Key::from_parts("a".as_ref(), 1, 1, Level::Full, "bc");
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn key_hash_is_stable() {
        // Valeur figée : si ce test casse, toutes les clés existantes sont invalidées.
        // Ne la changer qu'avec FORMAT_VERSION.
        let k = Key::from_parts(
            "/photos/a.RAF".as_ref(),
            1234,
            5678,
            Level::Thumbnail,
            "s=400",
        );
        assert_eq!(k.hash.len(), 64);
        assert_eq!(&k.hash[..16], STABLE_PREFIX);
    }

    const STABLE_PREFIX: &str = "272e1707a1420cd8";

    #[test]
    fn a_time_zone_shift_keeps_the_key() {
        // Issue n° 74 : FAT32 en heure locale, passage à l'heure d'hiver.
        let at = |ns: u128| Key::from_parts("a.RAF".as_ref(), 10, ns, Level::Standard, "p");
        let t = 1_792_000_000 * 1_000_000_000;
        let hour = 3600 * 1_000_000_000;
        for shift in [hour, 2 * hour, ZONE_STEP_NS * 22] {
            assert_eq!(at(t + shift).hash, at(t).hash);
            assert_eq!(at(t - shift).hash, at(t).hash);
        }
        // Une modification deux secondes plus tard (FAT32 : pas de 2 s) se voit.
        assert_ne!(at(t + 2_000_000_000).hash, at(t).hash);
    }

    #[test]
    fn entries_of_one_folder_share_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let key = |p: &str| Key::from_parts(p.as_ref(), 1, 1, Level::Full, "p");
        let shard = |p: &str| cache.entry_path(&key(p)).parent().unwrap().to_path_buf();
        assert_eq!(shard("/a/x.RAF"), shard("/a/y.JPG"));
        assert_ne!(shard("/a/x.RAF"), shard("/b/x.RAF"));
        assert_eq!(shard("/a/x.RAF").file_name().unwrap().len(), FOLDER_ID_LEN);
        // Même nom dans deux dossiers : même clé, rangée à deux endroits.
        assert_eq!(key("/a/x.RAF").hash, key("/b/x.RAF").hash);
    }

    #[test]
    fn relative_names_use_slashes() {
        let relative: PathBuf = ["2026", "séance"].iter().collect();
        assert_eq!(relative_name(&relative), "2026/séance");
        assert_eq!(relative_name(Path::new("")), "");
    }

    #[test]
    fn temp_paths_are_unique_and_beside_target() {
        let target = Path::new("/c/ab/abcd.jpg");
        let (a, b) = (temp_path(target), temp_path(target));
        assert_ne!(a, b);
        assert_eq!(a.parent(), target.parent());
        assert!(a.extension().is_some_and(|e| e == "tmp"));
    }

    #[test]
    fn the_wanted_root_follows_the_location() {
        let removable = [PathBuf::from("/Volumes/SD"), PathBuf::from("/Volumes/T7")];
        let folder = Path::new("/Volumes/T7/2026/séance");
        assert_eq!(wanted_root(Location::User, folder, &removable), None);
        assert_eq!(
            wanted_root(Location::Disk, folder, &removable),
            Some(PathBuf::from("/Volumes/T7"))
        );
        assert_eq!(
            wanted_root(Location::Disk, Path::new("/Users/u/Photos"), &removable),
            None,
            "pas sur un disque amovible : chez l'utilisateur"
        );
        assert_eq!(
            wanted_root(Location::Folder, folder, &removable),
            Some(folder.to_path_buf())
        );
    }
}

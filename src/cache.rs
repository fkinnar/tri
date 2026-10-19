//! Cache persistant des images calculées (vignettes, aperçus standard, 1:1).
//!
//! Un fichier JPEG par entrée, rangé par niveau ; aucune base de données. La clé dépend du
//! fichier source (chemin, taille, date de modification) et des paramètres de calcul : si
//! l'un change, l'ancienne entrée n'est simplement plus trouvée. Voir `doc/cache.md`.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bitmap::Bitmap;
use crate::jpeg::{self, Chroma};

/// Version du format du cache. L'incrémenter rend toutes les anciennes entrées invisibles.
const FORMAT_VERSION: &str = "v1";

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
    /// Image développée par un logiciel externe (darktable, [`crate::develop`]), à la taille
    /// de l'aperçu standard : le JPEG du logiciel, rangé tel quel.
    Developed,
}

impl Level {
    /// Tous les niveaux.
    pub const ALL: [Level; 9] = [
        Level::Thumbnail,
        Level::Embedded,
        Level::Standard,
        Level::EmbeddedFull,
        Level::Full,
        Level::Exposure,
        Level::Sharpness,
        Level::Similarity,
        Level::Developed,
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
            Self::Developed => "developed",
        }
    }
}

/// Identifie une entrée du cache. S'obtient avec [`Key::new`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    level: Level,
    hash: String,
}

impl Key {
    /// Clé pour l'image `source` au niveau `level`, calculée avec les paramètres `params`
    /// (description libre et stable, ex. `"half=1;side=2560"`).
    ///
    /// Lit les métadonnées du fichier (taille, date de modification) : erreur si `source`
    /// n'existe pas. Le chemin est rendu absolu, pour qu'un même fichier ait la même clé quel
    /// que soit le répertoire courant.
    pub fn new(source: &Path, level: Level, params: &str) -> io::Result<Key> {
        let path = std::path::absolute(source)?;
        let meta = fs::metadata(&path)?;
        let modified = meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Ok(Key::from_parts(
            path.as_os_str(),
            meta.len(),
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
        field(path.as_encoded_bytes());
        field(&size.to_le_bytes());
        field(&modified_ns.to_le_bytes());
        field(level.dir_name().as_bytes());
        field(params.as_bytes());
        Key {
            level,
            hash: h.finalize().to_hex().to_string(),
        }
    }

    /// Niveau de l'entrée.
    pub fn level(&self) -> Level {
        self.level
    }
}

/// Cache sur disque, enraciné dans un dossier. Ne garde aucun état en mémoire : plusieurs
/// threads (ou processus) peuvent utiliser le même dossier en même temps.
#[derive(Clone, Debug)]
pub struct Cache {
    root: PathBuf,
    quality: u8,
}

impl Cache {
    /// Ouvre (et crée si besoin) un cache dans `root`, qualité JPEG par défaut.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Cache> {
        let cache = Cache {
            root: root.into(),
            quality: DEFAULT_JPEG_QUALITY,
        };
        for level in Level::ALL {
            fs::create_dir_all(cache.level_dir(level))?;
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

    fn entry_path(&self, key: &Key) -> PathBuf {
        // Sous-dossier sur 2 caractères : évite des dizaines de milliers de fichiers par dossier.
        self.level_dir(key.level).join(&key.hash[..2]).join(format!(
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
    /// Une lecture réussie met à jour la date de modification du fichier, qui sert de date
    /// de dernier usage pour [`Cache::evict`].
    pub fn get(&self, key: &Key) -> Option<Bitmap> {
        let path = self.entry_path(key);
        let bytes = fs::read(&path).ok()?;
        let Ok(bitmap) = jpeg::decode(&bytes, 0) else {
            let _ = fs::remove_file(&path);
            return None;
        };
        if let Ok(file) = File::options().write(true).open(&path) {
            let _ = file.set_modified(SystemTime::now());
        }
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
    /// si elle n'existe pas. Une lecture réussie met à jour sa date (dernier usage).
    pub fn get_bytes(&self, key: &Key) -> Option<Vec<u8>> {
        let path = self.entry_path(key);
        let bytes = fs::read(&path).ok()?;
        if let Ok(file) = File::options().write(true).open(&path) {
            let _ = file.set_modified(SystemTime::now());
        }
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

struct Entry {
    path: PathBuf,
    bytes: u64,
    used: SystemTime,
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

    const STABLE_PREFIX: &str = "c5d0bcb0b20d2d6e";

    #[test]
    fn temp_paths_are_unique_and_beside_target() {
        let target = Path::new("/c/ab/abcd.jpg");
        let (a, b) = (temp_path(target), temp_path(target));
        assert_ne!(a, b);
        assert_eq!(a.parent(), target.parent());
        assert!(a.extension().is_some_and(|e| e == "tmp"));
    }
}

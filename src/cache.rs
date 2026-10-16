//! Cache persistant des images calculées (vignettes, aperçus standard, 1:1).
//!
//! Un fichier JPEG par entrée, rangé par niveau ; aucune base de données. La clé dépend du
//! fichier source (chemin, taille, date de modification) et des paramètres de calcul : si
//! l'un change, l'ancienne entrée n'est simplement plus trouvée. Voir `doc/cache.md`.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use image::{ExtendedColorType, ImageEncoder, codecs::jpeg::JpegEncoder};

use crate::bitmap::Bitmap;

/// Version du format du cache. L'incrémenter rend toutes les anciennes entrées invisibles.
const FORMAT_VERSION: &str = "v1";

/// Qualité JPEG des entrées.
const JPEG_QUALITY: u8 = 90;

/// Niveau de détail d'une entrée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Level {
    /// Vignette de la grille.
    Thumbnail,
    /// Aperçu plein écran, à la taille configurée.
    Standard,
    /// Rendu complet (zoom 100 %).
    Full,
}

impl Level {
    /// Tous les niveaux.
    pub const ALL: [Level; 3] = [Level::Thumbnail, Level::Standard, Level::Full];

    fn dir_name(self) -> &'static str {
        match self {
            Self::Thumbnail => "thumbnail",
            Self::Standard => "standard",
            Self::Full => "full",
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
}

impl Cache {
    /// Ouvre (et crée si besoin) un cache dans `root`.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Cache> {
        let cache = Cache { root: root.into() };
        for level in Level::ALL {
            fs::create_dir_all(cache.level_dir(level))?;
        }
        Ok(cache)
    }

    /// Ouvre le cache à son emplacement par défaut (voir [`default_dir`]).
    pub fn open_default() -> io::Result<Cache> {
        let dir = default_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "dossier de cache utilisateur introuvable",
            )
        })?;
        Cache::open(dir)
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
        self.level_dir(key.level)
            .join(&key.hash[..2])
            .join(format!("{}.jpg", key.hash))
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
        let img = match image::open(&path) {
            Ok(img) => img.into_rgb8(),
            Err(image::ImageError::IoError(e)) if e.kind() == io::ErrorKind::NotFound => {
                return None;
            }
            Err(_) => {
                let _ = fs::remove_file(&path);
                return None;
            }
        };
        if let Ok(file) = File::options().write(true).open(&path) {
            let _ = file.set_modified(SystemTime::now());
        }
        Some(Bitmap {
            width: img.width(),
            height: img.height(),
            data: img.into_raw(),
        })
    }

    /// Écrit une entrée (JPEG). L'écriture est atomique : fichier temporaire puis
    /// renommage, donc un lecteur ne voit jamais d'entrée à moitié écrite, même si le
    /// programme est interrompu.
    pub fn put(&self, key: &Key, bitmap: &Bitmap) -> io::Result<()> {
        let path = self.entry_path(key);
        let dir = path
            .parent()
            .expect("une entrée a toujours un dossier parent");
        fs::create_dir_all(dir)?;
        let tmp = temp_path(&path);
        let result = (|| {
            let mut out = BufWriter::new(File::create(&tmp)?);
            JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
                .write_image(
                    &bitmap.data,
                    bitmap.width,
                    bitmap.height,
                    ExtendedColorType::Rgb8,
                )
                .map_err(io::Error::other)?;
            out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
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
                if path.extension().is_none_or(|e| e != "jpg") {
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

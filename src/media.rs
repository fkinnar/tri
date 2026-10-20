//! Parcours de dossier et classification des fichiers image (RAW ou « plats »).
//!
//! Voir `doc/en/media.md` pour le détail.

use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Extensions RAW reconnues (formats photo gérés par LibRaw), en minuscules.
pub const RAW_EXTENSIONS: &[&str] = &[
    "3fr", "ari", "arw", "bay", "cr2", "cr3", "crw", "dcr", "dcs", "dng", "drf", "erf", "fff",
    "gpr", "iiq", "k25", "kdc", "mdc", "mef", "mos", "mrw", "nef", "nrw", "orf", "pef", "ptx",
    "pxn", "raf", "raw", "rw2", "rwl", "rwz", "sr2", "srf", "srw", "x3f",
];

/// Extensions d'images « plates » reconnues (affichées sans développement), en minuscules.
pub const FLAT_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "tif", "tiff", "webp", "heic", "heif", "hif",
];

/// Nature d'un fichier image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// Données capteur brutes, à développer (LibRaw).
    Raw,
    /// Image déjà développée (JPEG, TIFF, PNG...), simplement décodée.
    Flat,
}

/// Fichier image trouvé lors d'un parcours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaFile {
    /// Chemin complet (racine du parcours incluse).
    pub path: PathBuf,
    /// Nature déduite de l'extension.
    pub kind: MediaKind,
}

/// Nature d'un fichier d'après son extension (insensible à la casse), `None` si non géré.
/// Les fichiers cachés (nom commençant par `.`) sont ignorés : sur les cartes SD lues par
/// macOS, `._DSCF0001.RAF` est un fichier AppleDouble, pas une image.
pub fn kind_of(path: &Path) -> Option<MediaKind> {
    let name = path.file_name()?.to_str()?;
    if name.starts_with('.') {
        return None;
    }
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if RAW_EXTENSIONS.contains(&ext.as_str()) {
        Some(MediaKind::Raw)
    } else if FLAT_EXTENSIONS.contains(&ext.as_str()) {
        Some(MediaKind::Flat)
    } else {
        None
    }
}

/// Liste les images de `root`, jusqu'à `depth` niveaux (1 = le dossier seul, sans
/// sous-dossiers ; 0 = rien ; `usize::MAX` = sans limite). Le résultat est trié par chemin ; à chemin sans extension égal
/// (`DSCF0001.RAF` / `DSCF0001.JPG`), le RAW vient en premier.
///
/// Erreur si `root` n'est pas un dossier lisible ; les entrées illisibles à l'intérieur sont
/// ignorées.
pub fn scan(root: impl AsRef<Path>, depth: usize) -> io::Result<Vec<MediaFile>> {
    let root = root.as_ref();
    if !root.metadata()?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("{} n'est pas un dossier", root.display()),
        ));
    }
    if depth == 0 {
        // walkdir inverserait min_depth (1) et max_depth (0) : on traite le cas à part.
        return Ok(Vec::new());
    }
    let mut files: Vec<MediaFile> = WalkDir::new(root)
        .min_depth(1)
        .max_depth(depth)
        .into_iter()
        // Sous-dossiers cachés (`.thumbnails`, `.dtrash`…) écartés.
        .filter_entry(|e| e.depth() == 0 || !is_hidden_dir(e))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let kind = kind_of(e.path())?;
            Some(MediaFile {
                path: e.into_path(),
                kind,
            })
        })
        .collect();
    files.sort_by(compare);
    Ok(files)
}

/// Vrai pour un dossier caché (nom commençant par « . ») : ses images ne sont pas
/// listées quand on parcourt les sous-dossiers.
pub fn is_hidden_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir() && entry.file_name().to_string_lossy().starts_with('.')
}

fn compare(a: &MediaFile, b: &MediaFile) -> Ordering {
    a.path
        .with_extension("")
        .cmp(&b.path.with_extension(""))
        .then_with(|| (a.kind == MediaKind::Flat).cmp(&(b.kind == MediaKind::Flat)))
        .then_with(|| a.path.cmp(&b.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(name: &str) -> Option<MediaKind> {
        kind_of(Path::new(name))
    }

    #[test]
    fn raw_extensions_are_recognized_case_insensitively() {
        for name in [
            "a.RAF", "a.raf", "a.Raf", "a.CR3", "a.nef", "a.DNG", "a.ARW",
        ] {
            assert_eq!(kind(name), Some(MediaKind::Raw), "{name}");
        }
    }

    #[test]
    fn flat_extensions_are_recognized_case_insensitively() {
        for name in [
            "a.JPG", "a.jpeg", "a.Tif", "a.TIFF", "a.png", "a.webp", "a.HEIC", "a.heif", "a.HIF",
        ] {
            assert_eq!(kind(name), Some(MediaKind::Flat), "{name}");
        }
    }

    #[test]
    fn other_files_are_ignored() {
        for name in ["a.xmp", "a.txt", "a.mov", "sans_extension", "a.raf.bak", ""] {
            assert_eq!(kind(name), None, "{name}");
        }
    }

    #[test]
    fn hidden_and_appledouble_files_are_ignored() {
        assert_eq!(kind("._DSCF0001.RAF"), None);
        assert_eq!(kind(".cache.jpg"), None);
        assert_eq!(kind("/dossier/._x.JPG"), None);
    }

    #[test]
    fn extension_lists_are_lowercase_and_disjoint() {
        for ext in RAW_EXTENSIONS.iter().chain(FLAT_EXTENSIONS) {
            assert_eq!(*ext, ext.to_ascii_lowercase());
        }
        assert!(RAW_EXTENSIONS.iter().all(|e| !FLAT_EXTENSIONS.contains(e)));
    }

    #[test]
    fn raw_sorts_before_its_jpeg_twin() {
        let f = |p: &str, kind| MediaFile {
            path: p.into(),
            kind,
        };
        let mut v = [
            f("b.RAF", MediaKind::Raw),
            f("a.JPG", MediaKind::Flat),
            f("a.RAF", MediaKind::Raw),
        ];
        v.sort_by(compare);
        let names: Vec<_> = v.iter().map(|m| m.path.to_str().unwrap()).collect();
        assert_eq!(names, ["a.RAF", "a.JPG", "b.RAF"]);
    }
}

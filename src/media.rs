//! Parcours de dossier et classification des fichiers image (RAW ou « plats »).
//!
//! Voir `doc/en/media.md` pour le détail.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, Instant};

use std::fs;

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
    scan_counting(
        root,
        depth,
        Duration::MAX,
        &AtomicBool::new(false),
        &AtomicUsize::new(0),
    )
}

/// Comme [`scan`], pour un dossier lent (NAS, disque lent, beaucoup de sous-dossiers) :
/// `found` compte les images trouvées au fil du parcours, et `stop` l'interrompt (erreur
/// `Interrupted`).
///
/// `slow` : une demande du type d'une entrée plus lente que ce délai (partage monté par
/// GVFS : un aller-retour réseau par fichier) fait se fier aux noms pour la suite du
/// dossier (une image se reconnaît à son extension) ; `Duration::MAX` : toujours demander.
pub fn scan_counting(
    root: impl AsRef<Path>,
    depth: usize,
    slow: Duration,
    stop: &AtomicBool,
    found: &AtomicUsize,
) -> io::Result<Vec<MediaFile>> {
    let root = root.as_ref();
    if !root.metadata()?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("{} n'est pas un dossier", root.display()),
        ));
    }
    let mut files: Vec<MediaFile> = Vec::new();
    walk(root, depth, slow, stop, found, &mut files)?;
    files.sort_by(compare);
    Ok(files)
}

/// Extensions de fichiers qui accompagnent les images (sidecars, réglages) : jamais des
/// dossiers, inutile de le demander au système.
const COMPANIONS: &[&str] = &["xmp", "pp3", "arp", "cos", "rrdata", "dop", "json", "txt"];

/// Images de `dir` sur `depth` niveaux, ajoutées à `files`. Le type d'une entrée n'est
/// demandé au système que s'il le faut, et tant que c'est rapide : sur un partage monté
/// par GVFS sous Linux, le système ne le donne pas, le demander coûtait un aller-retour
/// réseau par fichier (retour du 29/10/2026 : 1 à 2 images par seconde). Une demande plus
/// lente que `slow` fait alors se fier aux noms pour le reste du dossier : une image se
/// reconnaît à son extension (un dossier nommé `x.RAF` passerait pour une image), un
/// sidecar n'est jamais un dossier, et sans sous-dossiers rien d'autre ne compte.
fn walk(
    dir: &Path,
    depth: usize,
    slow: Duration,
    stop: &AtomicBool,
    found: &AtomicUsize,
    files: &mut Vec<MediaFile>,
) -> io::Result<()> {
    if depth == 0 {
        return Ok(());
    }
    let mut subdirs = Vec::new();
    // Noms des images et des sidecars XMP du dossier, pour les clones darktable.
    let mut images: HashMap<String, (PathBuf, MediaKind)> = HashMap::new();
    let mut xmps: Vec<String> = Vec::new();
    // Type des entrées demandé tant que c'est rapide.
    let mut ask = true;
    let file_type = |entry: &fs::DirEntry, ask: &mut bool| {
        let started = Instant::now();
        let kind = entry.file_type().ok();
        if started.elapsed() > slow {
            *ask = false;
        }
        kind
    };
    for entry in fs::read_dir(dir)? {
        if stop.load(AtomicOrdering::Relaxed) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if let Some(kind) = kind_of(&path) {
            // Un dossier au nom d'image (`x.RAF`) n'en est pas une.
            if ask && file_type(&entry, &mut ask).is_some_and(|t| t.is_dir()) {
                if depth > 1 && !entry.file_name().to_string_lossy().starts_with('.') {
                    subdirs.push(path);
                }
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            images.insert(name, (path.clone(), kind));
            files.push(MediaFile { path, kind });
            found.fetch_add(1, AtomicOrdering::Relaxed);
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(image) = name
            .strip_suffix(".xmp")
            .or_else(|| name.strip_suffix(".XMP"))
        {
            xmps.push(image.to_string());
        }
        if depth == 1 {
            continue;
        }
        // Sous-dossiers cachés (`.thumbnails`, `.dtrash`…) écartés, sidecars passés.
        let companion = path
            .extension()
            .is_some_and(|e| COMPANIONS.iter().any(|c| e.eq_ignore_ascii_case(c)));
        if name.starts_with('.') || companion {
            continue;
        }
        // Liens symboliques pas suivis (comme avant).
        if file_type(&entry, &mut ask).is_some_and(|t| t.is_dir()) {
            subdirs.push(path);
        }
    }
    for image in xmps {
        if images.contains_key(&image.to_lowercase()) {
            continue;
        }
        let Some((original, _)) = clone_name(&image) else {
            continue;
        };
        if let Some((source, kind)) = images.get(&original.to_lowercase()) {
            let path = dir.join(&image);
            register_clone(&path, source);
            files.push(MediaFile { path, kind: *kind });
            found.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }
    for sub in subdirs {
        // Un sous-dossier illisible est ignoré, comme une entrée illisible.
        match walk(&sub, depth - 1, slow, stop, found, files) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// Clones (« duplicates ») darktable connus : chemin du clone → image d'origine.
static CLONES: LazyLock<RwLock<HashMap<PathBuf, PathBuf>>> = LazyLock::new(Default::default);

/// Original et numéro d'un nom de clone darktable : `IMG_01.RAF` → (`IMG.RAF`, 1). Le
/// nom seul : `IMG_0001.CR2` (Canon) en a l'air aussi, c'est le sidecar sans image qui
/// fait le clone ([`walk`]).
pub fn clone_name(name: &str) -> Option<(String, u32)> {
    let (stem, extension) = name.rsplit_once('.')?;
    let (base, number) = stem.rsplit_once('_')?;
    if base.is_empty() || number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((format!("{base}.{extension}"), number.parse().ok()?))
}

/// Retient que `clone` (sans fichier à lui, seulement son sidecar `clone.xmp`) est un
/// clone darktable de `source`.
pub fn register_clone(clone: &Path, source: &Path) {
    if let Ok(mut clones) = CLONES.write() {
        clones.insert(clone.to_path_buf(), source.to_path_buf());
    }
}

/// Fichier à lire pour les pixels de `path` : l'image d'origine pour un clone darktable
/// (une vignette par clone, demandé le 29/10/2026), `path` lui-même sinon. Le clone a
/// ses propres notes et son propre développement (son sidecar `IMG_01.RAF.xmp`), mais
/// les données de l'image d'origine.
pub fn source(path: &Path) -> Cow<'_, Path> {
    match CLONES.read().ok().and_then(|c| c.get(path).cloned()) {
        Some(source) => Cow::Owned(source),
        None => Cow::Borrowed(path),
    }
}

/// Vrai si `path` est un clone darktable (pas un fichier, voir [`source`]).
pub fn is_clone(path: &Path) -> bool {
    CLONES.read().is_ok_and(|c| c.contains_key(path))
}

/// Vrai pour un dossier caché (nom commençant par « . ») : ses images ne sont pas
/// listées quand on parcourt les sous-dossiers.
pub fn is_hidden_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir() && entry.file_name().to_string_lossy().starts_with('.')
}

/// Ordre des noms : chemin sans extension, puis le RAW avant l'image plate de même nom.
pub fn compare(a: &MediaFile, b: &MediaFile) -> Ordering {
    a.path
        .with_extension("")
        .cmp(&b.path.with_extension(""))
        .then_with(|| (a.kind == MediaKind::Flat).cmp(&(b.kind == MediaKind::Flat)))
        .then_with(|| a.path.cmp(&b.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subfolders_are_walked_without_hidden_ones_or_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for sub in ["2026", "2026/séance", ".thumbnails"] {
            std::fs::create_dir_all(d.join(sub)).unwrap();
        }
        for name in [
            "a.RAF",
            "a.RAF.xmp",
            "2026/b.jpg",
            "2026/séance/c.NEF",
            ".thumbnails/x.jpg",
        ] {
            std::fs::write(d.join(name), b"x").unwrap();
        }
        let names = |depth| -> Vec<String> {
            let mut list: Vec<String> = scan(d, depth)
                .unwrap()
                .iter()
                .map(|f| {
                    // Barres obliques partout (Windows compris).
                    let relative = f.path.strip_prefix(d).unwrap();
                    relative.to_string_lossy().replace('\\', "/")
                })
                .collect();
            list.sort();
            list
        };
        assert_eq!(names(1), ["a.RAF"]);
        assert_eq!(names(2), ["2026/b.jpg", "a.RAF"]);
        assert_eq!(names(3), ["2026/b.jpg", "2026/séance/c.NEF", "a.RAF"]);
        assert!(names(0).is_empty());
    }

    #[test]
    fn a_counting_scan_counts_images_and_stops_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.RAF", "a.JPG", "notes.txt"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let found = AtomicUsize::new(0);
        let files = scan_counting(
            dir.path(),
            1,
            Duration::MAX,
            &AtomicBool::new(false),
            &found,
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(found.load(AtomicOrdering::Relaxed), 2);
        let stopped = scan_counting(dir.path(), 1, Duration::MAX, &AtomicBool::new(true), &found);
        assert_eq!(stopped.unwrap_err().kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn darktable_clones_are_listed_and_read_from_their_original() {
        // Une vignette par clone (demandé le 29/10/2026) : `IMG_01.RAF.xmp` sans
        // `IMG_01.RAF` est le clone n° 1 de `IMG.RAF`.
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for name in [
            "IMG.RAF",
            "IMG.RAF.xmp",
            "IMG_01.RAF.xmp",
            "IMG_02.RAF.xmp",
            // Une vraie image de ce nom : pas un clone.
            "B.JPG",
            "B_01.JPG",
            "B_01.JPG.xmp",
            // Sans original : rien.
            "C_01.RAF.xmp",
            // Nom Canon : une vraie image, pas un clone de `IMG.CR2`.
            "IMG_0001.CR2",
        ] {
            std::fs::write(d.join(name), b"x").unwrap();
        }
        let files = scan(d, 1).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "B.JPG",
                "B_01.JPG",
                "IMG.RAF",
                "IMG_0001.CR2",
                "IMG_01.RAF",
                "IMG_02.RAF"
            ]
        );
        let clone = d.join("IMG_01.RAF");
        assert!(is_clone(&clone));
        assert_eq!(files[4].kind, MediaKind::Raw);
        assert_eq!(source(&clone), d.join("IMG.RAF"));
        assert!(!is_clone(&d.join("B_01.JPG")));
        assert_eq!(source(&d.join("B_01.JPG")), d.join("B_01.JPG"));
    }

    #[test]
    fn a_clone_is_decoded_and_stamped_from_its_original() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        image::RgbImage::new(6, 4).save(d.join("a.jpg")).unwrap();
        std::fs::write(d.join("a_01.jpg.xmp"), b"<x/>").unwrap();
        scan(d, 1).unwrap();
        let clone = d.join("a_01.jpg");
        let bitmap = crate::flat::decode(&clone).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (6, 4));
        assert_eq!(
            crate::cache::stamp(&clone),
            crate::cache::stamp(&d.join("a.jpg"))
        );
        assert!(crate::cache::stamp(&clone).is_some());
    }

    #[test]
    fn clone_names_give_their_original_and_number() {
        assert_eq!(clone_name("IMG_01.RAF"), Some(("IMG.RAF".into(), 1)));
        assert_eq!(
            clone_name("DSCF0001_12.jpg"),
            Some(("DSCF0001.jpg".into(), 12))
        );
        for name in ["IMG.RAF", "IMG_.RAF", "_01.RAF", "IMG_a1.RAF", "IMG_01"] {
            assert_eq!(clone_name(name), None, "{name}");
        }
    }

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

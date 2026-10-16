//! Tests d'intégration du parcours de dossier et des filtres, sur une arborescence temporaire.

use std::fs;
use std::path::Path;

use tri::filter::{Filter, KindFilter, ungrouped};
use tri::media::{MediaKind, scan};

/// Carte SD typique : paires RAW+JPEG, fichiers parasites, sous-dossier.
fn sd_card() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for name in [
        "DSCF0002.RAF",
        "DSCF0002.JPG",
        "DSCF0001.JPG",
        "DSCF0001.RAF",
        "DSCF0003.raf", // casse différente
        "DSCF0004.jpeg",
        "._DSCF0001.RAF", // AppleDouble macOS
        "DSCF0001.xmp",
        "notes.txt",
        "sub/IMG_0001.CR3",
        "sub/deep/IMG_0002.tif",
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }
    fs::create_dir(root.join("dossier.RAF")).unwrap(); // dossier, pas fichier
    dir
}

fn relative(root: &Path, files: &[tri::media::MediaFile]) -> Vec<String> {
    files
        .iter()
        .map(|f| {
            let rel = f.path.strip_prefix(root).unwrap();
            // Séparateur normalisé pour que le test passe aussi sous Windows.
            rel.components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect()
}

#[test]
fn scan_lists_images_sorted_with_raw_first() {
    let dir = sd_card();
    let files = scan(dir.path(), 1).unwrap();
    assert_eq!(
        relative(dir.path(), &files),
        [
            "DSCF0001.RAF",
            "DSCF0001.JPG",
            "DSCF0002.RAF",
            "DSCF0002.JPG",
            "DSCF0003.raf",
            "DSCF0004.jpeg",
        ]
    );
    let kinds: Vec<_> = files.iter().map(|f| f.kind).collect();
    use MediaKind::*;
    assert_eq!(kinds, [Raw, Flat, Raw, Flat, Raw, Flat]);
}

#[test]
fn scan_depth_controls_subfolders() {
    let dir = sd_card();
    let count = |depth| scan(dir.path(), depth).unwrap().len();
    assert_eq!(count(1), 6);
    assert_eq!(count(2), 7);
    assert_eq!(count(3), 8);
    assert_eq!(count(0), 0);
    assert_eq!(count(usize::MAX), 8);
}

#[test]
fn scan_of_empty_folder_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    assert!(scan(dir.path(), 1).unwrap().is_empty());
}

#[test]
fn scan_of_missing_folder_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(scan(dir.path().join("absent"), 1).is_err());
}

#[test]
fn scan_of_a_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.RAF");
    fs::write(&file, b"").unwrap();
    let err = scan(&file, 1).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotADirectory);
}

#[test]
fn kind_filter_on_raw_plus_jpeg_card() {
    let dir = sd_card();
    let all = || ungrouped(scan(dir.path(), 1).unwrap());
    let count = |f: KindFilter| f.apply(all()).iter().map(Vec::len).sum::<usize>();
    assert_eq!(count(KindFilter::All), 6);
    assert_eq!(count(KindFilter::RawOnly), 3);
    assert_eq!(count(KindFilter::FlatOnly), 3);

    let raws = KindFilter::RawOnly.apply(all());
    assert!(raws.iter().flatten().all(|f| f.kind == MediaKind::Raw));
}

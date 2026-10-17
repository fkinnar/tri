//! Tests d'intégration du module `folder` : ouverture d'un dossier temporaire, filtres
//! combinés.

use std::fs;

use tri::filter::KindFilter;
use tri::folder::Folder;
use tri::notes::{NotesRule, StarsRule};
use tri::xmp::Convention;

#[test]
fn opening_a_folder_reads_images_and_notes_and_filters_both_ways() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["A.RAF", "A.JPG", "B.RAF", "notes.txt"] {
        fs::write(dir.path().join(name), b"").unwrap();
    }
    let mut folder = Folder::open(dir.path(), 1, Convention::Darktable).unwrap();
    assert!(!folder.is_none());
    assert!(folder.dir.is_absolute());
    assert_eq!(folder.files.len(), 3);
    folder
        .notes
        .edit(&dir.path().join("B.RAF"), |n| n.set_stars(4))
        .unwrap();
    let names = |kind, rule: &NotesRule| -> Vec<String> {
        folder
            .filtered(kind, rule)
            .into_iter()
            .flatten()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    let four = NotesRule {
        stars: Some(StarsRule::AtLeast(4)),
        ..NotesRule::default()
    };
    assert_eq!(names(KindFilter::All, &NotesRule::default()).len(), 3);
    assert_eq!(names(KindFilter::All, &four), ["B.RAF"]);
    assert_eq!(names(KindFilter::FlatOnly, &four), Vec::<String>::new());
    // Relu depuis le disque : la note est bien dans le sidecar.
    let again = Folder::open(dir.path(), 1, Convention::Darktable).unwrap();
    assert_eq!(again.notes.notes(&dir.path().join("B.RAF")).stars, 4);
}

#[test]
fn a_missing_folder_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Folder::open(&dir.path().join("absent"), 1, Convention::Darktable).is_err());
}

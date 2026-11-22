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

/// « Toutes les photos » (#561) : les photos de l'index tout de suite, puis complétées
/// dossier par dossier (photo ajoutée vue) ; un dossier injoignable garde ce que l'index
/// en savait.
#[test]
fn all_photos_come_from_the_index_then_from_the_reachable_folders() {
    let dir = tempfile::tempdir().unwrap();
    let here = dir.path().join("ici");
    fs::create_dir(&here).unwrap();
    let jpeg = |path: &std::path::Path| {
        image::RgbImage::from_pixel(8, 8, image::Rgb([90, 90, 90]))
            .save(path)
            .unwrap()
    };
    jpeg(&here.join("a.jpg"));
    let gone = dir.path().join("debranche").join("b.jpg");
    let db = dir.path().join("index.sqlite");
    {
        let mut index = tri::index::Index::open(&db).unwrap();
        let stamp = |p: &std::path::Path| (p.to_path_buf(), 10, 1_700_000_000_000_000_000);
        index.seen(&here, 1, &[stamp(&here.join("a.jpg"))]).unwrap();
        index
            .seen(gone.parent().unwrap(), 1, &[stamp(&gone)])
            .unwrap();
    }
    let folder = Folder::all_photos(&db, Convention::Darktable).unwrap();
    assert!(tri::folder::is_all_photos(&folder.dir));
    assert_eq!(folder.files.len(), 2);
    // Ajoutée depuis : trouvée en relisant son dossier.
    jpeg(&here.join("c.jpg"));
    let stop = std::sync::atomic::AtomicBool::new(false);
    let completed = tri::folder::complete_from_disk(&folder.files, &stop);
    assert_eq!(
        completed.missing,
        vec![gone.parent().unwrap().to_path_buf()]
    );
    let names: Vec<String> = completed
        .files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["b.jpg", "a.jpg", "c.jpg"]);
}

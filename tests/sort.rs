//! Tests d'intégration de `tri::sort` : ordre des images d'un vrai dossier.

use std::path::Path;
use std::time::{Duration, SystemTime};

use tri::sort::{Order, SortKey, SortSettings, sort};
use tri::xmp::{Flag, Notes};

#[test]
fn a_scanned_folder_sorts_by_name_date_and_modification() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["B.RAF", "B.JPG", "A.JPG", "C.JPG"] {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }
    // C le plus ancien sur le disque.
    std::fs::File::options()
        .write(true)
        .open(dir.path().join("C.JPG"))
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(7200))
        .unwrap();
    let mut files = tri::media::scan(dir.path(), 1).unwrap();
    let names = |files: &[tri::media::MediaFile]| -> Vec<String> {
        files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    assert_eq!(names(&files), ["A.JPG", "B.RAF", "B.JPG", "C.JPG"]);
    let modified = Order {
        key: SortKey::Modified,
        descending: false,
    };
    let s = SortSettings::default();
    sort(&mut files, modified, |_| None, |_| Notes::EMPTY, &s);
    assert_eq!(names(&files)[0], "C.JPG");
    // Prise de vue : B avant A ; C sans date, à la fin.
    let captured = |p: &Path| match p.file_stem()?.to_str()? {
        "A" => Some(200),
        "B" => Some(100),
        _ => None,
    };
    let by_date = Order {
        key: SortKey::Captured,
        descending: false,
    };
    sort(&mut files, by_date, captured, |_| Notes::EMPTY, &s);
    assert_eq!(names(&files), ["B.RAF", "B.JPG", "A.JPG", "C.JPG"]);
    // Notes (31/10/2026) : lues dans les sidecars du dossier, la paire d'après son RAW.
    let mut book = tri::notes::NoteBook::load_files(&files, tri::xmp::Convention::Darktable);
    book.edit(&dir.path().join("C.JPG"), |n| n.stars = 4)
        .unwrap();
    book.edit(&dir.path().join("B.RAF"), |n| n.flag = Flag::Rejected)
        .unwrap();
    let by_stars = Order {
        key: SortKey::Stars,
        descending: true,
    };
    sort(
        &mut files,
        by_stars,
        |_| None,
        |p| book.notes(p).clone(),
        &s,
    );
    assert_eq!(names(&files), ["C.JPG", "A.JPG", "B.RAF", "B.JPG"]);
}

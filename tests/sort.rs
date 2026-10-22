//! Tests d'intégration de `tri::sort` : ordre des images d'un vrai dossier.

use std::path::Path;
use std::time::{Duration, SystemTime};

use tri::sort::{Order, SortKey, sort};

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
    sort(&mut files, modified, |_| None);
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
    sort(&mut files, by_date, captured);
    assert_eq!(names(&files), ["B.RAF", "B.JPG", "A.JPG", "C.JPG"]);
}

//! Tests d'intégration du module `files` : renommage et suppression sur des fichiers
//! temporaires (jamais la corbeille de l'utilisateur, sauf test ignoré explicite).

use std::path::Path;

use tri::files::{self, FileError, FileSettings};
use tri::xmp::{Convention, Notes};

fn touch(dir: &Path, names: &[&str]) {
    for name in names {
        std::fs::write(dir.join(name), name).unwrap();
    }
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn renaming_moves_the_pair_and_the_sidecars_with_their_notes() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    touch(d, &["IMG.RAF", "IMG.JPG", "AUTRE.RAF"]);
    let notes = Notes {
        stars: 4,
        ..Notes::default()
    };
    tri::xmp::write(&d.join("IMG.RAF"), &notes, Convention::Darktable).unwrap();
    tri::xmp::write(&d.join("IMG.JPG"), &notes, Convention::Adobe).unwrap();
    let plan =
        files::plan_rename(&d.join("IMG.RAF"), "Portrait", &FileSettings::default()).unwrap();
    assert_eq!(plan.moves.len(), 4);
    files::rename(&plan).unwrap();
    assert_eq!(
        listing(d),
        [
            "AUTRE.RAF",
            "Portrait.JPG",
            "Portrait.RAF",
            "Portrait.RAF.xmp",
            "Portrait.xmp"
        ]
    );
    assert_eq!(std::fs::read(d.join("Portrait.RAF")).unwrap(), b"IMG.RAF");
    assert_eq!(tri::xmp::read(&d.join("Portrait.RAF")).unwrap().stars, 4);
    assert_eq!(tri::xmp::read(&d.join("Portrait.JPG")).unwrap().stars, 4);
}

#[test]
fn a_taken_name_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    touch(d, &["IMG.RAF", "IMG.JPG", "B.JPG"]);
    let before = listing(d);
    // B.JPG existe : la paire IMG.RAF + IMG.JPG ne peut pas devenir B.
    let err = files::plan_rename(&d.join("IMG.RAF"), "B", &FileSettings::default());
    assert!(matches!(err, Err(FileError::Exists(p)) if p.ends_with("B.JPG")));
    assert_eq!(listing(d), before);
    // Sans les paires, seul IMG.RAF bouge : B.RAF est libre.
    let alone = FileSettings {
        pairs: false,
        ..FileSettings::default()
    };
    files::rename(&files::plan_rename(&d.join("IMG.RAF"), "B", &alone).unwrap()).unwrap();
    assert_eq!(listing(d), ["B.JPG", "B.RAF", "IMG.JPG"]);
}

#[test]
fn without_pairs_a_shared_sidecar_is_copied() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    touch(d, &["IMG.RAF", "IMG.JPG"]);
    let notes = Notes {
        stars: 2,
        ..Notes::default()
    };
    tri::xmp::write(&d.join("IMG.RAF"), &notes, Convention::Adobe).unwrap();
    let alone = FileSettings {
        pairs: false,
        ..FileSettings::default()
    };
    files::rename(&files::plan_rename(&d.join("IMG.RAF"), "SEUL", &alone).unwrap()).unwrap();
    assert_eq!(listing(d), ["IMG.JPG", "IMG.xmp", "SEUL.RAF", "SEUL.xmp"]);
    assert_eq!(tri::xmp::read(&d.join("SEUL.RAF")).unwrap().stars, 2);
    assert_eq!(tri::xmp::read(&d.join("IMG.JPG")).unwrap().stars, 2);
}

#[test]
fn changing_only_the_case_is_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    touch(d, &["img.jpg"]);
    files::rename(
        &files::plan_rename(&d.join("img.jpg"), "IMG", &FileSettings::default()).unwrap(),
    )
    .unwrap();
    assert_eq!(listing(d), ["IMG.jpg"]);
}

#[test]
fn deleting_removes_the_pair_and_its_sidecars_only() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    touch(
        d,
        &[
            "IMG.RAF",
            "IMG.JPG",
            "IMG.RAF.xmp",
            "IMG.xmp",
            "AUTRE.RAF",
            "AUTRE.RAF.xmp",
        ],
    );
    let group = files::plan_delete(&[d.join("IMG.JPG")], &FileSettings::default()).group;
    assert_eq!(group.files().count(), 4);
    files::delete(&group, false).unwrap();
    assert_eq!(listing(d), ["AUTRE.RAF", "AUTRE.RAF.xmp"]);
}

#[test]
fn a_deletion_reports_its_progress_file_by_file() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    touch(d, &["A.JPG", "B.JPG", "C.JPG"]);
    let images: Vec<_> = ["A.JPG", "B.JPG", "C.JPG"].map(|n| d.join(n)).into();
    let group = files::plan_delete(&images, &FileSettings::default()).group;
    let mut steps = Vec::new();
    files::delete_with_progress(&group, false, |done, total| steps.push((done, total))).unwrap();
    assert_eq!(steps, [(1, 3), (2, 3), (3, 3)]);
    assert!(listing(d).is_empty());
}

/// Met de vrais fichiers à la corbeille de l'utilisateur : seulement sur demande.
#[test]
#[ignore = "met un fichier temporaire dans la corbeille de l'utilisateur"]
fn trash_really_trashes() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), &["tri-test-corbeille.jpg"]);
    let group = files::plan_delete(
        &[dir.path().join("tri-test-corbeille.jpg")],
        &FileSettings::default(),
    )
    .group;
    files::delete(&group, true).unwrap();
    assert!(listing(dir.path()).is_empty());
}

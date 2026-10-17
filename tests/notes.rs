//! Tests d'intégration du module `notes` : notes d'un dossier temporaire, modification,
//! sidecars partagés, filtres.

use std::fs;
use std::path::{Path, PathBuf};

use tri::filter::{Filter, ungrouped};
use tri::media::{self, MediaKind};
use tri::notes::{ColorRule, NoteBook, NotesRule, StarsRule};
use tri::xmp::{self, Color, Convention, Flag};

fn file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, b"").unwrap();
    path
}

fn xmp_with(attrs: &str) -> String {
    format!(
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" {attrs}/></rdf:RDF></x:xmpmeta>"#
    )
}

#[test]
fn loads_existing_notes_and_reports_malformed_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let a = file(dir.path(), "A.RAF");
    let b = file(dir.path(), "B.RAF");
    let c = file(dir.path(), "C.RAF");
    fs::write(
        dir.path().join("A.RAF.xmp"),
        xmp_with(r#"xmp:Rating="4" xmp:Label="Red""#),
    )
    .unwrap();
    fs::write(dir.path().join("B.RAF.xmp"), "<pas du xml").unwrap();
    let book = NoteBook::load(
        [a.as_path(), b.as_path(), c.as_path()],
        Convention::Darktable,
    );
    assert_eq!(book.len(), 3);
    assert_eq!(book.notes(&a).stars, 4);
    assert_eq!(book.notes(&a).colors, [Color::Red]);
    assert!(book.error(&b).is_some(), "sidecar mal formé signalé");
    assert_eq!(book.notes(&c), &xmp::Notes::EMPTY);
    assert_eq!(
        book.sidecar(&c),
        Some(dir.path().join("C.RAF.xmp").as_path())
    );
}

#[test]
fn edit_writes_the_sidecar_and_returns_changed_images() {
    let dir = tempfile::tempdir().unwrap();
    let a = file(dir.path(), "A.RAF");
    let mut book = NoteBook::load([a.as_path()], Convention::Darktable);
    let changed = book.edit(&a, |n| n.set_stars(3)).unwrap();
    assert_eq!(changed, std::slice::from_ref(&a));
    assert_eq!(xmp::read(&a).unwrap().stars, 3, "relu depuis le disque");
    assert_eq!(book.notes(&a).stars, 3);
    assert!(
        book.edit(&a, |n| n.set_stars(3)).unwrap().is_empty(),
        "rien à faire"
    );
}

#[test]
fn shared_adobe_sidecar_updates_both_images_of_a_pair() {
    let dir = tempfile::tempdir().unwrap();
    let raf = file(dir.path(), "IMG.RAF");
    let jpg = file(dir.path(), "IMG.JPG");
    let other = file(dir.path(), "AUTRE.JPG");
    let mut book = NoteBook::load(
        [raf.as_path(), jpg.as_path(), other.as_path()],
        Convention::Adobe,
    );
    let changed = book.edit(&raf, |n| n.flag = Flag::Accepted).unwrap();
    assert_eq!(changed, [raf.clone(), jpg.clone()]);
    assert_eq!(book.notes(&jpg).flag, Flag::Accepted);
    assert_eq!(book.notes(&other).flag, Flag::None);
}

#[test]
fn darktable_pair_gets_the_same_notes_in_both_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let raf = file(dir.path(), "IMG.RAF");
    let jpg = file(dir.path(), "img.jpg");
    let other = file(dir.path(), "IMG_01.JPG");
    let mut book = NoteBook::load(
        [raf.as_path(), jpg.as_path(), other.as_path()],
        Convention::Darktable,
    );
    assert_eq!(
        book.siblings(&raf),
        std::slice::from_ref(&jpg),
        "casse ignorée, IMG_01 à part"
    );
    let changed = book.edit(&raf, |n| n.set_stars(5)).unwrap();
    assert_eq!(changed, [raf.clone(), jpg.clone()]);
    assert_eq!(
        xmp::read_sidecar(&dir.path().join("IMG.RAF.xmp"))
            .unwrap()
            .stars,
        5
    );
    assert_eq!(
        xmp::read_sidecar(&dir.path().join("img.jpg.xmp"))
            .unwrap()
            .stars,
        5
    );
    assert_eq!(book.notes(&other).stars, 0);
}

#[test]
fn a_pair_that_differs_is_brought_in_line() {
    let dir = tempfile::tempdir().unwrap();
    let raf = file(dir.path(), "IMG.RAF");
    let jpg = file(dir.path(), "IMG.JPG");
    fs::write(
        dir.path().join("IMG.RAF.xmp"),
        xmp_with(r#"xmp:Rating="3""#),
    )
    .unwrap();
    let mut book = NoteBook::load([raf.as_path(), jpg.as_path()], Convention::Darktable);
    // 3 étoiles de nouveau sur le RAF : rien ne change pour lui, mais le JPG suit.
    let changed = book.edit(&raf, |n| n.set_stars(3)).unwrap();
    assert_eq!(changed, std::slice::from_ref(&jpg));
    assert_eq!(book.notes(&jpg).stars, 3);
}

#[test]
fn a_malformed_sidecar_in_the_pair_is_reported_but_the_rest_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let raf = file(dir.path(), "IMG.RAF");
    let jpg = file(dir.path(), "IMG.JPG");
    fs::write(dir.path().join("IMG.JPG.xmp"), "<pas du xml").unwrap();
    let mut book = NoteBook::load([raf.as_path(), jpg.as_path()], Convention::Darktable);
    let err = book.edit(&raf, |n| n.set_stars(4)).unwrap_err();
    assert!(err.to_string().contains("IMG.JPG"), "{err}");
    assert_eq!(book.notes(&raf).stars, 4);
    assert_eq!(
        fs::read_to_string(dir.path().join("IMG.JPG.xmp")).unwrap(),
        "<pas du xml"
    );
}

#[test]
fn malformed_sidecar_is_never_edited() {
    let dir = tempfile::tempdir().unwrap();
    let a = file(dir.path(), "A.RAF");
    fs::write(dir.path().join("A.RAF.xmp"), "<pas du xml").unwrap();
    let mut book = NoteBook::load([a.as_path()], Convention::Darktable);
    assert!(book.edit(&a, |n| n.set_stars(2)).is_err());
    assert_eq!(
        fs::read_to_string(dir.path().join("A.RAF.xmp")).unwrap(),
        "<pas du xml"
    );
    assert_eq!(book.notes(&a).stars, 0);
}

#[test]
fn reload_sees_changes_made_by_other_software() {
    let dir = tempfile::tempdir().unwrap();
    let a = file(dir.path(), "A.RAF");
    let mut book = NoteBook::load([a.as_path()], Convention::Darktable);
    fs::write(dir.path().join("A.RAF.xmp"), xmp_with(r#"xmp:Rating="2""#)).unwrap();
    assert_eq!(book.notes(&a).stars, 0);
    book.reload();
    assert_eq!(book.notes(&a).stars, 2);
}

fn names(groups: &tri::filter::Groups) -> Vec<String> {
    groups
        .iter()
        .flatten()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn filters_on_a_scanned_folder() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["A.RAF", "B.RAF", "C.RAF", "D.JPG"] {
        file(dir.path(), name);
    }
    let files = media::scan(dir.path(), 1).unwrap();
    assert_eq!(files.len(), 4);
    let mut book = NoteBook::load_files(&files, Convention::Darktable);
    let path = |name: &str| -> PathBuf { dir.path().join(name) };
    book.edit(&path("A.RAF"), |n| n.set_stars(4)).unwrap();
    book.edit(&path("B.RAF"), |n| {
        n.set_stars(5);
        n.flag = Flag::Rejected;
    })
    .unwrap();
    book.edit(&path("C.RAF"), |n| n.toggle_color(Color::Green, false))
        .unwrap();

    let groups = || ungrouped(files.clone());
    let filtered = |rule: &NotesRule| names(&rule.filter(&book).apply(groups()));

    assert_eq!(filtered(&NotesRule::default()).len(), 4);
    let good = NotesRule {
        stars: Some(StarsRule::AtLeast(4)),
        flags: Some(vec![Flag::None, Flag::Accepted]),
        ..NotesRule::default()
    };
    assert_eq!(filtered(&good), ["A.RAF"]);
    let rejected = NotesRule {
        flags: Some(vec![Flag::Rejected]),
        ..NotesRule::default()
    };
    assert_eq!(filtered(&rejected), ["B.RAF"]);
    let green = NotesRule {
        colors: Some(ColorRule {
            any_of: vec![Color::Green],
            none: false,
        }),
        ..NotesRule::default()
    };
    assert_eq!(filtered(&green), ["C.RAF"]);
    let unrated = NotesRule {
        stars: Some(StarsRule::Exactly(0)),
        ..NotesRule::default()
    };
    assert_eq!(filtered(&unrated), ["C.RAF", "D.JPG"]);
    assert!(
        files
            .iter()
            .all(|f| f.kind == MediaKind::Raw || f.path.ends_with("D.JPG"))
    );
}

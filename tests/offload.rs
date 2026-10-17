//! Tests d'intégration du module `offload` : fausse carte dans un dossier temporaire.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, UNIX_EPOCH};

use tri::offload::{self, History, OffloadSettings};

/// 16/10/2026 à 10:00 UTC.
const SHOT: u64 = 1_792_144_800;

fn write(path: &Path, bytes: &[u8], secs: u64) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(secs))
        .unwrap();
}

/// Carte : deux RAW et un JPG dans deux dossiers du boîtier, une vidéo (ignorée).
fn card(root: &Path) {
    write(&root.join("DCIM/100_FUJI/DSCF0001.RAF"), b"raw-1", SHOT);
    write(&root.join("DCIM/100_FUJI/DSCF0001.JPG"), b"jpg-1", SHOT);
    write(
        &root.join("DCIM/101_FUJI/DSCF0002.RAF"),
        b"raw-2-longer",
        SHOT + 60,
    );
    write(
        &root.join("DCIM/101_FUJI/DSCF0003.MOV"),
        b"video",
        SHOT + 120,
    );
}

fn settings() -> OffloadSettings {
    OffloadSettings {
        destination: String::new(),
        verify: true,
        skip_known: true,
        history: String::new(),
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn a_card_is_copied_flat_verified_and_remembered() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, dst) = (
        tmp.path().join("carte"),
        tmp.path().join("photos/2026/séance"),
    );
    card(&src);
    let mut history = History::open(tmp.path().join("data/offload-history.tsv")).unwrap();
    let plan = offload::plan(&src, &history).unwrap();
    assert_eq!(plan.items.len(), 3);
    assert_eq!(plan.ignored, 1, "la vidéo");
    assert_eq!(plan.bytes(true), 5 + 5 + 12);
    assert_eq!(
        offload::suggest_folder(&plan, true).as_deref(),
        Some("2026/2026-10-16_"),
        "date de la plus ancienne image"
    );

    let mut seen = 0;
    let report = offload::run(
        &plan,
        &dst,
        &settings(),
        &mut history,
        &AtomicBool::new(false),
        |_| seen += 1,
    );
    assert!(seen > 0, "progression signalée");
    assert_eq!(report.copied.len(), 3);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(
        names(&dst),
        ["DSCF0001.JPG", "DSCF0001.RAF", "DSCF0002.RAF"],
        "à plat, sans temporaire"
    );
    let copy = dst.join("DSCF0002.RAF");
    assert_eq!(fs::read(&copy).unwrap(), b"raw-2-longer");
    assert_eq!(
        fs::metadata(&copy).unwrap().modified().unwrap(),
        UNIX_EPOCH + Duration::from_secs(SHOT + 60),
        "date d'origine conservée"
    );

    // Second déchargement, même vers un autre dossier : tout est déjà connu.
    let history = History::open(tmp.path().join("data/offload-history.tsv")).unwrap();
    let again = offload::plan(&src, &history).unwrap();
    assert!(
        again.items.iter().all(|i| i.known),
        "historique relu du disque"
    );
    assert_eq!(again.bytes(true), 0);
    let mut history = history;
    let other = tmp.path().join("photos/ailleurs");
    let report = offload::run(
        &again,
        &other,
        &settings(),
        &mut history,
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(report.skipped.len(), 3);
    assert!(names(&other).is_empty());
}

#[test]
fn name_clashes_and_identical_copies() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, dst) = (tmp.path().join("carte"), tmp.path().join("dest"));
    card(&src);
    // Même nom, autre contenu (compteur du boîtier remis à zéro) ; et une copie identique.
    write(&dst.join("DSCF0001.RAF"), b"autre photo", SHOT - 86_400);
    write(&dst.join("DSCF0002.RAF"), b"raw-2-longer", SHOT + 60);
    let mut history = History::in_memory();
    let plan = offload::plan(&src, &history).unwrap();
    let report = offload::run(
        &plan,
        &dst,
        &settings(),
        &mut history,
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(report.skipped, [src.join("DCIM/101_FUJI/DSCF0002.RAF")]);
    assert_eq!(fs::read(dst.join("DSCF0001-2.RAF")).unwrap(), b"raw-1");
    assert_eq!(
        fs::read(dst.join("DSCF0001.RAF")).unwrap(),
        b"autre photo",
        "rien écrasé"
    );
}

#[test]
fn only_verified_copies_are_deleted_from_the_card() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, dst) = (tmp.path().join("carte"), tmp.path().join("dest"));
    card(&src);
    let mut history = History::in_memory();
    let plan = offload::plan(&src, &history).unwrap();
    let report = offload::run(
        &plan,
        &dst,
        &settings(),
        &mut history,
        &AtomicBool::new(false),
        |_| {},
    );
    assert!(offload::delete_sources(&report).is_empty());
    assert!(!src.join("DCIM/100_FUJI/DSCF0001.RAF").exists());
    assert!(
        src.join("DCIM/101_FUJI/DSCF0003.MOV").exists(),
        "la vidéo n'est pas touchée"
    );
    assert_eq!(names(&dst).len(), 3, "copies intactes");
}

#[test]
fn cancelling_copies_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, dst) = (tmp.path().join("carte"), tmp.path().join("dest"));
    card(&src);
    let mut history = History::in_memory();
    let plan = offload::plan(&src, &history).unwrap();
    let report = offload::run(
        &plan,
        &dst,
        &settings(),
        &mut history,
        &AtomicBool::new(true),
        |_| {},
    );
    assert!(report.cancelled);
    assert!(report.copied.is_empty());
    assert!(
        offload::delete_sources(&report).is_empty(),
        "rien à supprimer"
    );
}

#[test]
fn destination_names_are_checked() {
    let root = PathBuf::from("/photos");
    assert!(offload::destination(&root, "../x").is_err());
    assert_eq!(
        offload::destination(&root, " 2026/2026-10-16_ ").unwrap(),
        root.join("2026/2026-10-16")
    );
}

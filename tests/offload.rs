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
        skip_known: false,
        history: String::new(),
        ..OffloadSettings::default()
    }
}

/// Réglages avec l'historique global (ignorer ce qui a déjà été déchargé n'importe où).
fn with_history() -> OffloadSettings {
    OffloadSettings {
        skip_known: true,
        ..settings()
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
    assert_eq!(plan.bytes(&settings()), 5 + 5 + 12);
    assert_eq!(
        offload::suggest_folder(&plan, &settings()).as_deref(),
        Some("2026-10-16_"),
        "date de la plus ancienne image"
    );

    let mut seen = 0;
    let mut copies = Vec::new();
    let report = offload::run(
        &plan,
        &dst,
        &settings(),
        &mut history,
        &AtomicBool::new(false),
        |_| seen += 1,
        |copy| copies.push(copy.to_path_buf()),
    );
    assert!(seen > 0, "progression signalée");
    // Chaque copie signalée dès qu'elle est faite (aperçus pendant la copie, issue n° 40).
    assert_eq!(
        copies,
        report
            .copied
            .iter()
            .map(|(_, c)| c.clone())
            .collect::<Vec<_>>()
    );
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

    // Historique global (option) : relu du disque, tout est connu, même ailleurs.
    let mut history = History::open(tmp.path().join("data/offload-history.tsv")).unwrap();
    let again = offload::plan(&src, &history).unwrap();
    assert!(
        again.items.iter().all(|i| i.known),
        "historique relu du disque"
    );
    assert_eq!(again.bytes(&with_history()), 0, "historique global : rien");
    assert_eq!(
        again.bytes(&settings()),
        22,
        "par défaut : tout, jugé sur la cible"
    );
    let other = tmp.path().join("photos/ailleurs");
    let stop = AtomicBool::new(false);
    let report = offload::run(
        &again,
        &other,
        &with_history(),
        &mut history,
        &stop,
        |_| {},
        |_| {},
    );
    assert_eq!(report.skipped.len(), 3);
    assert!(
        report.already_there.is_empty(),
        "rien vérifié dans « ailleurs »"
    );
    assert!(names(&other).is_empty());

    // Par défaut : même carte, second endroit → tout copié ; même endroit → doublons
    // reconnus (contenus comparés).
    assert_eq!(offload::already_in(&again, &settings(), &dst), (3, 22));
    assert_eq!(offload::already_in(&again, &settings(), &other), (0, 0));
    let report = offload::run(
        &again,
        &other,
        &settings(),
        &mut history,
        &stop,
        |_| {},
        |_| {},
    );
    assert_eq!(report.copied.len(), 3, "second endroit");
    let report = offload::run(
        &again,
        &dst,
        &settings(),
        &mut history,
        &stop,
        |_| {},
        |_| {},
    );
    assert!(report.copied.is_empty());
    assert_eq!(report.skipped.len(), 3, "même endroit : doublons ignorés");
    assert_eq!(report.already_there.len(), 3, "copies identiques vérifiées");
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
fn verified_duplicates_are_deleted_too_but_not_history_only_ones() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, dst) = (tmp.path().join("carte"), tmp.path().join("dest"));
    card(&src);
    let stop = AtomicBool::new(false);
    let mut history = History::in_memory();
    let plan = offload::plan(&src, &history).unwrap();
    offload::run(
        &plan,
        &dst,
        &settings(),
        &mut history,
        &stop,
        |_| {},
        |_| {},
    );
    // Seconde fois, même destination : rien de copié, tout « déjà là » et vérifié.
    let plan = offload::plan(&src, &history).unwrap();
    let report = offload::run(
        &plan,
        &dst,
        &settings(),
        &mut history,
        &stop,
        |_| {},
        |_| {},
    );
    assert!(report.copied.is_empty());
    assert!(offload::delete_sources(&report).is_empty());
    assert!(
        !src.join("DCIM/100_FUJI/DSCF0001.RAF").exists(),
        "doublon vérifié supprimé"
    );
    assert!(
        src.join("DCIM/101_FUJI/DSCF0003.MOV").exists(),
        "vidéo jamais touchée"
    );
    assert_eq!(names(&dst).len(), 3, "copies intactes");
    // Historique global seul (autre destination, rien vérifié) : rien n'est supprimé.
    card(&src);
    let plan = offload::plan(&src, &history).unwrap();
    let other = tmp.path().join("ailleurs");
    let report = offload::run(
        &plan,
        &other,
        &with_history(),
        &mut history,
        &stop,
        |_| {},
        |_| {},
    );
    assert_eq!(report.skipped.len(), 3);
    assert!(report.already_there.is_empty());
    assert!(offload::delete_sources(&report).is_empty());
    assert!(
        src.join("DCIM/100_FUJI/DSCF0001.RAF").exists(),
        "connu de l'historique : gardé"
    );
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
fn raw_only_leaves_the_jpegs_on_the_card() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, dst) = (tmp.path().join("carte"), tmp.path().join("dest"));
    card(&src);
    let raw_only = OffloadSettings {
        raw_only: true,
        ..settings()
    };
    let mut history = History::in_memory();
    let plan = offload::plan(&src, &history).unwrap();
    assert_eq!(plan.to_copy(&raw_only).count(), 2);
    assert_eq!(plan.bytes(&raw_only), 5 + 12);
    let mut counts = Vec::new();
    let report = offload::run(
        &plan,
        &dst,
        &raw_only,
        &mut history,
        &AtomicBool::new(false),
        |p| counts.push((p.files_done, p.files_total)),
        |_| {},
    );
    assert_eq!(report.copied.len(), 2);
    // L'avancement ne compte que les RAW, un par un (#38).
    assert!(counts.iter().all(|&(_, total)| total == 2), "{counts:?}");
    assert!(
        counts.windows(2).all(|w| w[1].0 - w[0].0 <= 1),
        "{counts:?}"
    );
    assert_eq!(counts.last(), Some(&(2, 2)));
    assert_eq!(report.left, [src.join("DCIM/100_FUJI/DSCF0001.JPG")]);
    assert_eq!(names(&dst), ["DSCF0001.RAF", "DSCF0002.RAF"]);
    // Suppression des JPEG laissés, sur demande : seulement eux.
    assert!(offload::delete_left(&report).is_empty());
    assert!(!src.join("DCIM/100_FUJI/DSCF0001.JPG").exists());
    assert!(
        src.join("DCIM/100_FUJI/DSCF0001.RAF").exists(),
        "le RAW n'est pas touché ici"
    );
    write(&src.join("DCIM/100_FUJI/DSCF0001.JPG"), b"jpg-1", SHOT);
    // Le JPEG laissé n'est pas « déjà déchargé » : un déchargement complet le prendra.
    let plan = offload::plan(&src, &history).unwrap();
    assert_eq!(plan.to_copy(&with_history()).count(), 1);
}

#[test]
fn the_root_is_remembered_and_a_known_card_still_gets_a_name() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let defaults = settings();
    assert_eq!(offload::root(&defaults, Some(&data)), defaults.destination);
    offload::remember_root(&data, " ~/Photos/Sources/2026 ").unwrap();
    assert_eq!(
        offload::root(&defaults, Some(&data)),
        "~/Photos/Sources/2026"
    );
    // Carte déjà entièrement déchargée (historique global) : rien à copier, mais un nom
    // quand même.
    let global = with_history();
    let src = tmp.path().join("carte");
    card(&src);
    let mut history = History::in_memory();
    let plan = offload::plan(&src, &history).unwrap();
    let stop = AtomicBool::new(false);
    offload::run(
        &plan,
        &tmp.path().join("d"),
        &global,
        &mut history,
        &stop,
        |_| {},
        |_| {},
    );
    let again = offload::plan(&src, &history).unwrap();
    assert_eq!(again.to_copy(&global).count(), 0);
    assert_eq!(
        offload::suggest_folder(&again, &global).as_deref(),
        Some("2026-10-16_")
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

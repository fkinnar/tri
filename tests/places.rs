//! Tests d'intégration du module `places` : sous-dossiers d'un dossier temporaire,
//! emplacements de la machine.

use std::fs;

use tri::places::{self, PlaceKind};

#[test]
fn subfolders_are_sorted_and_hide_dot_folders() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["b", "A", ".cache", "c"] {
        fs::create_dir(dir.path().join(name)).unwrap();
    }
    fs::write(dir.path().join("photo.RAF"), b"").unwrap();
    let names = |hidden| -> Vec<String> {
        places::subfolders(dir.path(), hidden)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    assert_eq!(
        names(false),
        ["A", "b", "c"],
        "casse ignorée, fichiers écartés"
    );
    assert_eq!(names(true), [".cache", "A", "b", "c"]);
}

#[cfg(unix)]
#[test]
fn a_link_to_a_folder_is_followed() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("vrai")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("vrai"), dir.path().join("lien")).unwrap();
    assert_eq!(places::subfolders(dir.path(), false).unwrap().len(), 2);
}

#[test]
fn a_missing_folder_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(places::subfolders(&dir.path().join("absent"), false).is_err());
}

#[test]
fn this_machine_has_a_home_and_a_root() {
    let all = places::places();
    if cfg!(unix) {
        assert!(all.iter().any(|p| p.kind == PlaceKind::Home));
        assert!(all.iter().any(|p| p.kind == PlaceKind::Root));
    }
    assert!(all.iter().all(|p| p.path.is_absolute()), "{all:?}");
}

fn tree_of(root: &std::path::Path) -> places::Tree {
    places::Tree::with_places(
        vec![places::Place {
            name: "Carte".into(),
            path: root.to_path_buf(),
            kind: PlaceKind::Volume,
        }],
        false,
    )
}

#[test]
fn the_tree_unfolds_lazily_and_lists_visible_rows() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("DCIM/100_FUJI")).unwrap();
    fs::create_dir_all(dir.path().join("MISC")).unwrap();
    let mut tree = tree_of(dir.path());
    let rows = |t: &places::Tree| -> Vec<(String, usize)> {
        t.rows().into_iter().map(|r| (r.name, r.depth)).collect()
    };
    assert_eq!(rows(&tree), [("Carte".to_string(), 0)]);
    tree.toggle(dir.path());
    assert_eq!(
        rows(&tree),
        [("Carte".into(), 0), ("DCIM".into(), 1), ("MISC".into(), 1)]
    );
    tree.toggle(&dir.path().join("DCIM"));
    assert_eq!(rows(&tree)[2], ("100_FUJI".into(), 2));
    tree.toggle(dir.path());
    assert_eq!(rows(&tree).len(), 1, "replié");
    // Déplié à nouveau : DCIM est resté déplié en dessous.
    tree.toggle(dir.path());
    assert_eq!(rows(&tree).len(), 4);
}

#[test]
fn reveal_unfolds_the_ancestors_of_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("DCIM/100_FUJI");
    fs::create_dir_all(&target).unwrap();
    let mut tree = tree_of(dir.path());
    assert!(tree.reveal(&target));
    let rows = tree.rows();
    let row = rows.iter().find(|r| r.path == target).expect("visible");
    assert_eq!(row.depth, 2);
    assert!(!row.expanded, "le dossier lui-même n'est pas déplié");
    assert!(!tree.reveal(std::path::Path::new("/ailleurs")));
}

#[test]
fn reload_sees_new_folders() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = tree_of(dir.path());
    tree.expand(dir.path());
    assert_eq!(tree.rows().len(), 1);
    fs::create_dir(dir.path().join("nouveau")).unwrap();
    assert_eq!(tree.rows().len(), 1, "pas relu tout seul");
    tree.reload();
    assert_eq!(tree.rows().len(), 2);
}

#[cfg(unix)]
#[test]
fn an_unreadable_folder_shows_an_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("verrouillé");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let mut tree = tree_of(dir.path());
    tree.expand(dir.path());
    tree.expand(&locked);
    let row = tree.rows().into_iter().find(|r| r.path == locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(row.error.is_some());
}

fn volume(path: &std::path::Path) -> places::Place {
    places::Place {
        name: path.file_name().unwrap().to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        kind: PlaceKind::Volume,
    }
}

#[test]
fn time_machine_volumes_are_recognised_by_their_content() {
    let dir = tempfile::tempdir().unwrap();
    let apfs = dir.path().join("MesSauvegardes");
    let hfs = dir.path().join("Ancien");
    let photos = dir.path().join("Photos");
    for d in [&apfs, &hfs, &photos] {
        fs::create_dir(d).unwrap();
    }
    fs::write(apfs.join("backup_manifest.plist"), b"").unwrap();
    fs::create_dir(hfs.join("Backups.backupdb")).unwrap();
    let filter = places::PlaceSettings::default();
    assert!(filter.hides(&volume(&apfs)), "APFS, quel que soit le nom");
    assert!(filter.hides(&volume(&hfs)));
    assert!(!filter.hides(&volume(&photos)));
    let shown = places::PlaceSettings {
        hide_backups: false,
        ..places::PlaceSettings::default()
    };
    assert!(!shown.hides(&volume(&apfs)), "option désactivée");
}

#[test]
fn excluded_places_match_name_or_path() {
    let filter = places::PlaceSettings {
        hide_backups: false,
        exclude: vec!["backup".into(), "/mnt/*".into()],
        ..places::PlaceSettings::default()
    };
    let place = |name: &str, path: &str| places::Place {
        name: name.into(),
        path: path.into(),
        kind: PlaceKind::Volume,
    };
    assert!(
        filter.hides(&place("Backup", "/Volumes/Backup")),
        "nom, sans casse"
    );
    assert!(filter.hides(&place("nas", "/mnt/nas")), "chemin");
    assert!(!filter.hides(&place("Work", "/Volumes/Work")));
}

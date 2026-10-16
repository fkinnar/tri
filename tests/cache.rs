//! Tests d'intégration du cache persistant, dans des dossiers temporaires.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tri::bitmap::Bitmap;
use tri::cache::{Cache, Key, Level, Usage};

/// Dégradé 64×48 : assez de structure pour vérifier que le JPEG relu est le bon.
fn gradient() -> Bitmap {
    let (w, h) = (64u32, 48u32);
    let data = (0..h)
        .flat_map(|y| (0..w).flat_map(move |x| [(x * 4) as u8, (y * 5) as u8, 128]))
        .collect();
    Bitmap {
        width: w,
        height: h,
        data,
    }
}

/// Un cache et un fichier source factice, dans un même dossier temporaire.
struct Setup {
    _dir: tempfile::TempDir,
    cache: Cache,
    source: PathBuf,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::open(dir.path().join("cache")).unwrap();
    let source = dir.path().join("DSCF0001.RAF");
    fs::write(&source, b"contenu RAW factice").unwrap();
    Setup {
        _dir: dir,
        cache,
        source,
    }
}

fn key(source: &Path, level: Level) -> Key {
    Key::new(source, level, "test").unwrap()
}

/// Tous les fichiers `.jpg` du cache (les tests ont besoin d'y toucher directement).
fn jpg_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "jpg") {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn put_then_get_roundtrips_through_jpeg() {
    let s = setup();
    let k = key(&s.source, Level::Standard);
    assert!(!s.cache.contains(&k));
    assert!(s.cache.get(&k).is_none());

    s.cache.put(&k, &gradient()).unwrap();
    assert!(s.cache.contains(&k));
    let back = s.cache.get(&k).unwrap();
    assert_eq!((back.width, back.height), (64, 48));
    // JPEG avec perte : écart moyen faible.
    let diff: u64 = back
        .data
        .iter()
        .zip(&gradient().data)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    assert!(
        diff / (back.data.len() as u64) < 4,
        "écart moyen trop grand"
    );
}

#[test]
fn put_overwrites_and_remove_deletes() {
    let s = setup();
    let k = key(&s.source, Level::Thumbnail);
    s.cache.put(&k, &gradient()).unwrap();
    s.cache.put(&k, &gradient().fit(16)).unwrap();
    assert_eq!(s.cache.get(&k).unwrap().width, 16);
    s.cache.remove(&k).unwrap();
    assert!(!s.cache.contains(&k));
    s.cache.remove(&k).unwrap(); // pas d'erreur si déjà absente
}

#[test]
fn modifying_the_source_changes_the_key() {
    let s = setup();
    let before = key(&s.source, Level::Standard);
    s.cache.put(&before, &gradient()).unwrap();

    // Même taille, autre date de modification.
    let file = fs::File::options().write(true).open(&s.source).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(60))
        .unwrap();
    drop(file);
    let touched = key(&s.source, Level::Standard);
    assert_ne!(touched, before);
    assert!(s.cache.get(&touched).is_none());

    // Autre taille.
    fs::write(&s.source, b"nouveau contenu, plus long").unwrap();
    assert_ne!(key(&s.source, Level::Standard), touched);
}

#[test]
fn key_depends_on_level_and_params_not_on_path_spelling() {
    let s = setup();
    assert_ne!(key(&s.source, Level::Standard), key(&s.source, Level::Full));
    assert_ne!(
        Key::new(&s.source, Level::Standard, "side=2560").unwrap(),
        Key::new(&s.source, Level::Standard, "side=3840").unwrap()
    );
    // `cargo test` s'exécute à la racine du paquet : chemin relatif et absolu se rejoignent.
    let absolute = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    assert_eq!(
        key(Path::new("Cargo.toml"), Level::Full),
        key(&absolute, Level::Full)
    );
}

#[test]
fn missing_source_is_an_error() {
    let s = setup();
    assert!(Key::new(&s.source.with_extension("absent"), Level::Full, "").is_err());
}

#[test]
fn levels_are_isolated_and_clearable() {
    let s = setup();
    let keys = Level::ALL.map(|l| key(&s.source, l));
    for k in &keys {
        s.cache.put(k, &gradient()).unwrap();
    }
    s.cache.clear(Some(Level::Standard)).unwrap();
    assert!(s.cache.contains(&keys[0]));
    assert!(!s.cache.contains(&keys[1]));
    assert!(s.cache.contains(&keys[2]));
    s.cache.clear(None).unwrap();
    assert!(keys.iter().all(|k| !s.cache.contains(k)));
    // Le cache reste utilisable après un vidage.
    s.cache.put(&keys[1], &gradient()).unwrap();
    assert!(s.cache.contains(&keys[1]));
}

#[test]
fn usage_counts_entries_and_ignores_temp_files() {
    let s = setup();
    assert_eq!(s.cache.usage(Level::Full).unwrap(), Usage::default());
    let k = key(&s.source, Level::Full);
    s.cache.put(&k, &gradient()).unwrap();
    let usage = s.cache.usage(Level::Full).unwrap();
    assert_eq!(usage.entries, 1);
    assert!(usage.bytes > 0);

    let entry = &jpg_files(s.cache.root())[0];
    fs::write(entry.with_extension("jpg.123-0.tmp"), b"en cours").unwrap();
    assert_eq!(s.cache.usage(Level::Full).unwrap(), usage);
}

#[test]
fn evict_removes_least_recently_used_first() {
    let s = setup();
    let names = ["a.RAF", "b.RAF", "c.RAF"];
    let keys: Vec<Key> = names
        .iter()
        .map(|n| {
            let src = s.source.with_file_name(n);
            fs::write(&src, n.as_bytes()).unwrap();
            let k = key(&src, Level::Full);
            s.cache.put(&k, &gradient()).unwrap();
            std::thread::sleep(Duration::from_millis(20));
            k
        })
        .collect();
    // a relu en dernier : b devient le moins récemment utilisé.
    s.cache.get(&keys[0]).unwrap();

    let total = s.cache.usage(Level::Full).unwrap().bytes;
    let freed = s.cache.evict(Level::Full, total - 1).unwrap();
    assert_eq!(freed.entries, 1);
    assert!(s.cache.contains(&keys[0]));
    assert!(!s.cache.contains(&keys[1]));
    assert!(s.cache.contains(&keys[2]));

    let nothing = s.cache.evict(Level::Full, u64::MAX).unwrap();
    assert_eq!(nothing, Usage::default());
    s.cache.evict(Level::Full, 0).unwrap();
    assert_eq!(s.cache.usage(Level::Full).unwrap().entries, 0);
}

#[test]
fn corrupt_entry_is_dropped() {
    let s = setup();
    let k = key(&s.source, Level::Standard);
    s.cache.put(&k, &gradient()).unwrap();
    let entry = &jpg_files(s.cache.root())[0];
    fs::write(entry, b"pas un JPEG").unwrap();
    assert!(s.cache.get(&k).is_none());
    assert!(
        !s.cache.contains(&k),
        "l'entrée corrompue doit être supprimée"
    );
}

#[test]
fn concurrent_puts_and_gets_never_see_partial_entries() {
    let s = setup();
    let k = key(&s.source, Level::Standard);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..20 {
                    s.cache.put(&k, &gradient()).unwrap();
                    if let Some(b) = s.cache.get(&k) {
                        assert_eq!((b.width, b.height), (64, 48));
                    }
                }
            });
        }
    });
    assert_eq!(s.cache.usage(Level::Standard).unwrap().entries, 1);
}

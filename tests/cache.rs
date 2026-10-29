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
fn quality_is_clamped_and_shrinks_entries() {
    let s = setup();
    assert_eq!(s.cache.quality(), tri::cache::DEFAULT_JPEG_QUALITY);
    assert_eq!(s.cache.clone().with_quality(0).quality(), 1);
    assert_eq!(s.cache.clone().with_quality(200).quality(), 100);
    let size_at = |quality: u8, level: Level| {
        let cache = s.cache.clone().with_quality(quality);
        let k = key(&s.source, level);
        cache.put(&k, &gradient()).unwrap();
        cache.usage(level).unwrap().bytes
    };
    let (low, high) = (size_at(20, Level::Thumbnail), size_at(98, Level::Standard));
    assert!(low < high, "qualité 20 : {low} o, qualité 98 : {high} o");
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
    // Relevés retenus : oubliés comme le fait la surveillance du dossier.
    tri::cache::forget_stamps([s.source.as_path()]);
    let touched = key(&s.source, Level::Standard);
    assert_ne!(touched, before);
    assert!(s.cache.get(&touched).is_none());

    // Autre taille.
    fs::write(&s.source, b"nouveau contenu, plus long").unwrap();
    tri::cache::forget_stamps([s.source.as_path()]);
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
    for (level, k) in Level::ALL.iter().zip(&keys) {
        assert_eq!(
            s.cache.contains(k),
            *level != Level::Standard,
            "{level:?} après vidage du niveau Standard"
        );
    }
    s.cache.clear(None).unwrap();
    assert!(keys.iter().all(|k| !s.cache.contains(k)));
    // Le cache reste utilisable après un vidage.
    let standard = key(&s.source, Level::Standard);
    s.cache.put(&standard, &gradient()).unwrap();
    assert!(s.cache.contains(&standard));
}

#[test]
fn a_folder_is_cleared_whatever_the_settings() {
    let s = setup();
    let other_dir = s.source.parent().unwrap().join("autre");
    fs::create_dir(&other_dir).unwrap();
    let other = other_dir.join("DSCF0002.RAF");
    fs::write(&other, b"autre RAW").unwrap();
    // Deux réglages différents (ex. deux tailles d'écran) : les deux entrées partent.
    let here = [
        Key::new(&s.source, Level::Standard, "side=2560").unwrap(),
        Key::new(&s.source, Level::Standard, "side=3840").unwrap(),
        Key::new(&s.source, Level::Thumbnail, "test").unwrap(),
    ];
    let elsewhere = key(&other, Level::Standard);
    for k in here.iter().chain([&elsewhere]) {
        s.cache.put(k, &gradient()).unwrap();
    }
    let folder = s.source.parent().unwrap();
    let freed = s.cache.clear_folder(folder, Some(Level::Standard)).unwrap();
    assert_eq!(freed.entries, 2);
    assert!(freed.bytes > 0);
    assert!(!s.cache.contains(&here[0]) && !s.cache.contains(&here[1]));
    assert!(s.cache.contains(&here[2]), "autre niveau gardé");
    assert!(s.cache.contains(&elsewhere), "sous-dossier gardé");
    assert_eq!(s.cache.clear_folder(folder, None).unwrap().entries, 1);
    assert!(!s.cache.contains(&here[2]));
    assert!(s.cache.contains(&elsewhere));
    // Rien à vider : pas d'erreur.
    assert_eq!(
        s.cache.clear_folder(folder, None).unwrap(),
        Usage::default()
    );
}

#[test]
fn old_formats_are_removed_in_the_background() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("v1").join("full").join("ab");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("x.jpg"), b"ancien").unwrap();
    let _cache = Cache::open(dir.path()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while dir.path().join("v1").exists() {
        assert!(std::time::Instant::now() < deadline, "ancien format resté");
        std::thread::sleep(Duration::from_millis(10));
    }
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

#[test]
fn a_disk_keeps_its_cache_and_finds_it_mounted_elsewhere() {
    // Issue n° 55 : un disque amovible branché sur plusieurs ordinateurs.
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::open(dir.path().join("cache")).unwrap();
    let disk = dir.path().join("disque");
    let photo = disk.join("2026/séance/DSCF0001.RAF");
    fs::create_dir_all(photo.parent().unwrap()).unwrap();
    fs::write(&photo, b"contenu RAW factice").unwrap();
    let key = Key::new(&photo, Level::Thumbnail, "s=400").unwrap();
    cache.put(&key, &gradient()).unwrap();
    let analysis = Key::new(&photo, Level::Exposure, "v").unwrap();
    cache.put_bytes(&analysis, b"histogrammes").unwrap();
    assert!(cache.disk_of(photo.parent().unwrap()).is_none());
    // Sur le disque : les entrées déjà calculées y passent, rien à recalculer.
    let moved = cache.use_disk(&disk).unwrap();
    assert_eq!(moved.entries, 2);
    assert!(disk.join(tri::cache::DISK_DIR).is_dir());
    assert_eq!(cache.disk_of(photo.parent().unwrap()), Some(disk.clone()));
    assert!(cache.contains(&key));
    assert_eq!(cache.usage(Level::Thumbnail).unwrap(), Usage::default());
    // Monté ailleurs (autre ordinateur, autre cache central) : retrouvé.
    let elsewhere = dir.path().join("media/autre-nom");
    fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
    fs::rename(&disk, &elsewhere).unwrap();
    let other = Cache::open(dir.path().join("autre-cache")).unwrap();
    let moved_photo = elsewhere.join("2026/séance/DSCF0001.RAF");
    let key = Key::new(&moved_photo, Level::Thumbnail, "s=400").unwrap();
    assert!(other.contains(&key), "cache du disque retrouvé");
    assert_eq!(
        other.get_bytes(&Key::new(&moved_photo, Level::Exposure, "v").unwrap()),
        Some(b"histogrammes".to_vec())
    );
    // Vidé par dossier comme le cache central.
    let image = Key::new(&moved_photo, Level::Standard, "x").unwrap();
    other.put(&image, &gradient()).unwrap();
    let freed = other
        .clear_folder(moved_photo.parent().unwrap(), Some(Level::Standard))
        .unwrap();
    assert_eq!(freed.entries, 1);
    // Plus sur le disque : retour dans le cache central, dossier caché supprimé.
    let back = other.release_disk(&elsewhere).unwrap();
    assert_eq!(back.entries, 2);
    assert!(!elsewhere.join(tri::cache::DISK_DIR).exists());
    assert!(other.disk_of(moved_photo.parent().unwrap()).is_none());
    assert!(other.contains(&key));
    assert_eq!(other.usage(Level::Thumbnail).unwrap().entries, 1);
}

#[test]
fn a_folder_keeps_its_cache_inside_or_brings_it_along_when_moved() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::open(dir.path().join("cache")).unwrap();
    let disk = dir.path().join("disque");
    let seance = disk.join("séance");
    let photo = seance.join("sous/IMG.JPG");
    fs::create_dir_all(photo.parent().unwrap()).unwrap();
    fs::write(&photo, b"jpeg").unwrap();
    // Cache sur le disque, puis dans la séance elle-même : il descend d'un cran.
    cache.use_disk(&disk).unwrap();
    let key = Key::new(&photo, Level::Thumbnail, "s").unwrap();
    cache.put(&key, &gradient()).unwrap();
    assert_eq!(cache.use_disk(&seance).unwrap().entries, 1);
    assert_eq!(cache.disk_of(photo.parent().unwrap()), Some(seance.clone()));
    assert!(cache.contains(&key));
    // Rendu à l'étage du dessus (le disque), pas au cache central.
    assert_eq!(cache.release_disk(&seance).unwrap().entries, 1);
    assert_eq!(cache.disk_of(photo.parent().unwrap()), Some(disk.clone()));
    assert!(cache.contains(&key));
    cache.release_disk(&disk).unwrap();
    // Dossier déplacé : ses entrées (sous-dossiers compris) le suivent.
    let moved = dir.path().join("ailleurs/séance");
    fs::create_dir_all(moved.parent().unwrap()).unwrap();
    fs::rename(&seance, &moved).unwrap();
    assert_eq!(cache.relocate(&seance, &moved).unwrap().entries, 1);
    let key = Key::new(&moved.join("sous/IMG.JPG"), Level::Thumbnail, "s").unwrap();
    assert!(cache.contains(&key), "le cache a suivi le dossier");
}

#[test]
fn only_a_1_1_read_counts_as_a_use() {
    // Issue n° 74 : une lecture n'écrit plus rien, sauf pour un 1:1 (date de dernier usage,
    // pour l'éviction) ; une écriture par lecture ralentissait tout sur une clé USB.
    let s = setup();
    let old = SystemTime::now() - Duration::from_secs(3600);
    let mut seen = Vec::new();
    let mut dates = Vec::new();
    for level in [Level::Standard, Level::Full] {
        let k = key(&s.source, level);
        s.cache.put(&k, &gradient()).unwrap();
        let path = jpg_files(s._dir.path())
            .into_iter()
            .find(|p| !seen.contains(p))
            .unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert!(s.cache.get(&k).is_some());
        dates.push(fs::metadata(&path).unwrap().modified().unwrap());
        seen.push(path);
    }
    assert_eq!(dates[0], old, "aperçu standard : date inchangée");
    assert!(dates[1] > old, "1:1 : date de dernier usage");
}

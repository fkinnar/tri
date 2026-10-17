//! Tests d'intégration du module settings : lecture de fichiers réels.

use std::fs;

use tri::settings::{self, Auto, EngineSettings};

#[test]
fn example_file_matches_the_engine_defaults() {
    let text = include_str!("../doc/config.example.toml");
    let parsed: EngineSettings = settings::parse(text).unwrap();
    assert_eq!(parsed, EngineSettings::default());
}

#[test]
fn valid_file_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        "[preview]\nthumbnail = 300\n[cache]\njpeg_quality = 80\n",
    )
    .unwrap();
    let loaded = settings::load::<EngineSettings>(Some(&path));
    assert_eq!(loaded.warning, None);
    assert_eq!(loaded.path.as_deref(), Some(path.as_path()));
    assert_eq!(loaded.settings.preview.thumbnail, 300);
    assert_eq!(loaded.settings.preview.standard, Auto::Auto);
    assert_eq!(loaded.settings.cache.jpeg_quality, 80);
}

#[test]
fn invalid_file_gives_defaults_and_a_warning_naming_the_file_and_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "[preview]\nthumbnial = 300\n").unwrap();
    let loaded = settings::load::<EngineSettings>(Some(&path));
    assert_eq!(loaded.settings, EngineSettings::default());
    let warning = loaded.warning.expect("avertissement");
    assert!(warning.contains("config.toml"), "{warning}");
    assert!(warning.contains("thumbnial"), "{warning}");
    assert!(warning.contains("réglages par défaut"), "{warning}");
}

#[test]
fn explicit_missing_file_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent.toml");
    let loaded = settings::load::<EngineSettings>(Some(&path));
    assert_eq!(loaded.settings, EngineSettings::default());
    assert_eq!(loaded.path, None);
    assert!(loaded.warning.unwrap().contains("absent.toml"));
}

#[test]
fn configured_cache_dir_and_quality_are_used() {
    let dir = tempfile::tempdir().unwrap();
    let s: EngineSettings = settings::parse(&format!(
        "[cache]\ndir = {:?}\njpeg_quality = 70",
        dir.path().join("cache").to_str().unwrap()
    ))
    .unwrap();
    let cache = s.open_cache().unwrap();
    assert_eq!(cache.root(), dir.path().join("cache"));
    assert_eq!(cache.quality(), 70);
}

#[test]
fn zoom_entries_are_trimmed_to_the_limit() {
    use tri::cache::{Key, Level};
    let dir = tempfile::tempdir().unwrap();
    let mut settings = tri::settings::EngineSettings::default();
    settings.cache.dir = Some(dir.path().join("cache"));
    settings.cache.zoom_max_mb = 1;
    let cache = settings.open_cache().unwrap();
    // Trois 1:1 de 600 ko : au-delà d'1 Mo, les deux plus anciens partent.
    for i in 0..3 {
        let source = dir.path().join(format!("img{i}.RAF"));
        std::fs::write(&source, b"raw").unwrap();
        let key = Key::new(&source, Level::Full, "test").unwrap();
        cache.put_bytes(&key, &vec![0u8; 600_000]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let freed = settings.trim_cache(&cache).unwrap();
    assert_eq!(freed.entries, 2);
    assert_eq!(cache.usage(Level::Full).unwrap().entries, 1);
    settings.cache.zoom_max_mb = 0;
    assert_eq!(
        settings.trim_cache(&cache).unwrap().entries,
        0,
        "0 : pas de limite"
    );
}

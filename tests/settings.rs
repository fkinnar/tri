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

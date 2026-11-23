//! Palettes de couleurs (`tri::palette`), par l'API publique : une vraie image décodée
//! comme dans le cache, et la section `[palette]` du fichier de configuration.

use tri::palette::{self, Palette, PaletteSettings};

#[test]
fn a_decoded_jpeg_gets_a_palette_whose_shares_add_up() {
    let bytes = std::fs::read("tests/fixtures/exif.jpg").unwrap();
    let image = tri::colour::decode_to_working(&bytes).expect("JPEG décodé");
    let p = palette::extract(&image, &PaletteSettings::default());
    assert!(!p.swatches.is_empty() && p.swatches.len() <= 6, "{p:?}");
    let total: f32 = p.swatches.iter().map(|s| s.share).sum();
    assert!((total - 1.0).abs() < 1e-3);
    assert!(p.swatches.windows(2).all(|w| w[0].share >= w[1].share));
    assert_eq!(Palette::from_bytes(&p.to_bytes()), Some(p.clone()));
    assert_eq!(palette::distance(&p, &p, 2.0), 0.0);
}

#[test]
fn palette_settings_are_read_from_the_configuration() {
    let s: tri::settings::EngineSettings =
        tri::settings::parse("[colours]\ncolors = 8\nmin_share = 0.05\n").unwrap();
    assert_eq!(s.palette.colors, 8);
    assert_eq!(s.palette.min_share, 0.05);
    assert_eq!(s.palette.side, PaletteSettings::default().side);
}

#[test]
fn a_palette_is_computed_from_the_cached_thumbnail_and_kept_per_source() {
    use std::sync::atomic::AtomicBool;
    use tri::cache::{Cache, Level};
    use tri::media::{MediaFile, MediaKind};
    use tri::palette::Source;
    let dir = tempfile::tempdir().unwrap();
    let photo = dir.path().join("photo.jpg");
    std::fs::copy("tests/fixtures/exif.jpg", &photo).unwrap();
    let cache = Cache::open(dir.path().join("cache")).unwrap();
    let file = MediaFile {
        path: photo,
        kind: MediaKind::Flat,
    };
    let settings = tri::preview::Settings::default();
    let cancel = AtomicBool::new(false);
    assert!(tri::preview::palette(&cache, &file, Source::Camera, &settings).is_none());
    tri::preview::ensure(
        &cache,
        &file,
        Level::Palette(Source::Camera),
        &settings,
        &cancel,
    )
    .unwrap();
    let camera = tri::preview::palette(&cache, &file, Source::Camera, &settings).unwrap();
    assert!(!camera.swatches.is_empty());
    // La vignette d'où elle vient est passée par le cache.
    let thumb = tri::preview::key(&file, Level::Thumbnail, &settings).unwrap();
    assert!(cache.contains(&thumb));
    // Une autre source : une autre entrée.
    assert!(tri::preview::palette(&cache, &file, Source::Raw, &settings).is_none());
}

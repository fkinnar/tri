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
    assert_eq!(palette::distance(&p, &p), 0.0);
}

#[test]
fn palette_settings_are_read_from_the_configuration() {
    let s: tri::settings::EngineSettings =
        tri::settings::parse("[palette]\ncolors = 8\nmin_share = 0.05\n").unwrap();
    assert_eq!(s.palette.colors, 8);
    assert_eq!(s.palette.min_share, 0.05);
    assert_eq!(s.palette.side, PaletteSettings::default().side);
}

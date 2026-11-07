//! Tests d'intégration de `tri::embedded` (API publique) : notes inscrites dans les
//! images elles-mêmes (issue n° 276).

use std::io::Cursor;

use tri::embedded::{self, Container};
use tri::xmp::{Color, Flag, Notes};

fn image(format: image::ImageFormat) -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbImage::from_pixel(8, 6, image::Rgb([30, 120, 200]))
        .write_to(&mut Cursor::new(&mut out), format)
        .unwrap();
    out
}

#[test]
fn ratings_round_trip_in_every_supported_format() {
    let dir = tempfile::tempdir().unwrap();
    let mut wanted = Notes::default();
    wanted.set_stars(3);
    wanted.flag = Flag::Accepted;
    wanted.colors = vec![Color::Blue];
    for (name, format) in [
        ("a.JPG", image::ImageFormat::Jpeg),
        ("b.png", image::ImageFormat::Png),
        ("c.webp", image::ImageFormat::WebP),
        ("d.TIFF", image::ImageFormat::Tiff),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, image(format)).unwrap();
        assert!(embedded::supports(&path), "{name}");
        assert!(embedded::would_change(&path, &wanted).unwrap(), "{name}");
        assert!(embedded::write(&path, &wanted).unwrap(), "{name}");
        let back = embedded::read(&path).unwrap().expect(name);
        assert_eq!(
            (back.stars, back.flag, back.colors.clone()),
            (3, Flag::Accepted, vec![Color::Blue])
        );
        image::open(&path).unwrap();
    }
}

#[test]
fn raws_and_unknown_files_carry_no_ratings() {
    for name in ["a.RAF", "b.CR3", "c.dng", "d.heic", "e.txt"] {
        assert_eq!(Container::of(std::path::Path::new(name)), None, "{name}");
    }
}

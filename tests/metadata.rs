//! Tests d'intégration du module `metadata` : EXIF d'une petite fixture synthétique
//! (`tests/fixtures/exif.jpg`, balises posées par ExifTool), vrai RAW (`TRI_TEST_RAW`,
//! ignoré par défaut).

use tri::media::{MediaFile, MediaKind};
use tri::metadata;

#[test]
fn exif_of_a_jpeg() {
    let file = MediaFile {
        path: "tests/fixtures/exif.jpg".into(),
        kind: MediaKind::Flat,
    };
    let m = metadata::read(&file).unwrap();
    assert_eq!(m.camera().as_deref(), Some("TestMake TestCam 1"));
    assert_eq!(m.lens.as_deref(), Some("Test 35mm F2"));
    assert_eq!(
        m.exposure_text().as_deref(),
        Some("1/250 s · f/2,8 · ISO 400")
    );
    assert!((m.exposure_bias.unwrap() + 0.7).abs() < 0.01);
    assert_eq!(m.program.as_deref(), Some("Priorité ouverture"));
    assert_eq!(m.flash, Some(true));
    assert_eq!(m.date.as_deref(), Some("2026-10-16 14:32:05"));
    assert_eq!(
        m.size,
        Some((12, 16)),
        "orientation 6 : dimensions échangées"
    );
    assert!(m.file_size > 0);
}

#[test]
fn an_image_without_exif_has_empty_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sans.png");
    image::RgbImage::new(8, 4).save(&path).unwrap();
    let file = MediaFile {
        path,
        kind: MediaKind::Flat,
    };
    let m = metadata::read(&file).unwrap();
    assert_eq!(m.camera(), None);
    assert_eq!(m.size, Some((8, 4)), "dimensions lues dans l'en-tête");
}

#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn a_real_raw() {
    let path = std::env::var("TRI_TEST_RAW").expect("TRI_TEST_RAW");
    let file = MediaFile {
        path: path.into(),
        kind: MediaKind::Raw,
    };
    let m = metadata::read(&file).unwrap();
    assert!(m.camera().is_some());
    assert!(m.exposure_text().is_some());
    assert!(m.size.is_some_and(|(w, h)| w > 1000 && h > 1000));
}

/// Collimateur d'un vrai RAF Fujifilm (`TRI_TEST_RAW`) : lu dans la note du fabricant,
/// dans l'image affichée (entre 0 et 1).
#[test]
#[ignore = "nécessite TRI_TEST_RAW (RAF Fujifilm)"]
fn a_fuji_raw_gives_its_focus_point() {
    let path = std::env::var("TRI_TEST_RAW").expect("TRI_TEST_RAW");
    let file = tri::media::MediaFile {
        path: path.into(),
        kind: tri::media::MediaKind::Raw,
    };
    let m = tri::metadata::read(&file).unwrap();
    let (x, y) = m.focus_point.expect("collimateur");
    println!("collimateur : ({x:.3}, {y:.3})");
    assert!((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y));
}

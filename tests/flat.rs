//! Tests d'intégration du décodage des images plates.
//!
//! Les images de test sont générées à la volée (PNG, TIFF, WebP, JPEG), sauf le HEIC :
//! `tests/fixtures/quadrants.heic` (64×48, quadrants rouge / vert / bleu / blanc), produit
//! par `heif-enc -q 95 -p chroma=444`. Un vrai HEIC d'iPhone peut être testé en plus via
//! `TRI_TEST_HEIC` (test ignoré par défaut).
//!
//! Les tests HEIF dépendent de la feature `heif`. Sans elle (`--no-default-features`), on
//! vérifie à la place que les HEIC donnent une erreur explicite.

use std::path::{Path, PathBuf};

use image::{ExtendedColorType, ImageEncoder, codecs::jpeg::JpegEncoder};
use tri::bitmap::Bitmap;
use tri::flat::{FlatError, decode};

/// Motif 4×2 aux pixels tous différents.
fn pattern() -> Vec<u8> {
    (0..24).map(|i| (i * 10) as u8).collect()
}

fn save(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    image::save_buffer(&path, &pattern(), 4, 2, ExtendedColorType::Rgb8).unwrap();
    path
}

fn assert_pattern(bmp: &Bitmap) {
    assert_eq!((bmp.width, bmp.height), (4, 2));
    assert_eq!(bmp.data, pattern());
}

/// Couleur proche à `tol` près par canal (formats avec perte).
fn assert_close(actual: Option<[u8; 3]>, expected: [u8; 3], tol: u8) {
    let actual = actual.expect("pixel hors image");
    for (a, e) in actual.iter().zip(expected) {
        assert!(a.abs_diff(e) <= tol, "{actual:?} trop loin de {expected:?}");
    }
}

#[test]
fn lossless_formats_decode_exactly() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.png", "a.tif", "a.webp"] {
        assert_pattern(&decode(save(dir.path(), name)).unwrap());
    }
}

#[test]
fn format_is_detected_from_content() {
    let dir = tempfile::tempdir().unwrap();
    let png = save(dir.path(), "a.png");
    let misnamed = dir.path().join("vraiment_un_png.jpg");
    std::fs::rename(png, &misnamed).unwrap();
    assert_pattern(&decode(misnamed).unwrap());
}

/// Bloc EXIF minimal (TIFF big-endian) ne contenant que la balise Orientation.
fn exif_orientation(value: u8) -> Vec<u8> {
    let mut exif = b"MM\0\x2a\0\0\0\x08".to_vec(); // en-tête, IFD0 à l'offset 8
    exif.extend([0, 1]); // une entrée
    exif.extend([0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, value, 0, 0]); // 0x0112, SHORT, 1 valeur
    exif.extend([0, 0, 0, 0]); // pas d'IFD suivant
    exif
}

#[test]
fn jpeg_exif_orientation_is_applied() {
    // 16×8 : moitié gauche rouge, moitié droite bleue, orientation 6 (90° horaire).
    let (w, h) = (16u32, 8u32);
    let data: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            if i % w < w / 2 {
                [255, 0, 0]
            } else {
                [0, 0, 255]
            }
        })
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pivote.jpg");
    let mut file = std::fs::File::create(&path).unwrap();
    let mut encoder = JpegEncoder::new_with_quality(&mut file, 95);
    encoder.set_exif_metadata(exif_orientation(6)).unwrap();
    encoder
        .write_image(&data, w, h, ExtendedColorType::Rgb8)
        .unwrap();
    drop(file);

    let bmp = decode(&path).unwrap();
    assert_eq!((bmp.width, bmp.height), (8, 16), "dimensions non pivotées");
    // Après rotation horaire, l'ancienne moitié gauche (rouge) est en haut.
    assert_close(bmp.pixel(4, 3), [255, 0, 0], 40);
    assert_close(bmp.pixel(4, 12), [0, 0, 255], 40);
}

fn heic_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/quadrants.heic")
}

#[test]
#[cfg(feature = "heif")]
fn heic_fixture_decodes_with_expected_colors() {
    const { assert!(tri::flat::HEIF_SUPPORTED) };
    let bmp = decode(heic_fixture()).unwrap();
    assert_eq!((bmp.width, bmp.height), (64, 48));
    assert_eq!(bmp.data.len(), 64 * 48 * 3);
    assert_close(bmp.pixel(10, 10), [255, 0, 0], 30);
    assert_close(bmp.pixel(50, 10), [0, 255, 0], 30);
    assert_close(bmp.pixel(10, 40), [0, 0, 255], 30);
    assert_close(bmp.pixel(50, 40), [255, 255, 255], 30);
}

#[test]
#[cfg(feature = "heif")]
fn heic_extension_is_case_insensitive_and_path_may_be_non_ascii() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("photo_été.HEIC");
    std::fs::copy(heic_fixture(), &path).unwrap();
    assert_eq!(decode(&path).unwrap().width, 64);
}

#[test]
#[cfg(not(feature = "heif"))]
fn heic_without_feature_is_an_explicit_error() {
    const { assert!(!tri::flat::HEIF_SUPPORTED) };
    assert!(matches!(
        decode(heic_fixture()),
        Err(FlatError::HeifNotCompiled)
    ));
}

#[test]
fn missing_file_is_an_io_error() {
    assert!(matches!(decode("absent.jpg"), Err(FlatError::Io(_))));
    #[cfg(feature = "heif")]
    assert!(matches!(decode("absent.heic"), Err(FlatError::Io(_))));
}

#[test]
fn garbage_is_a_decoding_error() {
    let dir = tempfile::tempdir().unwrap();
    let jpg = dir.path().join("faux.jpg");
    std::fs::write(&jpg, b"pas une image").unwrap();
    assert!(matches!(decode(jpg), Err(FlatError::Image(_))));
}

#[test]
#[cfg(feature = "heif")]
fn garbage_heic_is_a_heif_error() {
    let dir = tempfile::tempdir().unwrap();
    let heic = dir.path().join("faux.heic");
    std::fs::write(&heic, b"pas une image").unwrap();
    assert!(matches!(decode(heic), Err(FlatError::Heif(_))));
}

#[test]
#[cfg(feature = "heif")]
#[ignore = "nécessite TRI_TEST_HEIC"]
fn real_iphone_heic_decodes() {
    let path: PathBuf = std::env::var_os("TRI_TEST_HEIC")
        .expect("définir TRI_TEST_HEIC avec le chemin d'un fichier HEIC")
        .into();
    let bmp = decode(path).unwrap();
    assert!(bmp.width > 0 && bmp.height > 0);
    assert_eq!(bmp.data.len(), bmp.width as usize * bmp.height as usize * 3);
    assert!(bmp.data.iter().any(|&b| b != 0), "image entièrement noire");
}

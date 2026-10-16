//! Tests d'intégration du module raw, contre la vraie LibRaw.
//!
//! Les tests marqués `#[ignore]` ont besoin d'un vrai fichier RAW (non versionné) :
//!   TRI_TEST_RAW=/chemin/vers/photo.RAF cargo test -- --include-ignored

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use tri::raw::{RawError, RawFile, RenderOptions, Thumbnail};

/// Un rendu LibRaw occupe déjà plusieurs cœurs et ~200 Mo : lancés en parallèle, ces tests se
/// gênent au point d'être 50× plus lents. Chaque test sur vrai RAW prend donc ce verrou.
static LIBRAW: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    // Un test qui panique empoisonne le verrou ; les autres doivent quand même tourner.
    LIBRAW.lock().unwrap_or_else(|e| e.into_inner())
}

fn sample() -> PathBuf {
    std::env::var_os("TRI_TEST_RAW")
        .expect("définir TRI_TEST_RAW avec le chemin d'un fichier RAW")
        .into()
}

fn half() -> RenderOptions {
    RenderOptions {
        half_size: true, // rapide, suffisant pour vérifier la chaîne
        ..RenderOptions::default()
    }
}

#[test]
fn missing_file_is_an_error() {
    let res = RawFile::open("/chemin/qui/n/existe/pas.RAF");
    assert!(matches!(res, Err(RawError::LibRaw { .. })));
}

#[test]
fn non_raw_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("faux.RAF");
    std::fs::write(&path, b"ceci n'est pas un RAW").unwrap();
    assert!(matches!(RawFile::open(&path), Err(RawError::LibRaw { .. })));
}

#[test]
fn non_ascii_path_reaches_libraw() {
    // Le chemin doit arriver intact jusqu'à LibRaw : on obtient une erreur « format »,
    // pas InvalidPath.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("été_ñ_日本.RAF");
    std::fs::write(&path, b"pas un RAW").unwrap();
    assert!(matches!(RawFile::open(&path), Err(RawError::LibRaw { .. })));
}

#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn embedded_thumbnail_is_a_jpeg() {
    let _serial = serial();
    let mut raw = RawFile::open(sample()).unwrap();
    match raw.thumbnail().unwrap() {
        Thumbnail::Jpeg(bytes) => assert_eq!(&bytes[..2], &[0xFF, 0xD8], "marqueur SOI JPEG"),
        Thumbnail::Bitmap(bmp) => assert_eq!(bmp.data.len(), (bmp.width * bmp.height * 3) as usize),
    }
}

#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn render_produces_consistent_rgb8() {
    let _serial = serial();
    let bmp = RawFile::open(sample()).unwrap().render(&half()).unwrap();
    assert!(bmp.width > 0 && bmp.height > 0);
    assert_eq!(bmp.data.len(), bmp.width as usize * bmp.height as usize * 3);
    assert!(bmp.data.iter().any(|&b| b != 0), "image entièrement noire");
}

#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn thumbnail_then_render_on_same_handle() {
    let _serial = serial();
    let mut raw = RawFile::open(sample()).unwrap();
    raw.thumbnail().unwrap();
    raw.render(&half()).unwrap();
}

#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn white_balance_option_changes_output() {
    let _serial = serial();
    let camera = RawFile::open(sample()).unwrap().render(&half()).unwrap();
    let daylight = RawFile::open(sample())
        .unwrap()
        .render(&RenderOptions {
            camera_wb: false,
            ..half()
        })
        .unwrap();
    assert_eq!(
        (camera.width, camera.height),
        (daylight.width, daylight.height)
    );
    assert_ne!(
        camera.data, daylight.data,
        "le shim use_camera_wb n'a pas d'effet"
    );
}

#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn half_size_halves_dimensions() {
    let _serial = serial();
    let full = RawFile::open(sample())
        .unwrap()
        .render(&RenderOptions::default())
        .unwrap();
    let half = RawFile::open(sample()).unwrap().render(&half()).unwrap();
    assert_eq!(half.width, full.width / 2);
    assert_eq!(half.height, full.height / 2);
}

#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn rawfile_can_be_moved_to_another_thread() {
    let _serial = serial();
    let raw = RawFile::open(sample()).unwrap();
    let bmp = std::thread::spawn(move || {
        let mut raw = raw;
        raw.render(&half())
    })
    .join()
    .unwrap()
    .unwrap();
    assert!(bmp.width > 0);
}

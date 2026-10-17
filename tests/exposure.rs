//! Tests d'intégration du module `exposure` : image plate synthétique, vrai RAW
//! (`TRI_TEST_RAW`, ignoré par défaut).

use tri::exposure::{Exposure, ExposureSettings, Source, mask};

#[test]
fn a_flat_image_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gris.png");
    // Moitié gauche grise, moitié droite blanche (saturée).
    let img = image::RgbImage::from_fn(40, 20, |x, _| {
        if x < 20 {
            image::Rgb([100, 100, 100])
        } else {
            image::Rgb([255, 255, 255])
        }
    });
    img.save(&path).unwrap();
    let bitmap = tri::flat::decode(&path).unwrap();
    let settings = ExposureSettings {
        mask_size: 10,
        ..ExposureSettings::default()
    };
    let e = Exposure::from_bitmap(&bitmap, &settings);
    assert_eq!(e.source, Source::Flat);
    assert!((e.over_fraction() - 0.5).abs() < 1e-9);
    assert_eq!(e.headroom_ev(), 0.0, "déjà au blanc");
    assert_eq!((e.mask.width, e.mask.height), (10, 5));
    assert_eq!(e.mask.get(0, 0), 0);
    assert_eq!(e.mask.get(9, 4), mask::OVER);
    assert_eq!(Exposure::from_bytes(&e.to_bytes()).unwrap(), e);
}

#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn a_real_raw_uses_sensor_data() {
    let path = std::env::var("TRI_TEST_RAW").expect("TRI_TEST_RAW");
    let settings = ExposureSettings::default();
    let e = Exposure::from_raw(path.as_ref(), &settings).unwrap();
    let Source::Raw { bits, black, white } = e.source else {
        panic!("un RAW doit donner des données capteur");
    };
    assert!((12..=16).contains(&bits), "{bits} bits");
    assert!(white > black);
    assert_eq!(e.range, (white - black) as u32);
    let pixels: u64 = e.total.iter().sum();
    assert!(pixels > 1_000_000, "{pixels} pixels");
    assert!(e.headroom_ev() <= 0.0);
    assert!(e.mask.width.max(e.mask.height) <= settings.mask_size);
    assert_eq!(Exposure::from_bytes(&e.to_bytes()).unwrap(), e);
}

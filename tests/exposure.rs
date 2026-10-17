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
    assert_eq!(e.source, Source::Flat { bits: 8 });
    assert!((e.over_fraction() - 0.5).abs() < 1e-9);
    assert_eq!(e.headroom_ev(), 0.0, "déjà au blanc");
    assert_eq!((e.mask.width, e.mask.height), (10, 5));
    assert_eq!(e.mask.get(0, 0), 0);
    assert_eq!(e.mask.get(9, 4), mask::OVER);
    assert_eq!(Exposure::from_bytes(&e.to_bytes()).unwrap(), e);
}

#[test]
fn a_sixteen_bit_png_is_analysed_at_its_depth() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep.png");
    // Moitié gauche presque noire, juste au-dessus du seuil de -10 IL (en lumière linéaire)
    // en 16 bits, en dessous une fois ramenée à 3 sur 255 ; moitié droite blanche.
    let img = tri::flat::Rgb16::from_fn(40, 20, |x, _| {
        image::Rgb(if x < 20 { [850; 3] } else { [65_535; 3] })
    });
    img.save(&path).unwrap();
    let settings = ExposureSettings::default();
    let e = Exposure::from_deep(&tri::flat::decode_deep(&path).unwrap(), &settings);
    assert_eq!(e.source, Source::Flat { bits: 16 });
    assert_eq!(e.range, 65_535);
    assert!(e.stops > 15.0, "plus de 15 IL représentables : {}", e.stops);
    assert!((e.over_fraction() - 0.5).abs() < 1e-9);
    assert_eq!(
        e.under_fraction(),
        0.0,
        "850 / 65 535 : -9,96 IL, pas bouché"
    );
    // La même image ramenée en 8 bits : 850 → 3 sur 255, -10,1 IL : bouchée.
    let flat = Exposure::from_bitmap(&tri::flat::decode(&path).unwrap(), &settings);
    assert_eq!(flat.source, Source::Flat { bits: 8 });
    assert!((flat.under_fraction() - 0.5).abs() < 1e-9);
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

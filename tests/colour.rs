//! Gestion des couleurs (`tri::colour`, issue n° 422) : la même couleur, enregistrée dans
//! des espaces différents (profil ICC intégré), arrive au même endroit de l'espace de
//! travail ; les rendus des logiciels de développement sont convertis à la lecture du
//! cache, sans l'être deux fois.

use std::path::Path;

use image::{ImageEncoder, codecs::jpeg::JpegEncoder, codecs::png::PngEncoder};
use moxcms::{ColorProfile, Layout, TransformOptions};
use tri::bitmap::Bitmap;
use tri::cache::{Cache, Key, Level};
use tri::colour::{self, Origin};

/// La mire : une couleur vive de l'intérieur du sRGB.
const COLOUR: [u8; 3] = [200, 120, 60];
const SIDE: u32 = 16;

/// `COLOUR` (sRGB) exprimée dans `to`, en 8 ou 16 bits.
fn in_space(to: &ColorProfile) -> ([u8; 3], [u16; 3]) {
    let srgb = ColorProfile::new_srgb();
    let opts = TransformOptions::default();
    let t8 = srgb
        .create_transform_8bit(Layout::Rgb, to, Layout::Rgb, opts)
        .unwrap();
    let mut c8 = [0u8; 3];
    t8.transform(&COLOUR, &mut c8).unwrap();
    let t16 = srgb
        .create_transform_16bit(Layout::Rgb, to, Layout::Rgb, opts)
        .unwrap();
    let src16 = COLOUR.map(|v| u16::from(v) * 257);
    let mut c16 = [0u16; 3];
    t16.transform(&src16, &mut c16).unwrap();
    (c8, c16)
}

/// Où `COLOUR` doit arriver dans l'espace de travail.
fn expected() -> [u8; 3] {
    let mut b = Bitmap {
        width: 1,
        height: 1,
        data: COLOUR.to_vec(),
    };
    colour::bitmap_to_working(&mut b, &Origin::Srgb);
    [b.data[0], b.data[1], b.data[2]]
}

fn centre(b: &Bitmap) -> [u8; 3] {
    let i = ((b.height / 2 * b.width + b.width / 2) * 3) as usize;
    [b.data[i], b.data[i + 1], b.data[i + 2]]
}

fn assert_close(got: [u8; 3], want: [u8; 3], tol: i32, what: &str) {
    let ok = got
        .iter()
        .zip(want)
        .all(|(&a, b)| (i32::from(a) - i32::from(b)).abs() <= tol);
    assert!(ok, "{what} : {got:?}, attendu {want:?}");
}

fn write_jpeg(path: &Path, rgb: [u8; 3], icc: Option<Vec<u8>>) {
    let data: Vec<u8> = rgb.repeat((SIDE * SIDE) as usize);
    let mut out = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut out, 100);
    if let Some(icc) = icc {
        enc.set_icc_profile(icc).unwrap();
    }
    enc.write_image(&data, SIDE, SIDE, image::ExtendedColorType::Rgb8)
        .unwrap();
    std::fs::write(path, out).unwrap();
}

fn write_png16(path: &Path, rgb: [u16; 3], icc: Vec<u8>) {
    let data: Vec<u8> = rgb
        .repeat((SIDE * SIDE) as usize)
        .iter()
        .flat_map(|v| v.to_ne_bytes())
        .collect();
    let mut out = Vec::new();
    let mut enc = PngEncoder::new(&mut out);
    enc.set_icc_profile(icc).unwrap();
    enc.write_image(&data, SIDE, SIDE, image::ExtendedColorType::Rgb16)
        .unwrap();
    std::fs::write(path, out).unwrap();
}

#[test]
fn the_same_colour_from_jpegs_in_different_spaces_agrees() {
    let dir = tempfile::tempdir().unwrap();
    let want = expected();
    let spaces = [
        ("srgb-sans-profil", ColorProfile::new_srgb(), false),
        ("srgb", ColorProfile::new_srgb(), true),
        ("adobe", ColorProfile::new_adobe_rgb(), true),
        ("p3", ColorProfile::new_display_p3(), true),
        ("prophoto", ColorProfile::new_pro_photo_rgb(), true),
    ];
    for (name, profile, embed) in spaces {
        let path = dir.path().join(format!("{name}.jpg"));
        let (c8, _) = in_space(&profile);
        write_jpeg(&path, c8, embed.then(|| profile.encode().unwrap()));
        let b = tri::flat::decode_working(&path, 0).unwrap();
        // JPEG et 8 bits (ProPhoto surtout) : quelques niveaux de marge.
        assert_close(centre(&b), want, 4, name);
    }
}

#[test]
fn a_16_bit_prophoto_png_is_converted_at_its_depth() {
    let dir = tempfile::tempdir().unwrap();
    let pro = ColorProfile::new_pro_photo_rgb();
    let (_, c16) = in_space(&pro);
    let path = dir.path().join("prophoto16.png");
    write_png16(&path, c16, pro.encode().unwrap());
    let b = tri::flat::decode_working(&path, 0).unwrap();
    assert_close(centre(&b), expected(), 1, "PNG 16 bits ProPhoto");
}

#[test]
fn a_developed_render_is_converted_when_read_and_its_grid_copy_is_not_converted_twice() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::open(dir.path().join("cache")).unwrap();
    let source = dir.path().join("a.raf");
    std::fs::write(&source, b"raw").unwrap();
    // Le rendu d'un logiciel, en ProPhoto (le profil de sortie de l'historique), rangé tel
    // quel.
    let pro = ColorProfile::new_pro_photo_rgb();
    let (c8, _) = in_space(&pro);
    let render = dir.path().join("render.jpg");
    write_jpeg(&render, c8, Some(pro.encode().unwrap()));
    let level = Level::Developed(tri::develop::Engine::Darktable);
    let key = Key::new(&source, level, "rendu").unwrap();
    cache
        .put_bytes(&key, &std::fs::read(&render).unwrap())
        .unwrap();
    let read = cache.get(&key).unwrap();
    assert_close(centre(&read), expected(), 4, "rendu développé lu");
    // Sa copie pour la grille, écrite par `tri` (déjà convertie) : relue telle quelle.
    let copy_key = Key::new(&source, level, "rendu;grid=8").unwrap();
    cache.put(&copy_key, &read).unwrap();
    let copy = cache.get(&copy_key).unwrap();
    assert_close(centre(&copy), centre(&read), 2, "copie de la grille");
}

#[test]
fn an_icc_profile_can_be_embedded_without_recompressing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.jpg");
    write_jpeg(
        &path,
        COLOUR,
        Some(ColorProfile::new_srgb().encode().unwrap()),
    );
    let bytes = std::fs::read(&path).unwrap();
    let tagged = tri::jpeg::with_icc(&bytes, colour::working_icc()).unwrap();
    // L'ancien profil est remplacé, l'image reste lisible et identique.
    assert_eq!(
        tri::jpeg::icc_profile(&tagged).as_deref(),
        Some(colour::working_icc())
    );
    assert!(Origin::of_jpeg(&tagged).is_working());
    assert_eq!(
        tri::jpeg::decode(&tagged, 0).unwrap().data,
        tri::jpeg::decode(&bytes, 0).unwrap().data
    );
}

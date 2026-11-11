//! Mires de couleur pour vérifier la gestion des couleurs (`tri::colour`, issue n° 422) à
//! l'œil et par calcul :
//!
//!   cargo run --release --example colour_charts -- <dossier de sortie> [--side N] [--check-only]
//!
//! La **même** mire est écrite dans plusieurs espaces (sRGB, Adobe RGB, Display P3,
//! ProPhoto, Rec.2020, et les ProPhoto / Rec.2020 **linéaires** de darktable), en JPEG
//! 8 bits, TIFF 16 bits et PNG 8 et 16 bits (pas pour Display P3 ni Rec.2020, voir
//! `Space::png`), chacun avec son profil ICC intégré (plus un JPEG sRGB sans profil). Ouvertes dans `tri`, toutes doivent paraître identiques, et
//! identiques dans Aperçu / darktable.
//!
//! La mire, de haut en bas :
//! 1. primaires et secondaires sRGB (rouge, vert, bleu, cyan, magenta, jaune) ;
//! 2. couleurs courantes (peau, ciel, feuillage, orange, violet) et gris à 18 % ;
//! 3. dégradé gris, du noir au blanc ;
//! 4. dégradés du noir au rouge, au vert, au bleu (cassures en 8 bits) ;
//! 5. tour des teintes à pleine saturation ;
//! 6. couleurs **hors du sRGB** (primaires et secondaires Display P3, à côté du rouge sRGB
//!    pour comparer) : écrêtées dans les fichiers sRGB (normal), et en partie en Adobe RGB
//!    (le rouge P3) ; vives sur un écran large, presque pareilles sur un écran sRGB.
//!
//! Ensuite l'outil relit chaque fichier comme `tri` (`tri::flat::decode_working`) et
//! compare au centre de chaque case l'espace de travail attendu : écart maximal (niveaux
//! sur 255) des rangées 1, 2 et 6, et du dégradé gris. `--check-only` : relit les fichiers
//! déjà écrits sans les refaire.

use std::path::{Path, PathBuf};
use std::process::exit;

use image::{ImageEncoder, codecs::jpeg::JpegEncoder, codecs::png::PngEncoder};
use moxcms::{ColorProfile, Layout, ProfileText, ToneReprCurve, TransformOptions};
use tri::bitmap::Bitmap;
use tri::colour;

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

const USAGE: &str = "usage : colour_charts <dossier de sortie> [--side N] [--check-only]";

/// Les rangées de la mire (en fractions de la hauteur).
const ROWS: usize = 6;

/// Rangée 1 : primaires et secondaires sRGB.
const PRIMARIES: [[u8; 3]; 6] = [
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [0, 255, 255],
    [255, 0, 255],
    [255, 255, 0],
];

/// Rangée 2 : peau, ciel, feuillage, orange, violet, gris à 18 % (sRGB).
const COMMON: [[u8; 3]; 6] = [
    [224, 172, 140],
    [110, 160, 215],
    [80, 130, 60],
    [240, 140, 30],
    [120, 70, 160],
    [118, 118, 118],
];

/// Rangée 6 : couleurs Display P3 hors du sRGB, et le rouge sRGB en premier pour comparer
/// (la première case est en sRGB, les autres en P3).
const WIDE_P3: [[u8; 3]; 6] = [
    [255, 0, 0], // rouge **sRGB** (référence)
    [255, 0, 0], // rouge P3
    [0, 255, 0], // vert P3
    [0, 255, 255],
    [255, 0, 255],
    [255, 255, 0],
];

/// Un espace où écrire la mire.
struct Space {
    id: &'static str,
    profile: ColorProfile,
    /// Aussi en PNG. Pas pour Display P3 ni Rec.2020 : ImageIO (Aperçu, ColorSync)
    /// rejette leur profil dans un PNG (`sips -g profile` : « nil ») et les lit comme du
    /// sRGB, alors qu'il accepte le même profil dans un TIFF ou un JPEG (vérifié le
    /// 11/11/2026) ; la comparaison avec Aperçu y serait trompeuse.
    png: bool,
    /// Linéaire : 16 bits seulement (en 8 bits, les ombres se cassent ; `tri` refuse de
    /// l'exporter).
    linear: bool,
}

fn linear(mut p: ColorProfile, name: &str) -> ColorProfile {
    p.red_trc = Some(ToneReprCurve::Lut(Vec::new()));
    p.green_trc = Some(ToneReprCurve::Lut(Vec::new()));
    p.blue_trc = Some(ToneReprCurve::Lut(Vec::new()));
    p.cicp = None;
    p.description = Some(ProfileText::PlainString(name.into()));
    p
}

/// Sans balise CICP : celle du Display P3 de `moxcms` 0.8 dit « SMPTE RP 431 » (le P3 du
/// cinéma, blanc DCI) au lieu de « EG 432 » (Display P3, blanc D65) ; ColorSync la suit
/// et lisait la mire P3 de travers (rouge 249/2/4 au lieu de 255/0/0, vérifié le
/// 11/11/2026). Les primaires de la matrice suffisent.
fn plain(mut p: ColorProfile) -> ColorProfile {
    p.cicp = None;
    p
}

fn spaces() -> Vec<Space> {
    let spaces = vec![
        Space {
            id: "srgb",
            png: true,
            profile: ColorProfile::new_srgb(),
            linear: false,
        },
        Space {
            id: "adobergb",
            png: true,
            profile: ColorProfile::new_adobe_rgb(),
            linear: false,
        },
        Space {
            id: "displayp3",
            png: false,
            profile: ColorProfile::new_display_p3(),
            linear: false,
        },
        Space {
            id: "prophoto",
            png: true,
            profile: ColorProfile::new_pro_photo_rgb(),
            linear: false,
        },
        Space {
            id: "rec2020",
            png: false,
            profile: ColorProfile::new_bt2020(),
            linear: false,
        },
        Space {
            id: "prophoto-lineaire",
            png: true,
            profile: linear(ColorProfile::new_pro_photo_rgb(), "Linear ProPhoto RGB"),
            linear: true,
        },
        Space {
            id: "rec2020-lineaire",
            png: false,
            profile: linear(ColorProfile::new_bt2020(), "Linear Rec2020 RGB"),
            linear: true,
        },
    ];
    spaces
        .into_iter()
        .map(|s| Space {
            profile: plain(s.profile),
            ..s
        })
        .collect()
}

/// La mire en 16 bits, en deux couches : ce qui est défini en sRGB, et ce qui l'est en
/// Display P3 (rangée 6, cases 2 à 6) ; `true` dans le masque : pixel P3.
fn chart(w: u32, h: u32) -> (Vec<u16>, Vec<bool>) {
    let (w, h) = (w as usize, h as usize);
    let mut px = vec![0u16; w * h * 3];
    let mut p3 = vec![false; w * h];
    let up = |v: u8| u16::from(v) * 257;
    let row_h = h / ROWS;
    for y in 0..h {
        let row = (y / row_h).min(ROWS - 1);
        let in_row = y - row * row_h;
        for x in 0..w {
            let t = x as f32 / (w - 1) as f32;
            let cell = (x * 6 / w).min(5);
            let rgb: [u16; 3] = match row {
                0 => PRIMARIES[cell].map(up),
                1 => COMMON[cell].map(up),
                2 => {
                    let v = (t * 65535.0).round() as u16;
                    [v, v, v]
                }
                3 => {
                    let v = (t * 65535.0).round() as u16;
                    match in_row * 3 / row_h {
                        0 => [v, 0, 0],
                        1 => [0, v, 0],
                        _ => [0, 0, v],
                    }
                }
                4 => hue(t),
                _ => {
                    p3[y * w + x] = cell > 0;
                    WIDE_P3[cell].map(up)
                }
            };
            px[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&rgb);
        }
    }
    (px, p3)
}

/// Teinte `t` (0..1) à pleine saturation, en 16 bits.
fn hue(t: f32) -> [u16; 3] {
    let h = t * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [r, g, b].map(|c: f32| (c * 65535.0).round() as u16)
}

/// La mire convertie dans `to` (16 bits).
fn convert(px: &[u16], p3: &[bool], to: &ColorProfile) -> AnyResult<Vec<u16>> {
    let opts = TransformOptions::default();
    let from_srgb =
        ColorProfile::new_srgb().create_transform_16bit(Layout::Rgb, to, Layout::Rgb, opts)?;
    let from_p3 = ColorProfile::new_display_p3().create_transform_16bit(
        Layout::Rgb,
        to,
        Layout::Rgb,
        opts,
    )?;
    let mut a = vec![0u16; px.len()];
    let mut b = vec![0u16; px.len()];
    from_srgb.transform(px, &mut a)?;
    from_p3.transform(px, &mut b)?;
    Ok(a.chunks(3)
        .zip(b.chunks(3))
        .zip(p3)
        .flat_map(|((a, b), &wide)| if wide { b.to_vec() } else { a.to_vec() })
        .collect())
}

fn to8(px: &[u16]) -> Vec<u8> {
    px.iter()
        .map(|&v| ((u32::from(v) + 128) / 257) as u8)
        .collect()
}

fn write_jpeg(path: &Path, w: u32, h: u32, px: &[u8], icc: Option<Vec<u8>>) -> AnyResult<()> {
    let mut out = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut out, 95);
    if let Some(icc) = icc {
        enc.set_icc_profile(icc)?;
    }
    enc.write_image(px, w, h, image::ExtendedColorType::Rgb8)?;
    std::fs::write(path, out)?;
    Ok(())
}

fn write_png(path: &Path, w: u32, h: u32, px: &[u8], deep: bool, icc: Vec<u8>) -> AnyResult<()> {
    let mut out = Vec::new();
    let mut enc = PngEncoder::new(&mut out);
    enc.set_icc_profile(icc)?;
    let kind = if deep {
        image::ExtendedColorType::Rgb16
    } else {
        image::ExtendedColorType::Rgb8
    };
    enc.write_image(px, w, h, kind)?;
    std::fs::write(path, out)?;
    Ok(())
}

fn write_tiff(path: &Path, w: u32, h: u32, px: &[u8], icc: Vec<u8>) -> AnyResult<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = image::codecs::tiff::TiffEncoder::new(file);
    enc.set_icc_profile(icc)?;
    enc.write_image(px, w, h, image::ExtendedColorType::Rgb16)?;
    Ok(())
}

/// Octets « natifs » d'une image 16 bits, comme les attend `image`.
fn ne_bytes(px: &[u16]) -> Vec<u8> {
    px.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// Écrit toutes les mires ; renvoie les fichiers.
fn write_all(dir: &Path, side: u32) -> AnyResult<Vec<PathBuf>> {
    let (w, h) = (side, side * 3 / 4);
    let (px, p3) = chart(w, h);
    let mut files = Vec::new();
    for space in spaces() {
        let icc = space.profile.encode()?;
        let deep = convert(&px, &p3, &space.profile)?;
        let id = space.id;
        if !space.linear {
            let eight = to8(&deep);
            let jpg = dir.join(format!("{id}.jpg"));
            write_jpeg(&jpg, w, h, &eight, Some(icc.clone()))?;
            files.push(jpg);
            if space.png {
                let png8 = dir.join(format!("{id}-8.png"));
                write_png(&png8, w, h, &eight, false, icc.clone())?;
                files.push(png8);
            }
            if id == "srgb" {
                let bare = dir.join("srgb-sans-profil.jpg");
                write_jpeg(&bare, w, h, &eight, None)?;
                files.push(bare);
            }
        }
        if space.png {
            let png16 = dir.join(format!("{id}-16.png"));
            write_png(&png16, w, h, &ne_bytes(&deep), true, icc.clone())?;
            files.push(png16);
        }
        let tif = dir.join(format!("{id}-16.tif"));
        write_tiff(&tif, w, h, &ne_bytes(&deep), icc)?;
        files.push(tif);
    }
    Ok(files)
}

/// Pixel `(x, y)` en fractions de l'image.
fn at(b: &Bitmap, x: f32, y: f32) -> [u8; 3] {
    let (px, py) = (
        ((x * b.width as f32) as u32).min(b.width - 1),
        ((y * b.height as f32) as u32).min(b.height - 1),
    );
    let i = ((py * b.width + px) * 3) as usize;
    [b.data[i], b.data[i + 1], b.data[i + 2]]
}

fn gap(a: [u8; 3], b: [u8; 3]) -> i32 {
    a.iter()
        .zip(b)
        .map(|(&x, y)| (i32::from(x) - i32::from(y)).abs())
        .max()
        .unwrap_or(0)
}

/// Relit chaque fichier comme `tri` et le compare à l'attendu (la mire de référence,
/// définie en sRGB / P3, ramenée à l'espace de travail).
fn check(files: &[PathBuf], side: u32) -> AnyResult<()> {
    let (w, h) = (side, side * 3 / 4);
    let (px, p3) = chart(w, h);
    let reference = Bitmap {
        width: w,
        height: h,
        data: to8(&convert(&px, &p3, colour::working())?),
    };
    let row = |r: usize| (r as f32 + 0.5) / ROWS as f32;
    let cells = |r: usize| (0..6).map(move |c| ((c as f32 + 0.5) / 6.0, row(r)));
    println!(
        "{:<28} {:>9} {:>9} {:>9} {:>9}",
        "fichier", "primaires", "courantes", "gris", "hors sRGB"
    );
    for file in files {
        let got = tri::flat::decode_working(file, 0)?;
        if (got.width, got.height) != (w, h) {
            println!("{:<28} taille {}×{} ?", name(file), got.width, got.height);
            continue;
        }
        let worst = |points: &mut dyn Iterator<Item = (f32, f32)>| {
            points
                .map(|(x, y)| gap(at(&got, x, y), at(&reference, x, y)))
                .max()
                .unwrap_or(0)
        };
        let primaries = worst(&mut cells(0));
        let common = worst(&mut cells(1));
        let grey = worst(&mut (1..20).map(|i| (i as f32 / 20.0, row(2))));
        let wide = worst(&mut cells(5));
        println!(
            "{:<28} {primaries:>9} {common:>9} {grey:>9} {wide:>9}",
            name(file)
        );
    }
    println!(
        "\nÉcarts en niveaux sur 255, dans l'espace de travail de tri, au centre des cases. \
         Attendu : quelques niveaux au plus (JPEG, 8 bits ; ProPhoto 8 bits un peu plus). \
         « hors sRGB » : grand pour les fichiers sRGB (couleurs P3 écrêtées, c'est normal), \
         un peu pour l'Adobe RGB (rouge P3)."
    );
    Ok(())
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut dir, mut side, mut check_only) = (None, 1200u32, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--side" => side = args.next().and_then(|s| s.parse().ok()).unwrap_or(side),
            "--check-only" => check_only = true,
            _ if dir.is_none() => dir = Some(PathBuf::from(arg)),
            _ => {
                eprintln!("{USAGE}");
                exit(2);
            }
        }
    }
    let Some(dir) = dir else {
        eprintln!("{USAGE}");
        exit(2);
    };
    let side = side.max(120);
    let run = || -> AnyResult<()> {
        std::fs::create_dir_all(&dir)?;
        let files = if check_only {
            let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension()
                        .is_some_and(|e| ["jpg", "png", "tif"].contains(&e.to_str().unwrap_or("")))
                })
                .collect();
            files.sort();
            files
        } else {
            let files = write_all(&dir, side)?;
            println!("{} mires écrites dans {}\n", files.len(), dir.display());
            files
        };
        check(&files, side)
    };
    if let Err(e) = run() {
        eprintln!("erreur : {e}");
        exit(1);
    }
}

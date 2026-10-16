//! Outil de test du décodage : aperçu embarqué ou développement d'un RAW, décodage d'une
//! image plate.
//!
//!   cargo run --release --example decode -- thumb  <fichier RAW> <sortie>
//!   cargo run --release --example decode -- render <fichier RAW> <sortie.png> [--half] [--daylight] [--bright X] [--fit N]
//!   cargo run --release --example decode -- flat   <image plate> <sortie.png> [--fit N]
//!
//! `--fit N` réduit le résultat pour que son plus grand côté fasse au plus N pixels.

use std::path::Path;
use std::process::exit;
use std::time::Instant;

use tri::bitmap::Bitmap;
use tri::raw::{RawFile, RenderOptions, Thumbnail};

const USAGE: &str = "usage : decode <thumb|render|flat> <fichier> <sortie> \
                     [--half] [--daylight] [--bright X] [--fit N]";

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn save_png(bitmap: &Bitmap, out: &Path) -> AnyResult<()> {
    image::save_buffer(
        out,
        &bitmap.data,
        bitmap.width,
        bitmap.height,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(())
}

/// Réduit éventuellement le bitmap (`--fit`), puis l'écrit en PNG, en affichant les temps.
fn finish(bitmap: Bitmap, fit: Option<u32>, out: &Path) -> AnyResult<()> {
    let bitmap = match fit {
        Some(max_side) => {
            let start = Instant::now();
            let fitted = bitmap.fit(max_side);
            println!(
                "réduit à {}x{} en {:?}",
                fitted.width,
                fitted.height,
                start.elapsed()
            );
            fitted
        }
        None => bitmap,
    };
    let start = Instant::now();
    save_png(&bitmap, out)?;
    println!("écriture PNG : {:?}", start.elapsed());
    Ok(())
}

fn run() -> AnyResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [mode, input, output, flags @ ..] = args.as_slice() else {
        return Err(USAGE.into());
    };
    let output = Path::new(output);

    let mut options = RenderOptions::default();
    let mut fit = None;
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--half" => options.half_size = true,
            "--daylight" => options.camera_wb = false,
            "--bright" => options.bright = flags.next().ok_or(USAGE)?.parse()?,
            "--fit" => fit = Some(flags.next().ok_or(USAGE)?.parse()?),
            other => return Err(format!("option inconnue : {other}\n{USAGE}").into()),
        }
    }

    if mode == "flat" {
        let start = Instant::now();
        let bitmap = tri::flat::decode(input)?;
        println!(
            "décodé {}x{} en {:?}",
            bitmap.width,
            bitmap.height,
            start.elapsed()
        );
        finish(bitmap, fit, output)?;
        println!("écrit dans {}", output.display());
        return Ok(());
    }

    println!("LibRaw {}", tri::raw::version());
    let start = Instant::now();
    let mut raw = RawFile::open(input)?;
    let opened = start.elapsed();

    match mode.as_str() {
        "thumb" => match raw.thumbnail()? {
            Thumbnail::Jpeg(jpeg) => {
                println!(
                    "ouverture : {opened:?}, total LibRaw : {:?}",
                    start.elapsed()
                );
                std::fs::write(output, &jpeg)?;
                println!("aperçu JPEG, {} octets", jpeg.len());
            }
            Thumbnail::Bitmap(bitmap) => {
                println!("aperçu bitmap {}x{}", bitmap.width, bitmap.height);
                finish(bitmap, fit, output)?;
            }
        },
        "render" => {
            let bitmap = raw.render(&options)?;
            println!("rendu {}x{} ({options:?})", bitmap.width, bitmap.height);
            println!(
                "ouverture : {opened:?}, total LibRaw : {:?}",
                start.elapsed()
            );
            finish(bitmap, fit, output)?;
        }
        _ => return Err(USAGE.into()),
    }
    println!("écrit dans {}", output.display());
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        exit(1);
    }
}

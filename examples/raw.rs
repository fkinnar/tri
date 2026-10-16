//! Outil de test du module raw : extrait l'aperçu embarqué ou développe un RAW.
//!
//!   cargo run --release --example raw -- thumb  <fichier RAW> <sortie>
//!   cargo run --release --example raw -- render <fichier RAW> <sortie.png> [--half] [--daylight] [--bright X]

use std::path::Path;
use std::process::exit;
use std::time::Instant;

use tri::raw::{Bitmap, RawFile, RenderOptions, Thumbnail};

const USAGE: &str =
    "usage : raw <thumb|render> <fichier RAW> <sortie> [--half] [--daylight] [--bright X]";

fn save_png(bitmap: &Bitmap, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    image::save_buffer(
        out,
        &bitmap.data,
        bitmap.width,
        bitmap.height,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [mode, input, output, flags @ ..] = args.as_slice() else {
        return Err(USAGE.into());
    };
    let output = Path::new(output);

    let mut options = RenderOptions::default();
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--half" => options.half_size = true,
            "--daylight" => options.camera_wb = false,
            "--bright" => options.bright = flags.next().ok_or(USAGE)?.parse()?,
            other => return Err(format!("option inconnue : {other}\n{USAGE}").into()),
        }
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
                save_png(&bitmap, output)?;
                println!("aperçu bitmap {}x{}", bitmap.width, bitmap.height);
            }
        },
        "render" => {
            let bitmap = raw.render(&options)?;
            println!("rendu {}x{} ({options:?})", bitmap.width, bitmap.height);
            println!(
                "ouverture : {opened:?}, total LibRaw : {:?}",
                start.elapsed()
            );
            let write = Instant::now();
            save_png(&bitmap, output)?;
            println!("écriture PNG : {:?}", write.elapsed());
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

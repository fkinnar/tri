//! Démonstration en ligne de commande : liste les images d'un dossier, filtrées par nature.
//!
//!   tri <dossier> [--raw|--flat] [--depth N]

use std::process::exit;

use tri::filter::{Filter, KindFilter, ungrouped};
use tri::media::scan;

const USAGE: &str = "usage : tri <dossier> [--raw|--flat] [--depth N]";

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or(USAGE)?;
    let mut kind = KindFilter::All;
    let mut depth = 1;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--raw" => kind = KindFilter::RawOnly,
            "--flat" => kind = KindFilter::FlatOnly,
            "--depth" => depth = args.next().ok_or(USAGE)?.parse()?,
            other => return Err(format!("option inconnue : {other}\n{USAGE}").into()),
        }
    }

    let groups = kind.apply(ungrouped(scan(&dir, depth)?));
    for (index, group) in groups.iter().enumerate() {
        println!("===== GROUPE {index} ({} images)", group.len());
        for file in group {
            println!("{:?}\t{}", file.kind, file.path.display());
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        exit(1);
    }
}

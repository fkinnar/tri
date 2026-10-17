//! Démonstration en ligne de commande du moteur.
//!
//!   tri <dossier> [--raw|--flat] [--depth N]
//!       liste les images du dossier, filtrées par nature ;
//!   tri <dossier> --previews [--raw|--flat] [--depth N] [--side N] [--thumb N]
//!                            [--workers N] [--mixed] [--cache DOSSIER]
//!       calcule vignettes et aperçus standard en tâche de fond, avec progression
//!       (`--mixed` : sans attendre la fin des vignettes pour les aperçus) ;
//!   tri --clear-cache [thumbnail|embedded|standard|full] [--cache DOSSIER]
//!       vide le cache (un niveau, ou tout) ;
//!   tri --cache-usage [--cache DOSSIER]
//!       affiche la taille du cache par niveau ;
//!   tri <dossier> --convert-xmp darktable|adobe [--dry-run] [--raw|--flat] [--depth N]
//!       recopie les notes (étoiles, couleurs, accepté / rejeté) dans les sidecars de
//!       l'autre convention (`--dry-run` : sans rien écrire), voir `tri::xmp::convert`.
//!
//! Toutes les commandes lisent le fichier de configuration (`--config FICHIER`, sinon
//! l'emplacement par défaut, voir `tri::settings`) ; les options ci-dessus l'emportent sur
//! lui.

use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tri::cache::{Cache, Level};
use tri::filter::{Filter, KindFilter, ungrouped};
use tri::media::{MediaKind, scan};
use tri::scheduler::{CachedPreviews, Event, Progress, RawPreviews, Scheduler};
use tri::settings::{self, Auto, EngineSettings};
use tri::xmp::{self, Convention};

const USAGE: &str = "usage :
  tri <dossier> [--raw|--flat] [--depth N]
  tri <dossier> --previews [--raw|--flat] [--depth N] [--side N] [--thumb N] [--workers N] [--mixed] [--cache DOSSIER]
  tri --clear-cache [thumbnail|embedded|standard|full] [--cache DOSSIER]
  tri --cache-usage [--cache DOSSIER]
  tri <dossier> --convert-xmp darktable|adobe [--dry-run] [--raw|--flat] [--depth N]
option commune : --config FICHIER (réglages ; défaut : emplacement du système)";

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Args {
    dir: Option<PathBuf>,
    kind: KindFilter,
    depth: Option<usize>,
    previews: bool,
    clear: Option<Option<Level>>,
    usage: bool,
    side: Option<u32>,
    thumb: Option<u32>,
    workers: Option<usize>,
    mixed: bool,
    cache: Option<PathBuf>,
    config: Option<PathBuf>,
    convert: Option<Convention>,
    dry_run: bool,
}

fn parse_level(name: &str) -> Option<Level> {
    match name {
        "thumbnail" => Some(Level::Thumbnail),
        "embedded" => Some(Level::Embedded),
        "standard" => Some(Level::Standard),
        "full" => Some(Level::Full),
        _ => None,
    }
}

fn parse() -> AnyResult<Args> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1).peekable();
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(USAGE);
        match arg.as_str() {
            "--raw" => args.kind = KindFilter::RawOnly,
            "--flat" => args.kind = KindFilter::FlatOnly,
            "--depth" => args.depth = Some(value()?.parse()?),
            "--previews" => args.previews = true,
            "--side" => args.side = Some(value()?.parse()?),
            "--thumb" => args.thumb = Some(value()?.parse()?),
            "--config" => args.config = Some(value()?.into()),
            "--workers" => args.workers = Some(value()?.parse()?),
            "--mixed" => args.mixed = true,
            "--cache" => args.cache = Some(value()?.into()),
            "--cache-usage" => args.usage = true,
            "--convert-xmp" => {
                args.convert = Some(match value()?.as_str() {
                    "darktable" => Convention::Darktable,
                    "adobe" => Convention::Adobe,
                    other => {
                        return Err(format!(
                            "convention inconnue : {other} (darktable ou adobe)\n{USAGE}"
                        )
                        .into());
                    }
                })
            }
            "--dry-run" => args.dry_run = true,
            "--clear-cache" => {
                let level = it.peek().and_then(|n| parse_level(n));
                if level.is_some() {
                    it.next();
                }
                args.clear = Some(level);
            }
            other if other.starts_with("--") => {
                return Err(format!("option inconnue : {other}\n{USAGE}").into());
            }
            dir if args.dir.is_none() => args.dir = Some(dir.into()),
            other => return Err(format!("argument en trop : {other}\n{USAGE}").into()),
        }
    }
    Ok(args)
}

/// Réglages du fichier, corrigés par les options de la ligne de commande.
fn settings(args: &Args) -> EngineSettings {
    let loaded = settings::load::<EngineSettings>(args.config.as_deref());
    if let Some(warning) = &loaded.warning {
        eprintln!("attention : {warning}");
    }
    let mut s = loaded.settings;
    if let Some(side) = args.side {
        s.preview.standard = Auto::Fixed(side);
    }
    if let Some(thumb) = args.thumb {
        s.preview.thumbnail = thumb;
    }
    if let Some(w) = args.workers {
        s.scheduler.light_workers = Auto::Fixed(w);
    }
    if args.mixed {
        s.scheduler.thumbnails_first = false;
    }
    if let Some(dir) = &args.cache {
        s.cache.dir = Some(dir.clone());
    }
    s
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn print_usage(cache: &Cache) -> AnyResult<()> {
    println!("cache : {}", cache.root().display());
    for level in Level::ALL {
        let u = cache.usage(level)?;
        let avg = if u.entries == 0 {
            0
        } else {
            u.bytes / u.entries as u64
        };
        println!(
            "  {level:<9?} {:>6} entrées  {:>9.1} Mio  (moyenne {:.0} Kio)",
            u.entries,
            mib(u.bytes),
            avg as f64 / 1024.0
        );
    }
    Ok(())
}

/// Calcule les aperçus du dossier en affichant la progression sur une ligne.
fn previews(settings: &EngineSettings, files: Vec<tri::media::MediaFile>) -> AnyResult<()> {
    let cache = settings.open_cache()?;
    let n = files.len();
    let raws = files.iter().filter(|f| f.kind == MediaKind::Raw).count();
    // Pas d'écran : la taille « auto » prend sa valeur de repli. Démonstration et mesure :
    // tous les rendus RAW en fond.
    let preview = settings.preview(None);
    let config = settings.scheduler(RawPreviews::Background);
    println!(
        "{n} images ({raws} RAW), vignettes {} px, aperçus {} px, {} workers légers + 1 lourd, {}",
        preview.sizes.thumbnail,
        preview.sizes.standard,
        config.light_workers,
        if config.thumbnails_first {
            "vignettes d'abord"
        } else {
            "mélangé"
        }
    );
    println!("cache : {}", cache.root().display());

    let (tx, rx) = mpsc::channel();
    let producer = CachedPreviews {
        cache: cache.clone(),
        settings: preview,
    };
    let scheduler = Scheduler::new(producer, config, move |e| {
        let _ = tx.send(e);
    });
    let start = Instant::now();
    scheduler.set_folder(files);

    let (mut thumbs, mut embedded, mut standards) = (
        Progress::default(),
        Progress::default(),
        Progress::default(),
    );
    let (mut thumbs_done_at, mut embedded_done_at) = (None, None);
    let mut failures = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Progress {
                level, progress, ..
            }) => match level {
                Level::Thumbnail => thumbs = progress,
                Level::Embedded => embedded = progress,
                _ => standards = progress,
            },
            Ok(Event::Failed {
                path, level, error, ..
            }) => failures.push(format!("{level:?} {} : {error}", path.display())),
            Ok(Event::Ready { .. } | Event::Dropped { .. }) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let finished = |p: Progress, total: usize| p.done == total && p.total == total;
        if thumbs_done_at.is_none() && finished(thumbs, n) {
            thumbs_done_at = Some(start.elapsed());
        }
        if embedded_done_at.is_none() && finished(embedded, raws) {
            embedded_done_at = Some(start.elapsed());
        }
        print!(
            "\r  vignettes {:>4}/{:<4}   JPEG boîtier {:>4}/{:<4}   aperçus {:>4}/{:<4} {:>3.0} %   {:>6.1} s",
            thumbs.done,
            thumbs.total,
            embedded.done,
            embedded.total,
            standards.done,
            standards.total,
            standards.fraction() * 100.0,
            start.elapsed().as_secs_f64()
        );
        std::io::stdout().flush()?;
        if finished(thumbs, n) && finished(embedded, raws) && finished(standards, n) {
            break;
        }
    }
    let total = start.elapsed();
    println!();
    if let Some(t) = thumbs_done_at {
        println!("vignettes terminées en {t:.2?}");
    }
    if let Some(t) = embedded_done_at {
        println!("aperçus boîtier terminés en {t:.2?} (tout le dossier consultable)");
    }
    println!(
        "terminé en {total:.2?} ({:.0} ms par image en moyenne)",
        total.as_secs_f64() * 1000.0 / n.max(1) as f64
    );
    for f in &failures {
        eprintln!("échec : {f}");
    }
    print_usage(&cache)
}

/// Conversion des sidecars vers la convention `target`, avec bilan.
fn convert_xmp(
    files: &[tri::media::MediaFile],
    target: Convention,
    dry_run: bool,
) -> AnyResult<()> {
    let report = xmp::convert(files.iter().map(|f| f.path.as_path()), target, dry_run);
    let verb = if dry_run { "à écrire" } else { "écrit" };
    let verbs = if dry_run { "à écrire" } else { "écrits" };
    for path in &report.written {
        println!("{verb}\t{}", path.display());
    }
    for image in &report.conflicts {
        println!(
            "conflit\t{} : notes différentes de l'image qui partage le même sidecar, ignorée",
            image.display()
        );
    }
    for (image, error) in &report.errors {
        eprintln!("erreur\t{} : {error}", image.display());
    }
    println!(
        "{} images : {} sidecars {verbs}, {} inchangées, {} conflits, {} erreurs",
        files.len(),
        report.written.len(),
        report.unchanged,
        report.conflicts.len(),
        report.errors.len()
    );
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err("certains sidecars n'ont pas pu être convertis".into())
    }
}

fn run() -> AnyResult<()> {
    let args = parse()?;
    let settings = settings(&args);

    if let Some(level) = args.clear {
        let cache = settings.open_cache()?;
        cache.clear(level)?;
        println!(
            "cache vidé ({})",
            level.map_or("tout".into(), |l| format!("{l:?}"))
        );
        return print_usage(&cache);
    }
    if args.usage {
        return print_usage(&settings.open_cache()?);
    }

    let dir = args.dir.clone().ok_or(USAGE)?;
    let groups = args
        .kind
        .apply(ungrouped(scan(&dir, args.depth.unwrap_or(1))?));
    if let Some(target) = args.convert {
        let files: Vec<_> = groups.into_iter().flatten().collect();
        return convert_xmp(&files, target, args.dry_run);
    }
    if args.previews {
        return previews(&settings, groups.into_iter().flatten().collect());
    }
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

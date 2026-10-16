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
//!       affiche la taille du cache par niveau.

use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tri::cache::{Cache, Level};
use tri::filter::{Filter, KindFilter, ungrouped};
use tri::media::{MediaKind, scan};
use tri::preview::Sizes;
use tri::scheduler::{CachedPreviews, Config, Event, Progress, Scheduler};

const USAGE: &str = "usage :
  tri <dossier> [--raw|--flat] [--depth N]
  tri <dossier> --previews [--raw|--flat] [--depth N] [--side N] [--thumb N] [--workers N] [--mixed] [--cache DOSSIER]
  tri --clear-cache [thumbnail|embedded|standard|full] [--cache DOSSIER]
  tri --cache-usage [--cache DOSSIER]";

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Args {
    dir: Option<PathBuf>,
    kind: KindFilter,
    depth: Option<usize>,
    previews: bool,
    clear: Option<Option<Level>>,
    usage: bool,
    sizes: Sizes,
    workers: Option<usize>,
    mixed: bool,
    cache: Option<PathBuf>,
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
            "--side" => args.sizes.standard = value()?.parse()?,
            "--thumb" => args.sizes.thumbnail = value()?.parse()?,
            "--workers" => args.workers = Some(value()?.parse()?),
            "--mixed" => args.mixed = true,
            "--cache" => args.cache = Some(value()?.into()),
            "--cache-usage" => args.usage = true,
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

fn open_cache(args: &Args) -> AnyResult<Cache> {
    Ok(match &args.cache {
        Some(dir) => Cache::open(dir)?,
        None => Cache::open_default()?,
    })
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
fn previews(args: &Args, files: Vec<tri::media::MediaFile>) -> AnyResult<()> {
    let cache = open_cache(args)?;
    let n = files.len();
    let raws = files.iter().filter(|f| f.kind == MediaKind::Raw).count();
    let mut config = Config::default();
    if let Some(w) = args.workers {
        config.light_workers = w;
    }
    config.thumbnails_first = !args.mixed;
    println!(
        "{n} images ({raws} RAW), vignettes {} px, aperçus {} px, {} workers légers + 1 lourd, {}",
        args.sizes.thumbnail,
        args.sizes.standard,
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
        sizes: args.sizes,
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
            Ok(Event::Ready { .. }) => {}
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

fn run() -> AnyResult<()> {
    let args = parse()?;

    if let Some(level) = args.clear {
        let cache = open_cache(&args)?;
        cache.clear(level)?;
        println!(
            "cache vidé ({})",
            level.map_or("tout".into(), |l| format!("{l:?}"))
        );
        return print_usage(&cache);
    }
    if args.usage {
        return print_usage(&open_cache(&args)?);
    }

    let dir = args.dir.clone().ok_or(USAGE)?;
    let groups = args
        .kind
        .apply(ungrouped(scan(&dir, args.depth.unwrap_or(1))?));
    if args.previews {
        return previews(&args, groups.into_iter().flatten().collect());
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

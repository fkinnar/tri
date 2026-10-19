//! Démonstration en ligne de commande du moteur.
//!
//!   tri <dossier> [--raw|--flat] [--depth N]
//!       liste les images du dossier, filtrées par nature ;
//!   tri <dossier> --previews [--raw|--flat] [--depth N] [--side N] [--thumb N]
//!                            [--workers N] [--mixed] [--cache DOSSIER]
//!       calcule vignettes et aperçus standard en tâche de fond, avec progression
//!       (`--mixed` : sans attendre la fin des vignettes pour les aperçus) ;
//!   tri --clear-cache [thumbnail|embedded|standard|full|exposure] [--cache DOSSIER]
//!       vide le cache (un niveau, ou tout) ;
//!   tri --cache-usage [--cache DOSSIER]
//!       affiche la taille du cache par niveau ;
//!   tri <dossier> --exposure [--raw|--flat] [--depth N]
//!       exposition de chaque image (données du capteur pour un RAW) : marge avant
//!       saturation, pixels saturés et bouchés, voir `tri::exposure` ;
//!   tri <dossier> --metadata [--raw|--flat] [--depth N]
//!       métadonnées de prise de vue de chaque image, voir `tri::metadata` ;
//!   tri --offload <carte> [--to <racine>] [--folder <nom>] [--raw-only] [--dry-run] [--previews] [--delete-copied [--delete-left]] [--eject]
//!       décharge une carte : copie vérifiée des images vers <racine>/<nom> (défauts :
//!       `[offload] destination`, nom proposé AAAA/AAAA-MM-JJ), sans les fichiers déjà
//!       déchargés ; `--raw-only` laisse les JPEG sur la carte ; `--previews` calcule
//!       ensuite vignettes et aperçus ; `--delete-copied` supprime de la carte les
//!       fichiers copiés et vérifiés (`--delete-left` : aussi les JPEG laissés par
//!       `--raw-only`), `--eject` éjecte la carte ;
//!   tri <dossier> --convert-xmp darktable|adobe [--dry-run] [--raw|--flat] [--depth N]
//!       recopie les notes (étoiles, couleurs, accepté / rejeté) dans les sidecars de
//!       l'autre convention (`--dry-run` : sans rien écrire), voir `tri::xmp::convert` ;
//!   tri --rename <image> <nom> [--dry-run]
//!       renomme l'image (extension gardée), sa paire RAW+JPEG et ses sidecars ;
//!   tri --delete <image>... [--permanent] [--dry-run]
//!       met les images, leurs paires et leurs sidecars à la corbeille (`--permanent` :
//!       suppression définitive) ;
//!   tri --copy|--move <image>... --to <dossier> [--dry-run]
//!       copie ou déplace les images, leurs paires et leurs sidecars (copies vérifiées ;
//!       entre deux volumes, originaux supprimés seulement après) ;
//!   tri --reveal <image>
//!       montre l'image dans le gestionnaire de fichiers ;
//!   tri --edit <image>...
//!       ouvre les images dans l'éditeur externe (`[files] editor`), voir `tri::files` ;
//!   tri --export <profil> <image>... [--to <dossier>] [--libraw|--embedded] [--passthrough] [--dry-run]
//!       exporte les images développées par darktable avec un profil d'export
//!       (`[export.profiles.<profil>]`) ; dossier : `--to`, sinon celui du profil ; sans
//!       darktable (ou `--libraw`) : rendu de base par LibRaw ; `--embedded` : le JPEG
//!       boîtier ; `--passthrough` : JPEG sans recompression si possible ; voir
//!       `tri::export`.
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
use tri::t;
use tri::xmp::{self, Convention};

/// Mode d'emploi, dans la langue choisie.
fn usage() -> String {
    t!("cli-usage")
}

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Action sur des fichiers (`tri::files`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum FileAction {
    Rename,
    Delete,
    Reveal,
    Edit,
    Copy,
    Move,
    Export,
}

#[derive(Default)]
struct Args {
    dir: Option<PathBuf>,
    /// Arguments sans option après le premier (images d'une action sur fichiers).
    rest: Vec<String>,
    file_action: Option<FileAction>,
    /// Profil de `--export`.
    export_profile: Option<String>,
    /// `--libraw` : export sans darktable, même s'il est installé.
    libraw: bool,
    /// `--embedded` : exporter le JPEG boîtier.
    embedded: bool,
    /// `--passthrough` : JPEG sans recompression si possible.
    passthrough: bool,
    permanent: bool,
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
    exposure: bool,
    metadata: bool,
    sharpness: bool,
    groups: bool,
    threshold: Option<f32>,
    offload: Option<PathBuf>,
    to: Option<PathBuf>,
    folder: Option<String>,
    raw_only: bool,
    eject: bool,
    delete_copied: bool,
    delete_left: bool,
}

fn parse_level(name: &str) -> Option<Level> {
    match name {
        "thumbnail" => Some(Level::Thumbnail),
        "embedded" => Some(Level::Embedded),
        "standard" => Some(Level::Standard),
        "full" => Some(Level::Full),
        "exposure" => Some(Level::Exposure),
        "sharpness" => Some(Level::Sharpness),
        "similarity" => Some(Level::Similarity),
        _ => None,
    }
}

fn parse() -> AnyResult<Args> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1).peekable();
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or_else(usage);
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
                        let message = t!("cli-unknown-convention", value = other);
                        return Err(format!("{message}\n{}", usage()).into());
                    }
                })
            }
            "--dry-run" => args.dry_run = true,
            "--rename" => args.file_action = Some(FileAction::Rename),
            "--delete" => args.file_action = Some(FileAction::Delete),
            "--reveal" => args.file_action = Some(FileAction::Reveal),
            "--edit" => args.file_action = Some(FileAction::Edit),
            "--copy" => args.file_action = Some(FileAction::Copy),
            "--move" => args.file_action = Some(FileAction::Move),
            "--libraw" => args.libraw = true,
            "--embedded" => args.embedded = true,
            "--passthrough" => args.passthrough = true,
            "--export" => {
                args.file_action = Some(FileAction::Export);
                args.export_profile = Some(value()?);
            }
            "--permanent" => args.permanent = true,
            "--exposure" => args.exposure = true,
            "--metadata" => args.metadata = true,
            "--sharpness" => args.sharpness = true,
            "--groups" => args.groups = true,
            "--threshold" => args.threshold = Some(value()?.parse()?),
            "--offload" => args.offload = Some(value()?.into()),
            "--to" => args.to = Some(value()?.into()),
            "--folder" => args.folder = Some(value()?),
            "--raw-only" => args.raw_only = true,
            "--eject" => args.eject = true,
            "--delete-copied" => args.delete_copied = true,
            "--delete-left" => args.delete_left = true,
            "--clear-cache" => {
                let level = it.peek().and_then(|n| parse_level(n));
                if level.is_some() {
                    it.next();
                }
                args.clear = Some(level);
            }
            other if other.starts_with("--") => {
                return Err(
                    format!("{}\n{}", t!("cli-unknown-option", option = other), usage()).into(),
                );
            }
            dir if args.dir.is_none() => args.dir = Some(dir.into()),
            other => args.rest.push(other.into()),
        }
    }
    if args.file_action.is_none()
        && let Some(other) = args.rest.first()
    {
        return Err(format!(
            "{}\n{}",
            t!("cli-extra-argument", argument = other),
            usage()
        )
        .into());
    }
    Ok(args)
}

/// Réglages du fichier, corrigés par les options de la ligne de commande.
fn settings(args: &Args) -> EngineSettings {
    let loaded = settings::load::<EngineSettings>(args.config.as_deref());
    if let Some(warning) = &loaded.warning {
        eprintln!("{}", t!("warning-prefix", message = warning));
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
    println!(
        "{}",
        t!("cli-cache", path = cache.root().display().to_string())
    );
    for level in Level::ALL {
        let u = cache.usage(level)?;
        let avg = if u.entries == 0 {
            0
        } else {
            u.bytes / u.entries as u64
        };
        println!(
            "  {:<13} {}",
            format!("{level:?}"),
            t!(
                "cli-cache-level",
                entries = format!("{:>6}", u.entries),
                mib = format!("{:>9.1}", mib(u.bytes)),
                average = format!("{:.0}", avg as f64 / 1024.0)
            )
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
        "{}",
        t!(
            "cli-previews-start",
            n = n,
            raws = raws,
            thumbnail = preview.sizes.thumbnail,
            standard = preview.sizes.standard,
            workers = config.light_workers,
            order = if config.thumbnails_first {
                "thumbnails"
            } else {
                "mixed"
            }
        )
    );
    println!(
        "{}",
        t!("cli-cache", path = cache.root().display().to_string())
    );

    let (tx, rx) = mpsc::channel();
    let producer = CachedPreviews {
        cache: cache.clone(),
        settings: preview,
        // Rendus développés : dans l'interface seulement, à la demande.
        darktable: None,
    };
    let scheduler = Scheduler::new(producer, config, move |e| {
        let _ = tx.send(e);
    });
    let start = Instant::now();
    scheduler.set_folder(files);

    let (mut thumbs, mut embedded, mut standards, mut exposures) = (
        Progress::default(),
        Progress::default(),
        Progress::default(),
        Progress::default(),
    );
    let analyses = if config.exposure { n } else { 0 };
    let (mut thumbs_done_at, mut embedded_done_at) = (None, None);
    let mut failures = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Progress {
                level, progress, ..
            }) => match level {
                Level::Thumbnail => thumbs = progress,
                Level::Embedded => embedded = progress,
                Level::Exposure => exposures = progress,
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
        let count = |p: Progress| format!("{:>4}/{:<4}", p.done, p.total);
        print!(
            "\r  {}",
            t!(
                "cli-previews-progress",
                thumbnails = count(thumbs),
                embedded = count(embedded),
                histograms = count(exposures),
                previews = count(standards),
                percent = format!("{:>3.0}", standards.fraction() * 100.0),
                seconds = format!("{:>6.1}", start.elapsed().as_secs_f64())
            )
        );
        std::io::stdout().flush()?;
        if finished(thumbs, n)
            && finished(embedded, raws)
            && finished(standards, n)
            && finished(exposures, analyses)
        {
            break;
        }
    }
    let total = start.elapsed();
    println!();
    if let Some(t) = thumbs_done_at {
        println!("{}", t!("cli-thumbnails-done", time = format!("{t:.2?}")));
    }
    if let Some(t) = embedded_done_at {
        println!("{}", t!("cli-embedded-done", time = format!("{t:.2?}")));
    }
    println!(
        "{}",
        t!(
            "cli-all-done",
            time = format!("{total:.2?}"),
            ms = format!("{:.0}", total.as_secs_f64() * 1000.0 / n.max(1) as f64)
        )
    );
    for f in &failures {
        eprintln!("{}", t!("cli-failure", error = f.to_string()));
    }
    let freed = settings.trim_cache(&cache)?;
    if freed.entries > 0 {
        println!(
            "{}",
            t!(
                "cli-trimmed",
                n = freed.entries,
                mb = format!("{:.0}", freed.bytes as f64 / 1e6)
            )
        );
    }
    print_usage(&cache)
}

/// Déchargement d'une carte, avec progression et bilan ; puis aperçus si demandé.
fn offload(settings: &EngineSettings, args: &Args, card: &std::path::Path) -> AnyResult<()> {
    use tri::offload::{self, History};
    let mut chosen = settings.offload.clone();
    chosen.raw_only |= args.raw_only;
    let s = &chosen;
    let mut history = if args.dry_run {
        History::open_configured(s, settings.data_dir().as_deref())
            .unwrap_or_else(|_| History::in_memory())
    } else {
        History::open_configured(s, settings.data_dir().as_deref())?
    };
    let plan = offload::plan(card, &history)?;
    let root = args.to.clone().unwrap_or_else(|| {
        settings::expand_home(&offload::root(s, settings.data_dir().as_deref()))
    });
    let folder = match &args.folder {
        Some(f) => f.clone(),
        None => offload::suggest_folder(&plan, s).unwrap_or_default(),
    };
    let known = plan.items.iter().filter(|i| i.known).count();
    println!(
        "{}",
        t!(
            "cli-offload-plan",
            card = card.display().to_string(),
            n = plan.items.len(),
            known = known,
            ignored = plan.ignored,
            mb = tri::i18n::decimal(plan.bytes(s) as f64 / 1e6, 1)
        )
    );
    if plan.to_copy(s).next().is_none() {
        println!("{}", t!("cli-nothing-to-copy"));
        return Ok(());
    }
    let dest = offload::destination(&root, &folder)?;
    if !args.dry_run
        && let Some(data) = settings.data_dir()
    {
        let _ = offload::remember_root(&data, &root.to_string_lossy());
    }
    println!(
        "{}",
        t!("cli-destination", path = dest.display().to_string())
    );
    let (there, _) = offload::already_in(&plan, s, &dest);
    if there > 0 {
        println!("{}", t!("cli-already-there", n = there));
    }
    if args.dry_run {
        for item in plan.to_copy(s) {
            println!("{}\t{}", t!("cli-to-copy"), item.source.display());
        }
        return Ok(());
    }
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let report = offload::run(&plan, &dest, s, &mut history, &cancel, |p| {
        print!(
            "\r  {}",
            t!(
                "cli-offload-progress",
                done = p.files_done,
                total = p.files_total,
                mb = format!("{:.0}", p.bytes_done as f64 / 1e6),
                total_mb = format!("{:.0}", p.bytes_total as f64 / 1e6)
            )
        );
        let _ = std::io::stdout().flush();
    });
    println!();
    for (file, error) in &report.errors {
        eprintln!("{}\t{} : {error}", t!("cli-error"), file.display());
    }
    println!(
        "{}",
        t!(
            "cli-offload-done",
            copied = report.copied.len(),
            verified = if s.verify { "yes" } else { "no" },
            skipped = report.skipped.len(),
            left = report.left.len(),
            errors = report.errors.len(),
            time = format!("{:.1?}", report.elapsed)
        )
    );
    // En ligne de commande, rien ne se fait sur la carte sans option explicite.
    if args.delete_copied {
        let mut failed = offload::delete_sources(&report);
        if args.delete_left {
            failed.extend(offload::delete_left(&report));
        }
        let attempted = report.copied.len()
            + report.already_there.len()
            + if args.delete_left {
                report.left.len()
            } else {
                0
            };
        println!("{}", t!("cli-deleted", n = attempted - failed.len()));
        for (file, e) in &failed {
            eprintln!("{}\t{} : {e}", t!("cli-not-deleted"), file.display());
        }
    }
    if args.eject {
        match tri::places::eject(card) {
            Ok(()) => println!("{}", t!("cli-ejected")),
            Err(e) => eprintln!("{}", t!("cli-eject-failed", error = e)),
        }
    }
    if args.previews {
        let files = scan(&dest, 1)?;
        previews(settings, files)?;
    }
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err(t!("cli-some-not-copied").into())
    }
}

/// Exposition de chaque image : marge avant saturation (IL), pixels saturés et bouchés
/// (canal le plus touché), durée du calcul.
fn exposure(settings: &EngineSettings, files: &[tri::media::MediaFile]) -> AnyResult<()> {
    let s = &settings.exposure;
    println!("{}", t!("cli-exposure-header"));
    for file in files {
        let start = Instant::now();
        let result = match file.kind {
            MediaKind::Raw => {
                tri::exposure::Exposure::from_raw(&file.path, s).map_err(|e| e.to_string())
            }
            MediaKind::Flat => tri::flat::decode_deep(&file.path)
                .map(|b| tri::exposure::Exposure::from_deep(&b, s))
                .map_err(|e| e.to_string()),
        };
        let name = file.path.file_name().unwrap_or_default().to_string_lossy();
        match result {
            Ok(e) => {
                let source = match e.source {
                    tri::exposure::Source::Raw { bits, black, white } => {
                        t!(
                            "cli-exposure-raw-source",
                            bits = bits,
                            black = black,
                            white = white
                        )
                    }
                    tri::exposure::Source::Flat { bits } => format!("{bits} bits"),
                };
                println!(
                    "{name}\t{source}\t{:.2}\t{:.3} %\t{:.3} %\t{:.0} ms",
                    e.headroom_ev(),
                    e.over_fraction() * 100.0,
                    e.under_fraction() * 100.0,
                    start.elapsed().as_secs_f64() * 1000.0
                );
            }
            Err(e) => eprintln!("{name}\t{}", t!("cli-error-detail", error = e)),
        }
    }
    Ok(())
}

/// Score de netteté de chaque image, rapporté à la médiane du dossier ; les photos jugées
/// floues sont signalées (voir `tri::sharpness`).
fn sharpness(settings: &EngineSettings, files: &[tri::media::MediaFile]) -> AnyResult<()> {
    let preview = settings.preview(None);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut scores = Vec::new();
    for file in files {
        let start = Instant::now();
        let name = file.path.file_name().unwrap_or_default().to_string_lossy();
        match tri::preview::compute_sharpness(file, &preview, &cancel) {
            Ok(s) => scores.push((name.into_owned(), s.value, start.elapsed())),
            Err(e) => eprintln!("{name}\t{}", t!("cli-error-detail", error = e.to_string())),
        }
    }
    let ordered: Vec<(PathBuf, Option<f32>)> = scores
        .iter()
        .map(|(name, value, _)| (PathBuf::from(name), Some(*value)))
        .collect();
    // Piles d'images semblables (comme dans l'interface) : la netteté s'y juge.
    let by_name: std::collections::HashMap<String, &tri::media::MediaFile> = files
        .iter()
        .map(|f| {
            (
                f.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                f,
            )
        })
        .collect();
    let signatures: Vec<Option<tri::similarity::Signature>> = scores
        .iter()
        .map(|(name, _, _)| tri::preview::compute_signature(by_name[name], &preview, &cancel).ok())
        .collect();
    let refs: Vec<Option<&tri::similarity::Signature>> =
        signatures.iter().map(Option::as_ref).collect();
    let sim = &settings.similarity;
    let links = tri::similarity::distances(&refs, sim.shape_weight);
    let groups = if sim.background {
        tri::similarity::group(&links, settings.sharpness.group_threshold, sim.max_gap_s)
    } else {
        Vec::new()
    };
    let judgements = tri::sharpness::judge_in_groups(&ordered, &groups, &settings.sharpness);
    let ratio = |r: Option<f32>| r.map_or("-".to_string(), |r| format!("{r:.2}"));
    println!("{}", t!("cli-sharpness-header"));
    for (name, value, elapsed) in &scores {
        let j = judgements[&PathBuf::from(name)];
        println!(
            "{name}\t{:.1}\t{}\t{}\t{}\t{}\t{:.0} ms",
            value,
            ratio(j.folder_ratio),
            ratio(j.neighbour_ratio),
            ratio(j.group_ratio),
            if j.blurry {
                t!("cli-sharpness-blurry")
            } else {
                String::new()
            },
            elapsed.as_secs_f64() * 1000.0
        );
    }
    let median = tri::sharpness::median(scores.iter().map(|s| s.1)).unwrap_or(0.0);
    let s = &settings.sharpness;
    println!(
        "{}",
        t!(
            "cli-sharpness-summary",
            n = scores.len(),
            median = format!("{median:.1}"),
            blurry = judgements.values().filter(|j| j.blurry).count(),
            ratio = format!("{}", s.blur_ratio),
            neighbours = s.neighbours,
            neighbour_ratio = format!("{}", s.neighbour_ratio)
        )
    );
    Ok(())
}

/// Regroupement des images semblables : distance et écart de temps entre voisines, puis
/// les groupes au seuil `threshold` (sinon celui de la configuration), pour le calibrer
/// (voir `tri::similarity`).
fn similarity_groups(
    settings: &EngineSettings,
    files: &[tri::media::MediaFile],
    threshold: Option<f32>,
) -> AnyResult<()> {
    let preview = settings.preview(None);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let s = &settings.similarity;
    let start = Instant::now();
    let signatures: Vec<Option<tri::similarity::Signature>> = files
        .iter()
        .map(|f| tri::preview::compute_signature(f, &preview, &cancel).ok())
        .collect();
    let elapsed = start.elapsed();
    let refs: Vec<Option<&tri::similarity::Signature>> =
        signatures.iter().map(Option::as_ref).collect();
    let links = tri::similarity::distances(&refs, s.shape_weight);
    let name = |i: usize| {
        files[i]
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    println!("{}", t!("cli-groups-header"));
    for (i, link) in links.iter().enumerate() {
        let (d, gap) = match link {
            Some(l) => (
                format!("{:.3}", l.distance),
                l.gap_s.map_or("-".into(), |g| format!("{g:.0}")),
            ),
            None => ("-".into(), "-".into()),
        };
        println!("{}	{}	{d}	{gap}", name(i), name(i + 1));
    }
    let threshold = threshold.unwrap_or(s.threshold);
    let groups = tri::similarity::group(&links, threshold, s.max_gap_s);
    for g in &groups {
        let names: Vec<String> = g.clone().map(name).collect();
        println!("[{}] {}", g.len(), names.join(" "));
    }
    println!(
        "{}",
        t!(
            "cli-groups-summary",
            n = files.len(),
            groups = groups.len(),
            threshold = format!("{threshold}"),
            ms = format!(
                "{:.0}",
                elapsed.as_secs_f64() * 1000.0 / files.len().max(1) as f64
            )
        )
    );
    Ok(())
}

/// Conversion des sidecars vers la convention `target`, avec bilan.
fn convert_xmp(
    files: &[tri::media::MediaFile],
    target: Convention,
    dry_run: bool,
) -> AnyResult<()> {
    let report = xmp::convert(files.iter().map(|f| f.path.as_path()), target, dry_run);
    let mode = if dry_run { "dry-run" } else { "write" };
    for path in &report.written {
        println!("{}\t{}", t!("cli-xmp-written", mode = mode), path.display());
    }
    for image in &report.conflicts {
        println!(
            "{}",
            t!("cli-xmp-conflict", image = image.display().to_string())
        );
    }
    for (image, error) in &report.errors {
        eprintln!("{}\t{} : {error}", t!("cli-error"), image.display());
    }
    println!(
        "{}",
        t!(
            "cli-xmp-summary",
            n = files.len(),
            written = report.written.len(),
            mode = mode,
            unchanged = report.unchanged,
            conflicts = report.conflicts.len(),
            errors = report.errors.len()
        )
    );
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err(t!("cli-xmp-some-failed").into())
    }
}

/// `--rename`, `--delete`, `--reveal`, `--edit` : voir `tri::files`.
fn file_action(settings: &EngineSettings, args: &Args, action: FileAction) -> AnyResult<()> {
    let s = &settings.files;
    let first = args.dir.clone().ok_or_else(usage)?;
    match action {
        FileAction::Rename => {
            let [name] = args.rest.as_slice() else {
                return Err(usage().into());
            };
            let plan = tri::files::plan_rename(&first, name, s)?;
            for (from, to) in &plan.moves {
                println!("{} → {}", from.display(), to.display());
            }
            for (from, to) in &plan.copies {
                println!(
                    "{} → {} {}",
                    from.display(),
                    to.display(),
                    t!("cli-rename-shared-copy")
                );
            }
            if !args.dry_run {
                tri::files::rename(&plan)?;
            }
        }
        FileAction::Delete => {
            let images: Vec<PathBuf> = std::iter::once(first)
                .chain(args.rest.iter().map(PathBuf::from))
                .collect();
            let group = tri::files::plan_delete(&images, s);
            let trash = s.trash && !args.permanent;
            for file in group.files() {
                let what = if trash {
                    t!("cli-delete-trash")
                } else {
                    t!("cli-delete-permanent")
                };
                println!("{what} {}", file.display());
            }
            for file in &group.shared {
                println!("{} {}", t!("cli-delete-kept"), file.display());
            }
            if !args.dry_run {
                tri::files::delete(&group, trash)?;
            }
        }
        FileAction::Copy | FileAction::Move => {
            let images: Vec<PathBuf> = std::iter::once(first)
                .chain(args.rest.iter().map(PathBuf::from))
                .collect();
            let dest = args.to.clone().ok_or_else(usage)?;
            let mode = if action == FileAction::Copy {
                tri::files::TransferMode::Copy
            } else {
                tri::files::TransferMode::Move
            };
            let plan = tri::files::plan_transfer(&images, &dest, mode, s)?;
            for (from, to) in &plan.files {
                println!("{} → {}", from.display(), to.display());
            }
            for (from, to) in &plan.shared {
                println!(
                    "{} → {} {}",
                    from.display(),
                    to.display(),
                    t!("cli-rename-shared-copy")
                );
            }
            if !args.dry_run {
                tri::files::transfer(&plan, |done, total| {
                    eprint!("\r{done}/{total}");
                })?;
                eprintln!();
            }
        }
        FileAction::Export => {
            let images: Vec<PathBuf> = std::iter::once(first)
                .chain(args.rest.iter().map(PathBuf::from))
                .collect();
            export(settings, args, &images)?;
        }
        FileAction::Reveal => tri::files::reveal(&first)?,
        FileAction::Edit => {
            let images: Vec<PathBuf> = std::iter::once(first)
                .chain(args.rest.iter().map(PathBuf::from))
                .collect();
            tri::files::open_with(&s.editor, &images)?;
        }
    }
    if args.dry_run {
        println!("{}", t!("cli-dry-run"));
    }
    Ok(())
}

/// `--export` : voir `tri::export`.
fn export(settings: &EngineSettings, args: &Args, images: &[PathBuf]) -> AnyResult<()> {
    let id = args.export_profile.as_deref().unwrap_or_default();
    let profiles = &settings.export.profiles;
    let Some(profile) = profiles.get(id) else {
        let known: Vec<&str> = profiles.0.iter().map(|p| p.id.as_str()).collect();
        return Err(t!(
            "cli-export-unknown-profile",
            profile = id,
            known = known.join(", ")
        )
        .into());
    };
    profile.check()?;
    // Le profil s'applique-t-il à ce qu'on exporte (`applies_to`) ?
    let flat = args.embedded
        || images
            .iter()
            .all(|p| tri::media::kind_of(p) == Some(tri::media::MediaKind::Flat));
    if !profile.applies_to.allows(flat) {
        let key = if flat {
            "cli-export-raw-only"
        } else {
            "cli-export-as-is-raw"
        };
        return Err(t!(key, profile = id).into());
    }
    let mut profile = profile.clone();
    if args.passthrough {
        profile.passthrough = true;
    }
    let profile = &profile;
    let sources = tri::export::sources(images);
    // `--to`, sinon le dossier du profil (relatif : à côté de chaque image).
    let folder = match &args.to {
        Some(to) => to.display().to_string(),
        None => profile.folder.clone(),
    };
    let dest = sources
        .first()
        .map(|first| tri::export::folder_for(&folder, first))
        .unwrap_or_default();
    let summary = format!(
        "{} {} bits, {}, {}",
        profile.format.extension(),
        profile.bits,
        if profile.color.is_empty() {
            "-"
        } else {
            profile.color.as_str()
        },
        profile.max_side
    );
    println!(
        "{}",
        t!(
            "cli-export-plan",
            profile = id,
            summary = summary,
            dest = dest.display().to_string()
        )
    );
    for image in &sources {
        println!("  {}", image.display());
    }
    if args.dry_run {
        return Ok(());
    }
    let request =
        tri::export::Request::new(profile, &folder).sharpening(settings.export.sharpening);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let progress = |done: usize, total: usize, path: &std::path::Path| {
        eprintln!("{done}/{total} {}", path.display());
    };
    // darktable s'il est là (sauf `--libraw`), sinon rendu de base par LibRaw.
    let darktable = (!args.libraw && !args.embedded)
        .then(|| tri::develop::find_darktable(&settings.develop.darktable))
        .flatten();
    let report = match darktable {
        Some(cli) => {
            let dt = tri::develop::Darktable {
                cli,
                config_dir: settings.open_cache()?.root().join("darktable-export"),
                high_quality: true,
            };
            // Accentuation de sortie : par un TIFF temporaire (darktable n'en fait pas).
            if profile.sharpen == tri::export::Sharpen::None {
                tri::export::export(&dt, &sources, profile, &folder, &cancel, progress)?
            } else {
                tri::export::export_via_tiff(&dt, &sources, &request, &cancel, progress)?
            }
        }
        None => {
            if !args.embedded {
                eprintln!("{}", t!("cli-export-libraw"));
            }
            let local = if args.embedded {
                tri::export::Local::Embedded
            } else {
                tri::export::Local::LibRaw
            };
            tri::export::export_libraw(&sources, &request, local, &cancel, progress)?
        }
    };
    for (image, error) in &report.errors {
        eprintln!("{} : {error}", image.display());
    }
    println!("{}", t!("cli-export-done", n = report.written.len()));
    if report.passthrough > 0 {
        println!("{}", t!("cli-export-passthrough", n = report.passthrough));
    }
    if report.failed > 0 {
        return Err(t!("cli-export-failed", failed = report.failed).into());
    }
    Ok(())
}

fn run() -> AnyResult<()> {
    // Langue du système pour les arguments et la lecture de la configuration, puis celle
    // qu'elle choisit.
    tri::i18n::set_language(tri::i18n::Language::system());
    let args = parse()?;
    let settings = settings(&args);
    tri::i18n::set_language(settings.i18n.language.resolve());

    if let Some(level) = args.clear {
        let cache = settings.open_cache()?;
        cache.clear(level)?;
        println!(
            "{}",
            t!(
                "cli-cache-cleared",
                level = level.map_or(t!("cli-cache-all"), |l| format!("{l:?}"))
            )
        );
        return print_usage(&cache);
    }
    if args.usage {
        return print_usage(&settings.open_cache()?);
    }

    if let Some(card) = &args.offload {
        return offload(&settings, &args, card);
    }
    if let Some(action) = args.file_action {
        return file_action(&settings, &args, action);
    }
    let dir = args.dir.clone().ok_or_else(usage)?;
    let groups = args
        .kind
        .apply(ungrouped(scan(&dir, args.depth.unwrap_or(1))?));
    if args.metadata {
        for file in groups.into_iter().flatten() {
            let start = Instant::now();
            let name = file.path.file_name().unwrap_or_default().to_string_lossy();
            match tri::metadata::read(&file) {
                Ok(m) => {
                    println!(
                        "== {name} ({:.0} ms)",
                        start.elapsed().as_secs_f64() * 1000.0
                    );
                    for (label, value) in m.rows() {
                        println!("  {label:<11} {value}");
                    }
                }
                Err(e) => eprintln!("== {name} : {e}"),
            }
        }
        return Ok(());
    }
    if args.exposure {
        let files: Vec<_> = groups.into_iter().flatten().collect();
        return exposure(&settings, &files);
    }
    if args.sharpness {
        let files: Vec<_> = groups.into_iter().flatten().collect();
        return sharpness(&settings, &files);
    }
    if args.groups {
        let files: Vec<_> = groups.into_iter().flatten().collect();
        return similarity_groups(&settings, &files, args.threshold);
    }
    if let Some(target) = args.convert {
        let files: Vec<_> = groups.into_iter().flatten().collect();
        return convert_xmp(&files, target, args.dry_run);
    }
    if args.previews {
        return previews(&settings, groups.into_iter().flatten().collect());
    }
    for (index, group) in groups.iter().enumerate() {
        println!("===== {}", t!("cli-group", index = index, n = group.len()));
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

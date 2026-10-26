//! Tests d'intégration du scheduler.
//!
//! La logique (priorités, parallélisme, annulation, progression) est testée avec un faux
//! producteur qui enregistre ce qu'on lui demande. Un test optionnel (`TRI_TEST_RAW`) fait
//! tourner le vrai producteur sur un RAW et une image plate.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tri::cache::Level;
use tri::media::{MediaFile, MediaKind};
use tri::scheduler::{Config, Event, Failure, Producer, Progress, RawPreviews, Scheduler};

const LONG: Duration = Duration::from_secs(10);

/// Une tâche exécutée : (chemin, niveau, résultat).
type Entry = (PathBuf, Level, Result<(), Failure>);

/// Ce que le faux producteur a fait, et comment il doit se comporter.
#[derive(Default)]
struct FakeState {
    /// Tâches exécutées, dans l'ordre : (chemin, niveau, résultat).
    log: Mutex<Vec<Entry>>,
    /// Tâches en cours (pour détecter le démarrage d'un 1:1).
    started: Mutex<Vec<(PathBuf, Level)>>,
    /// Entrées considérées comme déjà en cache.
    cached: Mutex<HashSet<(PathBuf, Level)>>,
    /// Fichiers dont le calcul échoue.
    failing: Mutex<HashSet<PathBuf>>,
    /// Durée d'une tâche ordinaire.
    delay: Mutex<Duration>,
    /// Durée d'un 1:1 (interrompue par l'annulation).
    full_delay: Mutex<Duration>,
    /// Tâches lourdes / légères en cours, et maximums observés.
    heavy_now: AtomicUsize,
    heavy_max: AtomicUsize,
    light_now: AtomicUsize,
    light_max: AtomicUsize,
}

#[derive(Clone, Default)]
struct Fake(Arc<FakeState>);

impl Fake {
    fn with_delay(delay: Duration) -> Fake {
        let fake = Fake::default();
        *fake.0.delay.lock().unwrap() = delay;
        *fake.0.full_delay.lock().unwrap() = Duration::from_millis(300);
        fake
    }

    fn log(&self) -> Vec<Entry> {
        self.0.log.lock().unwrap().clone()
    }

    /// Fichiers traités pour un niveau, dans l'ordre.
    fn order(&self, level: Level) -> Vec<String> {
        self.log()
            .into_iter()
            .filter(|(_, l, _)| *l == level)
            .map(|(p, _, _)| p.to_string_lossy().into_owned())
            .collect()
    }

    fn mark_all_cached(&self, files: &[MediaFile], levels: &[Level]) {
        let mut cached = self.0.cached.lock().unwrap();
        for f in files {
            for l in levels {
                cached.insert((f.path.clone(), *l));
            }
        }
    }
}

fn bump(now: &AtomicUsize, max: &AtomicUsize) {
    let n = now.fetch_add(1, Ordering::SeqCst) + 1;
    max.fetch_max(n, Ordering::SeqCst);
}

impl Producer for Fake {
    fn is_done(&self, file: &MediaFile, level: Level) -> bool {
        self.0
            .cached
            .lock()
            .unwrap()
            .contains(&(file.path.clone(), level))
    }

    fn produce(&self, file: &MediaFile, level: Level, cancel: &AtomicBool) -> Result<(), Failure> {
        let heavy = std::thread::current()
            .name()
            .is_some_and(|n| n.contains("lourd"));
        let (now, max) = if heavy {
            (&self.0.heavy_now, &self.0.heavy_max)
        } else {
            (&self.0.light_now, &self.0.light_max)
        };
        bump(now, max);
        self.0
            .started
            .lock()
            .unwrap()
            .push((file.path.clone(), level));

        let delay = if level == Level::Full {
            *self.0.full_delay.lock().unwrap()
        } else {
            *self.0.delay.lock().unwrap()
        };
        let start = Instant::now();
        let mut result = Ok(());
        while start.elapsed() < delay {
            if cancel.load(Ordering::SeqCst) {
                result = Err(Failure::Cancelled);
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        if result.is_ok() && self.0.failing.lock().unwrap().contains(&file.path) {
            result = Err(Failure::Error("échec simulé".into()));
        }

        now.fetch_sub(1, Ordering::SeqCst);
        self.0
            .log
            .lock()
            .unwrap()
            .push((file.path.clone(), level, result.clone()));
        result
    }
}

fn files(n: usize, kind: MediaKind) -> Vec<MediaFile> {
    let ext = if kind == MediaKind::Raw { "RAF" } else { "JPG" };
    (0..n)
        .map(|i| MediaFile {
            path: format!("img{i}.{ext}").into(),
            kind,
        })
        .collect()
}

type Events = Arc<Mutex<Vec<Event>>>;

/// Scheduler en mode « mélangé » : l'ordre des tâches ne dépend que des priorités.
fn start(fake: &Fake, light_workers: usize) -> (Scheduler, Events) {
    start_with(
        fake,
        Config {
            light_workers,
            thumbnails_first: false,
            exposure: false,
            sharpness: false,
            similarity: false,
            ..Config::default()
        },
    )
}

fn start_with(fake: &Fake, config: Config) -> (Scheduler, Events) {
    let events: Events = Arc::default();
    let sink = events.clone();
    let scheduler = Scheduler::new(fake.clone(), config, move |e| sink.lock().unwrap().push(e));
    (scheduler, events)
}

fn ready(events: &Events, generation: u64, level: Level) -> Vec<usize> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            Event::Ready {
                generation: g,
                index,
                level: l,
                ..
            } if *g == generation && *l == level => Some(*index),
            _ => None,
        })
        .collect()
}

/// Demandes abandonnées signalées pour une génération : (position, niveau).
fn dropped(events: &Events, generation: u64) -> Vec<(usize, Level)> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            Event::Dropped {
                generation: g,
                index,
                level,
                ..
            } if *g == generation => Some((*index, *level)),
            _ => None,
        })
        .collect()
}

fn complete(n: usize) -> Option<Progress> {
    Some(Progress { done: n, total: n })
}

#[test]
fn processes_every_file_once_and_reports_progress() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let (scheduler, events) = start(&fake, 2);
    let mut all = files(3, MediaKind::Raw);
    all.extend(files(3, MediaKind::Flat));
    let generation = scheduler.set_folder(all);
    assert!(scheduler.wait_idle(LONG));

    for level in [Level::Thumbnail, Level::Standard] {
        let mut done = fake.order(level);
        done.sort();
        assert_eq!(
            done.len(),
            6,
            "{level:?} : chaque fichier exactement une fois"
        );
        done.dedup();
        assert_eq!(done.len(), 6);
        assert_eq!(scheduler.progress(level), complete(6));
        assert_eq!(ready(&events, generation, level).len(), 6);
    }
    assert!(
        fake.order(Level::Full).is_empty(),
        "pas de 1:1 sans demande"
    );
    assert_eq!(scheduler.progress(Level::Full), None);

    // Le dernier événement de progression de chaque niveau annonce 6/6.
    let last = |level| {
        events.lock().unwrap().iter().rev().find_map(|e| match e {
            Event::Progress {
                level: l, progress, ..
            } if *l == level => Some(*progress),
            _ => None,
        })
    };
    assert_eq!(last(Level::Thumbnail), complete(6));
    assert_eq!(last(Level::Standard), complete(6));
}

#[test]
fn events_arrive_in_order_and_progress_never_goes_back() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let mut all = files(30, MediaKind::Flat);
    all.extend(files(5, MediaKind::Raw));
    let (scheduler, events) = start(&fake, 4);
    scheduler.set_folder(all);
    assert!(scheduler.wait_idle(LONG));
    for level in [Level::Thumbnail, Level::Standard] {
        let done: Vec<usize> = events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                Event::Progress {
                    level: l, progress, ..
                } if *l == level => Some(progress.done),
                _ => None,
            })
            .collect();
        assert_eq!(done, (1..=35).collect::<Vec<_>>(), "{level:?}");
    }
}

#[test]
fn cached_entries_are_reported_without_recomputing() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let all = files(4, MediaKind::Raw);
    fake.mark_all_cached(&all, &[Level::Thumbnail]);
    let (scheduler, events) = start(&fake, 2);
    let generation = scheduler.set_folder(all);
    assert!(scheduler.wait_idle(LONG));
    assert!(fake.order(Level::Thumbnail).is_empty(), "rien à recalculer");
    assert_eq!(ready(&events, generation, Level::Thumbnail).len(), 4);
    assert_eq!(scheduler.progress(Level::Thumbnail), complete(4));
    assert_eq!(fake.order(Level::Standard).len(), 4);
}

#[test]
fn failures_are_reported_and_still_counted() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let all = files(3, MediaKind::Flat);
    fake.0.failing.lock().unwrap().insert(all[1].path.clone());
    let (scheduler, events) = start(&fake, 2);
    scheduler.set_folder(all);
    assert!(scheduler.wait_idle(LONG));
    let failed: Vec<(usize, Level)> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            Event::Failed {
                index,
                level,
                error,
                ..
            } => {
                assert_eq!(error, "échec simulé");
                Some((*index, *level))
            }
            _ => None,
        })
        .collect();
    assert_eq!(failed.len(), 2);
    assert!(failed.iter().all(|(i, _)| *i == 1));
    assert_eq!(scheduler.progress(Level::Thumbnail), complete(3));
    assert_eq!(scheduler.progress(Level::Standard), complete(3));
}

#[test]
fn heavy_worker_serves_focus_then_neighbours_then_distance() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let all = files(10, MediaKind::Raw);
    fake.mark_all_cached(&all, &[Level::Thumbnail]);
    let (scheduler, _) = start(&fake, 2);
    scheduler.set_paused(true);
    scheduler.set_folder(all);
    scheduler.set_focus(5);
    scheduler.set_paused(false);
    assert!(scheduler.wait_idle(LONG));
    let order: Vec<String> = [5, 6, 4, 7, 3, 8, 2, 9, 1, 0]
        .iter()
        .map(|i| format!("img{i}.RAF"))
        .collect();
    assert_eq!(fake.order(Level::Standard), order);
}

#[test]
fn thumbnails_first_holds_background_previews_until_the_grid_is_done() {
    let fake = Fake::with_delay(Duration::from_millis(5));
    let all = files(12, MediaKind::Raw);
    let (scheduler, _) = start_with(
        &fake,
        Config {
            light_workers: 2,
            thumbnails_first: true,
            exposure: false,
            sharpness: false,
            similarity: false,
            ..Config::default()
        },
    );
    scheduler.set_paused(true);
    scheduler.set_folder(all);
    scheduler.set_focus(4);
    scheduler.set_paused(false);
    assert!(scheduler.wait_idle(LONG));

    let log = fake.log();
    let last_thumbnail = log
        .iter()
        .rposition(|(_, l, _)| *l == Level::Thumbnail)
        .unwrap();
    // Avant la dernière vignette, seuls l'image affichée et ses voisines ont un aperçu.
    let early: HashSet<String> = log[..last_thumbnail]
        .iter()
        .filter(|(_, l, _)| *l == Level::Standard)
        .map(|(p, _, _)| p.to_string_lossy().into_owned())
        .collect();
    let allowed: HashSet<String> = [3, 4, 5].iter().map(|i| format!("img{i}.RAF")).collect();
    assert!(early.is_subset(&allowed), "aperçus trop tôt : {early:?}");
    assert_eq!(
        fake.order(Level::Standard).len(),
        12,
        "tout finit par être calculé"
    );
}

#[test]
fn embedded_previews_are_scheduled_for_raw_files_only() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let mut all = files(3, MediaKind::Raw);
    all.extend(files(2, MediaKind::Flat));
    let (scheduler, events) = start(&fake, 2);
    let generation = scheduler.set_folder(all);
    assert!(scheduler.wait_idle(LONG));
    assert_eq!(scheduler.progress(Level::Embedded), complete(3));
    let mut embedded = fake.order(Level::Embedded);
    embedded.sort();
    assert_eq!(embedded, ["img0.RAF", "img1.RAF", "img2.RAF"]);
    let mut ready_embedded = ready(&events, generation, Level::Embedded);
    ready_embedded.sort();
    assert_eq!(
        ready_embedded,
        [0, 1, 2],
        "aperçus boîtier des 3 RAW seulement"
    );
}

#[test]
fn raw_renders_wait_for_embedded_previews_in_thumbnails_first_mode() {
    let fake = Fake::with_delay(Duration::from_millis(5));
    let all = files(10, MediaKind::Raw);
    let (scheduler, _) = start_with(
        &fake,
        Config {
            light_workers: 1, // ordre des tâches légères déterministe
            thumbnails_first: true,
            exposure: false,
            sharpness: false,
            similarity: false,
            ..Config::default()
        },
    );
    scheduler.set_paused(true);
    scheduler.set_folder(all);
    scheduler.set_focus(4);
    scheduler.set_paused(false);
    assert!(scheduler.wait_idle(LONG));

    let log = fake.log();
    let last_light = log
        .iter()
        .rposition(|(_, l, _)| matches!(l, Level::Thumbnail | Level::Embedded))
        .unwrap();
    let early: HashSet<String> = log[..last_light]
        .iter()
        .filter(|(_, l, _)| *l == Level::Standard)
        .map(|(p, _, _)| p.to_string_lossy().into_owned())
        .collect();
    let allowed: HashSet<String> = [3, 4, 5].iter().map(|i| format!("img{i}.RAF")).collect();
    assert!(early.is_subset(&allowed), "rendus RAW trop tôt : {early:?}");
    // L'aperçu boîtier de l'image affichée passe avant toutes les vignettes (le journal suit
    // l'ordre de fin des tâches : un seul worker léger pour qu'il suive aussi l'ordre de départ).
    let first_light = log
        .iter()
        .find(|(_, l, _)| matches!(l, Level::Thumbnail | Level::Embedded))
        .unwrap();
    assert_eq!(
        (first_light.0.to_string_lossy().as_ref(), first_light.1),
        ("img4.RAF", Level::Embedded)
    );
}

#[test]
fn raw_renders_never_overlap_but_light_jobs_do() {
    let fake = Fake::with_delay(Duration::from_millis(40));
    let mut all = files(4, MediaKind::Raw);
    all.extend(files(8, MediaKind::Flat));
    let (scheduler, _) = start(&fake, 4);
    scheduler.set_folder(all);
    assert!(scheduler.wait_idle(LONG));
    assert_eq!(fake.0.heavy_max.load(Ordering::SeqCst), 1);
    assert!(
        fake.0.light_max.load(Ordering::SeqCst) > 1,
        "les workers légers doivent travailler en parallèle"
    );
}

#[test]
fn full_render_is_on_demand_and_dropped_when_focus_moves_away() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    *fake.0.full_delay.lock().unwrap() = Duration::from_secs(5);
    let all = files(5, MediaKind::Raw);
    fake.mark_all_cached(&all, &[Level::Thumbnail, Level::Embedded, Level::Standard]);
    let (scheduler, events) = start(&fake, 1);
    let generation = scheduler.set_folder(all);

    scheduler.request(1, Level::Full); // sans image affichée : sans effet
    scheduler.set_focus(1);
    scheduler.request(3, Level::Full); // pas voisine de l'image affichée : ignoré
    scheduler.request(1, Level::Full);
    let deadline = Instant::now() + LONG;
    while !fake
        .0
        .started
        .lock()
        .unwrap()
        .iter()
        .any(|(_, l)| *l == Level::Full)
    {
        assert!(Instant::now() < deadline, "le 1:1 n'a jamais démarré");
        std::thread::sleep(Duration::from_millis(2));
    }

    let moved = Instant::now();
    scheduler.set_focus(4); // 1 n'est plus voisine : son 1:1 en cours est annulé
    assert!(scheduler.wait_idle(LONG));
    assert!(
        moved.elapsed() < Duration::from_secs(2),
        "annulation trop lente"
    );
    let full = fake
        .log()
        .into_iter()
        .filter(|(_, l, _)| *l == Level::Full)
        .collect::<Vec<_>>();
    assert_eq!(
        full,
        [("img1.RAF".into(), Level::Full, Err(Failure::Cancelled))]
    );
    assert!(ready(&events, generation, Level::Full).is_empty());
    assert_eq!(
        dropped(&events, generation),
        [(1, Level::Full)],
        "l'annulation est signalée"
    );

    // Un 1:1 demandé pour l'image affichée aboutit.
    *fake.0.full_delay.lock().unwrap() = Duration::from_millis(10);
    scheduler.request(4, Level::Full);
    assert!(scheduler.wait_idle(LONG));
    assert_eq!(ready(&events, generation, Level::Full), [4]);
}

#[test]
fn zoom_of_neighbours_can_be_prefetched_and_survives_a_step() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    *fake.0.full_delay.lock().unwrap() = Duration::from_millis(20);
    let all = files(5, MediaKind::Raw);
    let (scheduler, events) = start(&fake, 1);
    let generation = scheduler.set_folder(all);
    scheduler.set_focus(2);
    for i in [2, 3, 1] {
        scheduler.request(i, Level::EmbeddedFull);
    }
    scheduler.set_focus(3); // 2 et 3 restent proches, 1 non
    assert!(scheduler.wait_idle(LONG));
    let mut ready_full = ready(&events, generation, Level::EmbeddedFull);
    ready_full.sort();
    let mut dropped_or_ready = ready_full.clone();
    dropped_or_ready.extend(dropped(&events, generation).iter().map(|(i, _)| *i));
    dropped_or_ready.sort();
    assert!(ready_full.contains(&2) && ready_full.contains(&3));
    assert_eq!(
        dropped_or_ready,
        [1, 2, 3],
        "1 calculée à temps ou abandonnée, jamais perdue"
    );
}

#[test]
fn new_folder_cancels_and_supersedes_the_previous_one() {
    let fake = Fake::with_delay(Duration::from_millis(50));
    let (scheduler, events) = start(&fake, 2);
    let first = scheduler.set_folder(files(20, MediaKind::Raw));
    std::thread::sleep(Duration::from_millis(20));
    let second = scheduler.set_folder(files(2, MediaKind::Flat));
    assert!(second > first);
    assert!(scheduler.wait_idle(LONG));

    assert_eq!(scheduler.progress(Level::Standard), complete(2));
    assert_eq!(ready(&events, second, Level::Thumbnail).len(), 2);
    assert_eq!(ready(&events, second, Level::Standard).len(), 2);
    // Le premier dossier a été abandonné bien avant la fin.
    assert!(ready(&events, first, Level::Standard).len() < 20);
    let raf_after_switch = events.lock().unwrap().iter().any(|e| {
        matches!(e, Event::Ready { generation, path, .. }
            if *generation == second && path.extension().is_some_and(|x| x == "RAF"))
    });
    assert!(!raf_after_switch);
}

#[test]
fn pause_holds_work_until_resumed() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let (scheduler, _) = start(&fake, 2);
    scheduler.set_paused(true);
    scheduler.set_folder(files(3, MediaKind::Flat));
    assert!(
        scheduler.wait_idle(LONG),
        "en pause sans tâche en cours = au repos"
    );
    std::thread::sleep(Duration::from_millis(30));
    assert!(fake.log().is_empty());
    assert_eq!(
        scheduler.progress(Level::Thumbnail),
        Some(Progress { done: 0, total: 3 })
    );
    scheduler.set_paused(false);
    assert!(scheduler.wait_idle(LONG));
    assert_eq!(scheduler.progress(Level::Thumbnail), complete(3));
}

#[test]
fn on_demand_policy_leaves_raw_renders_to_requests() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let mut all = files(6, MediaKind::Raw);
    all.extend(files(2, MediaKind::Flat));
    let (scheduler, events) = start_with(
        &fake,
        Config {
            light_workers: 2,
            thumbnails_first: true,
            raw_previews: RawPreviews::OnDemand,
            exposure: false,
            sharpness: false,
            similarity: false,
        },
    );
    let generation = scheduler.set_folder(all);
    assert!(scheduler.wait_idle(LONG));
    // En fond : aucun rendu RAW, mais les aperçus des images plates.
    let mut background = fake.order(Level::Standard);
    background.sort(); // deux workers : l'ordre de fin varie
    assert_eq!(
        background,
        ["img0.JPG", "img1.JPG"],
        "seuls les aperçus des images plates"
    );
    assert_eq!(scheduler.progress(Level::Standard), complete(2));

    // À la demande : l'image affichée et ses voisines, sans toucher à la progression.
    scheduler.set_focus(3);
    for i in [2, 3, 4, 0] {
        scheduler.request(i, Level::Standard); // 0 : trop loin, ignorée
    }
    assert!(scheduler.wait_idle(LONG));
    let mut requested = ready(&events, generation, Level::Standard)
        .into_iter()
        .filter(|&i| i < 6)
        .collect::<Vec<_>>();
    requested.sort();
    assert_eq!(requested, [2, 3, 4]);
    assert_eq!(scheduler.progress(Level::Standard), complete(2));

    // Déjà en cache : redemander donne juste un nouvel événement Ready.
    fake.mark_all_cached(&files(6, MediaKind::Raw)[3..4], &[Level::Standard]);
    let before = fake.order(Level::Standard).len();
    scheduler.request(3, Level::Standard);
    assert!(scheduler.wait_idle(LONG));
    assert_eq!(fake.order(Level::Standard).len(), before, "pas de recalcul");
}

#[test]
fn moving_focus_drops_requests_that_are_no_longer_near() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let all = files(10, MediaKind::Raw);
    let (scheduler, events) = start_with(
        &fake,
        Config {
            light_workers: 1,
            thumbnails_first: true,
            raw_previews: RawPreviews::OnDemand,
            exposure: false,
            sharpness: false,
            similarity: false,
        },
    );
    scheduler.set_paused(true);
    let generation = scheduler.set_folder(all);
    scheduler.set_focus(2);
    scheduler.request(2, Level::Standard);
    scheduler.request(3, Level::Standard);
    scheduler.request(2, Level::EmbeddedFull);
    scheduler.set_focus(3); // 2 reste voisine, avec son 1:1 (préchargement)
    scheduler.set_focus(8); // plus rien de proche
    scheduler.set_paused(false);
    assert!(scheduler.wait_idle(LONG));
    assert!(fake.order(Level::Standard).is_empty());
    assert!(fake.order(Level::EmbeddedFull).is_empty());
    let mut gone = dropped(&events, generation);
    gone.sort_by_key(|(i, l)| (*i, Level::ALL.iter().position(|a| a == l)));
    assert_eq!(
        gone,
        [
            (2, Level::Standard),
            (2, Level::EmbeddedFull),
            (3, Level::Standard)
        ],
        "chaque abandon est signalé"
    );
}

#[test]
fn shown_images_of_a_comparison_are_all_served() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let all = files(10, MediaKind::Raw);
    fake.mark_all_cached(&all, &[Level::Thumbnail, Level::Embedded, Level::Standard]);
    let (scheduler, events) = start(&fake, 1);
    let generation = scheduler.set_folder(all);
    // Comparaison de 2, 5 et 8 (5 active) : toutes acceptent un 1:1, pas les autres.
    // En pause pendant les demandes : sinon le worker peut partir sur 2 avant que 5 ne
    // soit demandée (test instable).
    scheduler.set_paused(true);
    scheduler.set_shown(&[5, 2, 8]);
    for i in [2, 5, 8, 6] {
        scheduler.request(i, Level::EmbeddedFull);
    }
    scheduler.set_paused(false);
    assert!(scheduler.wait_idle(LONG));
    let mut done: Vec<usize> = ready(&events, generation, Level::EmbeddedFull)
        .into_iter()
        .collect();
    done.sort();
    assert_eq!(done, [2, 5, 8], "6 n'est pas affichée");
    // L'image active d'abord.
    assert_eq!(fake.order(Level::EmbeddedFull)[0], "img5.RAF");
    // Retour au plein écran sur 5 : 2 et 8 ne sont plus affichées.
    scheduler.set_paused(true);
    scheduler.set_shown(&[2, 8]);
    scheduler.request(8, Level::Full);
    scheduler.set_focus(5);
    scheduler.set_paused(false);
    assert!(scheduler.wait_idle(LONG));
    assert!(fake.order(Level::Full).is_empty());
    assert_eq!(dropped(&events, generation), [(8, Level::Full)]);
}

#[test]
fn focus_out_of_range_is_ignored() {
    let fake = Fake::with_delay(Duration::from_millis(1));
    let (scheduler, _) = start(&fake, 1);
    scheduler.set_folder(files(2, MediaKind::Flat));
    scheduler.set_focus(99);
    scheduler.request(0, Level::Full);
    assert!(scheduler.wait_idle(LONG));
    assert!(fake.order(Level::Full).is_empty());
}

#[test]
fn dropping_stops_promptly_with_work_pending() {
    let fake = Fake::with_delay(Duration::from_millis(50));
    let (scheduler, _) = start(&fake, 2);
    scheduler.set_folder(files(200, MediaKind::Raw));
    std::thread::sleep(Duration::from_millis(30));
    let t = Instant::now();
    drop(scheduler);
    assert!(t.elapsed() < Duration::from_secs(1), "arrêt trop lent");
    assert!(fake.log().len() < 50);
}

/// Vrai producteur sur un vrai RAW et une image plate, avec un cache temporaire.
#[test]
#[ignore = "nécessite TRI_TEST_RAW"]
fn real_previews_fill_the_cache_and_are_reused() {
    use tri::cache::Cache;
    use tri::preview::{self, Settings, Sizes};
    use tri::scheduler::CachedPreviews;

    let sample: PathBuf = std::env::var_os("TRI_TEST_RAW")
        .expect("définir TRI_TEST_RAW")
        .into();
    let dir = tempfile::tempdir().unwrap();
    let photos = dir.path().join("photos");
    std::fs::create_dir(&photos).unwrap();
    let ext = sample.extension().unwrap().to_string_lossy().into_owned();
    std::fs::copy(&sample, photos.join(format!("a.{ext}"))).unwrap();
    image::RgbImage::from_pixel(3000, 2000, image::Rgb([200, 120, 40]))
        .save(photos.join("b.jpg"))
        .unwrap();

    let files = tri::media::scan(&photos, 1).unwrap();
    assert_eq!(files.len(), 2);
    let settings = Settings {
        sizes: Sizes {
            thumbnail: 400,
            standard: 1024,
        },
        ..Settings::default()
    };
    let cache = Cache::open(dir.path().join("cache")).unwrap();
    let run = |cache: &Cache| {
        let producer = CachedPreviews {
            cache: cache.clone(),
            settings,
            developers: tri::develop::Developers::none(),
        };
        let scheduler = Scheduler::new(
            producer,
            Config {
                exposure: false,
                sharpness: false,
                similarity: false,
                ..Config::default()
            },
            |_| {},
        );
        let t = Instant::now();
        scheduler.set_folder(files.clone());
        assert!(scheduler.wait_idle(Duration::from_secs(120)));
        assert_eq!(scheduler.progress(Level::Standard), complete(2));
        assert_eq!(
            scheduler.progress(Level::Embedded),
            complete(1),
            "un seul RAW"
        );
        t.elapsed()
    };

    let first = run(&cache);
    let long_side = |b: &tri::bitmap::Bitmap| b.width.max(b.height);
    for f in &files {
        let thumb = cache
            .get(&preview::key(f, Level::Thumbnail, &settings).unwrap())
            .unwrap();
        let standard = cache
            .get(&preview::key(f, Level::Standard, &settings).unwrap())
            .unwrap();
        assert_eq!(long_side(&thumb), 400, "{}", f.path.display());
        assert_eq!(long_side(&standard), 1024, "{}", f.path.display());
        assert_eq!(
            thumb.height > thumb.width,
            standard.height > standard.width,
            "vignette et aperçu dans le même sens : {}",
            f.path.display()
        );
    }

    // Aperçu boîtier : pour le RAW seulement, à la taille standard, dans le même sens que le
    // rendu RAW.
    for f in &files {
        let embedded = cache.get(&preview::key(f, Level::Embedded, &settings).unwrap());
        match f.kind {
            MediaKind::Raw => {
                let embedded = embedded.expect("aperçu boîtier du RAW");
                let standard = cache
                    .get(&preview::key(f, Level::Standard, &settings).unwrap())
                    .unwrap();
                assert_eq!(long_side(&embedded), 1024);
                assert_eq!(
                    embedded.height > embedded.width,
                    standard.height > standard.width,
                    "aperçu boîtier et rendu RAW dans le même sens"
                );
            }
            MediaKind::Flat => assert!(embedded.is_none(), "pas d'aperçu boîtier pour un JPEG"),
        }
    }

    // 1:1 du JPEG boîtier, à la demande : taille d'origine du JPEG embarqué, redressé.
    {
        let raw_index = files.iter().position(|f| f.kind == MediaKind::Raw).unwrap();
        let producer = CachedPreviews {
            cache: cache.clone(),
            settings,
            developers: tri::develop::Developers::none(),
        };
        let scheduler = Scheduler::new(
            producer,
            Config {
                exposure: false,
                sharpness: false,
                similarity: false,
                ..Config::default()
            },
            |_| {},
        );
        scheduler.set_folder(files.clone());
        scheduler.set_focus(raw_index);
        scheduler.request(raw_index, Level::EmbeddedFull);
        assert!(scheduler.wait_idle(Duration::from_secs(120)));
        let f = &files[raw_index];
        let full = cache
            .get(&preview::key(f, Level::EmbeddedFull, &settings).unwrap())
            .expect("1:1 du JPEG boîtier");
        let standard = cache
            .get(&preview::key(f, Level::Embedded, &settings).unwrap())
            .unwrap();
        println!("JPEG boîtier 1:1 : {}x{}", full.width, full.height);
        assert!(
            long_side(&full) > long_side(&standard),
            "plus grand que l'aperçu"
        );
        assert_eq!(full.height > full.width, standard.height > standard.width);
    }

    let second = run(&cache);
    println!("premier passage {first:?}, second (cache) {second:?}");
    assert!(second < first / 4, "le second passage doit venir du cache");
}

#[test]
fn exposure_analyses_run_after_previews_of_the_grid_and_before_raw_renders() {
    let fake = Fake::with_delay(Duration::from_millis(2));
    let mut all = files(8, MediaKind::Raw);
    all.extend(files(2, MediaKind::Flat));
    let (scheduler, events) = start_with(
        &fake,
        Config {
            light_workers: 2,
            thumbnails_first: true,
            raw_previews: RawPreviews::Background,
            exposure: true,
            sharpness: false,
            similarity: false,
        },
    );
    let generation = scheduler.set_folder(all);
    assert!(scheduler.wait_idle(LONG));
    assert_eq!(
        scheduler.progress(Level::Exposure),
        complete(10),
        "RAW et plates"
    );
    let mut analysed = ready(&events, generation, Level::Exposure);
    analysed.sort();
    assert_eq!(analysed, (0..10).collect::<Vec<_>>());
    let log = fake.log();
    let position = |pred: &dyn Fn(&Entry) -> bool| log.iter().position(pred).unwrap();
    let last_embedded = log
        .iter()
        .rposition(|(_, l, _)| *l == Level::Embedded)
        .unwrap();
    let first_raw_exposure =
        position(&|(p, l, _)| *l == Level::Exposure && p.to_string_lossy().ends_with(".RAF"));
    let last_raw_exposure = log
        .iter()
        .rposition(|(p, l, _)| *l == Level::Exposure && p.to_string_lossy().ends_with(".RAF"))
        .unwrap();
    let first_raw_render =
        position(&|(p, l, _)| *l == Level::Standard && p.to_string_lossy().ends_with(".RAF"));
    assert!(
        first_raw_exposure > last_embedded,
        "analyses de fond après les aperçus boîtier"
    );
    assert!(
        last_raw_exposure < first_raw_render,
        "toutes les analyses RAW avant le premier rendu RAW de fond (même worker)"
    );
}

#[test]
fn files_added_on_the_way_join_the_queue_without_restarting_it() {
    // Issue n° 40 : le déchargement ajoute chaque photo copiée, sans interrompre les
    // calculs en cours.
    let fake = Fake::with_delay(Duration::from_millis(20));
    let (scheduler, events) = start(&fake, 1);
    let all = files(4, MediaKind::Flat);
    let generation = scheduler.set_folder(all[..1].to_vec());
    scheduler.add_files(all[1..3].to_vec());
    scheduler.add_files(all[3..].to_vec());
    assert!(scheduler.wait_idle(LONG));
    let mut done = ready(&events, generation, Level::Thumbnail);
    done.sort();
    assert_eq!(
        done,
        vec![0, 1, 2, 3],
        "même génération, positions à la suite"
    );
    assert!(
        fake.log().iter().all(|(_, _, r)| r.is_ok()),
        "rien d'annulé"
    );
    assert_eq!(
        scheduler.progress(Level::Thumbnail),
        Some(Progress { done: 4, total: 4 })
    );
}

#[test]
fn a_refiltered_folder_keeps_the_work_in_progress() {
    // Issue n° 73 : refiltrer la grille (même dossier, liste réduite) n'abandonne plus le
    // calcul en cours d'une image qui reste ; celui d'une image retirée est annulé.
    let fake = Fake::with_delay(Duration::from_millis(150));
    let (scheduler, events) = start(&fake, 2);
    let all = files(3, MediaKind::Flat);
    scheduler.set_folder(all.clone());
    let deadline = Instant::now() + LONG;
    while fake.0.started.lock().unwrap().len() < 2 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    // img0 et img1 en cours ; img1 retirée, img0 passe en seconde place.
    let generation = scheduler.set_folder(vec![all[2].clone(), all[0].clone()]);
    assert!(scheduler.wait_idle(LONG));
    let thumbnails: Vec<_> = fake
        .log()
        .into_iter()
        .filter(|(_, l, _)| *l == Level::Thumbnail)
        .collect();
    let of = |name: &str| -> Vec<Result<(), Failure>> {
        thumbnails
            .iter()
            .filter(|(p, _, _)| p.ends_with(name))
            .map(|(_, _, r)| r.clone())
            .collect()
    };
    assert_eq!(
        of("img0.JPG"),
        vec![Ok(())],
        "gardée, calculée une seule fois"
    );
    assert_eq!(
        of("img1.JPG"),
        vec![Err(Failure::Cancelled)],
        "retirée : annulée"
    );
    let mut done = ready(&events, generation, Level::Thumbnail);
    done.sort();
    assert_eq!(done, vec![0, 1], "signalées à leur nouvelle place");
    assert_eq!(
        scheduler.progress(Level::Thumbnail),
        Some(Progress { done: 2, total: 2 })
    );
}

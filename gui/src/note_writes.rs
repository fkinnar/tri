//! Notes écrites hors du fil de l'interface, dans l'ordre.
//!
//! Retour du 19/11/2026 (`TRI_IO_WATCH`, montage iCloud) : noter une photo faisait une
//! dizaine d'accès disque sur le fil de l'interface (sidecars de la photo et de sa paire,
//! données de rapprochement, relecture), donc plusieurs secondes de fenêtre figée à chaque
//! étoile sur un support lent. Le moteur prépare désormais la note en mémoire
//! ([`tri::notes::NoteBook::prepare_edit`], affichée tout de suite) ; l'écriture se fait
//! ici, dans un seul fil (3 étoiles puis 4 : 4 sur le disque), et son résultat revient à
//! l'interface ([`tri::notes::NoteBook::finish_edit`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui;
use tri::notes::{EditDone, PendingEdit};

/// Le fil des écritures de notes, et ce qu'il reste à écrire.
pub struct NoteWrites {
    jobs: Sender<(u64, PendingEdit)>,
    results: Receiver<(u64, EditDone)>,
    /// Écritures demandées, pas encore revenues, par image (dossier ouvert).
    pending: HashMap<PathBuf, usize>,
    /// Dossier ouvert : une écriture d'un dossier quitté revient sans rien changer.
    generation: u64,
    /// Écritures envoyées au fil et pas encore faites, tous dossiers confondus
    /// ([`NoteWrites::flush`]).
    unfinished: Arc<AtomicUsize>,
    /// Depuis quand quelque chose attend (la tâche de la barre d'état n'apparaît
    /// qu'au-delà d'un instant).
    since: Option<Instant>,
}

impl NoteWrites {
    /// Démarre le fil ; `repaint` réveille l'interface à chaque écriture finie.
    pub fn new(repaint: Option<egui::Context>) -> NoteWrites {
        let (jobs, inbox) = mpsc::channel::<(u64, PendingEdit)>();
        let (outbox, results) = mpsc::channel();
        let unfinished = Arc::new(AtomicUsize::new(0));
        let left = unfinished.clone();
        let _ = std::thread::Builder::new()
            .name("tri-notes".into())
            .spawn(move || {
                for (generation, job) in inbox {
                    let done = job.run();
                    left.fetch_sub(1, Ordering::SeqCst);
                    if outbox.send((generation, done)).is_err() {
                        return;
                    }
                    if let Some(ctx) = &repaint {
                        ctx.request_repaint();
                    }
                }
            });
        NoteWrites {
            jobs,
            results,
            pending: HashMap::new(),
            generation: 0,
            unfinished,
            since: None,
        }
    }

    /// Envoie l'écriture `job`, faite après celles déjà demandées.
    pub fn submit(&mut self, job: PendingEdit) {
        for image in job.images() {
            *self.pending.entry(image.clone()).or_default() += 1;
        }
        self.since.get_or_insert_with(Instant::now);
        self.unfinished.fetch_add(1, Ordering::SeqCst);
        if self.jobs.send((self.generation, job)).is_err() {
            self.unfinished.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Écritures revenues pour le dossier ouvert, dans l'ordre. Une image n'est plus
    /// en attente que quand toutes les siennes sont revenues : [`NoteWrites::is_pending`]
    /// le dit à [`tri::notes::NoteBook::finish_edit`] juste après.
    pub fn take(&mut self) -> Vec<EditDone> {
        let mut done = Vec::new();
        for (generation, result) in self.results.try_iter() {
            if generation != self.generation {
                continue;
            }
            for image in result.images() {
                if let Some(n) = self.pending.get_mut(image) {
                    *n -= 1;
                    if *n == 0 {
                        self.pending.remove(image);
                    }
                }
            }
            done.push(result);
        }
        if self.pending.is_empty() {
            self.since = None;
        }
        done
    }

    /// Vrai si une écriture de `image` attend encore.
    pub fn is_pending(&self, image: &Path) -> bool {
        self.pending.contains_key(image)
    }

    /// Vrai si `sidecar` est celui d'une image dont une écriture attend : `IMG.RAF.xmp`
    /// pour `IMG.RAF`, `IMG.xmp` pour `IMG.RAF` et `IMG.JPG` (sans la casse).
    pub fn writes_sidecar(&self, sidecar: &Path) -> bool {
        let Some(stem) = sidecar
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
        else {
            return false;
        };
        let dir = sidecar.parent();
        self.pending.keys().any(|image| {
            let lower = |s: Option<&std::ffi::OsStr>| s.map(|s| s.to_string_lossy().to_lowercase());
            image.parent() == dir
                && (lower(image.file_name()) == Some(stem.clone())
                    || lower(image.file_stem()) == Some(stem.clone()))
        })
    }

    /// Images dont une écriture attend (dossier ouvert).
    pub fn waiting(&self) -> usize {
        self.pending.len()
    }

    /// Vrai si des écritures attendent depuis plus de `after` (tâche de la barre d'état).
    pub fn slow(&self, after: Duration) -> bool {
        self.since.is_some_and(|t| t.elapsed() >= after)
    }

    /// Autre dossier : les écritures en cours se finissent, sans rien changer à celui-ci.
    pub fn new_folder(&mut self) {
        self.generation += 1;
        self.pending.clear();
        self.since = None;
    }

    /// Attend que tout ce qui a été envoyé soit écrit, au plus `timeout` (fermeture,
    /// annulation) ; vrai si c'est fait.
    pub fn flush(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while self.unfinished.load(Ordering::SeqCst) > 0 {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        true
    }
}

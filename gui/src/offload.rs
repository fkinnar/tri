//! Boîte de déchargement d'une carte : l'interface du module moteur [`tri::offload`].
//! Inventaire et copie tournent dans des fils à part ; la boîte ne fait qu'afficher et
//! transmettre les choix.

use crate::cache_panel::size_text;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui::{self, Pos2};
use tri::offload::{self, History, OffloadSettings, OutOfSpace, Plan, Progress, Report};
use tri::scheduler::Scheduler;

/// De quoi calculer les aperçus de la séance pendant la copie (issue n° 40) : un
/// scheduler à part, gardé aussi par l'application (la boîte peut se fermer avant la fin
/// des calculs), et la place voulue pour le cache de la séance.
pub struct SessionPreviews {
    /// Scheduler de la séance (dossier vide au départ, photos ajoutées une à une).
    pub scheduler: Arc<Scheduler>,
    /// Cache à remplir.
    pub cache: tri::cache::Cache,
    /// `[cache] location`, appliqué à la séance avant la copie.
    pub location: tri::cache::Location,
    /// Réglages du panneau des dossiers (disques amovibles, pour `location = "disk"`).
    pub places: tri::places::PlaceSettings,
}

/// De quoi proposer « garder les données de tri » de la séance (issue n° 378) : le cache
/// (ses entrées déjà calculées suivent) et les racines des disques amovibles.
pub struct KeepData {
    pub cache: tri::cache::Cache,
    pub removable: Vec<PathBuf>,
}

/// Où poser l'interrupteur « garder les données de tri » pour la séance `dest` : la racine
/// du disque amovible qui la contient (vrai), sinon la séance elle-même (faux) ; `None` :
/// déjà gardées (un interrupteur au-dessus), ou rien pour proposer.
fn keep_root(keep: Option<&KeepData>, dest: &Path) -> Option<(PathBuf, bool)> {
    let keep = keep?;
    if tri::store::kept(dest) {
        return None;
    }
    Some(
        match keep
            .removable
            .iter()
            .filter(|r| dest.starts_with(r))
            .max_by_key(|r| r.components().count())
        {
            Some(root) => (root.clone(), true),
            None => (dest.to_path_buf(), false),
        },
    )
}

/// Ce que la boîte demande à l'application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Fermer la boîte.
    Close,
    /// Copie terminée : enchaîner ce qui a été coché. Boîte affichée : elle reste ouverte
    /// sur le bilan. En arrière-plan : elle n'existe plus, le bilan va dans la barre d'état.
    Finished {
        /// Dossier de la séance à ouvrir tout de suite (boîte affichée).
        open: Option<PathBuf>,
        /// Séance à *proposer* d'ouvrir (arrière-plan : ne pas changer de dossier sous les
        /// yeux de l'utilisateur).
        offer: Option<PathBuf>,
        /// Volume à éjecter.
        eject: Option<PathBuf>,
        /// Bilan en une ligne.
        summary: String,
    },
    /// Ouvrir ce dossier dans la grille (bouton du bilan).
    Open(PathBuf),
    /// Éjecter ce volume (la carte).
    Eject(PathBuf),
}

enum Message {
    Progress(Progress),
    /// Destination pleine : la copie attend la réponse (réessayer ou abandonner).
    Full(OutOfSpace),
    Done(Report),
}

/// Images déjà dans les destinations : (destinations, (RAW seulement, ignorer les déjà
/// déchargées), (nombre, octets)).
type Present = (Vec<PathBuf>, (bool, bool), (usize, u64));

/// Une séance de la carte dans la boîte (#507) : cochée ou non, et son dossier.
#[derive(Clone, Debug)]
struct SessionRow {
    session: offload::Session,
    on: bool,
    folder: String,
    /// Images et octets à copier, et si toutes sont déjà déchargées.
    count: usize,
    bytes: u64,
    known: bool,
    /// Photos choisies dans la galerie ([`crate::offload_picker`]) : leurs fichiers ;
    /// `None` : toute la séance.
    picked: Option<std::collections::HashSet<PathBuf>>,
}

impl SessionRow {
    /// Vrai si l'image `item` (de cette séance) fait partie du choix.
    fn keeps(&self, item: &offload::Item) -> bool {
        self.picked
            .as_ref()
            .is_none_or(|p| p.contains(&item.source))
    }
}

/// Ce que la copie fera : (images, dossier) par séance cochée, ou la carte entière vers
/// `folder` quand elle n'a qu'une séance (`sessions` vide).
fn targets(
    plan: &Plan,
    sessions: &[SessionRow],
    root: &str,
    folder: &str,
) -> Result<Vec<(Plan, PathBuf)>, String> {
    let root = tri::settings::expand_home(root.trim());
    // Une seule séance : le dossier du champ, les photos choisies dans sa galerie s'il y
    // en a.
    if sessions.len() <= 1 {
        let dest = offload::destination(&root, folder).map_err(|e| e.to_string())?;
        let plan = match sessions.first() {
            Some(row) => plan.subset(|i| row.keeps(i)),
            None => plan.clone(),
        };
        return Ok(vec![(plan, dest)]);
    }
    sessions
        .iter()
        .filter(|row| row.on)
        .map(|row| {
            let dest = offload::destination(&root, &row.folder).map_err(|e| e.to_string())?;
            let chosen: std::collections::HashSet<&Path> = row
                .session
                .items
                .iter()
                .map(|&i| &plan.items[i])
                .filter(|i| row.keeps(i))
                .map(|i| i.source.as_path())
                .collect();
            Ok((plan.subset(|i| chosen.contains(i.source.as_path())), dest))
        })
        .collect()
}

/// Les séances de `plan` (#507) : une ligne par séance s'il y en a plusieurs ; une seule
/// séance n'a pas de ligne, mais sa galerie (retour du 25/11/2026 : une carte d'un seul
/// jour n'avait pas de « Choisir les photos… »). Cochées d'office : celles qui ont du
/// nouveau (toutes, si la carte est déjà déchargée).
fn session_rows(plan: &Plan, settings: &OffloadSettings) -> Vec<SessionRow> {
    // Comptes remplis par la boîte, selon les choix du moment.
    let sessions = plan.sessions(settings.day_cut);
    let mut rows: Vec<SessionRow> = sessions
        .into_iter()
        .map(|session| {
            let known = session.items.iter().all(|&i| plan.items[i].known);
            SessionRow {
                folder: session.folder(),
                count: 0,
                bytes: 0,
                picked: None,
                on: !known,
                known,
                session,
            }
        })
        .collect();
    if rows.iter().all(|r| !r.on) {
        for row in &mut rows {
            row.on = true;
        }
    }
    rows
}

enum Stage {
    /// Inventaire de la carte en cours.
    Planning(Receiver<Result<(Plan, History), String>>),
    /// Choix de la destination.
    Ready {
        plan: Plan,
        history: Option<History>,
        root: String,
        /// Champ de la racine (complétion par Tab).
        root_field: crate::path_field::PathField,
        folder: String,
        error: Option<String>,
        /// Place libre dans la destination : (destination, octets), relue quand elle
        /// change (avertissement avant la copie).
        free: Option<(PathBuf, Option<u64>)>,
        /// Déjà présentes dans la destination : (destination, (RAW seulement, ignorer les
        /// déjà déchargées), (nombre, octets)), recalculé seulement quand la destination
        /// ou ces choix changent.
        present: Option<Present>,
        /// Séances de la carte (#507), quand elle en a plusieurs : un dossier par séance
        /// cochée.
        sessions: Vec<SessionRow>,
    },
    /// Copie en cours.
    Running {
        rx: Receiver<Message>,
        progress: Progress,
        cancel: Arc<AtomicBool>,
        /// Destination pleine : la question posée, en attente de réponse ([`Message::Full`]).
        full: Option<OutOfSpace>,
        /// Réponse à la copie : `true` réessayer, `false` abandonner.
        answer: Sender<bool>,
        /// Début de la copie, et dernier changement de l'avancement (#470 : dire depuis
        /// quand rien ne bouge).
        started: std::time::Instant,
        last_activity: std::time::Instant,
    },
    /// Bilan.
    Done {
        report: Report,
        /// Suites cochées déjà déclenchées.
        finished: bool,
        /// Suppression de la carte faite : échecs.
        deleted: Option<Vec<(PathBuf, String)>>,
        opened: bool,
        ejected: bool,
    },
    /// Erreur d'inventaire.
    Failed(String),
}

/// Boîte de déchargement de la carte `source`.
pub struct OffloadDialog {
    source: PathBuf,
    at: Pos2,
    settings: OffloadSettings,
    /// Dossier des données (historique, racine mémorisée).
    data: Option<PathBuf>,
    stage: Stage,
    /// En arrière-plan : la copie continue, la boîte n'est pas affichée.
    hidden: bool,
    /// « Décharger ici » : racine choisie dans le panneau des dossiers (proposée à la place
    /// de la racine retenue, et pas retenue pour les déchargements suivants).
    root_here: Option<String>,
    /// « Décharger ici » : supports amovibles possibles (dossier, nom), au choix.
    sources: Vec<(PathBuf, String)>,
    /// Carte suivante (issue n° 37) : le dossier de séance de la précédente, proposé à
    /// la place du nom suggéré.
    folder_here: Option<String>,
    /// Aperçus calculés pendant la copie ([`OffloadSettings::previews`]).
    previews: Option<SessionPreviews>,
    /// De quoi garder les données de tri de la séance ([`KeepData`]).
    keep: Option<KeepData>,
    /// Case « garder les données de tri » touchée ; `None` : cochée d'office sur un
    /// disque amovible (décidé le 11/11/2026).
    pub(crate) keep_choice: Option<bool>,
    /// `[files] group_pairs` : la case « Regrouper RAW et JPEG » cochée d'office pour
    /// `grouped`.
    group_pairs: tri::pairs::Grouping,
    /// Case « Regrouper RAW et JPEG » touchée (#392) ; `None` : le réglage.
    pub(crate) pairs_choice: Option<bool>,
    /// Galerie ouverte pour choisir les photos d'une séance (#507) : (rang de la séance,
    /// galerie) ; la boîte de déchargement revient à sa fermeture.
    pub(crate) picker: Option<(usize, crate::offload_picker::Picker)>,
}

impl OffloadDialog {
    /// Ouvre la boîte pour `source`, près de `at`, et lance l'inventaire en fond.
    pub fn new(
        source: PathBuf,
        at: Pos2,
        settings: OffloadSettings,
        data: Option<PathBuf>,
        ctx: &egui::Context,
    ) -> Self {
        let rx = plan_in_background(&source, &settings, data.as_deref(), ctx);
        OffloadDialog {
            source,
            at,
            settings,
            data,
            stage: Stage::Planning(rx),
            hidden: false,
            root_here: None,
            sources: Vec::new(),
            folder_here: None,
            previews: None,
            keep: None,
            keep_choice: None,
            group_pairs: tri::pairs::Grouping::Ask,
            pairs_choice: None,
            picker: None,
        }
    }

    /// « Décharger ici » (panneau des dossiers) : la boîte, racine `root` proposée, depuis
    /// le premier des supports `sources` (une liste permet d'en choisir un autre).
    pub fn here(
        root: &Path,
        sources: Vec<(PathBuf, String)>,
        at: Pos2,
        settings: OffloadSettings,
        data: Option<PathBuf>,
        ctx: &egui::Context,
    ) -> Option<Self> {
        let source = sources.first()?.0.clone();
        let mut dialog = OffloadDialog::new(source, at, settings, data, ctx);
        dialog.root_here = Some(root.display().to_string());
        dialog.sources = sources;
        Some(dialog)
    }

    /// Calcule les aperçus de la séance pendant la copie avec `previews` (voir
    /// [`SessionPreviews`]).
    pub fn set_previews(&mut self, previews: SessionPreviews) {
        self.previews = Some(previews);
    }

    /// `[files] group_pairs`, pour la case « Regrouper RAW et JPEG ».
    pub fn set_group_pairs(&mut self, setting: tri::pairs::Grouping) {
        self.group_pairs = setting;
    }

    /// Décision de regroupement à retenir pour la séance : la case touchée, sinon le
    /// réglage s'il n'est pas « demander » (alors la séance demandera à son ouverture).
    fn pairs_decision(&self) -> Option<bool> {
        self.pairs_choice.or(match self.group_pairs {
            tri::pairs::Grouping::Ask => None,
            setting => Some(setting.groups()),
        })
    }

    /// Propose de garder les données de tri de la séance avec `keep` ([`KeepData`]).
    pub fn set_keep(&mut self, keep: KeepData) {
        self.keep = Some(keep);
    }

    /// Nouvelle carte branchée alors que la boîte montre le bilan d'une autre (issue
    /// n° 37 : plusieurs cartes à la suite) : elle repart sur `source`, vers le même
    /// dossier de séance, avec les mêmes choix. Faux si la boîte n'en est pas là.
    /// La carte peut être montée au même chemin que la précédente (deux cartes du même
    /// boîtier, même nom de volume) : elle vient d'arriver, c'est une autre carte.
    pub fn next_card(&mut self, source: PathBuf, ctx: &egui::Context) -> bool {
        let Stage::Done { report, .. } = &self.stage else {
            return false;
        };
        if self.hidden {
            return false;
        }
        let destination = report.destination.clone();
        self.propose_session(&destination);
        self.change_source(source, ctx);
        true
    }

    /// Propose le dossier de séance `destination` (sa racine, son nom) plutôt qu'un
    /// nouveau : carte suivante d'un déchargement (issue n° 37).
    pub fn propose_session(&mut self, destination: &Path) {
        self.root_here = destination.parent().map(|p| p.display().to_string());
        self.folder_here = destination
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
    }

    /// Choix de la boîte (cases cochées), repris pour la carte suivante.
    pub fn settings(&self) -> &OffloadSettings {
        &self.settings
    }

    /// Change de support (liste de « décharger ici ») : nouvel inventaire, la racine
    /// saisie gardée.
    fn change_source(&mut self, source: PathBuf, ctx: &egui::Context) {
        if let Stage::Ready { root, .. } = &self.stage {
            self.root_here = Some(root.clone());
        }
        let rx = plan_in_background(&source, &self.settings, self.data.as_deref(), ctx);
        self.source = source;
        self.stage = Stage::Planning(rx);
    }

    /// Carte déchargée (tests).
    #[cfg(test)]
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Dossier de la séance du dernier déchargement fini.
    pub fn destination(&self) -> Option<PathBuf> {
        match &self.stage {
            Stage::Done { report, .. } => Some(report.destination.clone()),
            _ => None,
        }
    }

    /// Racine proposée par « décharger ici » (tests).
    #[cfg(test)]
    pub fn root_here(&self) -> Option<&str> {
        self.root_here.as_deref()
    }

    /// Vrai si la boîte est passée en arrière-plan.
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Passe en arrière-plan (tests : sans dépendre de la vitesse de la copie).
    #[cfg(test)]
    pub fn hide(&mut self) {
        self.hidden = true;
    }

    /// Réaffiche la boîte (clic sur la progression de la barre d'état).
    pub fn reveal(&mut self) {
        self.hidden = false;
    }

    /// Avancement, pour la barre d'état : (texte, fraction), pendant la copie.
    pub fn status(&self) -> Option<(String, f32)> {
        let Stage::Running { progress, .. } = &self.stage else {
            return None;
        };
        let fraction = if progress.bytes_total > 0 {
            progress.bytes_done as f32 / progress.bytes_total as f32
        } else {
            0.0
        };
        Some((
            t!(
                "offload-status",
                name = file_name(&self.source),
                done = progress.files_done,
                total = progress.files_total
            ),
            fraction,
        ))
    }

    /// En arrière-plan : suit la copie et, à la fin, enchaîne les suites cochées (sans
    /// ouvrir la séance : elle est seulement proposée).
    pub fn tick(&mut self) -> Option<Request> {
        self.poll();
        self.finish(false)
    }

    /// Fin de la copie, une seule fois : suppression de la carte, puis ouverture (boîte
    /// affichée) ou proposition d'ouverture (arrière-plan) de la séance, puis éjection.
    fn finish(&mut self, interactive: bool) -> Option<Request> {
        let Stage::Done {
            report,
            finished,
            deleted,
            opened,
            ejected,
        } = &mut self.stage
        else {
            return None;
        };
        if *finished {
            return None;
        }
        *finished = true;
        let settings = &self.settings;
        let complete = !report.cancelled && !report.out_of_space;
        if complete && settings.delete_after && !report.copied.is_empty() {
            let mut failed = offload::delete_sources(report);
            if settings.raw_only && settings.delete_left {
                failed.extend(offload::delete_left(report));
            }
            *deleted = Some(failed);
        }
        let session = (settings.open_after
            && (!report.copied.is_empty() || !report.skipped.is_empty()))
        .then(|| report.destination.clone());
        let (open, offer) = if interactive {
            (session, None)
        } else {
            (None, session)
        };
        let eject = (complete && settings.eject_after).then(|| self.source.clone());
        *opened = open.is_some();
        *ejected = eject.is_some();
        let mut summary = t!(
            "offload-summary",
            name = file_name(&self.source),
            copied = report.copied.len(),
            skipped = report.skipped.len(),
            errors = report.errors.len()
        );
        if report.cancelled {
            summary.push_str(&t!("offload-summary-cancelled"));
        }
        if report.out_of_space {
            summary.push_str(&t!("offload-summary-full"));
        }
        Some(Request::Finished {
            open,
            offer,
            eject,
            summary,
        })
    }

    /// Tests : une copie en arrière-plan qui vient de trouver la destination pleine
    /// ([`Message::Full`], comme `run` l'envoie) ; renvoie de quoi lire la réponse.
    #[cfg(test)]
    pub fn simulate_full(&mut self, stop: OutOfSpace) -> Receiver<bool> {
        let (tx, rx) = mpsc::channel();
        let (answer, answers) = mpsc::channel();
        let _ = tx.send(Message::Full(stop));
        self.stage = Stage::Running {
            rx,
            progress: Progress::default(),
            started: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
            full: None,
            answer,
        };
        self.hidden = true;
        answers
    }

    /// Vrai quand la copie attend la réponse à « destination pleine ».
    pub fn asks_full(&self) -> bool {
        matches!(self.stage, Stage::Running { full: Some(_), .. })
    }

    /// Vrai pendant la copie (la fermer l'interrompt).
    pub fn is_running(&self) -> bool {
        matches!(self.stage, Stage::Running { .. })
    }

    fn poll(&mut self) {
        match &mut self.stage {
            Stage::Planning(rx) => {
                if let Ok(result) = rx.try_recv() {
                    self.stage = match result {
                        Ok((plan, history)) => {
                            let folder = self.folder_here.clone().unwrap_or_else(|| {
                                offload::suggest_folder(&plan, &self.settings).unwrap_or_default()
                            });
                            // Carte suivante vers la même séance (issue n° 37) : pas de
                            // séances à choisir.
                            let sessions = match self.folder_here {
                                Some(_) => Vec::new(),
                                None => session_rows(&plan, &self.settings),
                            };
                            Stage::Ready {
                                sessions,
                                plan,
                                history: Some(history),
                                root: self.root_here.clone().unwrap_or_else(|| {
                                    offload::root(&self.settings, self.data.as_deref())
                                }),
                                root_field: Default::default(),
                                folder,
                                error: None,
                                free: None,
                                present: None,
                            }
                        }
                        Err(e) => Stage::Failed(e),
                    };
                }
            }
            Stage::Running {
                rx,
                progress,
                full,
                last_activity,
                ..
            } => {
                for message in rx.try_iter() {
                    match message {
                        Message::Progress(p) => {
                            if p != *progress {
                                *last_activity = std::time::Instant::now();
                            }
                            *progress = p;
                        }
                        // Destination pleine : la boîte revient (même en arrière-plan) et
                        // reste jusqu'à la réponse, sans limite de temps.
                        Message::Full(stop) => {
                            *full = Some(stop);
                            self.hidden = false;
                        }
                        Message::Done(report) => {
                            self.stage = Stage::Done {
                                report,
                                finished: false,
                                deleted: None,
                                opened: false,
                                ejected: false,
                            };
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let Stage::Ready {
            plan,
            history,
            root,
            folder,
            error,
            sessions,
            ..
        } = &mut self.stage
        else {
            return;
        };
        // Un dossier par séance cochée (#507), sinon la carte entière vers `folder`.
        let runs = match targets(plan, sessions, root, folder) {
            Ok(runs) if !runs.is_empty() => runs,
            Ok(_) => {
                *error = Some(t!("offload-no-session-chosen"));
                return;
            }
            Err(e) => {
                *error = Some(e);
                return;
            }
        };
        // Racine retenue pour les prochains déchargements (pas celle d'un « décharger
        // ici », choisie pour cette fois).
        if let Some(data) = self.data.as_ref().filter(|_| self.root_here.is_none()) {
            let _ = offload::remember_root(data, root);
        }
        let (tx, rx) = mpsc::channel();
        let (answer, answers) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut history = history.take().unwrap_or_default();
        let (settings, stop, repaint) = (self.settings.clone(), cancel.clone(), ctx.clone());
        let previews = self.previews.take().filter(|_| settings.previews);
        // « Garder les données de tri » : posé avant la copie, le cache se remplit
        // directement à sa place.
        let keep_choice = self.keep_choice;
        let keeps: Vec<Option<PathBuf>> = runs
            .iter()
            .map(|(_, destination)| {
                keep_root(self.keep.as_ref(), destination)
                    .filter(|(_, disk)| keep_choice.unwrap_or(*disk))
                    .map(|(root, _)| root)
            })
            .collect();
        let keep_cache = self.keep.as_ref().map(|k| k.cache.clone());
        let location_too = self.keep_choice != Some(false);
        // Paires de la séance regroupées ou non (#392), retenu une fois la copie faite
        // (le dossier existe, ses données de tri à leur place).
        let decision = self.pairs_decision();
        let data = self.data.clone();
        std::thread::spawn(move || {
            let mut all: Option<Report> = None;
            for ((plan, destination), keep_at) in runs.into_iter().zip(keeps) {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let pairs = decision.filter(|_| plan.pairs(&settings) > 0);
                let progress_tx = tx.clone();
                let progress_ctx = repaint.clone();
                let (full_tx, full_ctx) = (tx.clone(), repaint.clone());
                if let (Some(root), Some(cache)) = (&keep_at, &keep_cache) {
                    // Un échec (disque en lecture seule) laisse tout sur la machine.
                    let _ = std::fs::create_dir_all(&destination)
                        .and_then(|()| cache.use_disk(root).map(|_| ()));
                }
                if let Some(p) = previews.as_ref().filter(|_| location_too) {
                    session_cache(p, &destination);
                    p.scheduler.set_folder(Vec::new());
                }
                let report = offload::run(
                    &plan,
                    &destination,
                    &settings,
                    &mut history,
                    &stop,
                    |p| {
                        let _ = progress_tx.send(Message::Progress(p.clone()));
                        progress_ctx.request_repaint();
                    },
                    |copy| {
                        if let (Some(p), Some(kind)) = (&previews, tri::media::kind_of(copy)) {
                            p.scheduler.add_files(vec![tri::media::MediaFile {
                                path: copy.to_path_buf(),
                                kind,
                            }]);
                        }
                    },
                    // Destination pleine : on attend la réponse de l'utilisateur (Interrompre
                    // ou la boîte fermée valent abandon).
                    |full| {
                        let _ = full_tx.send(Message::Full(full.clone()));
                        full_ctx.request_repaint();
                        loop {
                            match answers.recv_timeout(std::time::Duration::from_millis(100)) {
                                Ok(retry) => return retry,
                                Err(mpsc::RecvTimeoutError::Disconnected) => return false,
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    if stop.load(Ordering::Relaxed) {
                                        return false;
                                    }
                                }
                            }
                        }
                    },
                );
                if let Some(grouped) = pairs
                    && destination.is_dir()
                {
                    let _ = tri::pairs::decide(data.as_deref(), &destination, Some(grouped));
                }
                let stopped = report.cancelled || report.out_of_space;
                match &mut all {
                    Some(all) => all.absorb(report),
                    None => all = Some(report),
                }
                if stopped {
                    break;
                }
            }
            let _ = tx.send(Message::Done(all.unwrap_or_default()));
            repaint.request_repaint();
        });
        self.stage = Stage::Running {
            rx,
            progress: Progress::default(),
            started: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            cancel,
            full: None,
            answer,
        };
    }

    /// Ouvre la galerie de la séance `i` (« Choisir les photos… », #507).
    pub fn pick_session(&mut self, i: usize, ctx: &egui::Context) {
        if let Stage::Ready { plan, sessions, .. } = &self.stage
            && let Some(row) = sessions.get(i)
        {
            let items: Vec<&offload::Item> =
                row.session.items.iter().map(|&k| &plan.items[k]).collect();
            let picker = crate::offload_picker::Picker::new(
                &row.session.day,
                &items,
                row.picked.as_ref(),
                self.settings.skip_known,
                ctx,
            );
            self.picker = Some((i, picker));
        }
    }

    /// Dessine la boîte (modale, près du clic) ; renvoie ce que l'utilisateur a demandé.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Request> {
        self.poll();
        // Galerie d'une séance ouverte : elle seule, la boîte revient ensuite.
        if let Some((row, picker)) = &mut self.picker {
            let outcome = picker.show(ctx, crate::marks::accent_mark(ctx));
            let row = *row;
            match outcome {
                crate::offload_picker::Outcome::Open => return None,
                crate::offload_picker::Outcome::Done(files, exclude_known) => {
                    // La case de la galerie est celle de la boîte (« Recopier »).
                    self.settings.skip_known = exclude_known;
                    if let Stage::Ready { sessions, .. } = &mut self.stage
                        && let Some(r) = sessions.get_mut(row)
                    {
                        let all = r.session.items.len() == files.len();
                        r.picked = (!all).then_some(files);
                        r.on = true;
                    }
                    self.picker = None;
                }
                crate::offload_picker::Outcome::Cancelled => self.picker = None,
            }
        }
        let mut pick: Option<usize> = None;
        // Fin de la copie sous les yeux de l'utilisateur : suites cochées, séance ouverte.
        let mut request = self.finish(true);
        let mut start = false;
        let mut background = false;
        let mut new_source = None;
        let source = self.source.clone();
        let name = source.file_name().map_or_else(
            || source.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let title = t!("offload-title", name = &name);
        let modal = crate::app::modal_at("offload_dialog", self.at)
            .title(title)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                // « Décharger ici » avec plusieurs supports : lequel décharger.
                let choosing = matches!(
                    self.stage,
                    Stage::Planning(_) | Stage::Ready { .. } | Stage::Failed(_)
                );
                if self.sources.len() > 1 && choosing {
                    ui.horizontal(|ui| {
                        ui.label(t!("offload-source"));
                        egui::ComboBox::from_id_salt("offload_source")
                            .selected_text(name.clone())
                            .show_ui(ui, |ui| {
                                for (path, label) in &self.sources {
                                    let current = *path == source;
                                    if ui.selectable_label(current, label).clicked() && !current {
                                        new_source = Some(path.clone());
                                    }
                                }
                            });
                    });
                }
                match &mut self.stage {
                    Stage::Planning(_) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t!("offload-planning"));
                        });
                    }
                    Stage::Failed(e) => {
                        ui.label(e.as_str());
                    }
                    Stage::Ready {
                        plan,
                        root,
                        root_field,
                        folder,
                        error,
                        free,
                        present,
                        sessions,
                        ..
                    } => {
                        let settings = &mut self.settings;
                        // Comptes des séances selon les choix du moment (RAW seulement, déjà
                        // déchargées).
                        for row in sessions.iter_mut() {
                            let copy = row
                                .session
                                .items
                                .iter()
                                .map(|&i| &plan.items[i])
                                .filter(|i| row.keeps(i))
                                .filter(|i| !(settings.skip_known && i.known))
                                .filter(|i| {
                                    !settings.raw_only || i.kind == tri::media::MediaKind::Raw
                                });
                            (row.count, row.bytes) =
                                copy.fold((0, 0), |(n, b), i| (n + 1, b + i.size));
                        }
                        let chosen = targets(plan, sessions, root, folder).ok();
                        let dests: Vec<PathBuf> =
                            chosen.iter().flatten().map(|(_, d)| d.clone()).collect();
                        let dest = dests.first().cloned();
                        // Plusieurs séances : seulement les cochées comptent.
                        let merged;
                        let plan: &Plan = if sessions.is_empty() {
                            plan
                        } else {
                            let on: std::collections::HashSet<&Path> = sessions
                                .iter()
                                .filter(|r| r.on)
                                .flat_map(|r| {
                                    r.session
                                        .items
                                        .iter()
                                        .map(|&i| &plan.items[i])
                                        .filter(|i| r.keeps(i))
                                })
                                .map(|i| i.source.as_path())
                                .collect();
                            merged = plan.subset(|i| on.contains(i.source.as_path()));
                            &merged
                        };
                        // Doublons jugés sur la destination choisie (même nom, même taille ;
                        // la copie compare ensuite les contenus).
                        let choices = (settings.raw_only, settings.skip_known);
                        let fresh = match &*present {
                            Some((p, made, _)) => *p != dests || *made != choices,
                            None => !dests.is_empty(),
                        };
                        if fresh && let Some(chosen) = &chosen {
                            let found = chosen.iter().fold((0, 0), |(n, b), (p, d)| {
                                let (m, c) = offload::already_in(p, settings, d);
                                (n + m, b + c)
                            });
                            *present = Some((dests.clone(), choices, found));
                        }
                        let (there, there_bytes) = match &*present {
                            Some((p, _, found)) if *p == dests => *found,
                            _ => (0, 0),
                        };
                        let selected = plan.to_copy(settings).count();
                        let count = selected - there;
                        // Ce que l'historique connaît déjà, et ce qui est nouveau (retour du
                        // 12/11/2026).
                        let known = plan.items.iter().filter(|i| i.known).count();
                        let mut text = if known > 0 {
                            t!(
                                "offload-images-known",
                                n = plan.items.len(),
                                known = known,
                                new = plan.items.len() - known
                            )
                        } else {
                            t!("offload-images", n = plan.items.len())
                        };
                        if there > 0 {
                            text.push_str(&t!("offload-already-there", n = there));
                        }
                        text.push_str(&t!(
                            "offload-to-copy",
                            n = count,
                            mb = megabytes(plan.bytes(settings) - there_bytes)
                        ));
                        ui.label(text);
                        if plan.ignored > 0 {
                            ui.weak(t!("offload-ignored", n = plan.ignored));
                        }
                        ui.add_space(6.0);
                        egui::Grid::new("offload_fields")
                            .num_columns(2)
                            .show(ui, |ui| {
                                ui.label(t!("offload-root"));
                                ui.vertical(|ui| {
                                    root_field.show(ui, root_field_id(), root, 300.0);
                                });
                                ui.end_row();
                                if sessions.len() <= 1 {
                                    ui.label(t!("offload-session"));
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(folder)
                                                .id(folder_field_id())
                                                .desired_width(300.0),
                                        )
                                        .on_hover_text(t!("offload-session-tip"));
                                        // Sa galerie, comme pour chaque séance d'une carte
                                        // de plusieurs jours.
                                        if let Some(row) = sessions.first() {
                                            let label = match &row.picked {
                                                Some(_) => t!("offload-pick-again"),
                                                None => t!("offload-pick"),
                                            };
                                            if ui
                                                .button(label)
                                                .on_hover_text(t!("offload-pick-tip"))
                                                .clicked()
                                            {
                                                pick = Some(0);
                                            }
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                        // Carte de plusieurs séances (#507) : une ligne par jour, cochée
                        // ou non, chacune dans son dossier.
                        if sessions.len() > 1 {
                            ui.add_space(4.0);
                            ui.label(t!("offload-sessions", n = sessions.len()));
                            egui::Grid::new("offload_sessions")
                                .num_columns(4)
                                .show(ui, |ui| {
                                    for (i, row) in sessions.iter_mut().enumerate() {
                                        ui.checkbox(&mut row.on, row.session.day.as_str());
                                        // Largeur imposée : dans la grille, le champ se
                                        // réduisait à quelques caractères.
                                        let h = ui.spacing().interact_size.y;
                                        ui.add_enabled_ui(row.on, |ui| {
                                            ui.add_sized(
                                                [220.0, h],
                                                egui::TextEdit::singleline(&mut row.folder).id(
                                                    egui::Id::unique(("offload_session_folder", i)),
                                                ),
                                            )
                                            .on_hover_text(t!("offload-session-tip"));
                                        });
                                        let text = if row.known && row.picked.is_none() {
                                            t!("offload-session-known", n = row.session.items.len())
                                        } else {
                                            t!(
                                                "offload-session-count",
                                                n = row.count,
                                                size = size_text(row.bytes)
                                            )
                                        };
                                        ui.weak(text);
                                        let label = match &row.picked {
                                            Some(_) => t!("offload-pick-again"),
                                            None => t!("offload-pick"),
                                        };
                                        if ui
                                            .button(label)
                                            .on_hover_text(t!("offload-pick-tip"))
                                            .clicked()
                                        {
                                            pick = Some(i);
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                        // Pas assez de place : le bouton le dit (« Décharger quand même »).
                        let mut short = false;
                        if let Some(dest) = &dest {
                            if dests.len() > 1 {
                                let root = dest.parent().unwrap_or(dest);
                                ui.weak(t!(
                                    "offload-destinations",
                                    n = dests.len(),
                                    path = root.display().to_string()
                                ));
                            } else {
                                ui.weak(t!(
                                    "offload-destination",
                                    path = dest.display().to_string()
                                ));
                            }
                            // Pas assez de place : dit avant la copie (elle peut encore
                            // passer, des images étant peut-être déjà là, ou de la place
                            // faite entre-temps).
                            if free.as_ref().is_none_or(|(d, _)| d != dest) {
                                *free = Some((dest.clone(), offload::free_space(dest).ok()));
                            }
                            let needed = plan.bytes(settings) - there_bytes;
                            if let Some((_, Some(room))) = free
                                && needed > *room
                            {
                                short = true;
                                // Court, en Go (retour du 18/11/2026 : long et confus) ;
                                // la suite est dans la bulle du bouton.
                                ui.colored_label(
                                    ui.visuals().warn_fg_color,
                                    t!(
                                        "offload-not-enough-room",
                                        size = size_text(needed),
                                        free = size_text(*room)
                                    ),
                                );
                            }
                        }
                        let keep_here = dest
                            .as_deref()
                            .and_then(|d| keep_root(self.keep.as_ref(), d));
                        if let Some(e) = error {
                            ui.colored_label(ui.visuals().error_fg_color, e.as_str());
                        }
                        // Choix faits avant la copie, enchaînés à la fin (retour du
                        // 17/10/2026 : ouvrir *et* éjecter, pas l'un ou l'autre).
                        ui.add_space(6.0);
                        // Le nom proposé suit ces choix (la plus récente image à copier),
                        // tant qu'on ne l'a pas changé.
                        let before = settings.clone();
                        // « RAW seulement » et « Regrouper RAW et JPEG » s'excluent (#469) :
                        // les deux restent à leur place (la mémoire des gestes), l'une grisée
                        // quand l'autre est cochée.
                        let pairs = plan.pairs(&OffloadSettings {
                            raw_only: false,
                            ..settings.clone()
                        });
                        let grouping =
                            pairs > 0 && self.pairs_choice.unwrap_or(self.group_pairs.groups());
                        let raw = ui
                            .add_enabled(
                                !grouping,
                                egui::Checkbox::new(&mut settings.raw_only, t!("offload-raw-only")),
                            )
                            .on_hover_text(t!("offload-raw-only-tip"))
                            .on_disabled_hover_text(t!("offload-raw-only-grouped"))
                            .changed();
                        // Ce qui a déjà été déchargé, n'importe où (historique global ;
                        // retour du 12/11/2026) n'est pas recopié ; cochée, si (une même
                        // carte déchargée à deux endroits). Seulement s'il y en a.
                        let mut again = !settings.skip_known;
                        let skip = known > 0
                            && ui
                                .checkbox(&mut again, t!("offload-again", n = known))
                                .on_hover_text(t!("offload-again-tip"))
                                .changed();
                        if skip {
                            settings.skip_known = !again;
                        }
                        if (raw || skip)
                            && offload::suggest_folder(plan, &before).as_deref()
                                == Some(folder.as_str())
                        {
                            *folder = offload::suggest_folder(plan, settings).unwrap_or_default();
                        }
                        if let Some((_, disk)) = keep_here {
                            let mut on = self.keep_choice.unwrap_or(disk);
                            let label = if disk {
                                t!("offload-keep-disk")
                            } else {
                                t!("offload-keep-folder")
                            };
                            if ui
                                .checkbox(&mut on, label)
                                .on_hover_text(t!("folder-menu-cache-tip"))
                                .changed()
                            {
                                self.keep_choice = Some(on);
                            }
                        }
                        // Paires RAW+JPEG sur la carte : les regrouper dans la séance (#392) ;
                        // grisée avec « RAW seulement » (pas de JPEG copié, #469).
                        if pairs > 0 {
                            let mut on = !settings.raw_only
                                && self.pairs_choice.unwrap_or(self.group_pairs.groups());
                            if ui
                                .add_enabled(
                                    !settings.raw_only,
                                    egui::Checkbox::new(
                                        &mut on,
                                        t!("offload-group-pairs", n = pairs),
                                    ),
                                )
                                .on_hover_text(t!("offload-group-pairs-tip"))
                                .on_disabled_hover_text(t!("offload-group-pairs-raw-only"))
                                .changed()
                            {
                                self.pairs_choice = Some(on);
                            }
                        }
                        ui.checkbox(&mut settings.open_after, t!("offload-open-after"));
                        if self.previews.is_some() {
                            ui.checkbox(&mut settings.previews, t!("offload-previews"))
                                .on_hover_text(t!("offload-previews-tip"));
                        }
                        ui.checkbox(&mut settings.eject_after, t!("offload-eject-after"));
                        ui.checkbox(&mut settings.delete_after, t!("offload-delete-after"))
                            .on_hover_text(t!("offload-delete-after-tip"));
                        // RAW seulement + suppression : les JPEG laissés aussi, sur demande.
                        if settings.raw_only && settings.delete_after {
                            ui.indent("delete_left", |ui| {
                                ui.checkbox(&mut settings.delete_left, t!("offload-delete-left"))
                                    .on_hover_text(t!("offload-delete-left-tip"));
                            });
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let label = if short {
                                t!("offload-start-anyway")
                            } else {
                                t!("offload-start")
                            };
                            let button = ui.add_enabled(count > 0, egui::Button::new(label));
                            let button = if short {
                                button.on_hover_text(t!("offload-start-anyway-tip"))
                            } else {
                                button
                            };
                            start = button.clicked();
                            if ui.button(t!("cancel")).clicked() {
                                request = Some(Request::Close);
                            }
                        });
                    }
                    Stage::Running {
                        full: Some(stop),
                        answer,
                        ..
                    } => {
                        // Destination pleine (12/11/2026 : l'échec passait inaperçu) :
                        // ce qui est fait, ce qui reste, et le choix, sans limite de temps.
                        ui.colored_label(ui.visuals().error_fg_color, t!("offload-full-title"));
                        ui.label(t!(
                            "offload-full-status",
                            done = stop.copied,
                            n = stop.left,
                            size = size_text(stop.needed),
                            free = stop.free.map_or_else(|| "?".to_string(), size_text)
                        ));
                        ui.weak(t!("offload-full-what"));
                        ui.add_space(6.0);
                        let mut reply = None;
                        ui.horizontal(|ui| {
                            if ui.button(t!("offload-full-retry")).clicked() {
                                reply = Some(true);
                            }
                            if ui.button(t!("offload-full-give-up")).clicked() {
                                reply = Some(false);
                            }
                        });
                        if let Some(retry) = reply {
                            let _ = answer.send(retry);
                            if let Stage::Running { full, .. } = &mut self.stage {
                                *full = None;
                            }
                        }
                    }
                    Stage::Running {
                        progress,
                        cancel,
                        started,
                        last_activity,
                        ..
                    } => {
                        let fraction = if progress.bytes_total > 0 {
                            progress.bytes_done as f32 / progress.bytes_total as f32
                        } else {
                            0.0
                        };
                        ui.add(egui::ProgressBar::new(fraction).text(t!(
                            "offload-progress",
                            done = progress.files_done,
                            total = progress.files_total,
                            mb = megabytes(progress.bytes_done),
                            total_mb = megabytes(progress.bytes_total)
                        )));
                        // Ce qui se fait, le débit, et depuis quand rien ne bouge (#470) :
                        // lent ou bloqué, et où.
                        let elapsed = started.elapsed().as_secs_f64();
                        let rate = if elapsed > 0.5 {
                            progress.bytes_done as f64 / 1_000_000.0 / elapsed
                        } else {
                            0.0
                        };
                        let file = progress
                            .current
                            .as_deref()
                            .map(file_name)
                            .unwrap_or_default();
                        ui.weak(t!(
                            "offload-activity",
                            phase = phase_name(progress.phase),
                            file = file,
                            rate = format!("{rate:.1}")
                        ));
                        let idle = last_activity.elapsed().as_secs();
                        if idle >= ACTIVITY_SILENCE_S {
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                t!("offload-idle", s = idle),
                            );
                        }
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_secs(1));
                        ui.horizontal(|ui| {
                            if ui.button(t!("offload-background")).clicked() {
                                background = true;
                            }
                            if ui.button(t!("offload-interrupt")).clicked() {
                                cancel.store(true, Ordering::Relaxed);
                            }
                        });
                    }
                    Stage::Done {
                        report,
                        finished,
                        deleted,
                        opened,
                        ejected,
                    } => {
                        let settings = &self.settings;
                        let _ = finished;
                        let mut text = t!(
                            "offload-done",
                            copied = report.copied.len(),
                            verified = if settings.verify { "yes" } else { "no" },
                            skipped = report.skipped.len(),
                            errors = report.errors.len()
                        );
                        if report.cancelled {
                            text.push_str(&t!("offload-done-cancelled"));
                        }
                        ui.label(text);
                        if report.out_of_space {
                            ui.colored_label(ui.visuals().error_fg_color, t!("offload-done-full"));
                        }
                        if !report.left.is_empty() {
                            ui.weak(t!("offload-left", n = report.left.len()));
                        }
                        ui.weak(t!(
                            "offload-destination",
                            path = report.destination.display().to_string()
                        ));
                        let shown = crate::app::dialog_settings(ui.ctx()).max_errors;
                        for (file, e) in report.errors.iter().take(shown) {
                            ui.colored_label(
                                ui.visuals().error_fg_color,
                                format!("{} : {e}", file_name(file)),
                            );
                        }
                        if let Some(errors) = deleted {
                            ui.label(if errors.is_empty() {
                                t!("offload-deleted")
                            } else {
                                t!("offload-not-deleted", n = errors.len())
                            });
                        }
                        if *ejected {
                            ui.weak(t!("offload-ejecting"));
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            // Ce qui n'a pas été coché reste possible ici.
                            if !*opened && ui.button(t!("offload-open")).clicked() {
                                request = Some(Request::Open(report.destination.clone()));
                            }
                            if !*ejected && ui.button(t!("offload-eject")).clicked() {
                                request = Some(Request::Eject(source.clone()));
                            }
                        });
                    }
                }
            });
        if let Some(source) = new_source {
            self.change_source(source, ctx);
        }
        // « Choisir les photos… » d'une séance : sa galerie, à la place de la boîte.
        if let Some(i) = pick {
            self.pick_session(i, ctx);
            return request;
        }
        if start {
            self.start(ctx);
        }
        // Échap ou clic à côté : fermer ; pendant la copie, passer en arrière-plan ; pas
        // tant que la question « destination pleine » attend sa réponse.
        let asking = self.asks_full();
        if !asking && (background || (modal.should_close() && self.is_running())) {
            self.hidden = true;
        } else if !asking && request.is_none() && modal.should_close() {
            request = Some(Request::Close);
        }
        request
    }
}

/// Identifiant du champ « dossier de la séance » (tests : lui donner le focus).
pub fn folder_field_id() -> egui::Id {
    egui::Id::unique("offload_folder_field")
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Mégaoctets, une décimale, à la façon de la langue (« 109,4 » en français).
/// Silence (secondes sans avancement) au-delà duquel la boîte le signale (#470).
const ACTIVITY_SILENCE_S: u64 = 3;

/// Nom d'une étape du déchargement.
fn phase_name(phase: offload::Phase) -> String {
    t!(match phase {
        offload::Phase::Preparing => "offload-phase-preparing",
        offload::Phase::Writing => "offload-phase-writing",
        offload::Phase::Syncing => "offload-phase-syncing",
        offload::Phase::Verifying => "offload-phase-verifying",
    })
}

fn megabytes(bytes: u64) -> String {
    tri::i18n::decimal(bytes as f64 / 1e6, 1)
}

/// Identifiant du champ de la racine du déchargement.
fn root_field_id() -> egui::Id {
    egui::Id::unique("offload_root_field")
}

/// Inventaire de la carte `source` dans un fil à part ; le résultat arrive sur le canal.
fn plan_in_background(
    source: &Path,
    settings: &OffloadSettings,
    data: Option<&Path>,
    ctx: &egui::Context,
) -> Receiver<Result<(Plan, History), String>> {
    let (tx, rx) = mpsc::channel();
    let (dir, s, repaint) = (source.to_path_buf(), settings.clone(), ctx.clone());
    let data_dir = data.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let result = History::open_configured(&s, data_dir.as_deref())
            .map_err(|e| t!("offload-history-unreadable", error = e.to_string()))
            .and_then(|history| {
                offload::plan(&dir, &history)
                    .map(|plan| (plan, history))
                    .map_err(|e| e.to_string())
            });
        let _ = tx.send(result);
        repaint.request_repaint();
    });
    rx
}

/// `[cache] location` appliqué au dossier de la séance avant la copie : son cache se
/// remplit directement à sa place (sans déplacement ensuite). Un échec (disque en lecture
/// seule) laisse le cache chez l'utilisateur.
fn session_cache(previews: &SessionPreviews, destination: &Path) {
    let removable = match previews.location {
        tri::cache::Location::Disk => tri::places::removable_roots(&previews.places),
        _ => Vec::new(),
    };
    let _ = previews
        .cache
        .apply(previews.location, destination, &removable);
}

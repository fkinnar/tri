//! Boîte de déchargement d'une carte : l'interface du module moteur [`tri::offload`].
//! Inventaire et copie tournent dans des fils à part ; la boîte ne fait qu'afficher et
//! transmettre les choix.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Pos2};
use tri::offload::{self, History, OffloadSettings, Plan, Progress, Report};

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
    Done(Report),
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
        /// Déjà présentes dans la destination : (destination, RAW seulement, (nombre,
        /// octets)), recalculé seulement quand la destination ou le choix RAW changent.
        present: Option<(PathBuf, bool, (usize, u64))>,
    },
    /// Copie en cours.
    Running {
        rx: Receiver<Message>,
        progress: Progress,
        cancel: Arc<AtomicBool>,
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
        let complete = !report.cancelled;
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
        Some(Request::Finished {
            open,
            offer,
            eject,
            summary,
        })
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
                            let folder =
                                offload::suggest_folder(&plan, &self.settings).unwrap_or_default();
                            Stage::Ready {
                                plan,
                                history: Some(history),
                                root: self.root_here.clone().unwrap_or_else(|| {
                                    offload::root(&self.settings, self.data.as_deref())
                                }),
                                root_field: Default::default(),
                                folder,
                                error: None,
                                present: None,
                            }
                        }
                        Err(e) => Stage::Failed(e),
                    };
                }
            }
            Stage::Running { rx, progress, .. } => {
                for message in rx.try_iter() {
                    match message {
                        Message::Progress(p) => *progress = p,
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
            ..
        } = &mut self.stage
        else {
            return;
        };
        let destination =
            match offload::destination(&tri::settings::expand_home(root.trim()), folder) {
                Ok(d) => d,
                Err(e) => {
                    *error = Some(e.to_string());
                    return;
                }
            };
        // Racine retenue pour les prochains déchargements (pas celle d'un « décharger
        // ici », choisie pour cette fois).
        if let Some(data) = self.data.as_ref().filter(|_| self.root_here.is_none()) {
            let _ = offload::remember_root(data, root);
        }
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (plan, mut history) = (plan.clone(), history.take().unwrap_or_default());
        let (settings, stop, repaint) = (self.settings.clone(), cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let progress_tx = tx.clone();
            let progress_ctx = repaint.clone();
            let report = offload::run(&plan, &destination, &settings, &mut history, &stop, |p| {
                let _ = progress_tx.send(Message::Progress(p.clone()));
                progress_ctx.request_repaint();
            });
            let _ = tx.send(Message::Done(report));
            repaint.request_repaint();
        });
        self.stage = Stage::Running {
            rx,
            progress: Progress::default(),
            cancel,
        };
    }

    /// Dessine la boîte (modale, près du clic) ; renvoie ce que l'utilisateur a demandé.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Request> {
        self.poll();
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
                        present,
                        ..
                    } => {
                        let settings = &mut self.settings;
                        let dest =
                            offload::destination(&tri::settings::expand_home(root.trim()), folder)
                                .ok();
                        // Doublons jugés sur la destination choisie (même nom, même taille ;
                        // la copie compare ensuite les contenus).
                        let fresh = match (&dest, &*present) {
                            (Some(d), Some((p, raw, _))) => p != d || *raw != settings.raw_only,
                            (Some(_), None) => true,
                            (None, _) => false,
                        };
                        if fresh && let Some(d) = &dest {
                            let found = offload::already_in(plan, settings, d);
                            *present = Some((d.clone(), settings.raw_only, found));
                        }
                        let (there, there_bytes) = match (&dest, &*present) {
                            (Some(d), Some((p, _, found))) if p == d => *found,
                            _ => (0, 0),
                        };
                        let selected = plan.to_copy(settings).count();
                        let count = selected - there;
                        let mut text = t!("offload-images", n = plan.items.len());
                        if settings.skip_known {
                            let known = plan.items.iter().filter(|i| i.known).count();
                            text.push_str(&t!("offload-known", n = known));
                        }
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
                                ui.label(t!("offload-session"));
                                ui.add(
                                    egui::TextEdit::singleline(folder)
                                        .id(folder_field_id())
                                        .desired_width(300.0),
                                )
                                .on_hover_text(t!("offload-session-tip"));
                                ui.end_row();
                            });
                        if let Some(dest) = &dest {
                            ui.weak(t!("offload-destination", path = dest.display().to_string()));
                        }
                        if let Some(e) = error {
                            ui.colored_label(egui::Color32::from_rgb(230, 90, 90), e.as_str());
                        }
                        // Choix faits avant la copie, enchaînés à la fin (retour du
                        // 17/10/2026 : ouvrir *et* éjecter, pas l'un ou l'autre).
                        ui.add_space(6.0);
                        ui.checkbox(&mut settings.raw_only, t!("offload-raw-only"))
                            .on_hover_text(t!("offload-raw-only-tip"));
                        ui.checkbox(&mut settings.open_after, t!("offload-open-after"));
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
                            start = ui
                                .add_enabled(count > 0, egui::Button::new(t!("offload-start")))
                                .clicked();
                            if ui.button(t!("cancel")).clicked() {
                                request = Some(Request::Close);
                            }
                        });
                    }
                    Stage::Running {
                        progress, cancel, ..
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
                        if let Some(current) = &progress.current {
                            ui.weak(file_name(current));
                        }
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
                        if !report.left.is_empty() {
                            ui.weak(t!("offload-left", n = report.left.len()));
                        }
                        ui.weak(t!(
                            "offload-destination",
                            path = report.destination.display().to_string()
                        ));
                        for (file, e) in report.errors.iter().take(5) {
                            ui.colored_label(
                                egui::Color32::from_rgb(230, 90, 90),
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
        if start {
            self.start(ctx);
        }
        // Échap ou clic à côté : fermer ; pendant la copie, passer en arrière-plan.
        if background || (modal.should_close() && self.is_running()) {
            self.hidden = true;
        } else if request.is_none() && modal.should_close() {
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

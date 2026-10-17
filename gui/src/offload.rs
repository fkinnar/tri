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
    /// Ouvrir ce dossier dans la grille (fin du déchargement).
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
        folder: String,
        error: Option<String>,
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
        confirm_delete: bool,
        deleted: Option<Vec<(PathBuf, String)>>,
    },
    /// Erreur d'inventaire.
    Failed(String),
}

/// Boîte de déchargement de la carte `source`.
pub struct OffloadDialog {
    source: PathBuf,
    at: Pos2,
    settings: OffloadSettings,
    stage: Stage,
}

impl OffloadDialog {
    /// Ouvre la boîte pour `source`, près de `at`, et lance l'inventaire en fond.
    pub fn new(source: PathBuf, at: Pos2, settings: OffloadSettings, ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let (dir, s, repaint) = (source.clone(), settings.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = History::open_configured(&s)
                .map_err(|e| format!("historique illisible : {e}"))
                .and_then(|history| {
                    offload::plan(&dir, &history)
                        .map(|plan| (plan, history))
                        .map_err(|e| e.to_string())
                });
            let _ = tx.send(result);
            repaint.request_repaint();
        });
        OffloadDialog {
            source,
            at,
            settings,
            stage: Stage::Planning(rx),
        }
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
                            let folder = offload::suggest_folder(&plan, self.settings.skip_known)
                                .unwrap_or_default();
                            Stage::Ready {
                                plan,
                                history: Some(history),
                                root: self.settings.destination.clone(),
                                folder,
                                error: None,
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
                                confirm_delete: false,
                                deleted: None,
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
        let mut request = None;
        let mut start = false;
        let source = self.source.clone();
        let name = source.file_name().map_or_else(
            || source.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let modal =
            crate::app::modal_at("offload_dialog", self.at).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.strong(format!("Décharger « {name} »"));
                ui.add_space(6.0);
                match &mut self.stage {
                    Stage::Planning(_) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("inventaire de la carte…");
                        });
                    }
                    Stage::Failed(e) => {
                        ui.label(e.as_str());
                    }
                    Stage::Ready {
                        plan,
                        root,
                        folder,
                        error,
                        ..
                    } => {
                        let skip = self.settings.skip_known;
                        let known = plan.items.iter().filter(|i| i.known).count();
                        let count = plan.to_copy(skip).count();
                        ui.label(format!(
                            "{} images, dont {known} déjà déchargées ; {count} à copier ({} Mo).",
                            plan.items.len(),
                            french_mb(plan.bytes(skip))
                        ));
                        if plan.ignored > 0 {
                            ui.weak(format!(
                            "{} autres fichiers (vidéos, fichiers du boîtier) ne sont pas copiés.",
                            plan.ignored
                        ));
                        }
                        ui.add_space(6.0);
                        egui::Grid::new("offload_fields")
                            .num_columns(2)
                            .show(ui, |ui| {
                                ui.label("Dossier racine");
                                ui.add(egui::TextEdit::singleline(root).desired_width(300.0));
                                ui.end_row();
                                ui.label("Dossier de la séance");
                                ui.add(egui::TextEdit::singleline(folder).desired_width(300.0))
                            .on_hover_text(
                                "Proposé : AAAA/AAAA-MM-JJ_ (date de la plus ancienne image) ; \
                                 à compléter (« _Self ») ou à remplacer entièrement.",
                            );
                                ui.end_row();
                            });
                        if let Ok(dest) =
                            offload::destination(&tri::settings::expand_home(root.trim()), folder)
                        {
                            ui.weak(format!("Destination : {}", dest.display()));
                        }
                        if let Some(e) = error {
                            ui.colored_label(egui::Color32::from_rgb(230, 90, 90), e.as_str());
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            start = ui
                                .add_enabled(count > 0, egui::Button::new("Décharger"))
                                .clicked();
                            if ui.button("Annuler").clicked() {
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
                        ui.add(egui::ProgressBar::new(fraction).text(format!(
                            "{} / {} fichiers · {} / {} Mo",
                            progress.files_done,
                            progress.files_total,
                            french_mb(progress.bytes_done),
                            french_mb(progress.bytes_total)
                        )));
                        if let Some(current) = &progress.current {
                            ui.weak(file_name(current));
                        }
                        if ui.button("Interrompre").clicked() {
                            cancel.store(true, Ordering::Relaxed);
                        }
                    }
                    Stage::Done {
                        report,
                        confirm_delete,
                        deleted,
                    } => {
                        let verified = if self.settings.verify {
                            " et vérifiés"
                        } else {
                            ""
                        };
                        ui.label(format!(
                        "{} fichiers copiés{verified}, {} ignorés (déjà déchargés), {} erreurs{}.",
                        report.copied.len(),
                        report.skipped.len(),
                        report.errors.len(),
                        if report.cancelled { " — interrompu" } else { "" }
                    ));
                        ui.weak(format!("Destination : {}", report.destination.display()));
                        for (file, e) in report.errors.iter().take(5) {
                            ui.colored_label(
                                egui::Color32::from_rgb(230, 90, 90),
                                format!("{} : {e}", file_name(file)),
                            );
                        }
                        if let Some(errors) = deleted {
                            ui.label(if errors.is_empty() {
                                "Fichiers copiés supprimés de la carte.".to_string()
                            } else {
                                format!("{} fichiers n'ont pas pu être supprimés.", errors.len())
                            });
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("Ouvrir le dossier").clicked() {
                                request = Some(Request::Open(report.destination.clone()));
                            }
                            if ui.button("Éjecter la carte").clicked() {
                                request = Some(Request::Eject(source.clone()));
                            }
                            if deleted.is_none()
                                && !report.copied.is_empty()
                                && ui.button("Supprimer de la carte…").clicked()
                            {
                                *confirm_delete = true;
                            }
                            if ui.button("Fermer").clicked() {
                                request = Some(Request::Close);
                            }
                        });
                        if *confirm_delete {
                            ui.add_space(6.0);
                            ui.label(format!(
                                "Supprimer de la carte les {} fichiers copiés{verified} ? \
                             Les autres (ignorés, en erreur, vidéos) restent.",
                                report.copied.len()
                            ));
                            ui.horizontal(|ui| {
                                if ui.button("Supprimer").clicked() {
                                    *deleted = Some(offload::delete_sources(report));
                                    *confirm_delete = false;
                                }
                                if ui.button("Garder").clicked() {
                                    *confirm_delete = false;
                                }
                            });
                        }
                    }
                }
            });
        if start {
            self.start(ctx);
        }
        // Échap ou clic à côté : fermer, sauf pendant la copie (bouton « Interrompre »).
        if request.is_none() && modal.should_close() && !self.is_running() {
            request = Some(Request::Close);
        }
        request
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Mégaoctets à la française (« 109,4 »).
fn french_mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1e6).replace('.', ",")
}

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
        let (tx, rx) = mpsc::channel();
        let (dir, s, repaint) = (source.clone(), settings.clone(), ctx.clone());
        let data_dir = data.clone();
        std::thread::spawn(move || {
            let result = History::open_configured(&s, data_dir.as_deref())
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
            data,
            stage: Stage::Planning(rx),
            hidden: false,
        }
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
            format!(
                "déchargement de « {} » {}/{}",
                file_name(&self.source),
                progress.files_done,
                progress.files_total
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
        let summary = format!(
            "« {} » : {} copiés, {} ignorés, {} erreurs{}",
            file_name(&self.source),
            report.copied.len(),
            report.skipped.len(),
            report.errors.len(),
            if report.cancelled { ", interrompu" } else { "" }
        );
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
                                root: offload::root(&self.settings, self.data.as_deref()),
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
        // Racine retenue pour les prochains déchargements.
        if let Some(data) = &self.data {
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
        let source = self.source.clone();
        let name = source.file_name().map_or_else(
            || source.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let modal = crate::app::modal_at("offload_dialog", self.at).show(ctx, |ui| {
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
                    present,
                    ..
                } => {
                    let settings = &mut self.settings;
                    let dest =
                        offload::destination(&tri::settings::expand_home(root.trim()), folder).ok();
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
                    let mut text = format!("{} images", plan.items.len());
                    if settings.skip_known {
                        let known = plan.items.iter().filter(|i| i.known).count();
                        text.push_str(&format!(", dont {known} déjà déchargées"));
                    }
                    if there > 0 {
                        text.push_str(&format!(", {there} déjà dans la destination"));
                    }
                    text.push_str(&format!(
                        " ; {count} à copier ({} Mo).",
                        french_mb(plan.bytes(settings) - there_bytes)
                    ));
                    ui.label(text);
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
                    if let Some(dest) = &dest {
                        ui.weak(format!("Destination : {}", dest.display()));
                    }
                    if let Some(e) = error {
                        ui.colored_label(egui::Color32::from_rgb(230, 90, 90), e.as_str());
                    }
                    // Choix faits avant la copie, enchaînés à la fin (retour du
                    // 17/10/2026 : ouvrir *et* éjecter, pas l'un ou l'autre).
                    ui.add_space(6.0);
                    ui.checkbox(&mut settings.raw_only, "RAW seulement")
                        .on_hover_text("Les JPEG du boîtier restent sur la carte.");
                    ui.checkbox(&mut settings.open_after, "Ouvrir le dossier à la fin");
                    ui.checkbox(&mut settings.eject_after, "Éjecter la carte à la fin");
                    ui.checkbox(
                        &mut settings.delete_after,
                        "Supprimer de la carte les fichiers copiés et vérifiés",
                    )
                    .on_hover_text(
                        "Ceux copiés et vérifiés pendant ce déchargement, et ceux déjà présents \
                         à l'identique dans la destination. Restent : erreurs, images \
                         laissées, vidéos et fichiers du boîtier.",
                    );
                    // RAW seulement + suppression : les JPEG laissés aussi, sur demande.
                    if settings.raw_only && settings.delete_after {
                        ui.indent("delete_left", |ui| {
                            ui.checkbox(
                                &mut settings.delete_left,
                                "Supprimer aussi les JPEG laissés sur la carte",
                            )
                            .on_hover_text("Ils ne sont pas copiés : ils seront perdus.");
                        });
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
                    ui.horizontal(|ui| {
                        if ui.button("Continuer en arrière-plan").clicked() {
                            background = true;
                        }
                        if ui.button("Interrompre").clicked() {
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
                    let verified = if settings.verify {
                        " et vérifiés"
                    } else {
                        ""
                    };
                    ui.label(format!(
                        "{} fichiers copiés{verified}, {} ignorés (déjà déchargés), {} erreurs{}.",
                        report.copied.len(),
                        report.skipped.len(),
                        report.errors.len(),
                        if report.cancelled {
                            " — interrompu"
                        } else {
                            ""
                        }
                    ));
                    if !report.left.is_empty() {
                        ui.weak(format!("{} JPEG laissés sur la carte.", report.left.len()));
                    }
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
                    if *ejected {
                        ui.weak("Éjection de la carte : voir la barre d'état.");
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        // Ce qui n'a pas été coché reste possible ici.
                        if !*opened && ui.button("Ouvrir le dossier").clicked() {
                            request = Some(Request::Open(report.destination.clone()));
                        }
                        if !*ejected && ui.button("Éjecter la carte").clicked() {
                            request = Some(Request::Eject(source.clone()));
                        }
                        if ui.button("Fermer").clicked() {
                            request = Some(Request::Close);
                        }
                    });
                }
            }
        });
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

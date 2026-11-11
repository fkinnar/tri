//! Boîte « Réinitialiser ce dossier… » (issue n° 411, décidée le 11/11/2026) : une case
//! par chose à effacer (cache, données de tri, notes, versions), ce que chacune concerne
//! mesuré en fond, et « Sous-dossiers compris ». Le travail est fait par le moteur
//! ([`tri::reset`]) ; la boîte ne fait que demander.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use tri::reset::{Parts, Survey};

/// Ce que la boîte demande à l'application.
pub enum Request {
    /// Réinitialiser `parts` dans `folders`.
    Run {
        folder: PathBuf,
        folders: Vec<PathBuf>,
        parts: Parts,
    },
    /// Fermée sans rien faire.
    Close,
}

/// La boîte ouverte.
pub struct ResetDialog {
    folder: PathBuf,
    pub(crate) parts: Parts,
    pub(crate) subfolders: bool,
    /// Notes permises (« Actions sur les fichiers », retour du 11/11/2026).
    notes_allowed: bool,
    /// Mesures, sans et avec sous-dossiers.
    surveys: [Option<Survey>; 2],
    measuring: [Option<Receiver<Survey>>; 2],
    cache: Option<tri::cache::Cache>,
    data: Option<PathBuf>,
    convention: tri::xmp::Convention,
}

impl ResetDialog {
    /// Ouvre la boîte sur `folder` (`disk` : un disque, sous-dossiers compris par
    /// défaut) ; `only_cache` : seule la case du cache cochée
    /// (`core.folders.cache_clear`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        folder: PathBuf,
        disk: bool,
        only_cache: bool,
        notes_allowed: bool,
        cache: Option<tri::cache::Cache>,
        data: Option<PathBuf>,
        convention: tri::xmp::Convention,
    ) -> Self {
        ResetDialog {
            folder,
            parts: Parts {
                cache: true,
                data: !only_cache,
                notes: false,
                versions: false,
            },
            subfolders: disk,
            notes_allowed,
            surveys: [None, None],
            measuring: [None, None],
            cache,
            data,
            convention,
        }
    }

    /// Le dossier réinitialisé.
    #[cfg(test)]
    pub fn folder(&self) -> &std::path::Path {
        &self.folder
    }

    fn depth(subfolders: bool) -> usize {
        if subfolders { usize::MAX } else { 1 }
    }

    /// La mesure en vigueur ; lancée en fond la première fois.
    fn survey(&mut self, ctx: &egui::Context) -> Option<Survey> {
        let i = usize::from(self.subfolders);
        if let Some(rx) = &self.measuring[i]
            && let Ok(survey) = rx.try_recv()
        {
            self.surveys[i] = Some(survey);
            self.measuring[i] = None;
        }
        if self.surveys[i].is_none() && self.measuring[i].is_none() {
            let (tx, rx) = mpsc::channel();
            let (cache, data, convention) =
                (self.cache.clone(), self.data.clone(), self.convention);
            let folders = tri::reset::folders(&self.folder, Self::depth(self.subfolders));
            let repaint = ctx.clone();
            std::thread::spawn(move || {
                let survey =
                    tri::reset::survey(cache.as_ref(), data.as_deref(), &folders, convention);
                let _ = tx.send(survey);
                repaint.request_repaint();
            });
            self.measuring[i] = Some(rx);
        }
        self.surveys[i].clone()
    }

    /// Ce qui sera vraiment effacé : les cases cochées et permises, et pas vides.
    fn effective(&self, survey: Option<&Survey>) -> Parts {
        let some = |n: usize| survey.is_none_or(|_| n > 0);
        let s = survey.cloned().unwrap_or_default();
        Parts {
            cache: self.parts.cache && some(s.cache.entries),
            data: self.parts.data && some(s.data),
            notes: self.parts.notes && self.notes_allowed && some(s.notes),
            versions: self.parts.versions && some(s.versions.1),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Request> {
        let survey = self.survey(ctx);
        let name = crate::app::folder_name(&self.folder);
        let (mut run, mut cancel) = (false, false);
        let modal = crate::app::modal_at("reset_dialog", crate::app::CENTRED)
            .title(t!("reset-title", name = &name))
            .show(ctx, |ui| {
                let s = survey.clone().unwrap_or_default();
                let known = survey.is_some();
                let detail = |ui: &mut egui::Ui, text: String| {
                    ui.indent("detail", |ui| {
                        ui.weak(if known { text } else { t!("reset-measuring") });
                    });
                };
                ui.add_enabled(
                    !known || s.cache.entries > 0,
                    egui::Checkbox::new(&mut self.parts.cache, t!("reset-cache")),
                );
                detail(
                    ui,
                    t!(
                        "reset-cache-detail",
                        size = crate::cache_panel::size_text(s.cache.bytes)
                    ),
                );
                ui.add_enabled(
                    !known || s.data > 0,
                    egui::Checkbox::new(&mut self.parts.data, t!("reset-data")),
                );
                detail(ui, t!("reset-data-detail", n = s.data));
                let notes = ui.add_enabled(
                    self.notes_allowed && (!known || s.notes > 0),
                    egui::Checkbox::new(&mut self.parts.notes, t!("reset-notes")),
                );
                if !self.notes_allowed {
                    notes.on_disabled_hover_text(t!("files-actions-off"));
                }
                detail(ui, t!("reset-notes-detail", n = s.notes));
                if known && s.conflicts > 0 && self.parts.notes {
                    ui.indent("conflicts", |ui| {
                        ui.weak(t!("reset-notes-conflicts", n = s.conflicts));
                    });
                }
                ui.add_enabled(
                    !known || s.versions.1 > 0,
                    egui::Checkbox::new(&mut self.parts.versions, t!("reset-versions")),
                );
                detail(
                    ui,
                    t!(
                        "reset-versions-detail",
                        n = s.versions.1,
                        photos = s.versions.0
                    ),
                );
                ui.add_space(6.0);
                ui.checkbox(&mut self.subfolders, t!("reset-subfolders"));
                ui.add_space(6.0);
                ui.weak(t!("reset-no-undo"));
                if self.parts.notes && self.notes_allowed {
                    ui.weak(t!("reset-notes-note"));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ready = known && !self.effective(survey.as_ref()).is_empty();
                    run = ui
                        .add_enabled(ready, egui::Button::new(t!("reset-confirm")))
                        .clicked();
                    cancel = ui.button(t!("cancel")).clicked();
                });
            });
        if run {
            return Some(Request::Run {
                folder: self.folder.clone(),
                folders: tri::reset::folders(&self.folder, Self::depth(self.subfolders)),
                parts: self.effective(survey.as_ref()),
            });
        }
        if cancel || modal.should_close() {
            return Some(Request::Close);
        }
        if self.measuring.iter().any(Option::is_some) {
            ctx.request_repaint_after(crate::app::poll_interval(ctx));
        }
        None
    }

    /// Le libellé du menu pour un dossier (`disk` : un disque).
    pub fn menu_label(disk: bool) -> String {
        if disk {
            t!("folder-menu-reset-disk")
        } else {
            t!("folder-menu-reset-folder")
        }
    }
}

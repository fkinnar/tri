//! Boîte « Regrouper RAW et JPEG ? » (issue n° 392, décidée le 09/11/2026) : à
//! l'ouverture d'un dossier qui a des paires RAW+JPEG et pas encore de décision
//! (`[files] group_pairs = "ask"`). La grille reste séparée en attendant ; la réponse est
//! retenue pour le dossier, et « Ne plus demander » en fait le réglage global. La boîte ne
//! fait que demander ([`tri::pairs`]).

use std::path::PathBuf;

use eframe::egui;

/// Ce que la boîte demande à l'application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Regrouper (`true`) ou séparer les paires de `folder` ; `always` : plus de question,
    /// la réponse devient le réglage global.
    Decide {
        folder: PathBuf,
        grouped: bool,
        always: bool,
    },
    /// Fermée sans répondre : on redemandera à la prochaine ouverture.
    Close,
}

/// La boîte ouverte.
pub struct PairsDialog {
    pub(crate) folder: PathBuf,
    /// Paires du dossier.
    pairs: usize,
    pub(crate) always: bool,
}

impl PairsDialog {
    /// Ouvre la boîte sur `folder`, qui a `pairs` paires.
    pub fn new(folder: PathBuf, pairs: usize) -> Self {
        PairsDialog {
            folder,
            pairs,
            always: false,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Request> {
        let name = crate::app::folder_name(&self.folder);
        let mut answer = None;
        let mut cancel = false;
        let modal = crate::app::modal_at("pairs_dialog", crate::app::CENTRED)
            .title(t!("pairs-ask-title"))
            .show(ctx, |ui| {
                ui.label(t!("pairs-ask-text", name = &name, n = self.pairs));
                ui.add_space(4.0);
                ui.weak(t!("pairs-ask-detail"));
                ui.add_space(6.0);
                ui.checkbox(&mut self.always, t!("pairs-ask-always"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(t!("pairs-group")).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(t!("pairs-separate")).clicked() {
                        answer = Some(false);
                    }
                    cancel = ui.button(t!("cancel")).clicked();
                });
            });
        if let Some(grouped) = answer {
            return Some(Request::Decide {
                folder: self.folder.clone(),
                grouped,
                always: self.always,
            });
        }
        (cancel || modal.should_close()).then_some(Request::Close)
    }
}

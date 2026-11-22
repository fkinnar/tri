//! Boîte « Retrouver « dossier » » (#561) : la recherche automatique n'a pas trouvé le
//! dossier déplacé d'une photo de « Toutes les photos ». On tape où il est maintenant
//! (complétion par Tab, « Parcourir… ») ; l'application vérifie que c'est bien lui
//! ([`tri::locate::verify`]) avant que l'index, le cache et les notes suivent.

use std::path::PathBuf;

use eframe::egui;

/// Ce que la boîte demande à l'application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Le dossier `old` est maintenant en `new` (à vérifier).
    Locate { old: PathBuf, new: PathBuf },
    /// Fermée.
    Close,
}

/// La boîte ouverte.
pub struct LocateDialog {
    /// Le dossier introuvable.
    pub(crate) old: PathBuf,
    /// Le chemin tapé.
    pub(crate) text: String,
    field: crate::path_field::PathField,
    /// Le dossier proposé n'a pas les mêmes photos.
    pub(crate) error: Option<String>,
}

impl LocateDialog {
    /// Ouvre la boîte sur le dossier introuvable `old`, le champ prérempli de son parent
    /// le plus proche qui existe.
    pub fn new(old: PathBuf) -> Self {
        let start = old
            .ancestors()
            .skip(1)
            .find(|a| a.is_dir())
            .map(|a| a.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut field = crate::path_field::PathField::default();
        field.replaced();
        LocateDialog {
            old,
            text: start,
            field,
            error: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Request> {
        let name = crate::app::folder_name(&self.old);
        let mut go = false;
        let mut cancel = false;
        let modal = crate::app::modal_at("locate_dialog", crate::app::CENTRED)
            .title(t!("locate-title", name = &name))
            .show(ctx, |ui| {
                ui.weak(self.old.display().to_string());
                ui.add_space(4.0);
                ui.label(t!("locate-not-found"));
                ui.add_space(4.0);
                let field = self.field.show(
                    ui,
                    egui::Id::unique("locate_dialog_path"),
                    &mut self.text,
                    420.0,
                );
                if field.changed() {
                    self.error = None;
                }
                go = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    go |= ui.button(t!("locate-ok")).clicked();
                    cancel = ui.button(t!("cancel")).clicked();
                });
            });
        if go && !self.text.trim().is_empty() {
            return Some(Request::Locate {
                old: self.old.clone(),
                new: tri::settings::expand_home(self.text.trim()),
            });
        }
        (cancel || modal.should_close()).then_some(Request::Close)
    }
}

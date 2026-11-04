//! Champ de saisie d'un chemin de dossier existant, avec complétion par Tab comme dans un
//! terminal (demandé le 18/10/2026 : partout où l'on tape un chemin existant). La
//! recherche des dossiers est dans le moteur ([`tri::files::complete_dir`]). Un bouton
//! « Parcourir… » ouvre aussi la fenêtre de choix de dossier du système (`rfd`,
//! 04/11/2026, demandé par l'utilisateur pour tous les champs de chemin).

use eframe::egui::{self, Key, Modifiers, Response, Ui};

/// État d'un champ (à garder d'une image à l'autre).
#[derive(Default)]
pub struct PathField {
    /// Dossiers proposés par le dernier Tab (plusieurs possibles).
    candidates: Vec<String>,
    /// Curseur à replacer en fin de saisie (après une complétion, ou un texte remplacé
    /// par l'appelant).
    cursor_to_end: bool,
    /// Dossier choisi par « Parcourir… » depuis le dernier [`PathField::take_browsed`].
    browsed: bool,
}

impl PathField {
    /// Le texte a été remplacé par l'appelant (destination récente…) : curseur en fin,
    /// propositions effacées.
    pub fn replaced(&mut self) {
        self.candidates.clear();
        self.cursor_to_end = true;
    }

    /// Tab : un seul dossier possible, il est pris ; sinon le début commun, et la liste.
    pub fn complete(&mut self, text: &mut String) {
        let found = tri::files::complete_dir(text);
        match found.as_slice() {
            [] => {}
            [one] => {
                *text = one.clone();
                self.candidates.clear();
            }
            _ => {
                let common = tri::files::common_prefix(&found);
                if common.len() > text.len() {
                    *text = common;
                }
                self.candidates = found;
            }
        }
        self.cursor_to_end = true;
    }

    /// Vrai une fois si « Parcourir… » vient de remplir le champ : à valider comme une
    /// saisie terminée (le champ, lui, n'a pas perdu le focus).
    pub fn take_browsed(&mut self) -> bool {
        std::mem::take(&mut self.browsed)
    }

    /// Le champ `id`, de largeur `width`, son bouton « Parcourir… », puis les dossiers
    /// proposés (12 au plus). Un dossier choisi remplace le texte ; la réponse du champ est
    /// alors marquée modifiée.
    pub fn show(&mut self, ui: &mut Ui, id: egui::Id, text: &mut String, width: f32) -> Response {
        let ctx = ui.ctx().clone();
        // Tab pour compléter, pas pour passer à l'élément suivant.
        if ctx.memory(|m| m.has_focus(id))
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab))
        {
            self.complete(text);
        }
        let (mut field, browse) = ui
            .horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(text)
                        .id(id)
                        .lock_focus(true)
                        .desired_width(width),
                );
                (field, ui.button(t!("path-browse")))
            })
            .inner;
        if browse.clicked()
            && let Some(dir) = rfd::FileDialog::new()
                .set_directory(start_dir(text))
                .pick_folder()
        {
            *text = dir.to_string_lossy().into_owned();
            self.replaced();
            self.browsed = true;
            field.mark_changed();
        }
        if std::mem::take(&mut self.cursor_to_end) {
            let mut state = egui::TextEdit::load_state(&ctx, id).unwrap_or_default();
            let end = egui::text::CCursor::new(text.chars().count());
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(&ctx, id);
        }
        if field.changed() {
            self.candidates.clear();
        }
        let shown = crate::app::dialog_settings(&ctx).completions;
        for candidate in self.candidates.iter().take(shown) {
            ui.weak(candidate);
        }
        field
    }
}

/// Dossier où « Parcourir… » s'ouvre : le chemin tapé, ou son plus proche parent qui
/// existe, sinon le dossier personnel, sinon la racine.
pub fn start_dir(text: &str) -> std::path::PathBuf {
    let typed = (!text.trim().is_empty()).then(|| tri::settings::expand_home(text.trim()));
    typed
        .iter()
        .flat_map(|p| p.ancestors())
        .find(|p| !p.as_os_str().is_empty() && p.is_dir())
        .map(std::path::Path::to_path_buf)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .filter(|h| h.is_dir())
        })
        .unwrap_or_else(|| std::path::PathBuf::from(std::path::MAIN_SEPARATOR_STR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browse_opens_at_the_nearest_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        let inside = dir.path().join("Photos");
        std::fs::create_dir(&inside).unwrap();
        let typed = inside.join("pas-encore").join("là");
        assert_eq!(start_dir(&typed.to_string_lossy()), inside);
        assert_eq!(start_dir(&inside.to_string_lossy()), inside);
        assert!(start_dir("").is_dir(), "dossier personnel ou racine");
    }

    #[test]
    fn tab_takes_a_single_folder_or_the_common_start() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["Photos-2025", "Photos-2026", "Autre"] {
            std::fs::create_dir(dir.path().join(d)).unwrap();
        }
        let sep = std::path::MAIN_SEPARATOR;
        let base = format!("{}{sep}", dir.path().display());
        let mut field = PathField::default();
        let mut text = format!("{base}Au");
        field.complete(&mut text);
        assert_eq!(text, format!("{base}Autre{sep}"));
        let mut text = format!("{base}Ph");
        field.complete(&mut text);
        assert_eq!(text, format!("{base}Photos-202"), "début commun");
        assert_eq!(field.candidates.len(), 2, "et la liste");
    }
}

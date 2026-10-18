//! Champ de saisie d'un chemin de dossier existant, avec complétion par Tab comme dans un
//! terminal (demandé le 18/10/2026 : partout où l'on tape un chemin existant). La
//! recherche des dossiers est dans le moteur ([`tri::files::complete_dir`]).

use eframe::egui::{self, Key, Modifiers, Response, Ui};

/// État d'un champ (à garder d'une image à l'autre).
#[derive(Default)]
pub struct PathField {
    /// Dossiers proposés par le dernier Tab (plusieurs possibles).
    candidates: Vec<String>,
    /// Curseur à replacer en fin de saisie (après une complétion, ou un texte remplacé
    /// par l'appelant).
    cursor_to_end: bool,
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

    /// Le champ `id`, de largeur `width`, puis les dossiers proposés (12 au plus).
    pub fn show(&mut self, ui: &mut Ui, id: egui::Id, text: &mut String, width: f32) -> Response {
        let ctx = ui.ctx().clone();
        // Tab pour compléter, pas pour passer à l'élément suivant.
        if ctx.memory(|m| m.has_focus(id))
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab))
        {
            self.complete(text);
        }
        let field = ui.add(
            egui::TextEdit::singleline(text)
                .id(id)
                .lock_focus(true)
                .desired_width(width),
        );
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
        for candidate in self.candidates.iter().take(12) {
            ui.weak(candidate);
        }
        field
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

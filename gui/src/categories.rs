//! Catégories communes à la fenêtre de réglages et à l'aide (15/11/2026, retour de
//! l'utilisateur : mêmes titres et même style dans les deux fenêtres, pour s'y retrouver
//! sans rien chercher). Chaque fenêtre range ses pages (réglages) ou ses thèmes (aide)
//! sous ces titres ; une catégorie sans rien dedans n'est pas montrée.

use eframe::egui::{self, RichText, Ui};

/// Identifiants des catégories, dans leur ordre (`category-<id>` dans les catalogues).
pub const IDS: [&str; 6] = [
    "general", "import", "sort", "display", "develop", "advanced",
];

/// Titre traduit de la catégorie `c` (indice dans [`IDS`]).
pub fn label(c: usize) -> String {
    t!(&format!("category-{}", IDS[c]))
}

/// Titre repliable de la catégorie `c`, au style commun (majuscules, couleur d'accent),
/// ouvert par défaut ; `open` : ouvert d'office (la page choisie au clavier y est).
/// `salt` distingue les deux fenêtres (état replié propre à chacune).
pub fn header(ui: &Ui, c: usize, salt: &str, open: bool) -> egui::CollapsingHeader {
    let title =
        RichText::new(label(c).to_uppercase()).color(crate::marks::accent_bright(ui.visuals()));
    let header = egui::CollapsingHeader::new(title)
        .id_salt((salt, c))
        .default_open(true);
    if open {
        header.open(Some(true))
    } else {
        header
    }
}

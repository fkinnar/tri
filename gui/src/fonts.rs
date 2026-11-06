//! Flèches des noms de touches (←, →, ↑, ↓ ; demandé le 06/11/2026 : « Flèche gauche »
//! était long et peu lisible). La police d'egui n'en a pas : une police du système qui les
//! a (Apple Symbols sur macOS, DejaVu Sans sous Linux, Segoe UI Symbol sous Windows) est
//! ajoutée en secours, pour les seuls caractères qui manquent. Rien n'est embarqué (pas de
//! licence à suivre). Sans elle, les touches restent écrites en toutes lettres.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;

/// Vrai une fois une police des flèches installée ([`install`]) : les noms de touches les
/// utilisent ([`crate::actions::key_label`]). Jamais dans les tests (pas d'installation) :
/// leurs textes restent stables.
static ARROWS: AtomicBool = AtomicBool::new(false);

/// Polices du système qui ont les flèches, essayées dans l'ordre, chacune avec l'échelle
/// qui met ses flèches à la taille du texte (retour du 07/11/2026 : celles d'Apple Symbols
/// sont petites et fines, celles de DejaVu Sans grosses).
const SYSTEM_FONTS: &[(&str, f32)] = &[
    // macOS.
    ("/System/Library/Fonts/Apple Symbols.ttf", 1.4),
    // Linux (Debian / Ubuntu / Mint, Arch, Fedora).
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 1.0),
    ("/usr/share/fonts/TTF/DejaVuSans.ttf", 1.0),
    ("/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf", 1.0),
    ("/usr/share/fonts/dejavu/DejaVuSans.ttf", 1.0),
    // Windows.
    ("C:\\Windows\\Fonts\\seguisym.ttf", 1.1),
];

/// Vrai si les noms de touches peuvent utiliser les flèches.
pub fn arrows() -> bool {
    ARROWS.load(Ordering::Relaxed)
}

/// Fichiers de police à essayer d'après `[gui] arrow_font`, avec leur échelle : vide,
/// ceux du système ; `"none"`, aucun (les mots) ; sinon ce fichier (échelle 1).
pub fn candidates(setting: &str) -> Vec<(String, f32)> {
    match setting.trim() {
        "" => SYSTEM_FONTS
            .iter()
            .map(|(path, scale)| (path.to_string(), *scale))
            .collect(),
        "none" => Vec::new(),
        path => vec![(path.to_string(), 1.0)],
    }
}

/// Ajoute la première police trouvée parmi les [`candidates`] en secours des polices
/// d'egui, et passe les noms de touches aux flèches. Vrai si une police a été chargée.
pub fn install(ctx: &egui::Context, setting: &str, scale: f32) -> bool {
    for (path, own) in candidates(setting) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::default();
        // À la taille du texte (l'échelle de cette police), puis le réglage par-dessus.
        let data = egui::FontData::from_owned(bytes).tweak(egui::FontTweak {
            scale: (own * scale).max(0.5),
            ..Default::default()
        });
        fonts
            .font_data
            .insert("tri-arrows".to_string(), Arc::new(data));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("tri-arrows".to_string());
        }
        ctx.set_fonts(fonts);
        ARROWS.store(true, Ordering::Relaxed);
        crate::trace::trace(|| format!("flèches des touches : {path}"));
        return true;
    }
    false
}

/// Le symbole d'une touche : les flèches et Entrée (↵) (demandé le 06/11/2026 : plus
/// courts) ; `None` pour les autres. Dans les polices de [`SYSTEM_FONTS`] (↵ plutôt que
/// ⏎ : du bloc des flèches, que DejaVu Sans a en entier). Espace reste écrite : ␣, collé
/// au bas de la ligne, ne se lisait pas (« <leader> ␣ », « Maj+␣ »).
pub fn arrow(key: egui::Key) -> Option<char> {
    match key {
        egui::Key::ArrowLeft => Some('←'),
        egui::Key::ArrowRight => Some('→'),
        egui::Key::ArrowUp => Some('↑'),
        egui::Key::ArrowDown => Some('↓'),
        egui::Key::Enter => Some('↵'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_for_arrow_keys_only() {
        assert_eq!(arrow(egui::Key::ArrowLeft), Some('←'));
        assert_eq!(arrow(egui::Key::ArrowDown), Some('↓'));
        assert_eq!(arrow(egui::Key::Enter), Some('↵'));
        assert_eq!(arrow(egui::Key::Space), None, "Espace reste écrite");
        assert_eq!(arrow(egui::Key::H), None);
    }

    #[test]
    fn the_setting_chooses_the_fonts() {
        assert!(!candidates("").is_empty());
        assert!(candidates("none").is_empty());
        assert_eq!(candidates("/x/y.ttf"), [("/x/y.ttf".to_string(), 1.0)]);
        // Fichier absent : rien d'installé, les mots restent.
        let ctx = egui::Context::default();
        assert!(!install(&ctx, "/nonexistent/font.ttf", 1.0));
        assert!(!arrows());
    }
}

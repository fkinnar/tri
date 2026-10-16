//! Actions nommées et raccourcis clavier.
//!
//! Une touche ne déclenche jamais directement une fonction : elle est traduite en
//! [`Action`], identifiée par un nom stable (`core.nav.next`...). Le futur fichier de
//! raccourcis JSON associera des touches à ces noms ; les plugins auront leur propre espace
//! (`plugin.<nom>.*`). Voir `doc/gui.md`.

use eframe::egui::{Key, Modifiers};

/// Ce que l'utilisateur demande, indépendamment de la touche (ou du clic) utilisée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Image précédente.
    NavPrev,
    /// Image suivante.
    NavNext,
    /// Ligne du dessus (grille).
    NavUp,
    /// Ligne du dessous (grille).
    NavDown,
    /// Première image.
    NavFirst,
    /// Dernière image.
    NavLast,
    /// Une page plus haut (grille).
    NavPageUp,
    /// Une page plus bas (grille).
    NavPageDown,
    /// Filtre suivant : tout → RAW → plates → tout.
    FilterCycle,
    /// Fermer ce qui est au premier plan (aide, plein écran...) / revenir en arrière.
    Back,
    /// Afficher / masquer l'aide des raccourcis.
    Help,
    /// Quitter l'application.
    Quit,
}

impl Action {
    /// Toutes les actions, pour les listes (aide, fichier de raccourcis).
    pub const ALL: [Action; 12] = [
        Action::NavPrev,
        Action::NavNext,
        Action::NavUp,
        Action::NavDown,
        Action::NavFirst,
        Action::NavLast,
        Action::NavPageUp,
        Action::NavPageDown,
        Action::FilterCycle,
        Action::Back,
        Action::Help,
        Action::Quit,
    ];

    /// Action correspondant à un identifiant (fichier de raccourcis, scripts de capture).
    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    /// Libellé pour l'utilisateur (aide, menus).
    pub fn label(self) -> &'static str {
        match self {
            Action::NavPrev => "Image précédente",
            Action::NavNext => "Image suivante",
            Action::NavUp => "Ligne du dessus",
            Action::NavDown => "Ligne du dessous",
            Action::NavFirst => "Première image",
            Action::NavLast => "Dernière image",
            Action::NavPageUp => "Page précédente",
            Action::NavPageDown => "Page suivante",
            Action::FilterCycle => "Changer de filtre : tout, RAW, plates",
            Action::Back => "Fermer / revenir",
            Action::Help => "Afficher ou masquer cette aide",
            Action::Quit => "Quitter",
        }
    }

    /// Vrai pour les actions encore actives quand une fenêtre (l'aide) est au premier plan.
    pub fn works_over_overlay(self) -> bool {
        matches!(self, Action::Back | Action::Help | Action::Quit)
    }

    /// Identifiant stable, utilisé par le fichier de raccourcis.
    pub fn id(self) -> &'static str {
        match self {
            Action::NavPrev => "core.nav.prev",
            Action::NavNext => "core.nav.next",
            Action::NavUp => "core.nav.up",
            Action::NavDown => "core.nav.down",
            Action::NavFirst => "core.nav.first",
            Action::NavLast => "core.nav.last",
            Action::NavPageUp => "core.nav.page_up",
            Action::NavPageDown => "core.nav.page_down",
            Action::FilterCycle => "core.filter.cycle",
            Action::Back => "core.view.back",
            Action::Help => "core.app.help",
            Action::Quit => "core.app.quit",
        }
    }
}

/// Une combinaison de touches : une touche, avec ou sans Maj. Les autres modificateurs
/// (Ctrl, Cmd, Alt) sont réservés au système et ne déclenchent rien ici.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// Touche.
    pub key: Key,
    /// Maj enfoncée.
    pub shift: bool,
}

const fn plain(key: Key) -> Chord {
    Chord { key, shift: false }
}

const fn shifted(key: Key) -> Chord {
    Chord { key, shift: true }
}

/// Profil de raccourcis livré par défaut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Flèches, Début / Fin, Page préc. / suiv.
    Classic,
    /// hjkl, g / G, Ctrl-u / Ctrl-d remplacés par u / d.
    Vim,
}

const CLASSIC: &[(Chord, Action)] = {
    use Action::*;
    &[
        (plain(Key::ArrowLeft), NavPrev),
        (plain(Key::ArrowRight), NavNext),
        (plain(Key::ArrowUp), NavUp),
        (plain(Key::ArrowDown), NavDown),
        (plain(Key::Home), NavFirst),
        (plain(Key::End), NavLast),
        (plain(Key::PageUp), NavPageUp),
        (plain(Key::PageDown), NavPageDown),
        (plain(Key::F), FilterCycle),
        (plain(Key::Escape), Back),
        (plain(Key::F1), Help),
        (plain(Key::Q), Quit),
    ]
};

const VIM: &[(Chord, Action)] = {
    use Action::*;
    &[
        (plain(Key::H), NavPrev),
        (plain(Key::L), NavNext),
        (plain(Key::K), NavUp),
        (plain(Key::J), NavDown),
        (plain(Key::G), NavFirst),
        (shifted(Key::G), NavLast),
        (plain(Key::U), NavPageUp),
        (plain(Key::D), NavPageDown),
        (plain(Key::F), FilterCycle),
        // « ? » arrive avec ou sans Maj selon la disposition du clavier.
        (plain(Key::Escape), Back),
        (plain(Key::Questionmark), Help),
        (shifted(Key::Questionmark), Help),
        (plain(Key::Q), Quit),
    ]
};

impl Profile {
    /// Raccourcis du profil.
    pub fn bindings(self) -> &'static [(Chord, Action)] {
        match self {
            Profile::Classic => CLASSIC,
            Profile::Vim => VIM,
        }
    }
}

/// Table touche → action.
#[derive(Clone, Debug)]
pub struct Keymap {
    bindings: Vec<(Chord, Action)>,
}

impl Keymap {
    /// Union de plusieurs profils (le premier l'emporte en cas de conflit).
    pub fn from_profiles(profiles: &[Profile]) -> Keymap {
        let mut bindings: Vec<(Chord, Action)> = Vec::new();
        for profile in profiles {
            for &(chord, action) in profile.bindings() {
                if !bindings.iter().any(|(c, _)| *c == chord) {
                    bindings.push((chord, action));
                }
            }
        }
        Keymap { bindings }
    }

    /// Action associée à une touche pressée, `None` si aucune (ou si Ctrl, Cmd ou Alt est
    /// enfoncé).
    pub fn action(&self, key: Key, modifiers: Modifiers) -> Option<Action> {
        if modifiers.ctrl || modifiers.command || modifiers.alt || modifiers.mac_cmd {
            return None;
        }
        let chord = Chord {
            key,
            shift: modifiers.shift,
        };
        self.bindings
            .iter()
            .find(|(c, _)| *c == chord)
            .map(|(_, a)| *a)
    }
}

impl Keymap {
    /// Touches associées à une action, lisibles (« ← », « Maj+G »...), pour l'aide.
    pub fn keys_for(&self, action: Action) -> Vec<String> {
        let mut keys: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(c, _)| {
                let name = key_label(c.key);
                if c.shift && c.key != Key::Questionmark {
                    format!("Maj+{name}")
                } else {
                    name
                }
            })
            .collect();
        keys.dedup();
        keys
    }
}

/// Nom lisible d'une touche, en français. Volontairement indépendant des noms d'egui et sans
/// symboles : « ⏴ » et même « ← » manquent dans la police par défaut d'egui (les tests
/// d'interface le vérifient), et les polices du système ne sont pas garanties.
pub fn key_label(key: Key) -> String {
    match key {
        Key::ArrowLeft => "Flèche gauche".into(),
        Key::ArrowRight => "Flèche droite".into(),
        Key::ArrowUp => "Flèche haut".into(),
        Key::ArrowDown => "Flèche bas".into(),
        Key::Home => "Début".into(),
        Key::End => "Fin".into(),
        Key::PageUp => "Page préc.".into(),
        Key::PageDown => "Page suiv.".into(),
        Key::Escape => "Échap".into(),
        Key::Enter => "Entrée".into(),
        Key::Space => "Espace".into(),
        Key::Questionmark => "?".into(),
        other => {
            let name = other.name();
            // Lettres en minuscules, comme on les tape (g et Maj+G sont deux touches).
            if name.len() == 1 {
                name.to_lowercase()
            } else {
                name.to_string()
            }
        }
    }
}

impl Default for Keymap {
    /// Les deux profils actifs ensemble : ils ne se recouvrent pas.
    fn default() -> Self {
        Keymap::from_profiles(&[Profile::Classic, Profile::Vim])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_namespaced_and_reversible() {
        let mut ids: Vec<&str> = Action::ALL.iter().map(|a| a.id()).collect();
        assert!(ids.iter().all(|id| id.starts_with("core.")));
        for a in Action::ALL {
            assert_eq!(Action::from_id(a.id()), Some(a));
        }
        assert_eq!(Action::from_id("plugin.x.y"), None);
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), Action::ALL.len());
    }

    #[test]
    fn both_profiles_work_together() {
        let km = Keymap::default();
        let none = Modifiers::NONE;
        assert_eq!(km.action(Key::ArrowRight, none), Some(Action::NavNext));
        assert_eq!(km.action(Key::L, none), Some(Action::NavNext));
        assert_eq!(km.action(Key::J, none), Some(Action::NavDown));
        assert_eq!(km.action(Key::G, none), Some(Action::NavFirst));
        assert_eq!(km.action(Key::G, Modifiers::SHIFT), Some(Action::NavLast));
        assert_eq!(km.action(Key::Z, none), None);
    }

    #[test]
    fn system_modifiers_are_left_alone() {
        let km = Keymap::default();
        assert_eq!(
            km.action(Key::Q, Modifiers::COMMAND),
            None,
            "Cmd+Q reste au système"
        );
        assert_eq!(km.action(Key::L, Modifiers::CTRL), None);
        assert_eq!(km.action(Key::L, Modifiers::ALT), None);
    }

    #[test]
    fn profiles_cover_every_action_and_have_no_internal_conflict() {
        for profile in [Profile::Classic, Profile::Vim] {
            let b = profile.bindings();
            for a in Action::ALL {
                assert!(
                    b.iter().any(|(_, x)| *x == a),
                    "{profile:?} : {a:?} sans touche"
                );
            }
            for (i, (c, _)) in b.iter().enumerate() {
                assert!(
                    !b[i + 1..].iter().any(|(d, _)| d == c),
                    "{profile:?} : doublon {c:?}"
                );
            }
        }
    }

    #[test]
    fn help_lists_readable_keys() {
        let km = Keymap::default();
        assert_eq!(km.keys_for(Action::NavLast), ["Fin", "Maj+g"]);
        assert_eq!(km.keys_for(Action::NavPrev), ["Flèche gauche", "h"]);
        assert_eq!(km.keys_for(Action::Help), ["F1", "?"]);
        assert_eq!(
            km.action(Key::Questionmark, Modifiers::SHIFT),
            Some(Action::Help)
        );
    }

    #[test]
    fn labels_are_distinct_and_readable() {
        let mut labels: Vec<&str> = Action::ALL.iter().map(|a| a.label()).collect();
        assert!(labels.iter().all(|l| !l.contains('.') || l.contains(' ')));
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), Action::ALL.len());
    }

    #[test]
    fn first_profile_wins_on_conflict() {
        let km = Keymap::from_profiles(&[Profile::Vim, Profile::Classic]);
        assert_eq!(
            km.action(Key::F, Modifiers::NONE),
            Some(Action::FilterCycle)
        );
    }
}

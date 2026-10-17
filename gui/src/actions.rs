//! Actions nommées et raccourcis clavier.
//!
//! Une touche ne déclenche jamais directement une fonction : elle est traduite en
//! [`Action`], identifiée par un nom stable (`core.nav.next`...). Le futur fichier de
//! raccourcis JSON associera des touches à ces noms ; les plugins auront leur propre espace
//! (`plugin.<nom>.*`). Un raccourci peut être une séquence de touches (`g g`, comme dans
//! vim) : voir [`Sequencer`]. Voir `doc/gui.md`.

use std::time::{Duration, Instant};

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
    /// Ouvrir l'image sélectionnée en plein écran.
    Open,
    /// Plein écran : basculer entre ajusté à la fenêtre et 100 %.
    ToggleZoom,
    /// Plein écran : basculer entre JPEG du boîtier et rendu RAW.
    ToggleSource,
    /// Image zoomée : voir plus à gauche.
    PanLeft,
    /// Image zoomée : voir plus à droite.
    PanRight,
    /// Image zoomée : voir plus haut.
    PanUp,
    /// Image zoomée : voir plus bas.
    PanDown,
    /// Fermer ce qui est au premier plan (aide, plein écran...) / revenir en arrière.
    Back,
    /// Afficher / masquer l'aide des raccourcis.
    Help,
    /// Quitter l'application.
    Quit,
}

impl Action {
    /// Toutes les actions, pour les listes (aide, fichier de raccourcis).
    pub const ALL: [Action; 19] = [
        Action::NavPrev,
        Action::NavNext,
        Action::NavUp,
        Action::NavDown,
        Action::NavFirst,
        Action::NavLast,
        Action::NavPageUp,
        Action::NavPageDown,
        Action::FilterCycle,
        Action::Open,
        Action::ToggleZoom,
        Action::ToggleSource,
        Action::PanLeft,
        Action::PanRight,
        Action::PanUp,
        Action::PanDown,
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
            Action::Open => "Ouvrir en plein écran",
            Action::ToggleZoom => "Zoom 100 % / ajusté",
            Action::ToggleSource => "Basculer JPEG boîtier / rendu RAW",
            Action::PanLeft => "Image zoomée : aller à gauche",
            Action::PanRight => "Image zoomée : aller à droite",
            Action::PanUp => "Image zoomée : aller en haut",
            Action::PanDown => "Image zoomée : aller en bas",
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
            Action::Open => "core.view.open",
            Action::ToggleZoom => "core.view.zoom",
            Action::ToggleSource => "core.view.source",
            Action::PanLeft => "core.view.pan_left",
            Action::PanRight => "core.view.pan_right",
            Action::PanUp => "core.view.pan_up",
            Action::PanDown => "core.view.pan_down",
            Action::Back => "core.view.back",
            Action::Help => "core.app.help",
            Action::Quit => "core.app.quit",
        }
    }
}

/// Une combinaison de touches : une touche, avec ou sans Maj. Les autres modificateurs
/// (Ctrl, Cmd, Alt) sont réservés au système et ne déclenchent rien ici (sur macOS,
/// Ctrl + flèches change de bureau).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// Touche.
    pub key: Key,
    /// Maj enfoncée.
    pub shift: bool,
}

impl Chord {
    /// Combinaison d'une touche pressée, `None` si Ctrl, Cmd ou Alt est enfoncé.
    pub fn from_event(key: Key, modifiers: Modifiers) -> Option<Chord> {
        if modifiers.ctrl || modifiers.command || modifiers.alt || modifiers.mac_cmd {
            return None;
        }
        Some(Chord {
            key,
            shift: modifiers.shift,
        })
    }
}

const fn plain(key: Key) -> Chord {
    Chord { key, shift: false }
}

const fn shifted(key: Key) -> Chord {
    Chord { key, shift: true }
}

/// Un raccourci : une touche, ou une séquence de touches (`g g`).
type Binding = (&'static [Chord], Action);

/// Profil de raccourcis livré par défaut. Dans le fichier de configuration : `"classic"`
/// ou `"vim"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    /// Flèches, Début / Fin, Page préc. / suiv. ; Maj + flèches dans une image zoomée.
    Classic,
    /// hjkl, gg / G, u / d (à la place de Ctrl-u / Ctrl-d) ; Maj + hjkl dans une image
    /// zoomée.
    Vim,
}

const CLASSIC: &[Binding] = {
    use Action::*;
    &[
        (&[plain(Key::ArrowLeft)], NavPrev),
        (&[plain(Key::ArrowRight)], NavNext),
        (&[plain(Key::ArrowUp)], NavUp),
        (&[plain(Key::ArrowDown)], NavDown),
        (&[plain(Key::Home)], NavFirst),
        (&[plain(Key::End)], NavLast),
        (&[plain(Key::PageUp)], NavPageUp),
        (&[plain(Key::PageDown)], NavPageDown),
        (&[plain(Key::F)], FilterCycle),
        (&[plain(Key::Enter)], Open),
        (&[plain(Key::Z)], ToggleZoom),
        (&[plain(Key::B)], ToggleSource),
        (&[shifted(Key::ArrowLeft)], PanLeft),
        (&[shifted(Key::ArrowRight)], PanRight),
        (&[shifted(Key::ArrowUp)], PanUp),
        (&[shifted(Key::ArrowDown)], PanDown),
        (&[plain(Key::Escape)], Back),
        (&[plain(Key::F1)], Help),
        (&[plain(Key::Q)], Quit),
    ]
};

const VIM: &[Binding] = {
    use Action::*;
    &[
        (&[plain(Key::H)], NavPrev),
        (&[plain(Key::L)], NavNext),
        (&[plain(Key::K)], NavUp),
        (&[plain(Key::J)], NavDown),
        (&[plain(Key::G), plain(Key::G)], NavFirst),
        (&[shifted(Key::G)], NavLast),
        (&[plain(Key::U)], NavPageUp),
        (&[plain(Key::D)], NavPageDown),
        (&[plain(Key::F)], FilterCycle),
        (&[plain(Key::Enter)], Open),
        (&[plain(Key::Z)], ToggleZoom),
        (&[plain(Key::B)], ToggleSource),
        (&[shifted(Key::H)], PanLeft),
        (&[shifted(Key::L)], PanRight),
        (&[shifted(Key::K)], PanUp),
        (&[shifted(Key::J)], PanDown),
        (&[plain(Key::Escape)], Back),
        // « ? » arrive avec ou sans Maj selon la disposition du clavier.
        (&[plain(Key::Questionmark)], Help),
        (&[shifted(Key::Questionmark)], Help),
        (&[plain(Key::Q)], Quit),
    ]
};

impl Profile {
    /// Raccourcis du profil.
    pub fn bindings(self) -> &'static [Binding] {
        match self {
            Profile::Classic => CLASSIC,
            Profile::Vim => VIM,
        }
    }
}

/// Table raccourci → action.
#[derive(Clone, Debug)]
pub struct Keymap {
    bindings: Vec<Binding>,
}

/// Résultat d'une touche passée à [`Keymap::lookup`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lookup {
    /// La séquence correspond exactement à un raccourci.
    Action(Action),
    /// La séquence est le début d'un raccourci plus long : attendre la suite.
    Prefix,
    /// Aucun raccourci ne commence ainsi.
    None,
}

impl Keymap {
    /// Union de plusieurs profils (le premier l'emporte en cas de conflit).
    pub fn from_profiles(profiles: &[Profile]) -> Keymap {
        let mut bindings: Vec<Binding> = Vec::new();
        for profile in profiles {
            for &(keys, action) in profile.bindings() {
                if !bindings.iter().any(|(k, _)| *k == keys) {
                    bindings.push((keys, action));
                }
            }
        }
        Keymap { bindings }
    }

    /// Action d'une touche seule (sans séquence en cours), `None` si aucune ou si Ctrl, Cmd
    /// ou Alt est enfoncé. L'application passe par [`Sequencer`] ; sert aux tests.
    #[cfg(test)]
    pub fn action(&self, key: Key, modifiers: Modifiers) -> Option<Action> {
        match self.lookup(&[Chord::from_event(key, modifiers)?]) {
            Lookup::Action(a) => Some(a),
            Lookup::Prefix | Lookup::None => None,
        }
    }

    fn lookup(&self, keys: &[Chord]) -> Lookup {
        if self
            .bindings
            .iter()
            .any(|(k, _)| k.len() > keys.len() && k.starts_with(keys))
        {
            return Lookup::Prefix;
        }
        match self.bindings.iter().find(|(k, _)| *k == keys) {
            Some(&(_, action)) => Lookup::Action(action),
            None => Lookup::None,
        }
    }

    /// Raccourcis associés à une action, lisibles (« Flèche gauche », « Maj+g », « gg »...),
    /// pour l'aide.
    pub fn keys_for(&self, action: Action) -> Vec<String> {
        let mut keys: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(k, _)| sequence_label(k))
            .collect();
        keys.dedup();
        keys
    }
}

/// Nom lisible d'une combinaison (« Maj+Flèche gauche », « g »).
fn chord_label(c: &Chord) -> String {
    let name = key_label(c.key);
    if c.shift && c.key != Key::Questionmark {
        format!("Maj+{name}")
    } else {
        name
    }
}

/// Nom lisible d'une séquence : « gg » comme on la tape quand ce ne sont que des lettres,
/// sinon « Échap puis g ».
fn sequence_label(keys: &[Chord]) -> String {
    let labels: Vec<String> = keys.iter().map(chord_label).collect();
    if labels.iter().all(|l| l.chars().count() == 1) {
        labels.concat()
    } else {
        labels.join(" puis ")
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

/// Reconnaissance des séquences de touches (`g g`), comme le `timeoutlen` de vim : une
/// touche qui commence une séquence est mise en attente ; la suivante doit arriver avant
/// `timeout`. Si elle ne complète aucune séquence, elle est interprétée seule (`g` puis
/// `j` : ligne du dessous).
#[derive(Clone, Debug)]
pub struct Sequencer {
    pending: Vec<Chord>,
    since: Option<Instant>,
    timeout: Duration,
}

impl Sequencer {
    /// Séquenceur avec un délai maximal `timeout` entre deux touches d'une séquence.
    pub fn new(timeout: Duration) -> Sequencer {
        Sequencer {
            pending: Vec::new(),
            since: None,
            timeout,
        }
    }

    /// Vrai si une séquence est commencée (et pas encore expirée à `now`).
    pub fn is_pending(&self, now: Instant) -> bool {
        self.since
            .is_some_and(|since| now.duration_since(since) <= self.timeout)
    }

    /// Traite une touche pressée à `now` ; renvoie l'action déclenchée, s'il y en a une.
    pub fn feed(&mut self, keymap: &Keymap, chord: Chord, now: Instant) -> Option<Action> {
        if !self.is_pending(now) {
            self.pending.clear();
        }
        let mut keys = std::mem::take(&mut self.pending);
        keys.push(chord);
        self.since = None;
        match keymap.lookup(&keys) {
            Lookup::Prefix => {
                self.pending = keys;
                self.since = Some(now);
                None
            }
            Lookup::Action(action) => Some(action),
            // Séquence interrompue : la dernière touche compte seule.
            Lookup::None if keys.len() > 1 => self.feed(keymap, chord, now),
            Lookup::None => None,
        }
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
        assert_eq!(km.action(Key::G, none), None, "g seul : début de « gg »");
        assert_eq!(km.action(Key::G, Modifiers::SHIFT), Some(Action::NavLast));
        assert_eq!(km.action(Key::W, none), None, "touche libre");
    }

    #[test]
    fn shift_moves_inside_a_zoomed_image() {
        let km = Keymap::default();
        let shift = Modifiers::SHIFT;
        assert_eq!(km.action(Key::ArrowLeft, shift), Some(Action::PanLeft));
        assert_eq!(km.action(Key::ArrowDown, shift), Some(Action::PanDown));
        assert_eq!(km.action(Key::H, shift), Some(Action::PanLeft));
        assert_eq!(km.action(Key::J, shift), Some(Action::PanDown));
        assert_eq!(km.action(Key::K, shift), Some(Action::PanUp));
        assert_eq!(km.action(Key::L, shift), Some(Action::PanRight));
        assert_eq!(
            km.action(Key::G, shift),
            Some(Action::NavLast),
            "Maj+G inchangé"
        );
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
        assert_eq!(Chord::from_event(Key::ArrowLeft, Modifiers::CTRL), None);
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
            for (i, (k, _)) in b.iter().enumerate() {
                assert!(
                    !b[i + 1..].iter().any(|(l, _)| l == k),
                    "{profile:?} : doublon {k:?}"
                );
            }
        }
    }

    #[test]
    fn no_binding_is_also_the_start_of_another() {
        // Sinon il faudrait attendre l'expiration pour savoir lequel déclencher.
        let km = Keymap::default();
        for (k, a) in &km.bindings {
            assert!(
                !km.bindings
                    .iter()
                    .any(|(l, _)| l.len() > k.len() && l.starts_with(k)),
                "{a:?} ({k:?}) est aussi le début d'une séquence"
            );
        }
    }

    #[test]
    fn help_lists_readable_keys() {
        let km = Keymap::default();
        assert_eq!(km.keys_for(Action::NavFirst), ["Début", "gg"]);
        assert_eq!(km.keys_for(Action::NavLast), ["Fin", "Maj+g"]);
        assert_eq!(km.keys_for(Action::NavPrev), ["Flèche gauche", "h"]);
        assert_eq!(km.keys_for(Action::PanLeft), ["Maj+Flèche gauche", "Maj+h"]);
        assert_eq!(km.keys_for(Action::Help), ["F1", "?"]);
        assert_eq!(
            km.action(Key::Questionmark, Modifiers::SHIFT),
            Some(Action::Help)
        );
        assert_eq!(
            sequence_label(&[plain(Key::Escape), plain(Key::G)]),
            "Échap puis g"
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

    fn g() -> Chord {
        plain(Key::G)
    }

    #[test]
    fn gg_goes_to_the_first_image() {
        let km = Keymap::default();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        assert_eq!(seq.feed(&km, g(), t), None);
        assert!(seq.is_pending(t));
        assert_eq!(
            seq.feed(&km, g(), t + Duration::from_millis(300)),
            Some(Action::NavFirst)
        );
        assert!(!seq.is_pending(t + Duration::from_millis(300)));
    }

    #[test]
    fn a_too_slow_second_g_starts_over() {
        let km = Keymap::default();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        seq.feed(&km, g(), t);
        let late = t + Duration::from_millis(1500);
        assert_eq!(seq.feed(&km, g(), late), None, "nouveau début de séquence");
        assert_eq!(
            seq.feed(&km, g(), late + Duration::from_millis(100)),
            Some(Action::NavFirst)
        );
    }

    #[test]
    fn an_interrupted_sequence_lets_the_last_key_act_alone() {
        let km = Keymap::default();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        seq.feed(&km, g(), t);
        assert_eq!(seq.feed(&km, plain(Key::J), t), Some(Action::NavDown));
        assert!(!seq.is_pending(t));
        // Une touche seule passe directement, sans attente.
        assert_eq!(seq.feed(&km, plain(Key::L), t), Some(Action::NavNext));
    }
}

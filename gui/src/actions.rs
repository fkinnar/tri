//! Actions nommées et raccourcis clavier.
//!
//! Une touche ne déclenche jamais directement une fonction : elle est traduite en
//! [`Action`], identifiée par un nom stable (`core.nav.next`...). La section `[keys]` du
//! fichier de configuration associe des touches à ces noms ; les plugins auront leur propre
//! espace (`plugin.<nom>.*`). Un raccourci peut être une séquence de touches (`g g`, comme
//! dans vim) : voir [`Sequencer`]. Voir `doc/gui.md`.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tri::i18n::Language;

use eframe::egui::{Event, Key, Modifiers};
use tri::xmp::{Color, Flag};

/// Thème d'une action, pour l'aide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    /// Se déplacer d'une image à l'autre, ouvrir, revenir.
    Navigation,
    /// Sélection multiple.
    Selection,
    /// Plein écran : zoom, source, déplacement, histogramme, zones.
    View,
    /// Étoiles, couleurs, drapeau.
    Rating,
    /// Filtres.
    Filters,
    /// Panneaux et barres.
    Panels,
    /// Renommer, supprimer, afficher, éditeur externe.
    Files,
    /// Annuler, rétablir, historique.
    Edit,
    /// Aide, quitter.
    App,
}

impl Group {
    /// Les thèmes, dans l'ordre de l'aide.
    pub const ALL: [Group; 9] = [
        Group::Navigation,
        Group::Selection,
        Group::View,
        Group::Rating,
        Group::Filters,
        Group::Panels,
        Group::Files,
        Group::Edit,
        Group::App,
    ];

    /// Titre dans l'aide, dans la langue de l'interface.
    pub fn label(self) -> String {
        t!(&format!("group-{self:?}").to_lowercase())
    }
}

/// Lettre sans accent, en minuscule (recherche floue).
fn fold(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'à' | 'â' | 'ä' | 'á' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' | 'í' => 'i',
        'ô' | 'ö' | 'ó' => 'o',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ç' => 'c',
        'œ' => 'o',
        other => other,
    }
}

/// Score de `query` dans `text` à la façon de fzf : chaque mot de la requête (séparés par
/// des espaces) doit s'y retrouver lettre par lettre, dans l'ordre, éventuellement avec
/// des trous (« pnd » trouve « panneau des dossiers »), sans tenir compte des accents ni
/// de la casse. `None` : pas de correspondance ; plus le score est grand, meilleure elle
/// est (lettres consécutives, débuts de mots). Requête vide : `Some(0)`.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let text: Vec<char> = text.chars().map(fold).collect();
    let mut total = 0;
    for word in query.split_whitespace() {
        let word: Vec<char> = word.chars().map(fold).collect();
        total += word_score(&word, &text)?;
    }
    Some(total)
}

fn word_score(word: &[char], text: &[char]) -> Option<i32> {
    // Meilleure correspondance gloutonne depuis chaque départ possible.
    let first = *word.first()?;
    (0..text.len())
        .filter(|&i| text[i] == first)
        .filter_map(|start| {
            let (mut score, mut pos, mut previous) = (0, start, None::<usize>);
            for &c in word {
                let found = (pos..text.len()).find(|&i| text[i] == c)?;
                score += 10;
                if previous.is_some_and(|p| p + 1 == found) {
                    score += 15; // lettres consécutives
                }
                if found == 0 || !text[found - 1].is_alphanumeric() {
                    // Début de mot : surtout pour la première lettre (« pnd »).
                    score += if previous.is_none() { 20 } else { 5 };
                }
                if let Some(p) = previous {
                    score -= (found - p - 1).min(10) as i32; // trou
                }
                previous = Some(found);
                pos = found + 1;
            }
            Some(score)
        })
        .max()
}

/// Filtres sur le drapeau proposés à l'utilisateur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlagFilter {
    /// Seulement les photos acceptées.
    Accepted,
    /// Masquer les photos rejetées.
    NotRejected,
    /// Seulement les photos rejetées.
    Rejected,
}

impl FlagFilter {
    /// Les trois filtres.
    pub const ALL: [FlagFilter; 3] = [
        FlagFilter::Accepted,
        FlagFilter::NotRejected,
        FlagFilter::Rejected,
    ];

    /// Drapeaux gardés par ce filtre.
    pub fn flags(self) -> Vec<Flag> {
        match self {
            FlagFilter::Accepted => vec![Flag::Accepted],
            FlagFilter::NotRejected => vec![Flag::None, Flag::Accepted],
            FlagFilter::Rejected => vec![Flag::Rejected],
        }
    }

    /// Nom court (barre de filtres, barre d'état).
    pub fn label(self) -> String {
        match self {
            FlagFilter::Accepted => t!("flag-filter-accepted"),
            FlagFilter::NotRejected => t!("flag-filter-not-rejected"),
            FlagFilter::Rejected => t!("flag-filter-rejected"),
        }
    }
}

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
    /// Filtre de type suivant : tout → RAW → plates → tout.
    FilterCycle,
    /// Filtre : au moins n étoiles (0 : étoiles indifférentes).
    FilterStars(u8),
    /// Filtre sur le drapeau (activer / désactiver).
    FilterFlag(FlagFilter),
    /// Filtre : ajouter ou retirer une couleur.
    FilterColor(Color),
    /// Effacer tous les filtres (type compris).
    FilterClear,
    /// Filtre : masquer les photos jugées floues.
    FilterBlurry,
    /// Filtre : seulement les photos jugées floues.
    FilterOnlyBlurry,
    /// Afficher / masquer la barre de filtres.
    ToggleFilterBar,
    /// Afficher / masquer le bandeau de notation.
    ToggleRatingBar,
    /// Avance automatique à l'image suivante après une note : oui / non.
    ToggleAutoAdvance,
    /// Comparer les images sélectionnées côte à côte.
    Compare,
    /// Afficher / masquer la carte de netteté (1:1).
    TogglePeaking,
    /// Carte de netteté : couleur suivante.
    PeakingColor,
    /// Afficher / masquer le collimateur de mise au point.
    ToggleFocusPoint,
    /// Sélectionner / désélectionner l'image courante.
    SelectToggle,
    /// Mode visuel : la plage suit l'image courante.
    SelectVisual,
    /// Tout sélectionner.
    SelectAll,
    /// Tout désélectionner.
    SelectClear,
    /// Renommer l'image (sa paire et ses sidecars suivent).
    FileRename,
    /// Mettre l'image à la corbeille (après confirmation).
    FileDelete,
    /// Afficher l'image dans le gestionnaire de fichiers.
    FileReveal,
    /// Ouvrir l'image dans l'éditeur externe.
    FileEdit,
    /// Annuler la dernière modification.
    Undo,
    /// Rétablir la dernière modification annulée.
    Redo,
    /// Liste de l'historique (retour à un pas donné).
    History,
    /// Copier les images vers un dossier.
    FileCopy,
    /// Déplacer les images vers un dossier.
    FileMove,
    /// Afficher / masquer le panneau des dossiers.
    ToggleFolderPanel,
    /// Relire les volumes (disque branché) et les dossiers du panneau.
    RefreshFolders,
    /// Plein écran : afficher / masquer l'histogramme.
    ToggleHistogram,
    /// Histogramme : basculer entre IL et linéaire.
    HistogramScale,
    /// Grand histogramme, en boîte au milieu de l'écran.
    HistogramLarge,
    /// Grand histogramme : canal suivant (tous, rouge, vert, bleu).
    HistogramChannel,
    /// Plein écran : afficher / masquer les zones saturées / bouchées.
    ToggleClipping,
    /// Afficher / masquer le panneau des métadonnées.
    ToggleMetadata,
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
    /// Fenêtre de réglages.
    Settings,
    /// Noter : 0 à 5 étoiles.
    Stars(u8),
    /// Ajouter ou retirer une couleur.
    Color(Color),
    /// Accepter, rejeter, ou retirer le drapeau.
    Flag(Flag),
    /// Quitter l'application.
    Quit,
}

impl Action {
    /// Toutes les actions, pour les listes (aide, fichier de raccourcis).
    pub const ALL: [Action; 79] = [
        Action::NavPrev,
        Action::NavNext,
        Action::NavUp,
        Action::NavDown,
        Action::NavFirst,
        Action::NavLast,
        Action::NavPageUp,
        Action::NavPageDown,
        Action::FilterCycle,
        Action::FilterStars(0),
        Action::FilterStars(1),
        Action::FilterStars(2),
        Action::FilterStars(3),
        Action::FilterStars(4),
        Action::FilterStars(5),
        Action::FilterFlag(FlagFilter::Accepted),
        Action::FilterFlag(FlagFilter::NotRejected),
        Action::FilterFlag(FlagFilter::Rejected),
        Action::FilterColor(Color::Red),
        Action::FilterColor(Color::Yellow),
        Action::FilterColor(Color::Green),
        Action::FilterColor(Color::Blue),
        Action::FilterColor(Color::Purple),
        Action::FilterClear,
        Action::FilterBlurry,
        Action::FilterOnlyBlurry,
        Action::ToggleFilterBar,
        Action::ToggleRatingBar,
        Action::ToggleAutoAdvance,
        Action::SelectToggle,
        Action::SelectVisual,
        Action::Compare,
        Action::TogglePeaking,
        Action::PeakingColor,
        Action::ToggleFocusPoint,
        Action::SelectAll,
        Action::SelectClear,
        Action::FileRename,
        Action::FileDelete,
        Action::FileReveal,
        Action::FileEdit,
        Action::FileCopy,
        Action::FileMove,
        Action::Undo,
        Action::Redo,
        Action::History,
        Action::ToggleFolderPanel,
        Action::RefreshFolders,
        Action::ToggleHistogram,
        Action::HistogramScale,
        Action::HistogramLarge,
        Action::HistogramChannel,
        Action::ToggleClipping,
        Action::ToggleMetadata,
        Action::Open,
        Action::ToggleZoom,
        Action::ToggleSource,
        Action::PanLeft,
        Action::PanRight,
        Action::PanUp,
        Action::PanDown,
        Action::Back,
        Action::Help,
        Action::Settings,
        Action::Quit,
        Action::Stars(0),
        Action::Stars(1),
        Action::Stars(2),
        Action::Stars(3),
        Action::Stars(4),
        Action::Stars(5),
        Action::Color(Color::Red),
        Action::Color(Color::Yellow),
        Action::Color(Color::Green),
        Action::Color(Color::Blue),
        Action::Color(Color::Purple),
        Action::Flag(Flag::Accepted),
        Action::Flag(Flag::Rejected),
        Action::Flag(Flag::None),
    ];

    /// Action correspondant à un identifiant (fichier de raccourcis, scripts de capture).
    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    /// Libellé compréhensible (aide, bulles, boutons), dans la langue de l'interface :
    /// identifiant `action-<identifiant de l'action>` des catalogues (`gui/locales`).
    pub fn label(self) -> String {
        let id = format!("action-{}", self.id().replace('.', "-"));
        t!(&id, os = crate::i18n::os())
    }

    /// Thème de l'action (aide).
    pub fn group(self) -> Group {
        use Action::*;
        match self {
            NavPrev | NavNext | NavUp | NavDown | NavFirst | NavLast | NavPageUp | NavPageDown
            | Open | Back => Group::Navigation,
            ToggleZoom | ToggleSource | PanLeft | PanRight | PanUp | PanDown | ToggleHistogram
            | HistogramScale | HistogramLarge | HistogramChannel | ToggleClipping
            | TogglePeaking | PeakingColor | ToggleFocusPoint => Group::View,
            Stars(_) | Color(_) | Flag(_) | ToggleAutoAdvance => Group::Rating,
            FilterCycle | FilterStars(_) | FilterFlag(_) | FilterColor(_) | FilterClear
            | FilterBlurry | FilterOnlyBlurry => Group::Filters,
            ToggleFilterBar | ToggleRatingBar | ToggleFolderPanel | ToggleMetadata
            | RefreshFolders => Group::Panels,
            FileRename | FileDelete | FileReveal | FileEdit | FileCopy | FileMove => Group::Files,
            Undo | Redo | History => Group::Edit,
            SelectToggle | SelectVisual | SelectAll | SelectClear | Compare => Group::Selection,
            Help | Settings | Quit => Group::App,
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
            Action::FilterStars(0) => "core.filter.stars.0",
            Action::FilterStars(1) => "core.filter.stars.1",
            Action::FilterStars(2) => "core.filter.stars.2",
            Action::FilterStars(3) => "core.filter.stars.3",
            Action::FilterStars(4) => "core.filter.stars.4",
            Action::FilterStars(_) => "core.filter.stars.5",
            Action::FilterFlag(FlagFilter::Accepted) => "core.filter.flag.accepted",
            Action::FilterFlag(FlagFilter::NotRejected) => "core.filter.flag.not_rejected",
            Action::FilterFlag(FlagFilter::Rejected) => "core.filter.flag.rejected",
            Action::FilterColor(Color::Red) => "core.filter.color.red",
            Action::FilterColor(Color::Yellow) => "core.filter.color.yellow",
            Action::FilterColor(Color::Green) => "core.filter.color.green",
            Action::FilterColor(Color::Blue) => "core.filter.color.blue",
            Action::FilterColor(Color::Purple) => "core.filter.color.purple",
            Action::FilterClear => "core.filter.clear",
            Action::FilterBlurry => "core.filter.blurry",
            Action::FilterOnlyBlurry => "core.filter.only_blurry",
            Action::ToggleFilterBar => "core.view.filter_bar",
            Action::ToggleRatingBar => "core.view.rating_bar",
            Action::ToggleAutoAdvance => "core.rate.auto_advance",
            Action::Compare => "core.view.compare",
            Action::TogglePeaking => "core.view.peaking",
            Action::PeakingColor => "core.view.peaking_color",
            Action::ToggleFocusPoint => "core.view.focus_point",
            Action::SelectToggle => "core.select.toggle",
            Action::SelectVisual => "core.select.visual",
            Action::SelectAll => "core.select.all",
            Action::SelectClear => "core.select.clear",
            Action::FileRename => "core.file.rename",
            Action::FileDelete => "core.file.delete",
            Action::FileCopy => "core.file.copy",
            Action::Undo => "core.edit.undo",
            Action::Redo => "core.edit.redo",
            Action::History => "core.edit.history",
            Action::FileMove => "core.file.move",
            Action::FileReveal => "core.file.reveal",
            Action::FileEdit => "core.file.edit",
            Action::ToggleFolderPanel => "core.view.folders",
            Action::RefreshFolders => "core.view.refresh_folders",
            Action::ToggleHistogram => "core.view.histogram",
            Action::HistogramScale => "core.view.histogram_scale",
            Action::HistogramLarge => "core.view.histogram_large",
            Action::HistogramChannel => "core.view.histogram_channel",
            Action::ToggleClipping => "core.view.clipping",
            Action::ToggleMetadata => "core.view.metadata",
            Action::Open => "core.view.open",
            Action::ToggleZoom => "core.view.zoom",
            Action::ToggleSource => "core.view.source",
            Action::PanLeft => "core.view.pan_left",
            Action::PanRight => "core.view.pan_right",
            Action::PanUp => "core.view.pan_up",
            Action::PanDown => "core.view.pan_down",
            Action::Back => "core.view.back",
            Action::Help => "core.app.help",
            Action::Settings => "core.app.settings",
            Action::Quit => "core.app.quit",
            Action::Stars(0) => "core.rate.star.0",
            Action::Stars(1) => "core.rate.star.1",
            Action::Stars(2) => "core.rate.star.2",
            Action::Stars(3) => "core.rate.star.3",
            Action::Stars(4) => "core.rate.star.4",
            Action::Stars(_) => "core.rate.star.5",
            Action::Color(Color::Red) => "core.rate.color.red",
            Action::Color(Color::Yellow) => "core.rate.color.yellow",
            Action::Color(Color::Green) => "core.rate.color.green",
            Action::Color(Color::Blue) => "core.rate.color.blue",
            Action::Color(Color::Purple) => "core.rate.color.purple",
            Action::Flag(Flag::Accepted) => "core.flag.accept",
            Action::Flag(Flag::Rejected) => "core.flag.reject",
            Action::Flag(Flag::None) => "core.flag.none",
        }
    }
}

/// Ce qui déclenche un raccourci : une touche, ou un caractère tapé. Les caractères servent
/// aux touches qu'egui ne nomme pas (« ° » de l'AZERTY) ; une touche nommée par egui est
/// toujours une [`Trigger::Key`], pour ne pas réagir deux fois à la même frappe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Touche.
    Key(Key),
    /// Caractère tapé.
    Char(char),
}

impl Trigger {
    /// Vrai pour les touches qui ne tapent aucun texte (Échap, F1 à F12) : elles restent
    /// des raccourcis même pendant la saisie dans un champ.
    pub fn types_no_text(self) -> bool {
        use Key::*;
        matches!(
            self,
            Trigger::Key(Escape | F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12)
        )
    }
}

/// Une combinaison : une touche avec ou sans Maj et Ctrl, ou un caractère. Cmd et Alt sont
/// réservés au système et ne déclenchent rien ici ; Ctrl + flèches aussi, par convention
/// (sur macOS, cela change de bureau).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// Touche ou caractère.
    pub trigger: Trigger,
    /// Maj enfoncée (toujours faux pour un caractère : Maj fait partie du caractère).
    pub shift: bool,
    /// Ctrl enfoncée.
    pub ctrl: bool,
}

impl Chord {
    /// Combinaison d'une touche pressée, `None` si Cmd ou Alt est enfoncé. Sous Linux et
    /// Windows, egui signale Ctrl aussi comme `command` : seul `mac_cmd`, ou `command` sans
    /// Ctrl, désigne Cmd.
    pub fn from_event(key: Key, modifiers: Modifiers) -> Option<Chord> {
        // Exception (décidée le 18/10/2026) : Cmd+Z / Cmd+Maj+Z sur macOS, réflexe de tout
        // utilisateur de Mac, valent Ctrl+Z / Ctrl+Maj+Z (annuler, rétablir).
        if cfg!(target_os = "macos") && modifiers.mac_cmd && !modifiers.alt && key == Key::Z {
            return Some(Chord {
                trigger: Trigger::Key(key),
                shift: modifiers.shift,
                ctrl: true,
            });
        }
        if modifiers.alt || modifiers.mac_cmd || (modifiers.command && !modifiers.ctrl) {
            return None;
        }
        Some(Chord {
            trigger: Trigger::Key(key),
            shift: modifiers.shift,
            ctrl: modifiers.ctrl,
        })
    }

    /// Combinaison d'un caractère tapé.
    pub const fn char(c: char) -> Chord {
        Chord {
            trigger: Trigger::Char(c),
            shift: false,
            ctrl: false,
        }
    }
}

const fn plain(key: Key) -> Chord {
    Chord {
        trigger: Trigger::Key(key),
        shift: false,
        ctrl: false,
    }
}

const fn shifted(key: Key) -> Chord {
    Chord {
        trigger: Trigger::Key(key),
        shift: true,
        ctrl: false,
    }
}

const fn ctrl_shift(key: Key) -> Chord {
    Chord {
        trigger: Trigger::Key(key),
        shift: true,
        ctrl: true,
    }
}

const fn ctrl(key: Key) -> Chord {
    Chord {
        trigger: Trigger::Key(key),
        shift: false,
        ctrl: true,
    }
}

/// Un raccourci d'un profil : une touche, ou une séquence de touches (`g g`).
type Binding = (&'static [Chord], Action);

/// Profil de raccourcis livré par défaut. Dans le fichier de configuration : `"classic"`
/// ou `"vim"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    /// Flèches, Début / Fin, Page préc. / suiv. ; Maj + flèches dans une image zoomée.
    Classic,
    /// hjkl, gg / G, Ctrl+U / Ctrl+D ; Maj + hjkl dans une image zoomée. `u` reste libre
    /// pour l'annulation (à venir), comme dans vim.
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
        (&[ctrl(Key::U)], NavPageUp),
        (&[ctrl(Key::D)], NavPageDown),
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
        // Comme `dd` dans nvim (une confirmation suit).
        (&[plain(Key::D), plain(Key::D)], FileDelete),
    ]
};

/// Sélection multiple, commune aux deux profils : tout au clavier (demande du
/// 17/10/2026) ; `v` comme le mode visuel de vim. Échap désélectionne aussi.
const SELECTION: &[Binding] = {
    use Action::*;
    &[
        (&[plain(Key::Space)], SelectToggle),
        (&[plain(Key::V)], SelectVisual),
        (&[plain(Key::C)], Compare),
        (&[ctrl(Key::A)], SelectAll),
        (
            &[Chord {
                shift: true,
                ..ctrl(Key::A)
            }],
            SelectClear,
        ),
    ]
};

/// Actions sur les fichiers, communes aux deux profils. Supprimer : Suppr, ou la touche
/// « effacer » des claviers Mac (Backspace) ; une confirmation suit toujours.
const FILES: &[Binding] = {
    use Action::*;
    &[
        (&[plain(Key::F2)], FileRename),
        (&[plain(Key::Delete)], FileDelete),
        (&[plain(Key::Backspace)], FileDelete),
        (&[shifted(Key::R)], FileReveal),
        (&[plain(Key::E)], FileEdit),
        // Comme la plupart des applications (Cmd+, sur macOS est pris par le système).
        (&[ctrl(Key::Comma)], Settings),
        // Comme dans vim : `y` copie (« yank »), `x` coupe (déplacer).
        (&[plain(Key::Y)], FileCopy),
        (&[plain(Key::X)], FileMove),
        // Annuler : `u` / `Ctrl+R` comme dans vim, `Ctrl+Z` / `Ctrl+Maj+Z` ailleurs (et
        // `Cmd+Z` / `Cmd+Maj+Z` sur macOS, voir `Chord::from_event`), dans les deux
        // profils.
        (&[plain(Key::U)], Undo),
        (&[ctrl(Key::Z)], Undo),
        (&[ctrl(Key::R)], Redo),
        (&[ctrl_shift(Key::Z)], Redo),
        (&[ctrl(Key::H)], History),
    ]
};

/// Notation, commune aux deux profils, pensée aussi pour l'AZERTY : chiffres avec ou sans Maj
/// (rangée du haut de l'AZERTY avec Maj, pavé numérique sans), violet sur « ° » (AZERTY),
/// « , » et « . » (séparateur décimal du pavé numérique selon le système), drapeaux sur Maj + P / X / U (`u` seul est réservé à
/// l'annulation).
const RATING: &[Binding] = {
    use Action::*;
    &[
        (&[plain(Key::Num0)], Stars(0)),
        (&[shifted(Key::Num0)], Stars(0)),
        (&[plain(Key::Num1)], Stars(1)),
        (&[shifted(Key::Num1)], Stars(1)),
        (&[plain(Key::Num2)], Stars(2)),
        (&[shifted(Key::Num2)], Stars(2)),
        (&[plain(Key::Num3)], Stars(3)),
        (&[shifted(Key::Num3)], Stars(3)),
        (&[plain(Key::Num4)], Stars(4)),
        (&[shifted(Key::Num4)], Stars(4)),
        (&[plain(Key::Num5)], Stars(5)),
        (&[shifted(Key::Num5)], Stars(5)),
        (&[plain(Key::Num6)], Color(tri::xmp::Color::Red)),
        (&[shifted(Key::Num6)], Color(tri::xmp::Color::Red)),
        (&[plain(Key::Num7)], Color(tri::xmp::Color::Yellow)),
        (&[shifted(Key::Num7)], Color(tri::xmp::Color::Yellow)),
        (&[plain(Key::Num8)], Color(tri::xmp::Color::Green)),
        (&[shifted(Key::Num8)], Color(tri::xmp::Color::Green)),
        (&[plain(Key::Num9)], Color(tri::xmp::Color::Blue)),
        (&[shifted(Key::Num9)], Color(tri::xmp::Color::Blue)),
        (&[Chord::char('°')], Color(tri::xmp::Color::Purple)),
        (&[plain(Key::Comma)], Color(tri::xmp::Color::Purple)),
        // Séparateur décimal du pavé numérique : « , » sur macOS, « . » sous Linux Mint.
        (&[plain(Key::Period)], Color(tri::xmp::Color::Purple)),
        (&[shifted(Key::P)], Flag(tri::xmp::Flag::Accepted)),
        (&[shifted(Key::X)], Flag(tri::xmp::Flag::Rejected)),
        (&[shifted(Key::U)], Flag(tri::xmp::Flag::None)),
        (&[shifted(Key::A)], ToggleAutoAdvance),
        (&[shifted(Key::N)], ToggleRatingBar),
        (&[shifted(Key::D)], ToggleFolderPanel),
        (&[plain(Key::F5)], RefreshFolders),
        (&[plain(Key::I)], ToggleHistogram),
        (&[shifted(Key::I)], HistogramScale),
        (&[shifted(Key::E)], HistogramLarge),
        (&[shifted(Key::C)], HistogramChannel),
        (&[plain(Key::O)], ToggleClipping),
        (&[plain(Key::P)], TogglePeaking),
        (&[ctrl(Key::P)], PeakingColor),
        (&[plain(Key::M)], ToggleFocusPoint),
        (&[shifted(Key::M)], ToggleMetadata),
    ]
};

/// Filtres, communs aux deux profils : préfixe `f` (choix du 17/10/2026), suivi de la
/// touche de notation correspondante ; `f f` change le type, `f Échap` efface tout.
const FILTERS: &[Binding] = {
    use Action::*;
    use tri::xmp::Color as C;
    const F: Chord = plain(Key::F);
    &[
        (&[F, F], FilterCycle),
        (&[F, plain(Key::Num0)], FilterStars(0)),
        (&[F, shifted(Key::Num0)], FilterStars(0)),
        (&[F, plain(Key::Num1)], FilterStars(1)),
        (&[F, shifted(Key::Num1)], FilterStars(1)),
        (&[F, plain(Key::Num2)], FilterStars(2)),
        (&[F, shifted(Key::Num2)], FilterStars(2)),
        (&[F, plain(Key::Num3)], FilterStars(3)),
        (&[F, shifted(Key::Num3)], FilterStars(3)),
        (&[F, plain(Key::Num4)], FilterStars(4)),
        (&[F, shifted(Key::Num4)], FilterStars(4)),
        (&[F, plain(Key::Num5)], FilterStars(5)),
        (&[F, shifted(Key::Num5)], FilterStars(5)),
        (&[F, plain(Key::P)], FilterFlag(FlagFilter::Accepted)),
        (&[F, shifted(Key::P)], FilterFlag(FlagFilter::Accepted)),
        (&[F, plain(Key::U)], FilterFlag(FlagFilter::NotRejected)),
        (&[F, shifted(Key::U)], FilterFlag(FlagFilter::NotRejected)),
        (&[F, plain(Key::X)], FilterFlag(FlagFilter::Rejected)),
        (&[F, shifted(Key::X)], FilterFlag(FlagFilter::Rejected)),
        (&[F, plain(Key::Num6)], FilterColor(C::Red)),
        (&[F, shifted(Key::Num6)], FilterColor(C::Red)),
        (&[F, plain(Key::Num7)], FilterColor(C::Yellow)),
        (&[F, shifted(Key::Num7)], FilterColor(C::Yellow)),
        (&[F, plain(Key::Num8)], FilterColor(C::Green)),
        (&[F, shifted(Key::Num8)], FilterColor(C::Green)),
        (&[F, plain(Key::Num9)], FilterColor(C::Blue)),
        (&[F, shifted(Key::Num9)], FilterColor(C::Blue)),
        (&[F, Chord::char('°')], FilterColor(C::Purple)),
        (&[F, plain(Key::Comma)], FilterColor(C::Purple)),
        (&[F, plain(Key::Period)], FilterColor(C::Purple)),
        (&[F, plain(Key::Escape)], FilterClear),
        (&[F, plain(Key::B)], FilterBlurry),
        (&[F, shifted(Key::B)], FilterOnlyBlurry),
        (&[shifted(Key::F)], ToggleFilterBar),
    ]
};

impl Profile {
    /// Raccourcis du profil, notation et filtres compris.
    pub fn bindings(self) -> impl Iterator<Item = &'static Binding> {
        let own = match self {
            Profile::Classic => CLASSIC,
            Profile::Vim => VIM,
        };
        own.iter()
            .chain(RATING)
            .chain(FILTERS)
            .chain(SELECTION)
            .chain(FILES)
    }
}

/// Table raccourci → action.
#[derive(Clone, Debug)]
pub struct Keymap {
    bindings: Vec<(Vec<Chord>, Action)>,
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
        let mut bindings: Vec<(Vec<Chord>, Action)> = Vec::new();
        for profile in profiles {
            for &(keys, action) in profile.bindings() {
                if !bindings.iter().any(|(k, _)| k == keys) {
                    bindings.push((keys.to_vec(), action));
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

    /// Applique la section `[keys]` du fichier de configuration : pour chaque action citée
    /// (par son identifiant), ses raccourcis **remplacent** ceux des profils (liste vide :
    /// plus aucun). Renvoie aussi les problèmes à signaler : action ou touche inconnue,
    /// raccourci attribué à deux actions (le premier dans l'ordre du fichier, trié par
    /// identifiant, est gardé ; un raccourci du fichier l'emporte sur celui d'un profil),
    /// raccourci qui ne peut pas se déclencher car il commence une séquence plus longue.
    pub fn with_overrides(
        mut self,
        overrides: &BTreeMap<String, Vec<String>>,
    ) -> (Keymap, Vec<String>) {
        let mut warnings = Vec::new();
        let mut custom: Vec<(Vec<Chord>, Action, &str)> = Vec::new();
        let mut replaced: Vec<Action> = Vec::new();
        for (id, specs) in overrides {
            let Some(action) = Action::from_id(id) else {
                warnings.push(format!("[keys] : action inconnue « {id} »"));
                continue;
            };
            replaced.push(action);
            for spec in specs {
                match parse_keys(spec) {
                    Ok(keys) => custom.push((keys, action, spec)),
                    Err(e) => warnings.push(t!("keys-error", id = id, error = e)),
                }
            }
        }
        self.bindings.retain(|(_, a)| !replaced.contains(a));
        let mut added: Vec<(Vec<Chord>, Action)> = Vec::new();
        for (keys, action, spec) in custom {
            if let Some((_, other)) = added.iter().find(|(k, _)| *k == keys) {
                if *other != action {
                    warnings.push(t!(
                        "keys-conflict",
                        spec = spec,
                        first = other.id(),
                        second = action.id()
                    ));
                }
                continue;
            }
            if let Some(i) = self.bindings.iter().position(|(k, _)| *k == keys) {
                warnings.push(t!(
                    "keys-replaces",
                    spec = spec,
                    action = action.id(),
                    other = self.bindings[i].1.id()
                ));
                self.bindings.remove(i);
            }
            added.push((keys, action));
        }
        self.bindings.extend(added);
        for (keys, action) in &self.bindings {
            if let Some((longer, _)) = self
                .bindings
                .iter()
                .find(|(l, _)| l.len() > keys.len() && l.starts_with(keys))
            {
                warnings.push(t!(
                    "keys-shadowed",
                    keys = sequence_label(keys),
                    action = action.id(),
                    longer = sequence_label(longer)
                ));
            }
        }
        (self, warnings)
    }

    /// Combinaisons tapées dans les événements d'une image d'egui, dans l'ordre.
    ///
    /// Seuls les caractères liés à un raccourci (« ° ») sont gardés : les autres
    /// accompagnent une touche déjà traitée, et interrompraient une séquence. Une touche
    /// suivie d'un tel caractère est ignorée : « ° » arrive aussi comme Maj + `Minus`
    /// (touche physique, egui ne connaissant pas « ° »), qui interromprait `f °`. egui-winit
    /// livre la touche et son caractère dans la même image (même événement winit).
    pub fn chords(&self, events: &[Event]) -> Vec<Chord> {
        let bound_char = |e: &Event| match e {
            Event::Text(text) => {
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if self.uses_char(c) => Some(c),
                    _ => None,
                }
            }
            _ => None,
        };
        // Ce qui suit la touche `n` : premier texte ou appui de touche (en sautant
        // relâchements et changements de modificateurs).
        let next_is_bound_char = |n: usize| {
            events[n + 1..]
                .iter()
                .find(|e| matches!(e, Event::Text(_) | Event::Key { pressed: true, .. }))
                .and_then(bound_char)
                .is_some()
        };
        events
            .iter()
            .enumerate()
            .filter_map(|(n, e)| match e {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !next_is_bound_char(n) => Chord::from_event(*key, *modifiers),
                Event::Text(_) => bound_char(e).map(Chord::char),
                _ => None,
            })
            .collect()
    }

    /// Vrai si le caractère `c` sert dans un raccourci : seuls ceux-là sont passés au
    /// [`Sequencer`] (les autres caractères tapés interrompraient une séquence).
    pub fn uses_char(&self, c: char) -> bool {
        self.bindings
            .iter()
            .any(|(k, _)| k.contains(&Chord::char(c)))
    }

    fn lookup(&self, keys: &[Chord]) -> Lookup {
        if self
            .bindings
            .iter()
            .any(|(k, _)| k.len() > keys.len() && k.starts_with(keys))
        {
            return Lookup::Prefix;
        }
        match self.bindings.iter().find(|(k, _)| k == keys) {
            Some(&(_, action)) => Lookup::Action(action),
            None => Lookup::None,
        }
    }

    /// Raccourcis associés à une action, lisibles (« Flèche gauche », « Maj+g », « gg »...),
    /// pour l'aide. Une touche liée avec et sans Maj n'apparaît qu'une fois (« 3 », pas
    /// « 3, Maj+3 »).
    pub fn keys_for(&self, action: Action) -> Vec<String> {
        let mine: Vec<&Vec<Chord>> = self
            .bindings
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(k, _)| k)
            .collect();
        let mut keys: Vec<String> = Vec::new();
        for k in &mine {
            let unshifted: Vec<Chord> = k.iter().map(|c| Chord { shift: false, ..*c }).collect();
            let shift_variant = k.iter().any(|c| c.shift) && mine.contains(&&unshifted);
            let label = sequence_label(k);
            if !shift_variant && !keys.contains(&label) {
                keys.push(label);
            }
        }
        keys
    }
}

/// Touche désignée par `name` : nom affiché par l'aide (« Flèche gauche », « Échap »,
/// « g »), ou nom d'egui (« ArrowLeft », « Escape », « PageUp », « 1 »...), sans tenir
/// compte de la casse.
fn key_named(name: &str) -> Option<Key> {
    let lower = name.to_lowercase();
    // Noms de toutes les langues : une configuration reste valable si la langue change.
    Key::ALL
        .iter()
        .copied()
        .find(|&k| {
            Language::ALL
                .iter()
                .any(|&l| key_label_in(l, k).to_lowercase() == lower)
        })
        .or_else(|| Key::from_name(name))
        .or_else(|| {
            Key::ALL
                .iter()
                .copied()
                .find(|k| k.name().to_lowercase() == lower)
        })
}

/// Retire le préfixe `prefix` (« Maj+ ») de `s`, sans tenir compte de la casse.
fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    // Un « + » seul est une touche, pas un modificateur vide.
    (s.len() > prefix.len() && s.get(..prefix.len())?.eq_ignore_ascii_case(prefix))
        .then(|| &s[prefix.len()..])
}

/// Une combinaison écrite dans le fichier de configuration : « Maj+P », « Ctrl+u »,
/// « Flèche gauche », « PageUp », « ° ». Une majuscule seule vaut Maj + la lettre (« G »,
/// comme dans vim). Un caractère qui n'est pas une touche connue d'egui devient un
/// [`Trigger::Char`] (sans modificateur).
fn parse_chord(text: &str) -> Result<Chord, String> {
    let (mut rest, mut shift, mut ctrl) = (text, false, false);
    // « Maj+ », « Shift+ » : préfixes de toutes les langues.
    let shift_prefixes: Vec<String> = Language::ALL
        .iter()
        .map(|&l| {
            crate::i18n::ensure();
            tri::i18n::translate_in(l, "key-shift-prefix", None)
        })
        .chain(["Shift+".to_string()])
        .collect();
    loop {
        if let Some(r) = shift_prefixes
            .iter()
            .find_map(|p| strip_prefix_ignore_case(rest, p))
        {
            (rest, shift) = (r, true);
        } else if let Some(r) = strip_prefix_ignore_case(rest, "Ctrl+") {
            (rest, ctrl) = (r, true);
        } else {
            break;
        }
    }
    let mut chars = rest.chars();
    let single = match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    };
    if single.is_some_and(|c| c.is_ascii_uppercase()) {
        shift = true;
    }
    match key_named(rest) {
        Some(key) => Ok(Chord {
            trigger: Trigger::Key(key),
            shift,
            ctrl,
        }),
        None => match single {
            Some(c) if !shift && !ctrl => Ok(Chord::char(c)),
            _ => Err(t!("keys-unknown-key", text = text)),
        },
    }
}

/// Un raccourci écrit dans le fichier de configuration : une combinaison, ou une séquence
/// séparée par des espaces (« g g »). Les noms en deux mots (« Flèche gauche ») sont
/// reconnus.
pub fn parse_keys(text: &str) -> Result<Vec<Chord>, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Err(t!("keys-empty"));
    }
    let mut chords = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if i + 1 < words.len()
            && let Ok(chord) = parse_chord(&format!("{} {}", words[i], words[i + 1]))
        {
            chords.push(chord);
            i += 2;
            continue;
        }
        chords.push(parse_chord(words[i])?);
        i += 1;
    }
    Ok(chords)
}

/// Nom lisible d'une combinaison (« Maj+Flèche gauche », « Ctrl+u », « g », « ° »).
fn chord_label(c: &Chord) -> String {
    let name = match c.trigger {
        Trigger::Key(key) => key_label(key),
        Trigger::Char(ch) => ch.to_string(),
    };
    let mut label = String::new();
    if c.ctrl {
        label.push_str("Ctrl+");
    }
    if c.shift && c.trigger != Trigger::Key(Key::Questionmark) {
        label.push_str(&t!("key-shift-prefix"));
    }
    label + &name
}

/// Nom lisible d'une séquence : « gg » comme on la tape quand ce ne sont que des lettres,
/// sinon « Échap puis g ».
fn sequence_label(keys: &[Chord]) -> String {
    let labels: Vec<String> = keys.iter().map(chord_label).collect();
    if labels.iter().all(|l| l.chars().count() == 1) {
        labels.concat()
    } else {
        labels.join(&t!("key-sequence-then"))
    }
}

/// Nom lisible d'une touche, dans la langue de l'interface. Volontairement indépendant des
/// noms d'egui et sans symboles : « ⏴ » et même « ← » manquent dans la police par défaut
/// d'egui (les tests d'interface le vérifient), et les polices du système ne sont pas
/// garanties.
pub fn key_label(key: Key) -> String {
    key_label_in(tri::i18n::language(), key)
}

/// [`key_label`] dans la langue `language`.
fn key_label_in(language: Language, key: Key) -> String {
    let id = match key {
        Key::ArrowLeft => Some("key-arrow-left"),
        Key::ArrowRight => Some("key-arrow-right"),
        Key::ArrowUp => Some("key-arrow-up"),
        Key::ArrowDown => Some("key-arrow-down"),
        Key::Home => Some("key-home"),
        Key::End => Some("key-end"),
        Key::PageUp => Some("key-page-up"),
        Key::PageDown => Some("key-page-down"),
        Key::Escape => Some("key-escape"),
        Key::Enter => Some("key-enter"),
        Key::Space => Some("key-space"),
        Key::Delete => Some("key-delete"),
        Key::Backspace => Some("key-backspace"),
        _ => None,
    };
    if let Some(id) = id {
        crate::i18n::ensure();
        return tri::i18n::translate_in(language, id, None);
    }
    match key {
        Key::Questionmark => "?".into(),
        Key::Comma => ",".into(),
        Key::Period => ".".into(),
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
        let since = self.since.take();
        match keymap.lookup(&keys) {
            Lookup::Prefix => {
                self.pending = keys;
                self.since = Some(now);
                None
            }
            Lookup::Action(action) => Some(action),
            // Touche liée à aucun raccourci, même seule, au milieu d'une séquence : ignorée,
            // la séquence reste en attente. « ° » arrive sur un AZERTY comme Maj + `Minus`
            // (touche physique, inconnue ici) *puis*, parfois dans une image suivante, comme
            // caractère : sans cela, la touche annulait `f` et le caractère notait au lieu
            // de filtrer (retour du 17/10/2026, macOS et Linux).
            Lookup::None if keys.len() > 1 && keymap.lookup(&[chord]) == Lookup::None => {
                keys.pop();
                self.pending = keys;
                self.since = since;
                None
            }
            // Séquence interrompue par une touche qui a un sens seule : elle compte seule.
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
        assert_eq!(km.action(Key::Q, Modifiers::MAC_CMD), None);
        assert_eq!(km.action(Key::L, Modifiers::CTRL), None, "Ctrl+L : libre");
        assert_eq!(km.action(Key::L, Modifiers::ALT), None);
        assert_eq!(Chord::from_event(Key::ArrowLeft, Modifiers::ALT), None);
        // Cmd+Z : annuler sur macOS (exception), rien ailleurs.
        let cmd_z = Chord::from_event(Key::Z, Modifiers::MAC_CMD | Modifiers::COMMAND);
        if cfg!(target_os = "macos") {
            assert_eq!(cmd_z, Chord::from_event(Key::Z, Modifiers::CTRL));
        } else {
            assert_eq!(cmd_z, None);
        }
    }

    #[test]
    fn vim_pages_with_ctrl_u_and_ctrl_d_and_u_undoes() {
        let km = Keymap::default();
        // Sous Linux et Windows, egui signale Ctrl aussi comme `command`.
        let linux_ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(km.action(Key::U, Modifiers::CTRL), Some(Action::NavPageUp));
        assert_eq!(km.action(Key::D, linux_ctrl), Some(Action::NavPageDown));
        assert_eq!(
            km.action(Key::U, Modifiers::NONE),
            Some(Action::Undo),
            "comme dans vim"
        );
        assert_eq!(km.action(Key::R, linux_ctrl), Some(Action::Redo));
        assert_eq!(km.action(Key::D, Modifiers::NONE), None);
        assert_eq!(km.keys_for(Action::NavPageUp), ["Page préc.", "Ctrl+u"]);
    }

    #[test]
    fn rating_keys_work_with_or_without_shift() {
        let km = Keymap::default();
        for (key, action) in [
            (Key::Num0, Action::Stars(0)),
            (Key::Num3, Action::Stars(3)),
            (Key::Num5, Action::Stars(5)),
            (Key::Num6, Action::Color(Color::Red)),
            (Key::Num9, Action::Color(Color::Blue)),
        ] {
            assert_eq!(km.action(key, Modifiers::NONE), Some(action), "{key:?}");
            assert_eq!(
                km.action(key, Modifiers::SHIFT),
                Some(action),
                "Maj+{key:?}"
            );
        }
        assert_eq!(
            km.action(Key::Comma, Modifiers::NONE),
            Some(Action::Color(Color::Purple))
        );
        assert_eq!(
            km.action(Key::P, Modifiers::SHIFT),
            Some(Action::Flag(Flag::Accepted))
        );
        assert_eq!(
            km.action(Key::X, Modifiers::SHIFT),
            Some(Action::Flag(Flag::Rejected))
        );
        assert_eq!(
            km.action(Key::U, Modifiers::SHIFT),
            Some(Action::Flag(Flag::None))
        );
        assert_eq!(
            km.keys_for(Action::Stars(3)),
            ["3"],
            "Maj+3 implicite dans l'aide"
        );
        assert_eq!(
            km.action(Key::Period, Modifiers::NONE),
            Some(Action::Color(Color::Purple)),
            "pavé numérique sous Linux"
        );
        assert_eq!(km.keys_for(Action::Color(Color::Purple)), ["°", ",", "."]);
    }

    #[test]
    fn a_bound_character_triggers_its_action() {
        let km = Keymap::default();
        assert!(km.uses_char('°'));
        assert!(!km.uses_char('g'), "g est une touche, pas un caractère");
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        assert_eq!(
            seq.feed(&km, Chord::char('°'), Instant::now()),
            Some(Action::Color(Color::Purple))
        );
    }

    #[test]
    fn profiles_cover_every_action_and_have_no_internal_conflict() {
        for profile in [Profile::Classic, Profile::Vim] {
            let b: Vec<&Binding> = profile.bindings().collect();
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
        let mut labels: Vec<String> = Action::ALL.iter().map(|a| a.label()).collect();
        assert!(labels.iter().all(|l| !l.contains('.') || l.contains(' ')));
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), Action::ALL.len());
    }

    #[test]
    fn first_profile_wins_on_conflict() {
        let km = Keymap::from_profiles(&[Profile::Vim, Profile::Classic]);
        assert_eq!(km.action(Key::Z, Modifiers::NONE), Some(Action::ToggleZoom));
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
    fn an_unbound_key_does_not_break_a_sequence() {
        let km = Keymap::default();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        // « f ° » sur un AZERTY, touche et caractère dans deux images différentes.
        assert_eq!(seq.feed(&km, plain(Key::F), t), None);
        assert_eq!(
            seq.feed(&km, shifted(Key::Minus), t),
            None,
            "touche inconnue ignorée"
        );
        assert!(seq.is_pending(t), "f toujours en attente");
        assert_eq!(
            seq.feed(&km, Chord::char('°'), t),
            Some(Action::FilterColor(Color::Purple))
        );
        // Le délai court toujours depuis f : la touche ignorée ne le prolonge pas.
        seq.feed(&km, plain(Key::F), t);
        seq.feed(&km, shifted(Key::Minus), t + Duration::from_millis(900));
        assert!(!seq.is_pending(t + Duration::from_millis(1100)));
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

    fn chord(key: Key, shift: bool, ctrl: bool) -> Chord {
        Chord {
            trigger: Trigger::Key(key),
            shift,
            ctrl,
        }
    }

    #[test]
    fn key_specs_are_parsed() {
        let ok = |s: &str| parse_keys(s).unwrap();
        assert_eq!(ok("p"), [plain(Key::P)]);
        assert_eq!(ok("Maj+P"), [shifted(Key::P)]);
        assert_eq!(ok("shift+p"), [shifted(Key::P)]);
        assert_eq!(
            ok("G"),
            [shifted(Key::G)],
            "majuscule = Maj, comme dans vim"
        );
        assert_eq!(ok("Ctrl+u"), [ctrl(Key::U)]);
        assert_eq!(ok("Ctrl+Maj+u"), [chord(Key::U, true, true)]);
        assert_eq!(ok("g g"), [plain(Key::G), plain(Key::G)]);
        assert_eq!(ok("Flèche gauche"), [plain(Key::ArrowLeft)]);
        assert_eq!(ok("Maj+Flèche gauche"), [shifted(Key::ArrowLeft)]);
        assert_eq!(ok("ArrowLeft"), [plain(Key::ArrowLeft)]);
        assert_eq!(ok("Échap"), [plain(Key::Escape)]);
        assert_eq!(ok("Escape g"), [plain(Key::Escape), plain(Key::G)]);
        assert_eq!(ok("Page préc."), [plain(Key::PageUp)]);
        assert_eq!(ok("3"), [plain(Key::Num3)]);
        assert_eq!(ok(","), [plain(Key::Comma)]);
        assert_eq!(ok("+"), [plain(Key::Plus)]);
        assert_eq!(ok("°"), [Chord::char('°')]);
        assert!(parse_keys("").is_err());
        assert!(parse_keys("Maj+°").is_err(), "Maj fait partie du caractère");
        assert!(parse_keys("Hyper").unwrap_err().contains("Hyper"));
    }

    #[test]
    fn every_help_label_reads_back_as_the_same_keys() {
        let km = Keymap::default();
        for (keys, _) in &km.bindings {
            // « ? » s'affiche sans Maj : selon la disposition, il vient avec ou sans.
            if keys == &[shifted(Key::Questionmark)] {
                continue;
            }
            // « gg » s'écrit « g g » dans le fichier.
            let label = sequence_label(keys);
            let spec = if keys.len() > 1 && label.chars().count() == keys.len() {
                label
                    .chars()
                    .map(String::from)
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                label.replace(" puis ", " ")
            };
            assert_eq!(&parse_keys(&spec).unwrap(), keys, "« {spec} »");
        }
    }

    fn overrides(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(id, keys)| (id.to_string(), keys.iter().map(|k| k.to_string()).collect()))
            .collect()
    }

    #[test]
    fn config_keys_replace_the_profile_keys_of_an_action() {
        let (km, warnings) =
            Keymap::default().with_overrides(&overrides(&[("core.flag.accept", &["a"])]));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            km.action(Key::A, Modifiers::NONE),
            Some(Action::Flag(Flag::Accepted))
        );
        assert_eq!(km.action(Key::P, Modifiers::SHIFT), None, "Maj+P remplacé");
        assert_eq!(km.keys_for(Action::Flag(Flag::Accepted)), ["a"]);

        let (km, _) = Keymap::default().with_overrides(&overrides(&[("core.app.quit", &[])]));
        assert_eq!(
            km.action(Key::Q, Modifiers::NONE),
            None,
            "liste vide : plus de touche"
        );
    }

    #[test]
    fn config_problems_are_reported() {
        let (km, warnings) = Keymap::default().with_overrides(&overrides(&[
            ("core.nope", &["x"]),
            ("core.flag.accept", &["Hyper", "b"]),
            ("core.rate.star.1", &["Maj+P"]),
        ]));
        let all = warnings.join("\n");
        assert!(all.contains("core.nope"), "{all}");
        assert!(all.contains("Hyper"), "{all}");
        // « b » était la bascule JPEG / RAW : le fichier l'emporte, et le dit.
        assert!(all.contains("core.view.source"), "{all}");
        assert_eq!(
            km.action(Key::B, Modifiers::NONE),
            Some(Action::Flag(Flag::Accepted))
        );
        assert_eq!(km.action(Key::P, Modifiers::SHIFT), Some(Action::Stars(1)));
    }

    #[test]
    fn two_actions_on_the_same_key_keep_the_first() {
        let (km, warnings) = Keymap::default().with_overrides(&overrides(&[
            ("core.flag.accept", &["a"]),
            ("core.flag.reject", &["a"]),
        ]));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(
            km.action(Key::A, Modifiers::NONE),
            Some(Action::Flag(Flag::Accepted))
        );
    }

    #[test]
    fn a_key_hidden_by_a_sequence_is_reported() {
        let (_, warnings) =
            Keymap::default().with_overrides(&overrides(&[("core.flag.accept", &["g"])]));
        assert!(
            warnings.iter().any(|w| w.contains("gg")),
            "g ne peut plus se déclencher : {warnings:?}"
        );
    }

    fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn a_character_replaces_the_key_that_typed_it() {
        let km = Keymap::default();
        // « f ° » sur un AZERTY : f, puis Maj + touche physique Minus et le caractère °.
        let events = [
            key_event(Key::F, true, Modifiers::NONE),
            Event::Text("f".into()),
            key_event(Key::F, false, Modifiers::NONE),
            key_event(Key::Minus, true, Modifiers::SHIFT),
            Event::Text("°".into()),
            key_event(Key::Minus, false, Modifiers::SHIFT),
        ];
        let chords = km.chords(&events);
        assert_eq!(chords, [plain(Key::F), Chord::char('°')]);
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let now = Instant::now();
        let actions: Vec<Action> = chords
            .into_iter()
            .filter_map(|c| seq.feed(&km, c, now))
            .collect();
        assert_eq!(actions, [Action::FilterColor(Color::Purple)]);
    }

    #[test]
    fn every_action_has_a_group_and_groups_cover_the_help() {
        for g in Group::ALL {
            assert!(Action::ALL.iter().any(|a| a.group() == g), "{g:?} vide");
        }
    }

    #[test]
    fn fuzzy_search_like_fzf() {
        let s = |q: &str, t: &str| fuzzy_score(q, t);
        assert!(s("pnd", "Afficher / masquer le panneau des dossiers").is_some());
        assert!(
            s("dossiers panneau", "le panneau des dossiers").is_some(),
            "mots en désordre"
        );
        assert!(s("etoile", "3 étoiles").is_some(), "accents ignorés");
        assert!(s("ETOILE", "3 étoiles").is_some(), "casse ignorée");
        assert!(s("xyz", "3 étoiles").is_none());
        assert!(
            s("dossiers zzz", "le panneau des dossiers").is_none(),
            "tous les mots"
        );
        assert_eq!(s("", "n'importe quoi"), Some(0));
        // Lettres consécutives et débuts de mots passent devant les correspondances éparses.
        assert!(s("zoom", "Zoom 100 % / ajusté") > s("zoom", "z o o m éparpillé"));
        assert!(s("fil", "Filtre de type") > s("fil", "Afficher le profil"));
    }

    #[test]
    fn key_names_are_read_in_every_language() {
        assert_eq!(parse_keys("Shift+P"), parse_keys("Maj+P"));
        assert_eq!(parse_keys("Left arrow"), parse_keys("Flèche gauche"));
        assert_eq!(parse_keys("Esc g"), parse_keys("Échap g"));
        assert_eq!(key_label_in(Language::En, Key::ArrowLeft), "Left arrow");
        assert_eq!(key_label_in(Language::Fr, Key::ArrowLeft), "Flèche gauche");
    }
}

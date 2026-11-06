//! Palette de commandes (06/11/2026, refonte de l'aide ; comme Ctrl+Maj+P dans VS Code,
//! `:` dans vim) : on tape quelques lettres, la meilleure action ou le meilleur réglage est
//! en tête, Entrée le lance (une action) ou l'ouvre dans la fenêtre de réglages (un
//! réglage). Même recherche que l'aide (`tri::fuzzy` : toutes les langues, fautes de
//! frappe, mots-clés). Ce module choisit et range ; l'affichage est dans `app.rs`.

use crate::actions::{Action, Keymap};
use crate::cheatsheet::{self, Place};
use crate::settings_window::SettingRef;

/// Ce qu'une ligne de la palette fait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// Lancer une action.
    Action(Action),
    /// Ouvrir la fenêtre de réglages sur un réglage (section, clé).
    Setting(String, String),
}

/// Une ligne de la palette.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Ce qu'elle fait.
    pub item: Item,
    /// Titre (libellé de l'action ; « Zoom › max » pour un réglage).
    pub title: String,
    /// Touches de l'action (vide : aucune, ou un réglage) : on apprend les raccourcis en
    /// passant.
    pub keys: String,
    /// Précision en dessous (description d'un réglage), vide sinon.
    pub detail: String,
}

/// Ce que la palette cherche, d'après le préfixe tapé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Only {
    /// Actions et réglages.
    All,
    /// Les actions seulement (`>tri`).
    Actions,
    /// Les réglages seulement (`?tri`).
    Settings,
}

/// Préfixes de la palette (`[gui.dialogs] palette_actions_prefix`,
/// `palette_settings_prefix`), `None` : aucun.
#[derive(Clone, Copy, Debug)]
pub struct Prefixes {
    /// Devant une recherche d'actions seulement (`>`, comme dans VS Code).
    pub actions: Option<char>,
    /// Devant une recherche de réglages seulement (`?`).
    pub settings: Option<char>,
}

/// Ce que cherche `query` et le reste de la recherche, sans son préfixe (idée de
/// l'utilisateur, 06/11/2026 : `>tri` les actions, comme VS Code, `?tri` les réglages).
pub fn split_prefix(query: &str, prefixes: Prefixes) -> (Only, &str) {
    let query = query.trim_start();
    for (prefix, only) in [
        (prefixes.actions, Only::Actions),
        (prefixes.settings, Only::Settings),
    ] {
        if let Some(prefix) = prefix
            && let Some(rest) = query.strip_prefix(prefix)
        {
            return (only, rest);
        }
    }
    (Only::All, query)
}

/// Lignes de la palette pour `query`, à l'endroit `place` : les actions qui y ont un sens
/// ([`cheatsheet::applies`], sans la palette elle-même) et les réglages `settings`, les
/// meilleurs d'abord (à égalité, une action avant un réglage, puis le titre le plus court),
/// `max` au plus ; sans celles que `usable` refuse (grisées ici). Un préfixe ([`split_prefix`]) limite aux actions ou aux réglages ; seul,
/// il les liste tous, dans l'ordre de l'aide ou du fichier (pour parcourir). Requête vide :
/// rien (la palette invite à taper).
pub fn search(
    keymap: &Keymap,
    usable: &dyn Fn(Action) -> bool,
    query: &str,
    prefixes: Prefixes,
    place: Place,
    settings: &[SettingRef],
    max: usize,
) -> Vec<Hit> {
    let (only, query) = split_prefix(query, prefixes);
    let browsing = query.trim().is_empty();
    if browsing && only == Only::All {
        return Vec::new();
    }
    // Une touche tapée (« ctrl f ») : les actions qui lui sont liées, seulement.
    let typed = keymap.actions_for_typed_keys(query);
    let mut found: Vec<((Hit, u8), tri::fuzzy::Match)> = Vec::new();
    for &action in Action::ALL.iter().filter(|_| only != Only::Settings) {
        if action == Action::Palette
            || !action.available()
            || !cheatsheet::applies(action, place)
            || !usable(action)
        {
            continue;
        }
        let keys = keymap.keys_for(action).join(" / ");
        let title = action.label();
        let mut fields = vec![title.clone(), keys.clone(), action.id().to_string()];
        fields.extend(action.search_texts());
        let fields: Vec<&str> = fields.iter().map(String::as_str).collect();
        let found_here = match &typed {
            Some(typed) => typed.contains(&action).then_some(tri::fuzzy::Match {
                exact: true,
                close: true,
                score: 0,
            }),
            None => tri::fuzzy::find(query, &fields),
        };
        if let Some(m) = found_here {
            let hit = Hit {
                item: Item::Action(action),
                title,
                keys,
                detail: String::new(),
            };
            found.push(((hit, 1), m));
        }
    }
    for s in settings
        .iter()
        .filter(|_| typed.is_none() && only != Only::Actions)
    {
        let id = format!("{}.{}", s.section, s.key);
        let keywords = crate::actions::keywords(&id);
        let section_keywords = crate::actions::keywords(&s.section);
        let fields = [
            s.key.as_str(),
            &id,
            &s.section_label,
            &s.doc,
            &keywords,
            &section_keywords,
        ];
        if let Some(m) = tri::fuzzy::find(query, &fields) {
            let hit = Hit {
                item: Item::Setting(s.section.clone(), s.key.clone()),
                title: format!("{} › {}", s.section_label, s.key),
                keys: String::new(),
                detail: s.doc.clone(),
            };
            found.push(((hit, 0), m));
        }
    }
    if browsing {
        // Tout parcourir : dans l'ordre (thèmes de l'aide, fichier d'exemple).
        found.sort_by_key(|((hit, _), _)| match hit.item {
            Item::Action(a) => crate::actions::Group::ALL
                .iter()
                .position(|g| *g == a.group()),
            Item::Setting(..) => None,
        });
        return found.into_iter().map(|((hit, _), _)| hit).collect();
    }
    let mut found = tri::fuzzy::relevant(found);
    // Trouvée dans son titre d'abord (« rendus vignette » : l'action « Rendu des
    // vignettes » avant un réglage qui n'a ces mots que dans sa description).
    found.sort_by_cached_key(|((hit, kind), m)| {
        // Des mots du titre, pas des lettres éparpillées (« crop » dans « Recadrage ›
        // shade_opacity »).
        let in_title = tri::fuzzy::find(query, &[&hit.title])
            .filter(|t| t.close)
            .map(|t| t.exact);
        (
            std::cmp::Reverse(in_title),
            std::cmp::Reverse(*m),
            std::cmp::Reverse(*kind),
            hit.title.chars().count(),
        )
    });
    found
        .into_iter()
        .take(max)
        .map(|((hit, _), _)| hit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keymap() -> Keymap {
        Keymap::default().with_leader("Ctrl+b").unwrap()
    }

    const PREFIXES: Prefixes = Prefixes {
        actions: Some('>'),
        settings: Some('?'),
    };

    fn settings() -> Vec<SettingRef> {
        crate::settings_window::settings_list(&[])
    }

    #[test]
    fn the_best_action_comes_first() {
        let hits = search(
            &keymap(),
            &|_| true,
            "recadrer",
            PREFIXES,
            Place::View,
            &settings(),
            20,
        );
        assert_eq!(hits[0].item, Item::Action(Action::Crop));
        assert!(!hits[0].keys.is_empty(), "les touches sont montrées");
        // Sans faute ni accent, en anglais.
        let hits = search(
            &keymap(),
            &|_| true,
            "crop",
            PREFIXES,
            Place::View,
            &settings(),
            20,
        );
        assert_eq!(hits[0].item, Item::Action(Action::Crop));
    }

    #[test]
    fn settings_are_found_too() {
        let hits = search(
            &keymap(),
            &|_| true,
            "loupe_key",
            PREFIXES,
            Place::Grid,
            &settings(),
            20,
        );
        assert_eq!(
            hits[0].item,
            Item::Setting("gui.zoom".to_string(), "loupe_key".to_string())
        );
        assert!(!hits[0].detail.is_empty(), "sa description");
    }

    #[test]
    fn only_what_applies_here_and_never_itself() {
        // Le recadrage n'a pas de sens dans la grille.
        let hits = search(
            &keymap(),
            &|_| true,
            "recadrer",
            PREFIXES,
            Place::Grid,
            &settings(),
            50,
        );
        assert!(!hits.iter().any(|h| h.item == Item::Action(Action::Crop)));
        let hits = search(
            &keymap(),
            &|_| true,
            "palette",
            PREFIXES,
            Place::Grid,
            &settings(),
            50,
        );
        assert!(!hits.iter().any(|h| h.item == Item::Action(Action::Palette)));
    }

    #[test]
    fn a_typed_key_finds_its_actions_only() {
        let hits = search(
            &keymap(),
            &|_| true,
            "ctrl f",
            PREFIXES,
            Place::Grid,
            &settings(),
            20,
        );
        let items: Vec<Item> = hits.into_iter().map(|h| h.item).collect();
        assert!(items.contains(&Item::Action(Action::Search)));
        assert!(items.contains(&Item::Action(Action::FocusMode)));
        assert_eq!(items.len(), 2, "{items:?}");
    }

    #[test]
    fn sorting_by_one_key_is_an_action() {
        let hits = search(
            &keymap(),
            &|_| true,
            "tri nom",
            PREFIXES,
            Place::Grid,
            &settings(),
            20,
        );
        assert_eq!(
            hits[0].item,
            Item::Action(Action::SortBy(tri::sort::SortKey::Name))
        );
    }

    #[test]
    fn a_prefix_limits_to_actions_or_settings() {
        let kinds = |q: &str| -> (usize, usize) {
            let hits = search(
                &keymap(),
                &|_| true,
                q,
                PREFIXES,
                Place::Grid,
                &settings(),
                500,
            );
            let actions = hits
                .iter()
                .filter(|h| matches!(h.item, Item::Action(_)))
                .count();
            (actions, hits.len() - actions)
        };
        let (a, s) = kinds("zoom");
        assert!(a > 0 && s > 0, "sans préfixe : les deux");
        let (a, s) = kinds(">zoom");
        assert!(a > 0 && s == 0, "> : les actions");
        let (a, s) = kinds("?zoom");
        assert!(a == 0 && s > 0, "? : les réglages");
        let (a, s) = kinds(" ? zoom");
        assert!(a == 0 && s > 0, "espaces tolérés");
        // Le préfixe seul : tout parcourir, sans limite.
        let all = search(
            &keymap(),
            &|_| true,
            ">",
            PREFIXES,
            Place::Grid,
            &settings(),
            5,
        );
        assert!(all.len() > 50, "{}", all.len());
        assert!(all.iter().all(|h| matches!(h.item, Item::Action(_))));
        // Préfixe vide : jamais reconnu.
        let none = Prefixes {
            actions: None,
            settings: None,
        };
        assert_eq!(split_prefix(">zoom", none), (Only::All, ">zoom"));
    }

    #[test]
    fn what_the_user_looked_for_is_found() {
        // Retours du 06/11/2026, un par recherche qui n'aboutissait pas.
        let first = |q: &str| {
            search(
                &keymap(),
                &|_| true,
                q,
                PREFIXES,
                Place::Grid,
                &settings(),
                20,
            )[0]
            .item
            .clone()
        };
        let top = |q: &str, n: usize| -> Vec<Item> {
            search(
                &keymap(),
                &|_| true,
                q,
                PREFIXES,
                Place::Grid,
                &settings(),
                20,
            )
            .into_iter()
            .take(n)
            .map(|h| h.item)
            .collect()
        };
        // Le libellé de la case du panneau.
        assert_eq!(
            first("Empiler automatiquement"),
            Item::Action(Action::StackMode)
        );
        // Pluriel et singulier.
        assert_eq!(
            first("rendus vignette"),
            Item::Action(Action::GridRenderNext)
        );
        // Les cases RAW / JPEG.
        let raw = Item::Action(Action::FilterKind(tri::filter::KindFilter::RawOnly));
        assert!(top("raw", 3).contains(&raw), "{:?}", top("raw", 5));
    }

    #[test]
    fn what_is_greyed_out_here_is_left_out() {
        let developed = Item::Action(Action::FilterDeveloped);
        let all = search(
            &keymap(),
            &|_| true,
            "developpees",
            PREFIXES,
            Place::Grid,
            &settings(),
            20,
        );
        assert!(all.iter().any(|h| h.item == developed));
        let usable = |a: Action| a != Action::FilterDeveloped;
        let hits = search(
            &keymap(),
            &usable,
            "developpees",
            PREFIXES,
            Place::Grid,
            &settings(),
            20,
        );
        assert!(!hits.iter().any(|h| h.item == developed));
    }

    #[test]
    fn empty_query_lists_nothing_and_max_is_kept() {
        assert!(
            search(
                &keymap(),
                &|_| true,
                "  ",
                PREFIXES,
                Place::Grid,
                &settings(),
                20
            )
            .is_empty()
        );
        assert!(
            search(
                &keymap(),
                &|_| true,
                "e",
                PREFIXES,
                Place::Grid,
                &settings(),
                5
            )
            .len()
                <= 5
        );
    }
}

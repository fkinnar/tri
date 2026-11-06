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

/// Lignes de la palette pour `query`, à l'endroit `place` : les actions qui y ont un sens
/// ([`cheatsheet::applies`], sans la palette elle-même) et les réglages `settings`, les
/// meilleurs d'abord (à égalité, une action avant un réglage, puis le titre le plus court),
/// `max` au plus. Requête vide : rien (la palette invite à taper).
pub fn search(
    keymap: &Keymap,
    query: &str,
    place: Place,
    settings: &[SettingRef],
    max: usize,
) -> Vec<Hit> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    // Une touche tapée (« ctrl f ») : les actions qui lui sont liées, seulement.
    let typed = keymap.actions_for_typed_keys(query);
    let mut found: Vec<((Hit, u8), tri::fuzzy::Match)> = Vec::new();
    for &action in Action::ALL.iter() {
        if action == Action::Palette || !action.available() || !cheatsheet::applies(action, place) {
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
    for s in settings.iter().filter(|_| typed.is_none()) {
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
    let mut found = tri::fuzzy::relevant(found);
    found.sort_by_cached_key(|((hit, kind), m)| {
        (
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

    fn settings() -> Vec<SettingRef> {
        crate::settings_window::settings_list(&[])
    }

    #[test]
    fn the_best_action_comes_first() {
        let hits = search(&keymap(), "recadrer", Place::View, &settings(), 20);
        assert_eq!(hits[0].item, Item::Action(Action::Crop));
        assert!(!hits[0].keys.is_empty(), "les touches sont montrées");
        // Sans faute ni accent, en anglais.
        let hits = search(&keymap(), "crop", Place::View, &settings(), 20);
        assert_eq!(hits[0].item, Item::Action(Action::Crop));
    }

    #[test]
    fn settings_are_found_too() {
        let hits = search(&keymap(), "loupe_key", Place::Grid, &settings(), 20);
        assert_eq!(
            hits[0].item,
            Item::Setting("gui.zoom".to_string(), "loupe_key".to_string())
        );
        assert!(!hits[0].detail.is_empty(), "sa description");
    }

    #[test]
    fn only_what_applies_here_and_never_itself() {
        // Le recadrage n'a pas de sens dans la grille.
        let hits = search(&keymap(), "recadrer", Place::Grid, &settings(), 50);
        assert!(!hits.iter().any(|h| h.item == Item::Action(Action::Crop)));
        let hits = search(&keymap(), "palette", Place::Grid, &settings(), 50);
        assert!(!hits.iter().any(|h| h.item == Item::Action(Action::Palette)));
    }

    #[test]
    fn a_typed_key_finds_its_actions_only() {
        let hits = search(&keymap(), "ctrl f", Place::Grid, &settings(), 20);
        let items: Vec<Item> = hits.into_iter().map(|h| h.item).collect();
        assert!(items.contains(&Item::Action(Action::Search)));
        assert!(items.contains(&Item::Action(Action::FocusMode)));
        assert_eq!(items.len(), 2, "{items:?}");
    }

    #[test]
    fn sorting_by_one_key_is_an_action() {
        let hits = search(&keymap(), "tri nom", Place::Grid, &settings(), 20);
        assert_eq!(
            hits[0].item,
            Item::Action(Action::SortBy(tri::sort::SortKey::Name))
        );
    }

    #[test]
    fn empty_query_lists_nothing_and_max_is_kept() {
        assert!(search(&keymap(), "  ", Place::Grid, &settings(), 20).is_empty());
        assert!(search(&keymap(), "e", Place::Grid, &settings(), 5).len() <= 5);
    }
}

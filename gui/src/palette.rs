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
    /// Un contrôle de l'interface sans action à lui (« Taille des vignettes », 07/11/2026),
    /// par son identifiant : la ligne montre le contrôle, Entrée lui donne le clavier.
    Control(&'static str),
}

/// Identifiants des mots-clés des contrôles qui ont leur ligne (`kw-control-…` des
/// catalogues).
pub const CONTROL_KEYWORDS: &[&str] = &["control.grid_size"];

/// Un contrôle de l'interface qui a sa propre ligne dans la palette.
pub struct ControlRef {
    /// Identifiant (`grid.size`).
    pub id: &'static str,
    /// Nom (« Taille des vignettes »).
    pub title: String,
    /// Mots cherchés en plus du nom (mots-clés).
    pub texts: Vec<String>,
}

/// Lignes des contrôles `controls` pour `query`, placées avant les actions : seulement
/// ceux qui correspondent de près (nom ou mots-clés ; « rendu » ne doit pas donner la
/// taille des vignettes par une ressemblance lointaine). Aucun pour une recherche de
/// réglages seulement.
pub fn control_hits(query: &str, prefixes: Prefixes, controls: &[ControlRef]) -> Vec<Hit> {
    let (only, query) = split_prefix(query, prefixes);
    if only == Only::Settings || query.trim().is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(Hit, bool)> = controls
        .iter()
        .filter_map(|c| {
            let mut fields = vec![c.title.as_str()];
            fields.extend(c.texts.iter().map(String::as_str));
            // Chaque mot tapé au début d'un mot du nom ou des mots-clés (« nettes » est
            // dans « vignettes », pas au début ; « rendu » ressemble à « réduire »).
            if !tri::fuzzy::starts_words(query, &fields) {
                return None;
            }
            let close = tri::fuzzy::starts_words(query, &[&c.title]);
            let hit = Hit {
                item: Item::Control(c.id),
                title: c.title.clone(),
                keys: String::new(),
                detail: String::new(),
            };
            Some((hit, close))
        })
        .collect();
    found.sort_by_key(|(_, close)| std::cmp::Reverse(*close));
    found.into_iter().map(|(h, _)| h).collect()
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

/// Une ligne trouvée, avant le classement : la ligne, action (1) ou réglage (0), si elle
/// vaut à l'endroit où l'on est, et ses noms (titre, libellés des contrôles).
type Candidate = (Hit, u8, bool, Vec<String>);

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

/// Lignes de la palette pour `query` : toutes les actions (sans la palette elle-même) et
/// les réglages `settings`, sans filtre (retour du 06/11/2026), les meilleurs d'abord ;
/// à correspondance égale, celles qui valent ici (aux endroits `places`,
/// [`cheatsheet::applies`], et que `usable` accepte) avant les autres, une action avant
/// un réglage, puis le titre le plus court. `max` au plus (0 : sans limite). Un préfixe
/// ([`split_prefix`]) limite aux actions ou aux réglages ; seul, il les liste tous, dans
/// l'ordre de l'aide ou du fichier (pour parcourir). Requête vide : rien (la palette
/// invite à taper).
pub fn search(
    keymap: &Keymap,
    usable: &dyn Fn(Action) -> bool,
    query: &str,
    prefixes: Prefixes,
    places: &[Place],
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
    // (ligne, action ou réglage, vaut ici, ses noms), correspondance. Les noms : le titre,
    // et pour une action les libellés des contrôles qui la font (« Nettes », « Taille des
    // vignettes » : retour du 07/11/2026, trouvés après des réglages qui n'avaient ces
    // mots qu'en partie).
    let mut found: Vec<(Candidate, tri::fuzzy::Match)> = Vec::new();
    for &action in Action::ALL.iter().filter(|_| only != Only::Settings) {
        if action == Action::Palette || !action.available() {
            continue;
        }
        let here = places.iter().any(|&p| cheatsheet::applies(action, p)) && usable(action);
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
            let mut names = vec![title.clone()];
            names.extend(action.interface_labels().iter().map(|id| t!(*id)));
            let hit = Hit {
                item: Item::Action(action),
                title,
                keys,
                detail: String::new(),
            };
            found.push(((hit, 1, here, names), m));
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
            let names = vec![hit.title.clone()];
            // Un réglage ne « vaut » pas ici plus qu'ailleurs : il passe après les actions
            // à nom égal, même celles d'un autre endroit (07/11/2026 : « netteté » donnait
            // des réglages avant la carte de netteté).
            found.push(((hit, 0, false, names), m));
        }
    }
    if browsing {
        // Tout parcourir : dans l'ordre (thèmes de l'aide, fichier d'exemple).
        found.sort_by_key(|((hit, _, _, _), _)| match hit.item {
            Item::Action(a) => crate::actions::Group::ALL
                .iter()
                .position(|g| *g == a.group()),
            Item::Setting(..) | Item::Control(_) => None,
        });
        return found.into_iter().map(|((hit, _, _, _), _)| hit).collect();
    }
    let mut found = tri::fuzzy::relevant(found);
    // Trouvée dans son titre d'abord (« rendus vignette » : l'action « Rendu des
    // vignettes » avant un réglage qui n'a ces mots que dans sa description).
    found.sort_by_cached_key(|((hit, kind, here, names), m)| {
        // Des mots d'un de ses noms, pas des lettres éparpillées (« crop » dans
        // « Recadrage › shade_opacity »).
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let in_title = tri::fuzzy::find(query, &names)
            .filter(|t| t.close)
            .map(|t| t.exact);
        // Puis une action avant un réglage (07/11/2026 : « loupe » donnait d'abord
        // « Zoom et loupe › max », qui n'ouvre que la configuration).
        (
            std::cmp::Reverse(in_title),
            std::cmp::Reverse(*here),
            std::cmp::Reverse(*kind),
            std::cmp::Reverse(*m),
            hit.title.chars().count(),
        )
    });
    let max = if max == 0 { usize::MAX } else { max };
    found
        .into_iter()
        .take(max)
        .map(|((hit, _, _, _), _)| hit)
        .collect()
}

/// Fichier de l'historique de la palette, dans le dossier de données.
const HISTORY_FILE: &str = "palette-history";

/// Ligne du fichier pour `item` : `action<TAB>id` ou `setting<TAB>section<TAB>clé` (une
/// clé de `[keys]` contient des points : la tabulation sépare sans ambiguïté).
fn history_line(item: &Item) -> String {
    match item {
        Item::Action(a) => format!("action\t{}", a.id()),
        Item::Setting(section, key) => format!("setting\t{section}\t{key}"),
        Item::Control(id) => format!("control\t{id}"),
    }
}

/// L'inverse de [`history_line`] ; `None` pour une ligne illisible ou une action qui
/// n'existe plus.
fn history_item(line: &str) -> Option<Item> {
    let mut parts = line.split('\t');
    match (parts.next()?, parts.next()?, parts.next()) {
        ("action", id, None) => Action::from_id(id).map(Item::Action),
        ("setting", section, Some(key)) => Some(Item::Setting(section.into(), key.into())),
        _ => None,
    }
}

/// Historique de la palette (07/11/2026, choix de l'utilisateur : ce qui a été lancé
/// depuis la palette, gardé d'une session à l'autre), le plus récent d'abord ; vide sans
/// fichier.
pub fn load_history(data: &std::path::Path) -> Vec<Item> {
    std::fs::read_to_string(data.join(HISTORY_FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(history_item)
        .collect()
}

/// Écrit l'historique `items` dans le dossier de données.
pub fn save_history(data: &std::path::Path, items: &[Item]) -> std::io::Result<()> {
    let text: String = items.iter().map(|i| history_line(i) + "\n").collect();
    std::fs::create_dir_all(data)?;
    std::fs::write(data.join(HISTORY_FILE), text)
}

/// `item` vient d'être lancé : en tête de l'historique, sans doublon, `max` au plus
/// (0 : pas d'historique).
pub fn remember(history: &mut Vec<Item>, item: Item, max: usize) {
    history.retain(|i| *i != item);
    history.insert(0, item);
    history.truncate(max);
}

/// Les lignes de l'historique, comme la recherche les montrerait : une action qui
/// n'existe pas ici (`available`) ou un réglage disparu sont sautés.
pub fn history_hits(history: &[Item], keymap: &Keymap, settings: &[SettingRef]) -> Vec<Hit> {
    history
        .iter()
        .filter_map(|item| match item {
            Item::Action(action) if action.available() && *action != Action::Palette => Some(Hit {
                item: item.clone(),
                title: action.label(),
                keys: keymap.keys_for(*action).join(" / "),
                detail: String::new(),
            }),
            Item::Setting(section, key) => settings
                .iter()
                .find(|s| s.section == *section && s.key == *key)
                .map(|s| Hit {
                    item: item.clone(),
                    title: format!("{} › {}", s.section_label, s.key),
                    keys: String::new(),
                    detail: s.doc.clone(),
                }),
            Item::Action(_) | Item::Control(_) => None,
        })
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
            &[Place::View],
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
            &[Place::View],
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
            &[Place::Grid],
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
    fn everything_is_offered_what_applies_here_first() {
        // Retour du 06/11/2026 : tout, sans filtre. Le recadrage n'a pas de sens dans la
        // grille : proposé quand même, après ce qui vaut ici.
        let hits = search(
            &keymap(),
            &|_| true,
            "recadrer",
            PREFIXES,
            &[Place::Grid],
            &settings(),
            0,
        );
        assert!(hits.iter().any(|h| h.item == Item::Action(Action::Crop)));
        let hits = search(
            &keymap(),
            &|_| true,
            "palette",
            PREFIXES,
            &[Place::Grid],
            &settings(),
            0,
        );
        assert!(!hits.iter().any(|h| h.item == Item::Action(Action::Palette)));
        // À correspondance égale, ce qui vaut ici d'abord : « zoom » en grille, le zoom
        // des vignettes avant le déplacement dans une image zoomée.
        let hits = search(
            &keymap(),
            &|_| true,
            "zoom",
            PREFIXES,
            &[Place::Grid],
            &[],
            0,
        );
        let first_elsewhere = hits.iter().position(
            |h| matches!(h.item, Item::Action(a) if !cheatsheet::applies(a, Place::Grid)),
        );
        let last_here = hits.iter().rposition(
            |h| matches!(h.item, Item::Action(a) if cheatsheet::applies(a, Place::Grid)),
        );
        assert!(first_elsewhere.is_some());
        assert!(last_here < first_elsewhere, "{hits:?}");
    }

    #[test]
    fn a_typed_key_finds_its_actions_only() {
        let hits = search(
            &keymap(),
            &|_| true,
            "ctrl f",
            PREFIXES,
            &[Place::Grid],
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
            &[Place::Grid],
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
                &[Place::Grid],
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
            &[Place::Grid],
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
    fn the_history_keeps_what_was_launched_most_recent_first() {
        let zoom = Item::Action(Action::ZoomIn);
        let sort = Item::Action(Action::SortNext);
        let setting = Item::Setting("keys".into(), "core.flag.accept".into());
        let mut history = Vec::new();
        for item in [&zoom, &sort, &setting, &zoom] {
            remember(&mut history, item.clone(), 20);
        }
        assert_eq!(
            history,
            [zoom.clone(), setting.clone(), sort.clone()],
            "sans doublon"
        );
        remember(&mut history, sort.clone(), 2);
        assert_eq!(history, [sort.clone(), zoom.clone()], "taille limitée");
        // Écrit puis relu à l'identique ; une ligne illisible est sautée.
        let dir = tempfile::tempdir().unwrap();
        let history = vec![setting.clone(), zoom.clone()];
        save_history(dir.path(), &history).unwrap();
        assert_eq!(load_history(dir.path()), history);
        std::fs::write(
            dir.path().join(HISTORY_FILE),
            "action\tcore.nothing\nn'importe quoi\naction\tcore.nav.next\n",
        )
        .unwrap();
        assert_eq!(load_history(dir.path()), [Item::Action(Action::NavNext)]);
        assert!(load_history(&dir.path().join("absent")).is_empty());
        // Les lignes : titre et touches de l'action ; un réglage inconnu sauté.
        let hits = history_hits(
            &[zoom, Item::Setting("nulle".into(), "part".into())],
            &keymap(),
            &settings(),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, Action::ZoomIn.label());
        assert!(!hits[0].keys.is_empty());
    }

    #[test]
    fn panel_words_find_their_action_before_the_settings() {
        // Retours du 07/11/2026 : « nettes » (la case du panneau) ne trouvait que des
        // réglages, « taille des vignettes » rien ; « loupe » et « netteté » donnaient
        // d'abord des réglages, qui n'ouvrent que la configuration.
        let top = |q: &str, n: usize| -> Vec<Item> {
            search(
                &keymap(),
                &|_| true,
                q,
                PREFIXES,
                &[Place::Grid],
                &settings(),
                20,
            )
            .into_iter()
            .take(n)
            .map(|h| h.item)
            .collect()
        };
        assert_eq!(top("nettes", 1), [Item::Action(Action::FilterBlurry)]);
        assert_eq!(
            top("exposées", 1),
            [Item::Action(Action::FilterWellExposed)]
        );
        assert_eq!(top("sens", 1), [Item::Action(Action::SortReverse)]);
        for q in ["loupe", "netteté", "grossissement"] {
            assert!(
                matches!(top(q, 1).first(), Some(Item::Action(_))),
                "{q} : {:?}",
                top(q, 3)
            );
        }
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
                &[Place::Grid],
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
                &[Place::Grid],
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
        assert!(matches!(
            first("rendus vignette"),
            Item::Action(Action::ToggleSource | Action::GridRenderNext)
        ));
        // Un rendu précis (07/11/2026).
        assert_eq!(
            first("rendu darktable"),
            Item::Action(Action::Render(crate::grid_render::GridRender::Engine(
                tri::develop::Engine::Darktable
            )))
        );
        assert_eq!(
            first("rendu libraw"),
            Item::Action(Action::Render(crate::grid_render::GridRender::LibRaw))
        );
        // Les cases RAW / JPEG.
        let raw = Item::Action(Action::FilterKind(tri::filter::KindFilter::RawOnly));
        assert!(top("raw", 3).contains(&raw), "{:?}", top("raw", 5));
    }

    #[test]
    fn what_is_greyed_out_here_comes_after() {
        let developed = Item::Action(Action::FilterDeveloped);
        let usable = |a: Action| a != Action::FilterDeveloped;
        let hits = search(
            &keymap(),
            &usable,
            "developpees",
            PREFIXES,
            &[Place::Grid],
            &settings(),
            0,
        );
        let at = hits.iter().position(|h| h.item == developed);
        assert!(
            at.is_some(),
            "proposé quand même (refusé avec sa raison si lancé)"
        );
    }

    #[test]
    fn empty_query_lists_nothing_and_max_is_kept() {
        assert!(
            search(
                &keymap(),
                &|_| true,
                "  ",
                PREFIXES,
                &[Place::Grid],
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
                &[Place::Grid],
                &settings(),
                5
            )
            .len()
                <= 5
        );
    }
}

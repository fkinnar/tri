//! Recherche floue à la façon de fzf : aide, réglages, filtre de la grille par nom de
//! fichier, recherche de dossiers. Dans le moteur pour servir aussi en ligne de commande
//! et à un futur TUI. Voir `doc/en/fuzzy.md`.

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
pub fn score(query: &str, text: &str) -> Option<i32> {
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

/// Correspondance d'une requête dans une entrée (réglage, action de l'aide), pour ranger
/// les résultats : une correspondance **exacte** (chaque mot tapé apparaît tel quel,
/// « vim » dans « … vim nvim … ») passe avant les lettres éparpillées (« v…i…m »), puis
/// le score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Match {
    /// Chaque mot de la requête apparaît d'un seul tenant (sans accents ni casse).
    pub exact: bool,
    /// Score de [`score`].
    pub score: i32,
}

/// Correspondance de `query` dans les champs d'une entrée (libellé, identifiant,
/// description, mots-clés…) : chaque mot dans un même champ, les mots pouvant se trouver
/// dans des champs différents. Requête vide : `Some` (score 0).
pub fn find(query: &str, fields: &[&str]) -> Option<Match> {
    let fields: Vec<Vec<char>> = fields
        .iter()
        .map(|f| f.chars().map(fold).collect())
        .collect();
    let (mut score, mut exact) = (0, true);
    for word in query.split_whitespace() {
        let word: Vec<char> = word.chars().map(fold).collect();
        score += fields.iter().filter_map(|f| word_score(&word, f)).max()?;
        exact &= fields
            .iter()
            .any(|f| f.windows(word.len()).any(|w| w == word.as_slice()));
    }
    Some(Match { exact, score })
}

/// Garde les meilleures correspondances : s'il y en a d'exactes, celles-là seulement
/// (retour du 22/10/2026 : « vim » trouvait le bon réglage, perdu parmi tous ceux où
/// traînent un v, un i et un m).
pub fn relevant<T>(mut found: Vec<(T, Match)>) -> Vec<(T, Match)> {
    if found.iter().any(|(_, m)| m.exact) {
        found.retain(|(_, m)| m.exact);
    }
    found
}

/// Filtre de la grille par nom de fichier (recherche `/`), sur le trait commun des
/// filtres : les images dont le nom correspond à `query`. Requête vide : tout.
pub struct NameFilter<'a> {
    /// Ce qui est tapé.
    pub query: &'a str,
}

impl crate::filter::Filter for NameFilter<'_> {
    fn apply(&self, groups: crate::filter::Groups) -> crate::filter::Groups {
        if self.query.trim().is_empty() {
            return groups;
        }
        crate::filter::retain(groups, |file| {
            let name = file.path.file_name().unwrap_or_default().to_string_lossy();
            score(self.query, &name).is_some()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::Filter;
    use crate::media::{MediaFile, MediaKind};

    #[test]
    fn the_name_filter_keeps_matching_files_only() {
        let file = |name: &str| MediaFile {
            path: std::path::PathBuf::from("/photos").join(name),
            kind: MediaKind::Raw,
        };
        let groups = vec![
            vec![file("DSCF0305.RAF"), file("DSCF0306.RAF")],
            vec![file("IMG_1.jpg")],
        ];
        let kept = NameFilter { query: "0306" }.apply(groups.clone());
        assert_eq!(kept.len(), 1, "groupe vidé retiré");
        assert_eq!(kept[0][0].path.file_name().unwrap(), "DSCF0306.RAF");
        // Sur le nom seul, pas le dossier (« photos » ne correspond pas à tout).
        assert!(
            NameFilter { query: "photos" }
                .apply(groups.clone())
                .is_empty()
        );
        assert_eq!(
            NameFilter { query: " " }.apply(groups).len(),
            2,
            "vide : tout"
        );
    }

    #[test]
    fn letters_in_order_without_accents_nor_case() {
        assert!(score("pnd", "Panneau des dossiers").is_some());
        assert!(
            score("dehors 2026", "2026/2026-10-17_Self_Dehors").is_some(),
            "mots dans le désordre"
        );
        assert!(score("ete", "Été").is_some());
        assert_eq!(score("xyz", "DSCF0305.RAF"), None);
        assert_eq!(score("", "n'importe quoi"), Some(0));
        // Lettres consécutives et débuts de mots d'abord.
        assert!(score("dscf", "DSCF0305.RAF") > score("dscf", "d_s_c_f.jpg"));
    }

    #[test]
    fn exact_words_hide_scattered_letters() {
        let profile = find(
            "vim",
            &["Jeu de raccourcis", "gui.profile", "vim nvim hjkl"],
        )
        .unwrap();
        let scattered = find("vim", &["Vignettes : taille minimale", "gui.grid.cell_min"]).unwrap();
        assert!(profile.exact && !scattered.exact);
        assert!(profile > scattered);
        // Mots répartis sur plusieurs champs, accents ignorés.
        assert!(
            find("theme ecran", &["Thème", "Plein écran"])
                .unwrap()
                .exact
        );
        assert_eq!(find("xyz", &["abc"]), None);
        assert_eq!(
            find("wqx", &["w", "q", "x"]),
            None,
            "un mot ne s'étale pas sur plusieurs champs"
        );
        let kept = relevant(vec![("a", scattered), ("b", profile)]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].0, "b");
        let none_exact = relevant(vec![("a", scattered)]);
        assert_eq!(none_exact.len(), 1, "sans exacte : les éparpillées restent");
    }
}

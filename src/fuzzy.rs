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
/// « vim » dans « … vim nvim … ») passe avant une correspondance **proche** (chaque mot
/// tel quel ou à une faute de frappe près d'un mot du texte, « noter » et « notation »),
/// qui passe avant les lettres éparpillées (« v…i…m »), puis le score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Match {
    /// Chaque mot de la requête apparaît d'un seul tenant (sans accents ni casse).
    pub exact: bool,
    /// Chaque mot de la requête apparaît tel quel ou à quelques fautes près
    /// ([`typos_allowed`]) d'un mot du texte, ou de son début.
    pub close: bool,
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
    let (mut score, mut exact, mut close) = (0, true, true);
    for word in query.split_whitespace() {
        let word: Vec<char> = word.chars().map(fold).collect();
        // Une lettre seule n'est pas un mot (retour du 06/11/2026 : le « f » de « ctrl f »
        // se trouvait dans presque tout) : elle ne trouve qu'une lettre seule, une touche
        // (« x », « f f ») ou un mot d'une lettre.
        if let [letter] = word.as_slice() {
            fields
                .iter()
                .any(|f| f.split(|c| !c.is_alphanumeric()).any(|w| w == [*letter]))
                .then_some(())?;
            score += 30;
            continue;
        }
        let typo = fields.iter().filter_map(|f| typo_score(&word, f)).max();
        // Lettres dans l'ordre, sinon un mot à une faute près (« histrogramme »).
        score += fields
            .iter()
            .filter_map(|f| word_score(&word, f))
            .max()
            .or(typo)?;
        // Au singulier comme au pluriel (« rendus vignette » : « Rendu des vignettes »).
        let stem = singular(&word);
        let whole = fields
            .iter()
            .any(|f| f.windows(stem.len()).any(|w| w == stem));
        exact &= whole;
        close &= whole || typo.is_some();
    }
    Some(Match {
        exact,
        close,
        score,
    })
}

/// Le mot sans sa marque du pluriel (« s » ou « x » final, au-delà de 3 lettres) : le
/// français et l'anglais l'ajoutent, et un texte au singulier contient alors le mot tapé
/// au pluriel sans elle.
fn singular(word: &[char]) -> &[char] {
    match word {
        [rest @ .., 's' | 'x'] if word.len() > 3 => rest,
        _ => word,
    }
}

/// Fautes tolérées dans un mot tapé de `len` lettres : aucune en dessous de 4 (trop de
/// mots courts se ressemblent), une jusqu'à 7, deux au-delà.
fn typos_allowed(len: usize) -> usize {
    match len {
        0..=3 => 0,
        4..=7 => 1,
        _ => 2,
    }
}

/// Score d'un mot tapé avec des fautes de frappe (lettre en trop, en moins, remplacée,
/// deux lettres inversées) : retrouvé dans un mot du texte, ou au début d'un mot (on tape
/// encore : « histr » pour « histogramme »), à [`typos_allowed`] fautes près. Plus bas que
/// toute correspondance sans faute ; `None` sinon.
fn typo_score(word: &[char], text: &[char]) -> Option<i32> {
    let allowed = typos_allowed(word.len());
    if allowed == 0 {
        return None;
    }
    text.split(|c| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter_map(|candidate| {
            // Le mot entier, ou son début de la longueur tapée (± les fautes permises).
            let lengths = word.len().saturating_sub(allowed)..=word.len() + allowed;
            lengths
                .filter(|&n| n > 0 && n <= candidate.len())
                .map(|n| distance(word, &candidate[..n]))
                .min()
        })
        .filter(|&d| d <= allowed)
        .map(|d| (word.len() as i32) * 4 - 10 * d as i32)
        .max()
}

/// Distance d'édition entre `a` et `b` (Damerau-Levenshtein restreinte : insertion,
/// suppression, remplacement, et inversion de deux lettres voisines comptent pour un).
fn distance(a: &[char], b: &[char]) -> usize {
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }
    d[n][m]
}

/// Garde les meilleures correspondances : s'il y en a d'exactes, celles-là seulement
/// (retour du 22/10/2026 : « vim » trouvait le bon réglage, perdu parmi tous ceux où
/// traînent un v, un i et un m) ; sinon, s'il y en a de proches, celles-là (06/11/2026 :
/// « noter » remontait 44 actions aux lettres éparpillées).
pub fn relevant<T>(mut found: Vec<(T, Match)>) -> Vec<(T, Match)> {
    if found.iter().any(|(_, m)| m.exact) {
        found.retain(|(_, m)| m.exact);
    } else if found.iter().any(|(_, m)| m.close) {
        found.retain(|(_, m)| m.close);
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
    fn a_typo_is_forgiven_in_longer_words() {
        // Lettre en trop, inversée, remplacée, en moins.
        for typo in ["histrogramme", "hsitogramme", "histigramme", "histgramme"] {
            assert!(find(typo, &["Afficher l'histogramme"]).is_some(), "{typo}");
        }
        // En cours de frappe, avec une faute.
        assert!(find("histr", &["histogramme"]).is_some());
        // Une faute dès 4 lettres, aucune en dessous (trop de mots courts se ressemblent).
        assert!(find("pile", &["vile"]).is_some());
        assert!(find("abc", &["abd"]).is_none());
        // Deux fautes au-delà de 7 lettres, pas trois.
        assert!(find("comparasion", &["comparaison"]).is_some());
        assert!(find("xxmparasion", &["comparaison"]).is_none());
        // Toujours plus bas qu'une correspondance sans faute.
        let typo = find("histrogramme", &["histogramme"]).unwrap();
        let clean = find("histogramme", &["histogramme"]).unwrap();
        assert!(typo < clean);
        assert!(!typo.exact);
        assert!(typo.close);
    }

    #[test]
    fn a_single_letter_only_finds_a_single_letter() {
        // Une touche, ou un mot d'une lettre.
        assert!(find("x", &["Supprimer", "x"]).is_some());
        assert!(find("f", &["Filtres", "f f"]).is_some());
        assert!(find("a", &["photo à droite"]).is_some());
        // Pas dans un mot.
        assert!(find("f", &["Filtre : floues", "Ctrl+Maj+s"]).is_none());
        assert!(find("zoom f", &["Zoom 100 %", "z"]).is_none());
        assert!(find("zoom z", &["Zoom 100 %", "z"]).is_some_and(|m| m.exact));
    }

    #[test]
    fn plural_or_singular_is_exact() {
        assert!(find("rendus vignette", &["Rendu des vignettes"]).is_some_and(|m| m.exact));
        assert!(find("filtres", &["Filtre de type"]).is_some_and(|m| m.exact));
        assert!(find("panneaux", &["panneau des dossiers"]).is_some_and(|m| m.exact));
        // Trois lettres ou moins : tel quel (« gps », « iso »).
        assert!(find("gps", &["gp"]).is_none_or(|m| !m.exact));
    }

    #[test]
    fn close_words_hide_scattered_letters() {
        let found = vec![
            ("note", find("noter", &["Retirer la note"]).unwrap()),
            (
                "scattered",
                find("noter", &["nom de fichier : tout retirer"]).unwrap(),
            ),
        ];
        let kept: Vec<&str> = relevant(found).into_iter().map(|(n, _)| n).collect();
        assert_eq!(kept, ["note"]);
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

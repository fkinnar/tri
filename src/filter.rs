//! Filtres : transforment la liste des images à afficher.
//!
//! Les images circulent en groupes (`Groups`, un `Vec<Vec<MediaFile>>`) pour permettre plus
//! tard la détection de séries ; sans regroupement, il y a un seul groupe. Voir
//! `doc/en/filter.md`.

use crate::media::{MediaFile, MediaKind};

/// Images organisées en groupes (séries, rafales...). Un groupe n'est jamais vide.
pub type Groups = Vec<Vec<MediaFile>>;

/// Met une liste à plat dans un groupe unique (aucun groupe si la liste est vide).
pub fn ungrouped(files: Vec<MediaFile>) -> Groups {
    if files.is_empty() {
        Vec::new()
    } else {
        vec![files]
    }
}

/// Un filtre transforme des groupes en groupes : il peut retirer des images, les réordonner
/// ou les regrouper. Les filtres natifs et ceux des plugins implémentent ce même trait.
pub trait Filter {
    /// Applique le filtre. Ne doit pas renvoyer de groupe vide.
    fn apply(&self, groups: Groups) -> Groups;
}

/// Applique une suite de filtres dans l'ordre.
pub fn apply_all(filters: &[Box<dyn Filter>], groups: Groups) -> Groups {
    filters.iter().fold(groups, |g, f| f.apply(g))
}

/// Garde les images d'un groupe qui vérifient `keep`, puis retire les groupes vidés.
/// Brique commune aux filtres qui ne regroupent pas.
pub fn retain(groups: Groups, mut keep: impl FnMut(&MediaFile) -> bool) -> Groups {
    groups
        .into_iter()
        .map(|g| g.into_iter().filter(&mut keep).collect::<Vec<_>>())
        .filter(|g| !g.is_empty())
        .collect()
}

/// Filtre par nature de fichier (ex. carte SD en RAW+JPEG : tout, RAW seuls ou JPEG seuls).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KindFilter {
    /// RAW et images plates.
    #[default]
    All,
    /// RAW seulement.
    RawOnly,
    /// Images plates seulement.
    FlatOnly,
}

impl KindFilter {
    /// Vrai si une image de nature `kind` passe le filtre.
    pub fn accepts(self, kind: MediaKind) -> bool {
        match self {
            Self::All => true,
            Self::RawOnly => kind == MediaKind::Raw,
            Self::FlatOnly => kind == MediaKind::Flat,
        }
    }
}

impl Filter for KindFilter {
    fn apply(&self, groups: Groups) -> Groups {
        retain(groups, |f| self.accepts(f.kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, kind: MediaKind) -> MediaFile {
        MediaFile {
            path: name.into(),
            kind,
        }
    }

    fn names(groups: &Groups) -> Vec<Vec<&str>> {
        groups
            .iter()
            .map(|g| g.iter().map(|f| f.path.to_str().unwrap()).collect())
            .collect()
    }

    fn sample() -> Groups {
        vec![
            vec![
                file("a.RAF", MediaKind::Raw),
                file("a.JPG", MediaKind::Flat),
            ],
            vec![file("b.JPG", MediaKind::Flat)],
        ]
    }

    #[test]
    fn ungrouped_makes_one_group_or_none() {
        assert!(ungrouped(vec![]).is_empty());
        assert_eq!(ungrouped(vec![file("a.RAF", MediaKind::Raw)]).len(), 1);
    }

    #[test]
    fn kind_filter_all_keeps_everything() {
        assert_eq!(KindFilter::All.apply(sample()), sample());
    }

    #[test]
    fn kind_filter_raw_only_drops_emptied_groups() {
        assert_eq!(names(&KindFilter::RawOnly.apply(sample())), [["a.RAF"]]);
    }

    #[test]
    fn kind_filter_flat_only() {
        let groups = KindFilter::FlatOnly.apply(sample());
        assert_eq!(names(&groups), [vec!["a.JPG"], vec!["b.JPG"]]);
    }

    #[test]
    fn default_is_all() {
        assert_eq!(KindFilter::default(), KindFilter::All);
    }

    #[test]
    fn apply_all_chains_in_order() {
        let filters: Vec<Box<dyn Filter>> = vec![
            Box::new(KindFilter::FlatOnly),
            Box::new(KindFilter::RawOnly),
        ];
        assert!(apply_all(&filters, sample()).is_empty());
        assert_eq!(apply_all(&[], sample()), sample());
    }
}

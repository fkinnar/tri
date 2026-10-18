//! Piles de la grille (images semblables regroupées, comme Capture One) : la **vue** de la
//! liste des images. La liste complète reste celle du moteur (calculs en fond, netteté de
//! chaque image du groupe) ; la grille n'en montre que les couvertures des piles repliées
//! et le contenu des piles dépliées. Le regroupement lui-même est dans le moteur
//! ([`tri::similarity`]).

use std::ops::Range;

/// Piles d'une liste d'images, et ce que la grille en montre.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stacks {
    /// Les groupes, rangs dans la liste complète, dans l'ordre.
    ranges: Vec<Range<usize>>,
    /// Pour chaque image de la liste complète : son groupe.
    of: Vec<usize>,
    /// Couverture de chaque groupe (rang dans la liste complète).
    covers: Vec<usize>,
    /// Groupe replié (une seule vignette montrée) ; un groupe d'une image ne l'est jamais.
    collapsed: Vec<bool>,
    /// Images montrées par la grille, dans l'ordre (rangs dans la liste complète).
    pub shown: Vec<usize>,
}

impl Stacks {
    /// Piles des groupes `ranges` (couvrant `0..n`, voir [`tri::similarity::group`]) ;
    /// `cover` choisit la couverture d'un groupe, `expanded` dit s'il est déplié. Sans
    /// `enabled`, tout est montré, chaque image seule.
    pub fn build(
        n: usize,
        ranges: Vec<Range<usize>>,
        enabled: bool,
        cover: impl Fn(&Range<usize>) -> usize,
        expanded: impl Fn(&Range<usize>) -> bool,
    ) -> Stacks {
        let ranges: Vec<Range<usize>> = if enabled {
            // Un groupe hors de la liste (liste vide) : ignoré.
            ranges
                .into_iter()
                .filter(|r| r.end <= n && !r.is_empty())
                .collect()
        } else {
            (0..n).map(|i| i..i + 1).collect()
        };
        let mut s = Stacks {
            of: vec![0; n],
            ..Stacks::default()
        };
        for (g, r) in ranges.iter().enumerate() {
            for i in r.clone() {
                s.of[i] = g;
            }
            let c = cover(r);
            let collapsed = r.len() > 1 && !expanded(r);
            s.covers.push(c);
            s.collapsed.push(collapsed);
            if collapsed {
                s.shown.push(c);
            } else {
                s.shown.extend(r.clone());
            }
        }
        s.ranges = ranges;
        s
    }

    /// Groupe de l'image `index` (liste complète).
    pub fn range_of(&self, index: usize) -> Option<&Range<usize>> {
        self.ranges.get(*self.of.get(index)?)
    }

    /// Couverture du groupe de l'image `index`.
    pub fn cover_of(&self, index: usize) -> Option<usize> {
        self.covers.get(*self.of.get(index)?).copied()
    }

    /// Nombre d'images du groupe, si l'image `index` est la vignette d'une pile repliée.
    pub fn collapsed_count(&self, index: usize) -> Option<usize> {
        let g = *self.of.get(index)?;
        (self.collapsed[g] && self.covers[g] == index).then(|| self.ranges[g].len())
    }

    /// Vrai si l'image `index` est dans une pile dépliée (de plus d'une image).
    pub fn in_open_stack(&self, index: usize) -> bool {
        self.of
            .get(index)
            .is_some_and(|&g| !self.collapsed[g] && self.ranges[g].len() > 1)
    }

    /// Rang de l'image `index` dans la vue, `None` si elle est cachée dans une pile.
    pub fn position(&self, index: usize) -> Option<usize> {
        self.shown.iter().position(|&i| i == index)
    }

    /// Image montrée à la place de `index` : elle-même, sinon la couverture de sa pile.
    pub fn visible(&self, index: usize) -> usize {
        if self.position(index).is_some() {
            index
        } else {
            self.cover_of(index).unwrap_or(index)
        }
    }

    /// Nombre de piles (groupes de plus d'une image).
    pub fn count(&self) -> usize {
        self.ranges.iter().filter(|r| r.len() > 1).count()
    }

    /// Nombre de groupes.
    pub fn groups(&self) -> usize {
        self.ranges.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_stacks_show_their_cover_only() {
        // 0 | 1 2 3 | 4 5 ; couverture : la dernière du groupe ; 4-5 déplié.
        let s = Stacks::build(
            6,
            vec![0..1, 1..4, 4..6],
            true,
            |r| r.end - 1,
            |r| r.start == 4,
        );
        assert_eq!(s.shown, [0, 3, 4, 5]);
        assert_eq!(s.collapsed_count(3), Some(3));
        assert_eq!(s.collapsed_count(1), None, "pas la couverture");
        assert_eq!(s.collapsed_count(0), None, "une seule image : pas une pile");
        assert!(s.in_open_stack(4) && !s.in_open_stack(3) && !s.in_open_stack(0));
        assert_eq!(s.visible(2), 3, "cachée : sa couverture");
        assert_eq!(s.visible(5), 5);
        assert_eq!(s.position(3), Some(1));
        assert_eq!((s.count(), s.groups()), (2, 3));
        assert_eq!(s.range_of(2), Some(&(1..4)));
    }

    #[test]
    fn an_empty_list_has_no_stack() {
        let s = Stacks::build(
            0,
            std::iter::once(0..1).collect(),
            true,
            |r| r.start,
            |_| false,
        );
        assert!(s.shown.is_empty());
        assert_eq!(s.visible(0), 0);
    }

    #[test]
    fn without_stacks_everything_is_shown() {
        let s = Stacks::build(
            3,
            std::iter::once(0..3).collect(),
            false,
            |r| r.start,
            |_| false,
        );
        assert_eq!(s.shown, [0, 1, 2]);
        assert_eq!(s.count(), 0);
    }
}

//! Piles de la grille (images semblables regroupées, comme Capture One) : la **vue** de la
//! liste des images. La liste complète reste celle du moteur (calculs en fond, netteté de
//! chaque image du groupe) ; la grille n'en montre que les couvertures des piles repliées
//! et le contenu des piles dépliées. Le regroupement lui-même est dans le moteur
//! ([`tri::similarity`]).
//!
//! Second niveau (décidé le 30/10/2026) : la **pile des développements** d'une photo (la
//! photo et ses développements darktable / RapidRAW, qui se suivent dans la liste), repliée
//! par défaut sur une couverture, dépliable ([`Stacks::with_developments`]). Les deux
//! niveaux s'emboîtent : une pile de rafale repliée montre une image ; dépliée, chacune de
//! ses photos montre sa couverture ou tous ses développements.

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
    /// Piles actives (sinon chaque image forme son groupe, et les piles de développements
    /// se cherchent sur toute la liste).
    grouped: bool,
    /// Piles de développements : la photo et ses développements, rangs dans la liste
    /// complète, dans l'ordre ; vides tant que [`Stacks::with_developments`] n'a pas été
    /// appelé.
    photos: Vec<Range<usize>>,
    /// Pour chaque image : sa pile de développements.
    photo_of: Vec<usize>,
    /// Couverture de chaque pile de développements.
    photo_covers: Vec<usize>,
    /// Pile de développements repliée (une seule vignette) ; jamais pour une image seule.
    photo_folded: Vec<bool>,
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
            grouped: enabled,
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

    /// Ajoute les piles de développements : `same_photo(i, j)` dit si deux images voisines
    /// sont la même photo (elle et ses développements), `cover` choisit la couverture d'une
    /// pile, `unfolded` dit si elle est dépliée. Une pile ne déborde jamais d'une pile de
    /// rafale ; une pile de rafale repliée montre la couverture de la pile de
    /// développements de sa couverture, jamais un développement caché.
    pub fn with_developments(
        mut self,
        same_photo: impl Fn(usize, usize) -> bool,
        cover: impl Fn(&Range<usize>) -> usize,
        unfolded: impl Fn(&Range<usize>) -> bool,
    ) -> Stacks {
        let n = self.of.len();
        self.photos.clear();
        self.photo_covers.clear();
        self.photo_folded.clear();
        self.photo_of = vec![0; n];
        let regions: Vec<Range<usize>> = if self.grouped {
            self.ranges.clone()
        } else {
            std::iter::once(0..n).collect()
        };
        for region in regions {
            let mut start = region.start;
            for i in region.start..=region.end {
                if i == region.end || (i > start && !same_photo(i - 1, i)) {
                    if start < i {
                        let run = start..i;
                        let p = self.photos.len();
                        for k in run.clone() {
                            self.photo_of[k] = p;
                        }
                        self.photo_covers
                            .push(cover(&run).clamp(run.start, run.end - 1));
                        self.photo_folded.push(run.len() > 1 && !unfolded(&run));
                        self.photos.push(run);
                    }
                    start = i;
                }
            }
        }
        for g in 0..self.ranges.len() {
            self.covers[g] = self.photo_visible(self.covers[g]);
        }
        self.shown.clear();
        for (g, r) in self.ranges.iter().enumerate() {
            if self.collapsed[g] {
                self.shown.push(self.covers[g]);
            } else {
                for i in r.clone() {
                    let p = self.photo_of[i];
                    if !self.photo_folded[p] || self.photo_covers[p] == i {
                        self.shown.push(i);
                    }
                }
            }
        }
        self
    }

    /// Image montrée pour `index` dans sa pile de développements : elle-même, ou la
    /// couverture si la pile est repliée.
    fn photo_visible(&self, index: usize) -> usize {
        match self.photo_of.get(index) {
            Some(&p) if self.photo_folded[p] => self.photo_covers[p],
            _ => index,
        }
    }

    /// Pile de développements de l'image `index` (elle et ses développements), si elle en a.
    pub fn developments_of(&self, index: usize) -> Option<&Range<usize>> {
        let r = self.photos.get(*self.photo_of.get(index)?)?;
        (r.len() > 1).then_some(r)
    }

    /// Nombre d'images de la pile de développements, si l'image `index` est la vignette
    /// d'une pile repliée.
    pub fn folded_count(&self, index: usize) -> Option<usize> {
        let p = *self.photo_of.get(index)?;
        (self.photo_folded[p] && self.photo_covers[p] == index).then(|| self.photos[p].len())
    }

    /// Vrai si l'image `index` est dans une pile de développements dépliée.
    pub fn in_unfolded_developments(&self, index: usize) -> bool {
        match self.photo_of.get(index) {
            Some(&p) => !self.photo_folded[p] && self.photos[p].len() > 1,
            None => false,
        }
    }

    /// Développement d'à côté dans la pile de `index` (`step` : +1 ou -1), en boucle ;
    /// `None` sans développement.
    pub fn step_in_developments(&self, index: usize, step: isize) -> Option<usize> {
        let r = self.developments_of(index)?;
        let len = r.len() as isize;
        let at = (index - r.start) as isize;
        Some(r.start + ((at + step).rem_euclid(len)) as usize)
    }

    /// Groupe de l'image `index` (liste complète).
    pub fn range_of(&self, index: usize) -> Option<&Range<usize>> {
        self.ranges.get(*self.of.get(index)?)
    }

    /// Couverture du groupe de l'image `index`.
    #[cfg(test)]
    pub fn cover_of(&self, index: usize) -> Option<usize> {
        self.covers.get(*self.of.get(index)?).copied()
    }

    /// Nombre d'images du groupe, si l'image `index` est la vignette d'une pile repliée.
    pub fn collapsed_count(&self, index: usize) -> Option<usize> {
        let g = *self.of.get(index)?;
        (self.collapsed[g] && self.covers[g] == index).then(|| self.ranges[g].len())
    }

    /// Rang de l'image `index` dans la vue, `None` si elle est cachée dans une pile.
    pub fn position(&self, index: usize) -> Option<usize> {
        self.shown.iter().position(|&i| i == index)
    }

    /// Image montrée à la place de `index` : elle-même, sinon la couverture de sa pile
    /// (de rafale repliée, sinon de développements repliée).
    pub fn visible(&self, index: usize) -> usize {
        if self.position(index).is_some() {
            return index;
        }
        let Some(&g) = self.of.get(index) else {
            return index;
        };
        if self.collapsed.get(g) == Some(&true) {
            return self.covers[g];
        }
        self.photo_visible(index)
    }

    /// Rang du groupe de l'image `index` (pour alterner les fonds pendant le réglage).
    pub fn group_index(&self, index: usize) -> Option<usize> {
        self.of.get(index).copied()
    }

    /// Couverture du groupe d'avant (`forward` faux) ou d'après celui de l'image `index`.
    pub fn neighbour_group(&self, index: usize, forward: bool) -> Option<usize> {
        let g = *self.of.get(index)?;
        let other = if forward { g + 1 } else { g.checked_sub(1)? };
        self.covers.get(other).copied()
    }

    /// Image d'à côté dans la pile de `index` (`step` : +1 ou -1) ; au bout, rien, ou la
    /// première / la dernière si `wrap`.
    pub fn step_in_stack(&self, index: usize, step: isize, wrap: bool) -> Option<usize> {
        let r = self.range_of(index)?;
        let next = index as isize + step;
        if (r.start as isize..r.end as isize).contains(&next) {
            Some(next as usize)
        } else if wrap && r.len() > 1 {
            Some(if step > 0 { r.start } else { r.end - 1 })
        } else {
            None
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
        assert_eq!(s.visible(2), 3, "cachée : sa couverture");
        assert_eq!(s.visible(5), 5);
        assert_eq!(s.position(3), Some(1));
        assert_eq!((s.count(), s.groups()), (2, 3));
        assert_eq!(s.range_of(2), Some(&(1..4)));
    }

    #[test]
    fn stepping_inside_a_stack_and_between_stacks() {
        let s = Stacks::build(6, vec![0..1, 1..4, 4..6], true, |r| r.start, |_| false);
        assert_eq!(s.step_in_stack(1, 1, false), Some(2));
        assert_eq!(s.step_in_stack(3, 1, false), None, "au bout : on s'arrête");
        assert_eq!(s.step_in_stack(3, 1, true), Some(1), "ou on boucle");
        assert_eq!(s.step_in_stack(1, -1, true), Some(3));
        assert_eq!(s.step_in_stack(0, 1, true), None, "une seule image");
        assert_eq!(s.neighbour_group(2, true), Some(4));
        assert_eq!(s.neighbour_group(2, false), Some(0));
        assert_eq!(s.neighbour_group(5, true), None);
        assert_eq!(s.group_index(5), Some(2));
    }

    #[test]
    fn development_stacks_fold_inside_burst_stacks() {
        // Liste : 0 A | 1 B, 2 B@dt, 3 B@rr | 4 C, 5 C@dt ; rafale 1..6 dépliée ; B repliée
        // (couverture : B@dt), C dépliée.
        let same = |i: usize, j: usize| matches!((i, j), (1, 2) | (2, 3) | (4, 5));
        let s = Stacks::build(6, vec![0..1, 1..6], true, |r| r.start, |_| true).with_developments(
            same,
            |r| if r.start == 1 { 2 } else { r.start },
            |r| r.start == 4,
        );
        assert_eq!(s.shown, [0, 2, 4, 5]);
        assert_eq!(s.folded_count(2), Some(3));
        assert_eq!(s.folded_count(1), None, "pas la couverture");
        assert_eq!(s.folded_count(0), None, "pas de développement");
        assert!(s.in_unfolded_developments(4) && s.in_unfolded_developments(5));
        assert!(!s.in_unfolded_developments(0) && !s.in_unfolded_developments(2));
        assert_eq!(
            s.visible(3),
            2,
            "développement caché : la couverture de sa photo"
        );
        assert_eq!(s.visible(5), 5);
        assert_eq!(s.developments_of(3), Some(&(1..4)));
        assert_eq!(s.developments_of(0), None);
        assert_eq!(s.step_in_developments(3, 1), Some(1), "en boucle");
        assert_eq!(s.step_in_developments(1, -1), Some(3));
        assert_eq!(s.step_in_developments(0, 1), None);
        // Rafale repliée sur une image cachée d'une pile de développements : la
        // couverture de la pile de développements.
        let s = Stacks::build(6, vec![0..1, 1..6], true, |_| 3, |_| false).with_developments(
            same,
            |r| r.start,
            |_| false,
        );
        assert_eq!(s.shown, [0, 1]);
        assert_eq!(s.visible(3), 1);
        assert_eq!(s.cover_of(5), Some(1));
    }

    #[test]
    fn without_burst_stacks_developments_still_stack() {
        let same = |i: usize, j: usize| (i, j) == (0, 1);
        let s = Stacks::build(
            3,
            std::iter::once(0..3).collect(),
            false,
            |r| r.start,
            |_| false,
        )
        .with_developments(same, |r| r.end - 1, |_| false);
        assert_eq!(s.shown, [1, 2]);
        assert_eq!(s.folded_count(1), Some(2));
        assert_eq!(s.visible(0), 1);
        assert_eq!(s.count(), 0, "pas de pile de rafale");
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

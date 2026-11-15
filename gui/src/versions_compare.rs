//! Comparaison des versions d'une image (#393, décidé le 09/11/2026) : un volet par
//! version, recadré, dans le mode comparaison ; l'image entière en premier volet sur
//! demande (« Montrer l'original »). Ce module ne garde que l'état (volets, volet actif,
//! versions marquées) ; le dessin et les actions sont dans `app.rs`.
//!
//! Pourquoi pas dans la grille : une version est un cadre pris dans la même image, pas une
//! autre photo ; dépliées, elles rempliraient la grille de morceaux d'une même photo et
//! brouilleraient les piles qui servent au tri.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// Un volet : l'image entière (`None`) ou la version n° `i` (`Some(i)`).
pub type Pane = Option<usize>;

/// État de la comparaison des versions de `image`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionsCompare {
    /// L'image dont on compare les versions.
    pub image: PathBuf,
    /// L'image entière en premier volet.
    pub original: bool,
    /// Volet actif (rang dans [`VersionsCompare::panes`]).
    pub active: usize,
    /// Versions marquées (Espace, Ctrl+clic), exportées ensemble.
    pub marked: BTreeSet<usize>,
}

impl VersionsCompare {
    /// Comparaison des versions de `image`, la première active, sans l'original.
    pub fn new(image: PathBuf) -> VersionsCompare {
        VersionsCompare {
            image,
            original: false,
            active: 0,
            marked: BTreeSet::new(),
        }
    }

    /// Les volets, pour une image qui a `count` versions : l'original d'abord s'il est
    /// montré, puis chaque version.
    pub fn panes(&self, count: usize) -> Vec<Pane> {
        self.original
            .then_some(None)
            .into_iter()
            .chain((0..count).map(Some))
            .collect()
    }

    /// Le volet actif (ramené dans les bornes : une version a pu être supprimée).
    pub fn active_pane(&self, count: usize) -> Pane {
        let panes = self.panes(count);
        panes
            .get(self.active.min(panes.len().saturating_sub(1)))
            .copied()
            .flatten()
    }

    /// Rend actif le volet `pane`, s'il est montré.
    pub fn activate(&mut self, pane: Pane, count: usize) {
        if let Some(at) = self.panes(count).iter().position(|p| *p == pane) {
            self.active = at;
        }
    }

    /// Montre ou cache l'original ; le volet actif reste le même (l'original caché : la
    /// première version).
    pub fn toggle_original(&mut self, count: usize) {
        let active = self.active_pane(count);
        self.original = !self.original;
        self.active = 0;
        self.activate(active, count);
    }

    /// Marque ou démarque la version active (l'original ne se marque pas).
    pub fn toggle_mark(&mut self, count: usize) {
        if let Some(i) = self.active_pane(count)
            && !self.marked.remove(&i)
        {
            self.marked.insert(i);
        }
    }

    /// Versions à exporter : les marquées, sinon la version active (rien pour l'original).
    pub fn to_export(&self, count: usize) -> Vec<usize> {
        let marked: Vec<usize> = self.marked.iter().copied().filter(|&i| i < count).collect();
        if !marked.is_empty() {
            return marked;
        }
        self.active_pane(count).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panes_are_the_versions_and_the_original_on_request() {
        let mut c = VersionsCompare::new("/p/a.RAF".into());
        assert_eq!(c.panes(3), [Some(0), Some(1), Some(2)]);
        assert_eq!(c.active_pane(3), Some(0));
        c.active = 2;
        // L'original montré : le volet actif reste la troisième version.
        c.toggle_original(3);
        assert_eq!(c.panes(3), [None, Some(0), Some(1), Some(2)]);
        assert_eq!(c.active_pane(3), Some(2));
        c.active = 0;
        assert_eq!(c.active_pane(3), None, "l'original");
        // Caché : l'original actif devient la première version.
        c.toggle_original(3);
        assert_eq!(c.active_pane(3), Some(0));
        // Une version supprimée entre-temps : ramené dans les bornes.
        c.active = 5;
        assert_eq!(c.active_pane(2), Some(1));
    }

    #[test]
    fn marked_versions_are_exported_together() {
        let mut c = VersionsCompare::new("/p/a.RAF".into());
        assert_eq!(c.to_export(3), [0], "sans marque : la version active");
        c.toggle_mark(3);
        c.active = 2;
        c.toggle_mark(3);
        assert_eq!(c.to_export(3), [0, 2]);
        c.toggle_mark(3);
        assert_eq!(c.to_export(3), [0], "démarquée");
        // L'original ne se marque ni ne s'exporte comme une version.
        c.marked.clear();
        c.toggle_original(3);
        c.active = 0;
        c.toggle_mark(3);
        assert!(c.marked.is_empty());
        assert!(c.to_export(3).is_empty());
    }
}

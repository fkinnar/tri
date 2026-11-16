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

use tri::versions::Source;

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
    /// Rendus masqués (filtre du panneau de droite, retour du 16/11/2026) : leurs versions
    /// n'ont pas de volet.
    pub hidden: Vec<Source>,
}

impl VersionsCompare {
    /// Comparaison des versions de `image`, la première active, sans l'original.
    pub fn new(image: PathBuf) -> VersionsCompare {
        VersionsCompare {
            image,
            original: false,
            active: 0,
            marked: BTreeSet::new(),
            hidden: Vec::new(),
        }
    }

    /// Les volets, pour une image dont les versions ont les rendus `sources` : l'original
    /// d'abord s'il est montré, puis chaque version d'un rendu non masqué.
    pub fn panes(&self, sources: &[Source]) -> Vec<Pane> {
        self.original
            .then_some(None)
            .into_iter()
            .chain(
                sources
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| !self.hidden.contains(s))
                    .map(|(i, _)| Some(i)),
            )
            .collect()
    }

    /// Montre ou masque les versions du rendu `source` ; jamais tous les volets masqués
    /// (sans l'original, le dernier rendu montré reste). Les versions masquées perdent
    /// leur marque. Vrai si le filtre a changé.
    pub fn toggle_render(&mut self, source: Source, sources: &[Source]) -> bool {
        let active = self.active_pane(sources);
        if let Some(at) = self.hidden.iter().position(|s| *s == source) {
            self.hidden.remove(at);
        } else {
            self.hidden.push(source);
            if !self.original && self.panes(sources).is_empty() {
                self.hidden.pop();
                return false;
            }
            self.marked
                .retain(|&i| sources.get(i).is_some_and(|s| *s != source));
        }
        self.active = 0;
        self.activate(active, sources);
        true
    }

    /// Le volet actif (ramené dans les bornes : une version a pu être supprimée).
    pub fn active_pane(&self, sources: &[Source]) -> Pane {
        let panes = self.panes(sources);
        panes
            .get(self.active.min(panes.len().saturating_sub(1)))
            .copied()
            .flatten()
    }

    /// Rend actif le volet `pane`, s'il est montré.
    pub fn activate(&mut self, pane: Pane, sources: &[Source]) {
        if let Some(at) = self.panes(sources).iter().position(|p| *p == pane) {
            self.active = at;
        }
    }

    /// Montre ou cache l'original ; le volet actif reste le même (l'original caché : la
    /// première version).
    pub fn toggle_original(&mut self, sources: &[Source]) {
        let active = self.active_pane(sources);
        self.original = !self.original;
        // Sans l'original, il faut au moins un volet : les rendus reviennent.
        if !self.original && self.panes(sources).is_empty() {
            self.hidden.clear();
        }
        self.active = 0;
        self.activate(active, sources);
    }

    /// Marque ou démarque la version active (l'original ne se marque pas).
    pub fn toggle_mark(&mut self, sources: &[Source]) {
        if let Some(i) = self.active_pane(sources)
            && !self.marked.remove(&i)
        {
            self.marked.insert(i);
        }
    }

    /// Versions à exporter : les marquées, sinon la version active (rien pour l'original).
    pub fn to_export(&self, sources: &[Source]) -> Vec<usize> {
        let marked: Vec<usize> = self
            .marked
            .iter()
            .copied()
            .filter(|&i| i < sources.len())
            .collect();
        if !marked.is_empty() {
            return marked;
        }
        self.active_pane(sources).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` versions, toutes du même rendu.
    fn same(n: usize) -> Vec<Source> {
        vec![Source::Embedded; n]
    }

    #[test]
    fn the_panes_are_the_versions_and_the_original_on_request() {
        let mut c = VersionsCompare::new("/p/a.RAF".into());
        assert_eq!(c.panes(&same(3)), [Some(0), Some(1), Some(2)]);
        assert_eq!(c.active_pane(&same(3)), Some(0));
        c.active = 2;
        // L'original montré : le volet actif reste la troisième version.
        c.toggle_original(&same(3));
        assert_eq!(c.panes(&same(3)), [None, Some(0), Some(1), Some(2)]);
        assert_eq!(c.active_pane(&same(3)), Some(2));
        c.active = 0;
        assert_eq!(c.active_pane(&same(3)), None, "l'original");
        // Caché : l'original actif devient la première version.
        c.toggle_original(&same(3));
        assert_eq!(c.active_pane(&same(3)), Some(0));
        // Une version supprimée entre-temps : ramené dans les bornes.
        c.active = 5;
        assert_eq!(c.active_pane(&same(2)), Some(1));
    }

    #[test]
    fn versions_are_filtered_by_render() {
        // Retour du 16/11/2026 : un filtre par rendu dans le panneau de droite.
        let sources = [
            Source::Embedded,
            Source::Raw,
            Source::Embedded,
            Source::Developed,
        ];
        let mut c = VersionsCompare::new("/p/a.RAF".into());
        c.active = 1;
        c.marked.insert(0);
        assert!(c.toggle_render(Source::Embedded, &sources));
        assert_eq!(c.panes(&sources), [Some(1), Some(3)]);
        assert_eq!(c.active_pane(&sources), Some(1), "l'active reste");
        assert!(c.marked.is_empty(), "masquée : démarquée");
        assert!(c.toggle_render(Source::Developed, &sources));
        // Le dernier rendu montré ne se masque pas (sans l'original).
        assert!(!c.toggle_render(Source::Raw, &sources));
        assert_eq!(c.panes(&sources), [Some(1)]);
        // Avec l'original, si.
        c.toggle_original(&sources);
        assert!(c.toggle_render(Source::Raw, &sources));
        assert_eq!(c.panes(&sources), [None]);
        // L'original caché : les rendus reviennent.
        c.toggle_original(&sources);
        assert_eq!(c.panes(&sources).len(), 4);
    }

    #[test]
    fn marked_versions_are_exported_together() {
        let mut c = VersionsCompare::new("/p/a.RAF".into());
        assert_eq!(
            c.to_export(&same(3)),
            [0],
            "sans marque : la version active"
        );
        c.toggle_mark(&same(3));
        c.active = 2;
        c.toggle_mark(&same(3));
        assert_eq!(c.to_export(&same(3)), [0, 2]);
        c.toggle_mark(&same(3));
        assert_eq!(c.to_export(&same(3)), [0], "démarquée");
        // L'original ne se marque ni ne s'exporte comme une version.
        c.marked.clear();
        c.toggle_original(&same(3));
        c.active = 0;
        c.toggle_mark(&same(3));
        assert!(c.marked.is_empty());
        assert!(c.to_export(&same(3)).is_empty());
    }
}

//! Sélection multiple : les images marquées d'une liste, en plus de l'image courante.
//!
//! Indépendante de l'affichage (interface graphique, futur TUI) : on marque une image,
//! une plage (« mode visuel » à la vim, ou Maj+clic), tout, et les actions (notes,
//! corbeille, éditeur) s'appliquent à [`Selection::targets`] — les images marquées encore
//! visibles, ou l'image courante s'il n'y en a pas. Les images sont repérées par leur
//! chemin : la sélection survit à un changement de filtre ou à une relecture du dossier.
//! Voir `doc/en/selection.md`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::media::MediaFile;

/// Plage en cours de « mode visuel » : de `anchor` jusqu'à l'image courante.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Visual {
    anchor: PathBuf,
    /// Marques d'avant le mode visuel (rétablies s'il est annulé).
    base: HashSet<PathBuf>,
}

/// Images marquées.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    marked: HashSet<PathBuf>,
    visual: Option<Visual>,
}

impl Selection {
    /// Nombre d'images marquées.
    pub fn len(&self) -> usize {
        self.marked.len()
    }

    /// Vrai si aucune image n'est marquée.
    pub fn is_empty(&self) -> bool {
        self.marked.is_empty()
    }

    /// Vrai si `path` est marquée.
    pub fn contains(&self, path: &Path) -> bool {
        self.marked.contains(path)
    }

    /// Vrai pendant le mode visuel.
    pub fn is_visual(&self) -> bool {
        self.visual.is_some()
    }

    /// Marque `path`, ou la démarque si elle l'était.
    pub fn toggle(&mut self, path: &Path) {
        if !self.marked.remove(path) {
            self.marked.insert(path.to_path_buf());
        }
    }

    /// Démarque tout (et quitte le mode visuel).
    pub fn clear(&mut self) {
        self.marked.clear();
        self.visual = None;
    }

    /// Marque toutes les images de `list`.
    pub fn select_all(&mut self, list: &[MediaFile]) {
        self.visual = None;
        self.marked.extend(list.iter().map(|f| f.path.clone()));
    }

    /// Marque les images de `list` entre `from` et `to` (compris, dans un sens ou dans
    /// l'autre), en plus des marques existantes (Maj+clic).
    pub fn add_range(&mut self, list: &[MediaFile], from: usize, to: usize) {
        let (a, b) = (from.min(to), from.max(to));
        let last = list.len().saturating_sub(1);
        if list.is_empty() || a > last {
            return;
        }
        self.marked
            .extend(list[a..=b.min(last)].iter().map(|f| f.path.clone()));
    }

    /// Entre en mode visuel à l'image `cursor` de `list` : la plage part d'elle.
    pub fn start_visual(&mut self, list: &[MediaFile], cursor: usize) {
        let Some(file) = list.get(cursor) else {
            return;
        };
        self.visual = Some(Visual {
            anchor: file.path.clone(),
            base: self.marked.clone(),
        });
        self.update_visual(list, cursor);
    }

    /// Mode visuel : la plage va maintenant de l'ancre à `cursor` (à appeler quand
    /// l'image courante change). Ancre disparue de la liste (filtre) : plage depuis
    /// `cursor`.
    pub fn update_visual(&mut self, list: &[MediaFile], cursor: usize) {
        let Some(visual) = &self.visual else {
            return;
        };
        let anchor = list
            .iter()
            .position(|f| f.path == visual.anchor)
            .unwrap_or(cursor);
        self.marked = visual.base.clone();
        self.add_range(list, anchor, cursor);
    }

    /// Quitte le mode visuel ; `keep` : garder la plage marquée (sinon les marques
    /// d'avant sont rétablies).
    pub fn end_visual(&mut self, keep: bool) {
        if let Some(visual) = self.visual.take()
            && !keep
        {
            self.marked = visual.base;
        }
    }

    /// Images sur lesquelles agir, dans l'ordre de `list` : les images marquées encore
    /// dans la liste (un filtre a pu en masquer : on n'agit pas sur ce qu'on ne voit pas),
    /// sinon l'image `cursor`.
    pub fn targets(&self, list: &[MediaFile], cursor: usize) -> Vec<PathBuf> {
        let marked: Vec<PathBuf> = list
            .iter()
            .filter(|f| self.marked.contains(&f.path))
            .map(|f| f.path.clone())
            .collect();
        if !marked.is_empty() {
            return marked;
        }
        list.get(cursor)
            .map(|f| vec![f.path.clone()])
            .unwrap_or_default()
    }

    /// Nombre d'images marquées encore dans `list`.
    pub fn visible(&self, list: &[MediaFile]) -> usize {
        list.iter()
            .filter(|f| self.marked.contains(&f.path))
            .count()
    }

    /// Après une relecture du dossier : oublie les images qui n'existent plus.
    pub fn retain_existing(&mut self) {
        self.marked.retain(|p| p.exists());
        if let Some(v) = &mut self.visual {
            v.base.retain(|p| p.exists());
        }
    }

    /// Après un renommage : `from` s'appelle maintenant `to`.
    pub fn renamed(&mut self, from: &Path, to: &Path) {
        if self.marked.remove(from) {
            self.marked.insert(to.to_path_buf());
        }
        if let Some(v) = &mut self.visual {
            if v.anchor == from {
                v.anchor = to.to_path_buf();
            }
            if v.base.remove(from) {
                v.base.insert(to.to_path_buf());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaKind;

    fn list(n: usize) -> Vec<MediaFile> {
        (0..n)
            .map(|i| MediaFile {
                path: PathBuf::from(format!("/p/img{i}.jpg")),
                kind: MediaKind::Flat,
            })
            .collect()
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn without_marks_the_target_is_the_cursor() {
        let l = list(3);
        let s = Selection::default();
        assert_eq!(names(&s.targets(&l, 1)), ["img1"]);
        assert!(s.targets(&[], 0).is_empty());
    }

    #[test]
    fn toggle_and_targets_in_list_order() {
        let l = list(4);
        let mut s = Selection::default();
        s.toggle(&l[3].path);
        s.toggle(&l[0].path);
        s.toggle(&l[2].path);
        s.toggle(&l[2].path);
        assert_eq!(names(&s.targets(&l, 1)), ["img0", "img3"], "pas le curseur");
        // Une image masquée par un filtre n'est pas une cible.
        assert_eq!(names(&s.targets(&l[1..], 0)), ["img3"]);
        assert_eq!(s.visible(&l[1..]), 1);
    }

    #[test]
    fn visual_mode_follows_the_cursor_and_can_be_cancelled() {
        let l = list(6);
        let mut s = Selection::default();
        s.toggle(&l[5].path);
        s.start_visual(&l, 2);
        assert_eq!(names(&s.targets(&l, 2)), ["img2", "img5"]);
        s.update_visual(&l, 4);
        assert_eq!(names(&s.targets(&l, 4)), ["img2", "img3", "img4", "img5"]);
        s.update_visual(&l, 0);
        assert_eq!(
            names(&s.targets(&l, 0)),
            ["img0", "img1", "img2", "img5"],
            "vers l'arrière, la plage se réduit"
        );
        s.end_visual(false);
        assert_eq!(
            names(&s.targets(&l, 0)),
            ["img5"],
            "annulé : marques d'avant"
        );
        s.start_visual(&l, 1);
        s.update_visual(&l, 2);
        s.end_visual(true);
        assert!(!s.is_visual());
        assert_eq!(names(&s.targets(&l, 0)), ["img1", "img2", "img5"]);
    }

    #[test]
    fn ranges_all_and_renames() {
        let l = list(5);
        let mut s = Selection::default();
        s.add_range(&l, 3, 1);
        assert_eq!(s.len(), 3);
        s.add_range(&l, 4, 99);
        assert_eq!(s.len(), 4, "borne hors liste ramenée à la fin");
        s.clear();
        s.select_all(&l);
        assert_eq!(s.len(), 5);
        s.renamed(&l[0].path, Path::new("/p/nouveau.jpg"));
        assert!(s.contains(Path::new("/p/nouveau.jpg")));
        assert!(!s.contains(&l[0].path));
    }
}

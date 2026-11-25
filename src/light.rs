//! Mode léger d'un dossier (#597) : sur un NAS ou iCloud, seules les vignettes sont
//! calculées en fond. L'aperçu boîtier de l'image affichée et de ses voisines vient à la
//! demande ([`crate::scheduler::Scheduler::set_light`]), les analyses passent « à la
//! demande » ([`lighten`]), et « Tout calculer » remplit le cache d'un dossier à la fois.
//!
//! Où il s'applique : les emplacements réseau d'office (`[light] network`), plus les
//! dossiers choisis (`folders`) et moins les exceptions (`except`). Le plus profond
//! l'emporte : un dossier ajouté vaut pour ses sous-dossiers, une exception aussi.
//!
//! Voir `doc/en/light.md`.

use std::path::{Path, PathBuf};

use crate::scheduler::{Analyses, Analysis};
use crate::settings::expand_home;

/// Réglages du mode léger (section `[light]` du fichier de configuration).
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightSettings {
    /// Emplacements réseau (partages, iCloud) en mode léger d'office.
    pub network: bool,
    /// Dossiers en mode léger, sous-dossiers compris (`~` accepté).
    pub folders: Vec<String>,
    /// Dossiers sortis du mode léger, sous-dossiers compris.
    pub except: Vec<String>,
}

impl Default for LightSettings {
    fn default() -> Self {
        Self {
            network: true,
            folders: Vec::new(),
            except: Vec::new(),
        }
    }
}

/// Une règle qui s'applique à un dossier : sa racine, sa profondeur, et si elle met en
/// mode léger.
struct Rule {
    root: PathBuf,
    depth: usize,
    on: bool,
}

impl LightSettings {
    /// Racine du mode léger qui s'applique à `folder` (le dossier ajouté, ou
    /// l'emplacement réseau `network_root`), `None` hors du mode léger.
    pub fn root_of(&self, folder: &Path, network_root: Option<&Path>) -> Option<PathBuf> {
        self.rule(folder, network_root, None)
            .filter(|r| r.on)
            .map(|r| r.root)
    }

    /// Vrai si `folder` est en mode léger.
    pub fn is_light(&self, folder: &Path, network_root: Option<&Path>) -> bool {
        self.root_of(folder, network_root).is_some()
    }

    /// Repère du panneau des dossiers : `Some(true)` si le mode léger commence à `path`,
    /// `Some(false)` si `path` en sort (exception sous une racine du mode léger), `None`
    /// sinon.
    pub fn mark(&self, path: &Path, network_root: Option<&Path>) -> Option<bool> {
        match self.rule(path, network_root, None) {
            Some(r) if r.root == path && r.on => Some(true),
            Some(r) if r.root == path => path
                .parent()
                .filter(|parent| self.is_light(parent, network_root))
                .map(|_| false),
            _ => None,
        }
    }

    /// Listes `(folders, except)` une fois `folder` mis en mode léger (`on`) ou sorti :
    /// sa propre entrée retirée, puis ajoutée à la bonne liste seulement si ce qui
    /// l'entoure ne donne pas déjà le résultat voulu.
    pub fn toggled(
        &self,
        folder: &Path,
        network_root: Option<&Path>,
        on: bool,
    ) -> (Vec<String>, Vec<String>) {
        let others = |list: &[String]| -> Vec<String> {
            list.iter()
                .filter(|p| expand_home(p) != folder)
                .cloned()
                .collect()
        };
        let (mut folders, mut except) = (others(&self.folders), others(&self.except));
        let inherited = self
            .rule(folder, network_root, Some(folder))
            .is_some_and(|r| r.on);
        if inherited != on {
            let entry = folder.to_string_lossy().into_owned();
            if on {
                folders.push(entry);
            } else {
                except.push(entry);
            }
        }
        (folders, except)
    }

    /// La règle la plus profonde qui s'applique à `folder` ; à profondeur égale, un choix
    /// de l'utilisateur l'emporte sur l'emplacement réseau. `skip` : entrée ignorée.
    fn rule(
        &self,
        folder: &Path,
        network_root: Option<&Path>,
        skip: Option<&Path>,
    ) -> Option<Rule> {
        let depth = |p: &Path| p.components().count();
        let mut best: Option<Rule> = network_root
            .filter(|root| self.network && folder.starts_with(root))
            .map(|root| Rule {
                root: root.to_path_buf(),
                depth: depth(root),
                on: true,
            });
        for (list, on) in [(&self.folders, true), (&self.except, false)] {
            for entry in list {
                let root = expand_home(entry);
                if Some(root.as_path()) == skip || !folder.starts_with(&root) {
                    continue;
                }
                let d = depth(&root);
                if best.as_ref().is_none_or(|b| d >= b.depth) {
                    best = Some(Rule { root, depth: d, on });
                }
            }
        }
        best
    }
}

/// Analyses en mode léger : celles calculées en fond passent à la demande (outil grisé
/// jusqu'à « Calculer »), les autres restent telles quelles.
pub fn lighten(analyses: Analyses) -> Analyses {
    let light = |a: Analysis| match a {
        Analysis::Background => Analysis::OnDemand,
        other => other,
    };
    Analyses {
        exposure: light(analyses.exposure),
        sharpness: light(analyses.sharpness),
        stacks: light(analyses.stacks),
        palettes: light(analyses.palettes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(folders: &[&str], except: &[&str]) -> LightSettings {
        LightSettings {
            network: true,
            folders: folders.iter().map(|s| s.to_string()).collect(),
            except: except.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn a_network_place_is_light_with_its_subfolders_unless_excepted() {
        let nas = Path::new("/mnt/nas");
        let s = settings(&[], &["/mnt/nas/2026/fast"]);
        assert_eq!(
            s.root_of(Path::new("/mnt/nas/2026"), Some(nas)),
            Some(nas.to_path_buf())
        );
        assert!(!s.is_light(Path::new("/mnt/nas/2026/fast/a"), Some(nas)));
        assert!(!s.is_light(Path::new("/home/me/photos"), None));
        let off = LightSettings {
            network: false,
            ..s.clone()
        };
        assert!(!off.is_light(Path::new("/mnt/nas/2026"), Some(nas)));
    }

    #[test]
    fn the_deepest_choice_wins() {
        let s = settings(&["/photos", "/photos/a/b"], &["/photos/a"]);
        assert!(s.is_light(Path::new("/photos/x"), None));
        assert!(!s.is_light(Path::new("/photos/a/c"), None));
        assert_eq!(
            s.root_of(Path::new("/photos/a/b/c"), None),
            Some(PathBuf::from("/photos/a/b"))
        );
    }

    #[test]
    fn marks_show_where_light_mode_starts_and_stops() {
        let nas = Path::new("/mnt/nas");
        let s = settings(&["/photos"], &["/mnt/nas/fast", "/elsewhere"]);
        assert_eq!(s.mark(nas, Some(nas)), Some(true));
        assert_eq!(s.mark(Path::new("/mnt/nas/2026"), Some(nas)), None);
        assert_eq!(s.mark(Path::new("/mnt/nas/fast"), Some(nas)), Some(false));
        assert_eq!(s.mark(Path::new("/photos"), None), Some(true));
        // Une exception hors du mode léger ne marque rien.
        assert_eq!(s.mark(Path::new("/elsewhere"), None), None);
    }

    #[test]
    fn toggling_keeps_the_lists_minimal() {
        let nas = Path::new("/mnt/nas");
        let s = settings(&[], &[]);
        // Sortir un sous-dossier du NAS : une exception.
        let (folders, except) = s.toggled(Path::new("/mnt/nas/fast"), Some(nas), false);
        assert!(folders.is_empty());
        assert_eq!(except, ["/mnt/nas/fast"]);
        // Y revenir : l'exception disparaît, rien n'est ajouté.
        let s = settings(&[], &["/mnt/nas/fast"]);
        let (folders, except) = s.toggled(Path::new("/mnt/nas/fast"), Some(nas), true);
        assert!(folders.is_empty() && except.is_empty());
        // Un dossier local : ajouté, puis retiré.
        let (folders, _) = s.toggled(Path::new("/photos"), None, true);
        assert_eq!(folders, ["/photos"]);
        let s = settings(&["/photos"], &[]);
        let (folders, except) = s.toggled(Path::new("/photos"), None, false);
        assert!(folders.is_empty() && except.is_empty());
    }

    #[test]
    fn light_analyses_wait_for_a_request() {
        let analyses = Analyses {
            exposure: Analysis::Background,
            sharpness: Analysis::OnDemand,
            stacks: Analysis::Off,
            palettes: Analysis::Background,
        };
        let light = lighten(analyses);
        assert_eq!(light.exposure, Analysis::OnDemand);
        assert_eq!(light.sharpness, Analysis::OnDemand);
        assert_eq!(light.stacks, Analysis::Off);
        assert_eq!(light.palettes, Analysis::OnDemand);
    }
}

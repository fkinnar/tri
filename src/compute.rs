//! Calculs choisis sur un dossier, en fond (#555, idée de l'utilisateur, 21/11/2026) :
//! préparer un dossier sans l'ouvrir ni attendre (avant une recherche de palettes, avant
//! de trier sur un portable, avant de débrancher un disque lent), en choisissant quoi
//! calculer : aperçus, rendus RAW, histogrammes, netteté, empreintes des piles, palettes.
//!
//! Chaque calcul ([`Kind`]) passe par le producteur du moteur ([`Producer`], le même que
//! l'interface) : mêmes clés de cache, ce qui y est déjà est sauté (sauf « recréer »,
//! [`run`]). Un calcul est une tâche à part ; plusieurs tournent en même temps, mais le
//! travail LibRaw et les logiciels de développement, qui occupent déjà tous les cœurs,
//! passent **l'un après l'autre** ([`heavy_turn`], mesuré le 16/10/2026 : plusieurs
//! rendus à la fois sont bien plus lents qu'en série).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::cache::Level;
use crate::develop::Engine;
use crate::media::{MediaFile, MediaKind};
use crate::palette::{Source, SourceKind};
use crate::scheduler::{Failure, Producer};

/// Ce qu'on peut calculer sur un dossier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Vignettes et aperçus boîtier (images plates : leur aperçu).
    Previews,
    /// Rendus RAW (aperçus standard des RAW, LibRaw).
    Raw,
    /// Histogrammes (analyse d'exposition).
    Exposure,
    /// Scores de netteté.
    Sharpness,
    /// Empreintes de regroupement (piles).
    Signatures,
    /// Palettes de couleurs, d'une source.
    Palette(SourceKind),
}

impl Kind {
    /// Tous les calculs proposés, dans l'ordre des menus : palettes du JPEG boîtier, des
    /// données brutes, puis des rendus de chaque logiciel.
    pub fn all() -> Vec<Kind> {
        let mut all = vec![
            Kind::Previews,
            Kind::Raw,
            Kind::Exposure,
            Kind::Sharpness,
            Kind::Signatures,
            Kind::Palette(SourceKind::Camera),
            Kind::Palette(SourceKind::Raw),
        ];
        all.extend(Engine::ALL.map(|e| Kind::Palette(SourceKind::Developed(e))));
        all
    }

    /// Identifiant (ligne de commande, actions) : `previews`, `raw`, `exposure`,
    /// `sharpness`, `signatures`, `palette-camera`, `palette-raw`, `palette-darktable`…
    pub fn id(self) -> String {
        match self {
            Kind::Previews => "previews".into(),
            Kind::Raw => "raw".into(),
            Kind::Exposure => "exposure".into(),
            Kind::Sharpness => "sharpness".into(),
            Kind::Signatures => "signatures".into(),
            Kind::Palette(k) => format!("palette-{}", k.id()),
        }
    }

    /// Calcul d'après son identifiant ([`Kind::id`]) ; `palette:raw` accepté aussi.
    pub fn parse(text: &str) -> Option<Kind> {
        let text = text.trim().to_lowercase().replace(':', "-");
        Kind::all().into_iter().find(|k| k.id() == text)
    }

    /// Niveaux de cache à calculer pour `file` (aucun : rien à faire pour lui).
    pub fn levels(self, file: &MediaFile) -> Vec<Level> {
        let raw = file.kind == MediaKind::Raw;
        match self {
            Kind::Previews if raw => vec![Level::Thumbnail, Level::Embedded],
            Kind::Previews => vec![Level::Thumbnail, Level::Standard],
            Kind::Raw if raw => vec![Level::Standard],
            Kind::Raw => Vec::new(),
            Kind::Exposure => vec![Level::Exposure],
            Kind::Sharpness => vec![Level::Sharpness],
            Kind::Signatures => vec![Level::Similarity],
            Kind::Palette(k) => k.source_for(file).map(Level::Palette).into_iter().collect(),
        }
    }

    /// Niveaux de cache que ce calcul remplit (pour « recréer » : effacés d'abord).
    pub fn cache_levels(self) -> Vec<Level> {
        match self {
            Kind::Previews => vec![Level::Thumbnail, Level::Embedded, Level::Standard],
            Kind::Raw => vec![Level::Standard],
            Kind::Exposure => vec![Level::Exposure],
            Kind::Sharpness => vec![Level::Sharpness],
            Kind::Signatures => vec![Level::Similarity],
            Kind::Palette(SourceKind::Camera | SourceKind::Cover) => {
                vec![Level::Palette(Source::Camera)]
            }
            Kind::Palette(SourceKind::Raw) => vec![Level::Palette(Source::Raw)],
            Kind::Palette(SourceKind::Developed(e)) => vec![Level::Palette(Source::Developed(e))],
        }
    }
}

/// Bilan d'un calcul : images calculées, déjà en cache, échecs (et la raison du premier),
/// interrompu.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Images dont au moins un niveau a été calculé.
    pub computed: usize,
    /// Images déjà entièrement en cache.
    pub cached: usize,
    /// Images en échec.
    pub failed: usize,
    /// Raison du premier échec.
    pub error: Option<String>,
    /// Interrompu avant la fin.
    pub cancelled: bool,
}

/// Tour du travail lourd (LibRaw, logiciels de développement) : un seul calcul à la fois,
/// toutes tâches confondues.
static HEAVY: Mutex<()> = Mutex::new(());

/// Attend le tour du travail lourd ; le rend en lâchant la garde.
pub fn heavy_turn() -> MutexGuard<'static, ()> {
    HEAVY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Calcule `kind` pour les images `files`, par `producer` ; `recreate` : refait même ce
/// qui est en cache (l'appelant efface d'abord ses niveaux, [`Kind::cache_levels`]).
/// `step(faites, total)` après chaque image ; `cancel` interrompt entre deux images (et le
/// calcul en cours, s'il le permet).
pub fn run(
    files: &[MediaFile],
    kind: Kind,
    producer: &dyn Producer,
    cancel: &AtomicBool,
    step: &dyn Fn(usize, usize),
) -> Summary {
    let todo: Vec<(&MediaFile, Vec<Level>)> = files
        .iter()
        .map(|f| (f, kind.levels(f)))
        .filter(|(_, levels)| !levels.is_empty())
        .collect();
    let total = todo.len();
    let mut summary = Summary::default();
    step(0, total);
    for (done, (file, levels)) in todo.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            break;
        }
        let mut computed = false;
        let mut failed = None;
        for level in levels {
            if producer.is_done(file, level) {
                continue;
            }
            let _turn = crate::scheduler::is_heavy(file.kind, level).then(heavy_turn);
            match producer.produce(file, level, cancel) {
                Ok(()) => computed = true,
                Err(Failure::Cancelled) => {
                    summary.cancelled = true;
                    break;
                }
                Err(Failure::Error(e)) => {
                    failed = Some(e);
                    break;
                }
            }
        }
        if summary.cancelled {
            break;
        }
        match failed {
            Some(e) => {
                summary.failed += 1;
                summary.error.get_or_insert(e);
            }
            None if computed => summary.computed += 1,
            None => summary.cached += 1,
        }
        step(done + 1, total);
    }
    summary
}

/// « Recréer » : efface du cache ce que `kind` calcule pour les images du dossier `dir`,
/// avant [`run`].
pub fn clear(
    cache: &crate::cache::Cache,
    dir: &std::path::Path,
    kind: Kind,
) -> std::io::Result<()> {
    for level in kind.cache_levels() {
        cache.clear_folder(dir, Some(level))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Faux producteur : ce qui est « en cache », ce qui a été calculé.
    #[derive(Default)]
    struct Fake {
        cached: HashSet<(PathBuf, Level)>,
        made: Mutex<Vec<(PathBuf, Level)>>,
        fail: Option<PathBuf>,
    }

    impl Producer for Fake {
        fn is_done(&self, file: &MediaFile, level: Level) -> bool {
            self.cached.contains(&(file.path.clone(), level))
        }

        fn produce(&self, file: &MediaFile, level: Level, _: &AtomicBool) -> Result<(), Failure> {
            if self.fail.as_ref() == Some(&file.path) {
                return Err(Failure::Error("cassé".into()));
            }
            self.made.lock().unwrap().push((file.path.clone(), level));
            Ok(())
        }
    }

    fn file(name: &str, kind: MediaKind) -> MediaFile {
        MediaFile {
            path: PathBuf::from(name),
            kind,
        }
    }

    #[test]
    fn ids_round_trip() {
        for kind in Kind::all() {
            assert_eq!(Kind::parse(&kind.id()), Some(kind), "{kind:?}");
        }
        assert_eq!(
            Kind::parse("palette:raw"),
            Some(Kind::Palette(SourceKind::Raw))
        );
        assert_eq!(Kind::parse("nothing"), None);
    }

    #[test]
    fn cached_entries_are_skipped_and_raw_renders_concern_raws_only() {
        let files = [
            file("a.raf", MediaKind::Raw),
            file("b.jpg", MediaKind::Flat),
        ];
        let fake = Fake {
            cached: [(PathBuf::from("a.raf"), Level::Thumbnail)].into(),
            ..Fake::default()
        };
        let steps = Mutex::new(Vec::new());
        let s = run(
            &files,
            Kind::Previews,
            &fake,
            &AtomicBool::new(false),
            &|d, t| steps.lock().unwrap().push((d, t)),
        );
        assert_eq!((s.computed, s.cached, s.failed), (2, 0, 0));
        assert_eq!(
            *fake.made.lock().unwrap(),
            vec![
                (PathBuf::from("a.raf"), Level::Embedded),
                (PathBuf::from("b.jpg"), Level::Thumbnail),
                (PathBuf::from("b.jpg"), Level::Standard),
            ]
        );
        assert_eq!(*steps.lock().unwrap(), vec![(0, 2), (1, 2), (2, 2)]);
        let fake = Fake::default();
        let s = run(
            &files,
            Kind::Raw,
            &fake,
            &AtomicBool::new(false),
            &|_, _| {},
        );
        assert_eq!(s.computed, 1, "seul le RAF a un rendu RAW");
    }

    #[test]
    fn failures_are_counted_and_cancel_stops() {
        let files = [
            file("a.jpg", MediaKind::Flat),
            file("b.jpg", MediaKind::Flat),
        ];
        let fake = Fake {
            fail: Some(PathBuf::from("a.jpg")),
            ..Fake::default()
        };
        let s = run(
            &files,
            Kind::Sharpness,
            &fake,
            &AtomicBool::new(false),
            &|_, _| {},
        );
        assert_eq!((s.computed, s.failed), (1, 1));
        assert_eq!(s.error.as_deref(), Some("cassé"));
        let s = run(
            &files,
            Kind::Sharpness,
            &Fake::default(),
            &AtomicBool::new(true),
            &|_, _| {},
        );
        assert!(s.cancelled && s.computed == 0);
    }
}

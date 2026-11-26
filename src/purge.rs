//! Ménage de ce qui grandit sans limite (#604, idée de l'utilisateur : des photographes
//! ont vraiment beaucoup d'images) :
//!
//! - l'**historique des déchargements** (`offload-history.tsv`) : une carte qui revient
//!   avec des photos de plus de quelques semaines est très peu probable ; les entrées
//!   plus anciennes que `[purge] offload_history` (âge de la *photo*, sa date de prise de
//!   vue) partent dans une archive à côté, jamais relue
//!   ([`crate::offload::History::purge`]) ;
//! - le **cache** : les entrées des dossiers qui n'existent plus (`cache_gone`), et celles
//!   des dossiers pas ouverts depuis `cache_unused` ([`crate::cache::Cache::purge`]) ;
//! - les **données de tri** des dossiers qui n'existent plus (`folder_data_gone`,
//!   [`crate::store::purge_gone`]).
//!
//! **Un dossier introuvable n'a pas forcément disparu** : disque débranché, carte retirée,
//! NAS hors ligne, partage pas monté. [`gone`] ne le dit disparu que si son volume est là
//! et lui pas. Les données gardées sur un disque (`.tri/`) voyagent avec lui : jamais
//! touchées.
//!
//! Quand : en fond au lancement de l'interface, une fois par `interval_hours` au plus
//! ([`due`]) ; « Nettoyer maintenant » dans le panneau du cache ; `tri --purge`.
//! Voir `doc/en/purge.md`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Deserialize;

/// Combien de temps garder (liste déroulante des réglages).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Keep {
    /// Une semaine.
    #[serde(rename = "1w")]
    Week,
    /// Un mois (30 jours).
    #[serde(rename = "1m")]
    Month,
    /// Trois mois.
    #[serde(rename = "3m")]
    Quarter,
    /// Un an.
    #[serde(rename = "1y")]
    Year,
    /// Trois ans.
    #[serde(rename = "3y")]
    ThreeYears,
    /// Toujours.
    #[serde(rename = "never")]
    Never,
}

impl Keep {
    /// Durée gardée ; `None` : toujours.
    pub fn duration(self) -> Option<Duration> {
        const DAY: u64 = 24 * 3600;
        let days = match self {
            Keep::Week => 7,
            Keep::Month => 30,
            Keep::Quarter => 91,
            Keep::Year => 365,
            Keep::ThreeYears => 3 * 365,
            Keep::Never => return None,
        };
        Some(Duration::from_secs(days * DAY))
    }

    /// Date avant laquelle on ne garde plus, depuis `now` ; `None` : toujours gardé.
    pub fn cutoff(self, now: SystemTime) -> Option<SystemTime> {
        now.checked_sub(self.duration()?)
    }
}

/// Réglages du ménage (section `[purge]` du fichier de configuration).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PurgeSettings {
    /// Historique des déchargements : âge des photos au-delà duquel leurs entrées partent
    /// dans l'archive.
    pub offload_history: Keep,
    /// Cache : entrées des dossiers pas ouverts depuis ce temps.
    pub cache_unused: Keep,
    /// Cache : entrées des dossiers qui n'existent plus.
    pub cache_gone: bool,
    /// Données de tri des dossiers qui n'existent plus.
    pub folder_data_gone: bool,
    /// Ménage automatique au plus une fois par tant d'heures (0 : jamais d'office).
    pub interval_hours: u32,
}

impl Default for PurgeSettings {
    fn default() -> Self {
        Self {
            offload_history: Keep::Year,
            cache_unused: Keep::Never,
            cache_gone: true,
            folder_data_gone: true,
            interval_hours: 24,
        }
    }
}

/// Ce que le ménage a fait (ou ferait, à l'essai).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Entrées de l'historique des déchargements archivées.
    pub history: usize,
    /// Cache libéré.
    pub cache: crate::cache::Usage,
    /// Données de tri de dossiers disparus supprimées.
    pub folders: usize,
    /// Erreurs (étape et message).
    pub errors: Vec<String>,
}

impl Report {
    /// Vrai si rien n'a été (ou ne serait) fait.
    pub fn is_empty(&self) -> bool {
        self.history == 0 && self.cache.entries == 0 && self.folders == 0
    }
}

/// Vrai si `folder` (absolu) a disparu : il n'existe plus **et** son volume est là. Un
/// dossier sur un disque débranché, une carte retirée, un partage pas monté n'a pas
/// disparu : son plus proche parent existant est un point de montage vide ou un dossier
/// de montage (`/Volumes`, `/media/<utilisateur>`, `/run/media/<utilisateur>`, `/run`, `/mnt`,
/// GVFS…), la racine d'un disque, ou rien n'existe au-dessus (lecteur Windows absent).
pub fn gone(folder: &Path) -> bool {
    if !folder.is_absolute() || !matches!(folder.try_exists(), Ok(false)) {
        return false;
    }
    let mut above = folder.parent();
    while let Some(dir) = above {
        match dir.try_exists() {
            // La racine d'un disque comme seul parent : trop incertain, gardé.
            Ok(true) => {
                return dir.parent().is_some() && !mount_container(dir) && !empty_dir(dir);
            }
            Ok(false) => above = dir.parent(),
            Err(_) => return false,
        }
    }
    false
}

/// Dossier qui reçoit des montages (un volume absent y laisse un vide).
fn mount_container(dir: &Path) -> bool {
    let parent = dir.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    [
        "/Volumes",
        "/media",
        "/mnt",
        // `/run/media` pas encore créé (Ubuntu, aucun disque branché depuis le démarrage).
        "/run",
        "/run/media",
        "/net",
        "/Network",
        "/cygdrive",
    ]
    .iter()
    .any(|c| dir == Path::new(c))
        || parent == Path::new("/media")
        || parent == Path::new("/run/media")
        || name == "gvfs"
        || dir.ends_with("Library/CloudStorage")
}

/// Dossier vide (un point de montage dont le volume est absent, NFS par exemple).
fn empty_dir(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
}

/// Fichier qui retient la date du dernier ménage, dans le dossier de données.
const STAMP: &str = "last-purge";

/// Vrai si le ménage automatique est dû : `interval_hours` écoulées depuis le dernier
/// (jamais si 0).
pub fn due(data: &Path, settings: &PurgeSettings, now: SystemTime) -> bool {
    if settings.interval_hours == 0 {
        return false;
    }
    let every = Duration::from_secs(u64::from(settings.interval_hours) * 3600);
    match fs::metadata(data.join(STAMP)).and_then(|m| m.modified()) {
        Ok(last) => now.duration_since(last).is_ok_and(|d| d >= every),
        Err(_) => true,
    }
}

/// Retient que le ménage vient d'être fait.
pub fn mark_done(data: &Path) -> io::Result<()> {
    fs::create_dir_all(data)?;
    fs::write(data.join(STAMP), b"")
}

/// Fait le ménage (ou l'essaie, `dry_run` : rien n'est touché, le bilan dit ce qui
/// partirait). `cache` : le cache central ; `data` : le dossier de données ; `history` :
/// le fichier de l'historique des déchargements.
pub fn run(
    settings: &PurgeSettings,
    cache: Option<&crate::cache::Cache>,
    data: Option<&Path>,
    history: Option<PathBuf>,
    now: SystemTime,
    dry_run: bool,
) -> Report {
    let mut report = Report::default();
    if let (Some(path), Some(before)) = (history, settings.offload_history.cutoff(now)) {
        match crate::offload::History::open(path).and_then(|mut h| h.purge(before, dry_run)) {
            Ok(n) => report.history = n,
            Err(e) => report.errors.push(format!("offload history: {e}")),
        }
    }
    if let Some(cache) = cache {
        let unused = settings.cache_unused.cutoff(now);
        if settings.cache_gone || unused.is_some() {
            let gone_too = settings.cache_gone;
            match cache.purge(unused, |f| gone_too && gone(f), dry_run) {
                Ok(usage) => report.cache = usage,
                Err(e) => report.errors.push(format!("cache: {e}")),
            }
        }
    }
    if let Some(data) = data
        && settings.folder_data_gone
    {
        report.folders = crate::store::purge_gone(data, gone, dry_run);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_gone_only_when_its_volume_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        fs::create_dir_all(photos.join("garde")).unwrap();
        assert!(!gone(&photos.join("garde")), "existe");
        assert!(gone(&photos.join("supprimé")), "parent présent : disparu");
        assert!(gone(&photos.join("a/b/c")), "plus haut présent : disparu");
        // Point de montage vide (NAS pas monté) : pas disparu.
        let mount = dir.path().join("nas");
        fs::create_dir_all(&mount).unwrap();
        assert!(!gone(&mount.join("2026")));
        // Dossiers de montage : un disque absent n'a pas disparu.
        assert!(!gone(Path::new("/Volumes/CARTE-ABSENTE/DCIM")));
        assert!(!gone(Path::new("/media/alice/CARTE/DCIM")));
        assert!(!gone(Path::new("/run/media/alice/CARTE/DCIM")));
        assert!(!gone(Path::new("relatif/dossier")));
    }

    #[test]
    fn the_purge_is_due_once_per_interval() {
        let dir = tempfile::tempdir().unwrap();
        let settings = PurgeSettings::default();
        let now = SystemTime::now();
        assert!(due(dir.path(), &settings, now), "jamais fait");
        mark_done(dir.path()).unwrap();
        assert!(!due(dir.path(), &settings, now));
        assert!(due(
            dir.path(),
            &settings,
            now + Duration::from_secs(25 * 3600)
        ));
        let off = PurgeSettings {
            interval_hours: 0,
            ..settings
        };
        assert!(!due(
            dir.path(),
            &off,
            now + Duration::from_secs(1000 * 3600)
        ));
    }

    #[test]
    fn keep_durations() {
        assert_eq!(Keep::Never.duration(), None);
        assert_eq!(Keep::Week.duration(), Some(Duration::from_secs(7 * 86400)));
        let now = SystemTime::now();
        assert!(Keep::Year.cutoff(now).unwrap() < now);
    }
}

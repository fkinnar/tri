//! Journal des exports (20/10/2026, idée de l'utilisateur) : chaque export terminé, avec
//! les fichiers écrits et leurs dossiers, pour retrouver après coup où ils sont partis.
//! Gardé d'une session à l'autre dans le dossier de données (`exports.jsonl`, un objet
//! JSON par ligne : un export ajouté ne réécrit pas le reste), limité aux
//! `[export] report_max` plus récents, effaçable.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Fichier du journal dans le dossier de données.
const FILE: &str = "exports.jsonl";

/// Comment un export s'est arrêté avant la fin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stop {
    /// Arrêté par l'utilisateur (les fichiers déjà écrits restent, non listés).
    Cancelled,
    /// Échec d'ensemble (darktable introuvable…), avec son message.
    Failed(String),
}

/// Un export terminé.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Fin de l'export, en secondes depuis 1970 (UTC).
    pub finished: u64,
    /// Nom du préréglage d'export à ce moment-là.
    pub preset: String,
    /// Nom de la version exportée (recadrage), sinon l'image entière.
    #[serde(default)]
    pub version: Option<String>,
    /// Nombre d'images demandées.
    pub total: usize,
    /// Fichiers écrits, dans l'ordre.
    pub written: Vec<PathBuf>,
    /// Images sans fichier écrit, et pourquoi.
    #[serde(default)]
    pub errors: Vec<(PathBuf, String)>,
    /// Dont copiés sans recompression.
    #[serde(default)]
    pub passthrough: usize,
    /// Arrêt avant la fin, sinon `None`.
    #[serde(default)]
    pub stopped: Option<Stop>,
}

impl Entry {
    /// Export qui se termine maintenant.
    pub fn now(preset: &str, version: Option<&str>, total: usize) -> Entry {
        Entry {
            finished: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            preset: preset.to_string(),
            version: version.map(str::to_string),
            total,
            written: Vec::new(),
            errors: Vec::new(),
            passthrough: 0,
            stopped: None,
        }
    }

    /// Avec le bilan de l'export (`report`).
    pub fn with_report(mut self, report: &crate::export::Report) -> Entry {
        self.written = report.written.clone();
        self.errors = report.errors.clone();
        self.passthrough = report.passthrough;
        self
    }

    /// Les dossiers de destination, dans l'ordre de leur premier fichier, chacun avec ses
    /// fichiers (un dossier relatif aux images en donne un par dossier d'images).
    pub fn folders(&self) -> Vec<(PathBuf, Vec<&PathBuf>)> {
        let mut folders: Vec<(PathBuf, Vec<&PathBuf>)> = Vec::new();
        for file in &self.written {
            let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
            match folders.iter_mut().find(|(d, _)| *d == dir) {
                Some((_, files)) => files.push(file),
                None => folders.push((dir, vec![file])),
            }
        }
        folders
    }

    /// Secondes écoulées depuis la fin de l'export, à l'instant `now` (secondes depuis
    /// 1970).
    pub fn age(&self, now: u64) -> u64 {
        now.saturating_sub(self.finished)
    }
}

/// Le journal du dossier de données `data`, du plus ancien au plus récent. Une ligne
/// illisible (fichier tronqué, version future) est ignorée.
pub fn read(data: &Path) -> Vec<Entry> {
    fs::read_to_string(data.join(FILE))
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Ajoute `entry` au journal ; au-delà de `max` exports (0 : sans limite), les plus
/// anciens sont retirés.
pub fn append(data: &Path, entry: &Entry, max: usize) -> io::Result<()> {
    fs::create_dir_all(data)?;
    let line = serde_json::to_string(entry).map_err(io::Error::other)?;
    let path = data.join(FILE);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{line}")?;
    drop(file);
    let entries = read(data);
    if max > 0 && entries.len() > max {
        let mut text = String::new();
        for e in &entries[entries.len() - max..] {
            text.push_str(&serde_json::to_string(e).map_err(io::Error::other)?);
            text.push('\n');
        }
        fs::write(&path, text)?;
    }
    Ok(())
}

/// Efface le journal.
pub fn clear(data: &Path) -> io::Result<()> {
    match fs::remove_file(data.join(FILE)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(preset: &str, files: &[&str]) -> Entry {
        Entry {
            written: files.iter().map(PathBuf::from).collect(),
            ..Entry::now(preset, None, files.len())
        }
    }

    #[test]
    fn the_log_is_kept_limited_and_cleared() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path()).is_empty(), "pas encore de journal");
        for i in 0..5 {
            append(dir.path(), &entry(&format!("p{i}"), &["/a/x.jpg"]), 3).unwrap();
        }
        let names: Vec<String> = read(dir.path()).into_iter().map(|e| e.preset).collect();
        assert_eq!(names, ["p2", "p3", "p4"], "les 3 plus récents");
        // Une ligne abîmée n'empêche pas de lire les autres.
        let path = dir.path().join(FILE);
        let mut text = fs::read_to_string(&path).unwrap();
        text.push_str("{pas du json\n");
        fs::write(&path, text).unwrap();
        assert_eq!(read(dir.path()).len(), 3);
        clear(dir.path()).unwrap();
        assert!(read(dir.path()).is_empty());
        clear(dir.path()).unwrap();
    }

    #[test]
    fn files_are_grouped_by_folder() {
        let e = entry(
            "web",
            &["/a/export/1.jpg", "/b/export/2.jpg", "/a/export/3.jpg"],
        );
        let folders = e.folders();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].0, PathBuf::from("/a/export"));
        assert_eq!(folders[0].1.len(), 2);
        assert_eq!(folders[1].0, PathBuf::from("/b/export"));
        assert_eq!(e.age(e.finished + 90), 90);
        assert_eq!(e.age(0), 0);
    }

    #[test]
    fn a_stopped_export_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let mut e = entry("edit", &[]);
        e.stopped = Some(Stop::Failed("darktable-cli introuvable".into()));
        e.errors = vec![(PathBuf::from("/a/DSCF1.RAF"), "illisible".into())];
        e.version = Some("Story".into());
        append(dir.path(), &e, 0).unwrap();
        assert_eq!(read(dir.path()), [e]);
    }
}

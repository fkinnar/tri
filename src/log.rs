//! Journal de l'application (issue n° 398) : `tri.log` dans le dossier de données.
//!
//! Pourquoi : un rendu darktable échouait dans trois dossiers avec pour seul message
//! « 1 échec (Darktable : darktable-cli) », sans image ni raison (un `.xmp` abîmé, trouvé
//! en effaçant tous les sidecars). Chez un utilisateur, un tel signalement ne se
//! diagnostique pas. Le message de la barre d'état reste court ; le détail complet
//! (image, logiciel, code de sortie, messages, arguments…) va au journal.
//!
//! - Le **moteur** écrit (il connaît le détail) : l'interface et la ligne de commande
//!   partagent le même fichier.
//! - Une entrée : une ligne d'en-tête (date locale, niveau, source, identifiant, message),
//!   puis les lignes du détail, en retrait de deux espaces. Lisible tel quel, joignable à
//!   un signalement, relu par [`read`] pour la fenêtre du journal.
//! - Taille plafonnée : au-delà de `[log] max_kb`, le fichier devient `tri.1.log` (le
//!   précédent `tri.2.log`…), `[log] keep` anciens fichiers gardés.
//! - Sans [`init`] (tests, outils), rien n'est écrit.
//! - Les plantages (anciennement `crash.log`) y vont aussi, niveau [`Level::Crash`].

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;

/// Nom du journal, dans le dossier de données.
pub const FILE: &str = "tri.log";

/// Ancien journal des plantages, repris dans [`FILE`] au premier lancement.
pub const OLD_CRASH_FILE: &str = "crash.log";

/// Section `[log]` du fichier de configuration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LogSettings {
    /// Taille au-delà de laquelle le journal passe à un nouveau fichier (Ko).
    pub max_kb: u64,
    /// Anciens fichiers gardés (`tri.1.log`, `tri.2.log`…) ; 0 : aucun.
    pub keep: usize,
    /// Trace détaillée (étapes du chargement des images, images lentes de l'interface)
    /// aussi au journal : pour reproduire un problème à la demande. Volumineux.
    pub trace: bool,
}

impl Default for LogSettings {
    fn default() -> Self {
        LogSettings {
            max_kb: 1024,
            keep: 3,
            trace: false,
        }
    }
}

/// Gravité d'une entrée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Trace détaillée (`[log] trace`).
    Trace,
    /// Information (lancement, réglages…).
    Info,
    /// Avertissement : quelque chose n'a pas marché, sans empêcher le reste.
    Warning,
    /// Erreur : une action a échoué.
    Error,
    /// Plantage (erreur Rust, `panic`) ou erreur de la carte graphique.
    Crash,
}

impl Level {
    /// Tous les niveaux, du moins au plus grave.
    pub const ALL: [Level; 5] = [
        Level::Trace,
        Level::Info,
        Level::Warning,
        Level::Error,
        Level::Crash,
    ];

    /// Mot du fichier (anglais, fixe : le journal se joint aux signalements).
    pub fn tag(self) -> &'static str {
        match self {
            Level::Trace => "TRACE",
            Level::Info => "INFO",
            Level::Warning => "WARN",
            Level::Error => "ERROR",
            Level::Crash => "CRASH",
        }
    }

    /// Inverse de [`Level::tag`].
    pub fn parse(tag: &str) -> Option<Level> {
        Level::ALL.into_iter().find(|l| l.tag() == tag)
    }
}

/// Une entrée relue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Identifiant (`<processus>-<numéro>`), unique d'un lancement à l'autre en pratique.
    pub id: String,
    /// Date et heure locales, `AAAA-MM-JJ HH:MM:SS.mmm`.
    pub time: String,
    /// Gravité.
    pub level: Level,
    /// Partie de `tri` (`develop`, `export`, `xmp`, `gui`…).
    pub source: String,
    /// Message, une ligne.
    pub message: String,
    /// Détail, plusieurs lignes possibles (vide s'il n'y en a pas).
    pub detail: String,
}

impl Entry {
    /// L'entrée telle qu'écrite dans le fichier (en-tête, puis détail en retrait).
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "{} {} [{}] #{} {}\n",
            self.time,
            self.level.tag(),
            self.source,
            self.id,
            self.message
        );
        for line in self.detail.lines() {
            text.push_str("  ");
            text.push_str(line);
            text.push('\n');
        }
        text
    }
}

struct Logger {
    dir: PathBuf,
    settings: LogSettings,
}

static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// Dossier propre au fil courant, prioritaire sur [`init`] : les tests de l'interface
    /// tournent en parallèle, chacun avec son dossier de données temporaire.
    static THREAD_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Active le journal dans `dir` (le dossier de données) et reprend l'ancien `crash.log`.
pub fn init(dir: PathBuf, settings: LogSettings) {
    import_old_crashes(&dir, &settings);
    *lock() = Some(Logger { dir, settings });
}

/// Nouveaux réglages (fenêtre de réglages), sans changer de dossier.
pub fn set_settings(settings: LogSettings) {
    if let Some(logger) = lock().as_mut() {
        logger.settings = settings;
    }
}

/// Journal propre au fil courant (tests) : les écritures de ce fil vont dans `dir`, avec
/// les réglages par défaut ; `None` revient au journal commun.
pub fn set_thread_dir(dir: Option<PathBuf>) {
    THREAD_DIR.with(|d| *d.borrow_mut() = dir);
}

fn lock() -> std::sync::MutexGuard<'static, Option<Logger>> {
    // Un plantage pendant une écriture ne doit pas couper le journal ensuite.
    LOGGER.lock().unwrap_or_else(|e| e.into_inner())
}

/// Dossier et réglages en vigueur pour le fil courant.
fn current() -> Option<(PathBuf, LogSettings)> {
    if let Some(dir) = THREAD_DIR.with(|d| d.borrow().clone()) {
        return Some((dir, LogSettings::default()));
    }
    lock().as_ref().map(|l| (l.dir.clone(), l.settings.clone()))
}

/// Le dossier du journal (celui du fil courant, sinon celui de [`init`]).
pub fn dir() -> Option<PathBuf> {
    current().map(|(dir, _)| dir)
}

/// Le fichier du journal en cours, s'il est actif.
pub fn path() -> Option<PathBuf> {
    dir().map(|d| d.join(FILE))
}

/// Vrai si la trace détaillée va au journal (`[log] trace`).
pub fn tracing() -> bool {
    current().is_some_and(|(_, s)| s.trace)
}

/// Numéro de la dernière entrée écrite par ce processus (0 : aucune). Une erreur montrée
/// dans la barre d'état retient ce numéro, pour ouvrir le journal sur son entrée.
pub fn last_number() -> u64 {
    NEXT.load(Ordering::Relaxed) - 1
}

/// Identifiant de l'entrée numéro `number` de ce processus.
pub fn id_of(number: u64) -> String {
    format!("{}-{number}", std::process::id())
}

/// Écrit une entrée ; renvoie son identifiant, ou `None` si le journal n'est pas actif
/// (ou si l'écriture a échoué : le journal ne doit jamais faire échouer une action).
/// Un message de plusieurs lignes garde la première en message, le reste en détail.
pub fn write(level: Level, source: &str, message: &str, detail: &str) -> Option<String> {
    let (dir, settings) = current()?;
    if level == Level::Trace && !settings.trace {
        return None;
    }
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let entry = entry(id_of(number), level, source, message, detail);
    append(&dir, &settings, &entry).ok()?;
    Some(entry.id)
}

/// Une entrée à la date de maintenant (message sur une ligne, le reste en détail).
pub fn entry(id: String, level: Level, source: &str, message: &str, detail: &str) -> Entry {
    let mut lines = message.trim().lines();
    let first = lines.next().unwrap_or_default().trim().to_string();
    let rest: Vec<&str> = lines.collect();
    let mut full = rest.join("\n");
    if !detail.trim().is_empty() {
        if !full.is_empty() {
            full.push('\n');
        }
        full.push_str(detail.trim_end());
    }
    Entry {
        id,
        time: chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string(),
        level,
        source: source.to_string(),
        message: first,
        detail: full,
    }
}

/// Erreur : une action a échoué.
pub fn error(source: &str, message: &str, detail: &str) -> Option<String> {
    write(Level::Error, source, message, detail)
}

/// Avertissement.
pub fn warning(source: &str, message: &str, detail: &str) -> Option<String> {
    write(Level::Warning, source, message, detail)
}

/// Information.
pub fn info(source: &str, message: &str, detail: &str) -> Option<String> {
    write(Level::Info, source, message, detail)
}

/// Trace détaillée, seulement si `[log] trace` est actif (le message n'est construit que
/// dans ce cas).
pub fn trace(source: &str, message: impl FnOnce() -> String) {
    if tracing() {
        write(Level::Trace, source, &message(), "");
    }
}

/// Ajoute `entry` au journal de `dir`, en passant à un nouveau fichier au-delà de la
/// taille maximale. Utilisable sans [`init`] (crochet des plantages).
pub fn append(dir: &Path, settings: &LogSettings, entry: &Entry) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE);
    let text = entry.to_text();
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size > 0 && size + text.len() as u64 > settings.max_kb.max(1) * 1024 {
        rotate(dir, settings.keep);
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(text.as_bytes())?;
    Ok(path)
}

/// Nom du `n`-ième ancien fichier (`tri.1.log`…) ; 0 : le journal en cours.
pub fn file_name(n: usize) -> String {
    if n == 0 {
        FILE.to_string()
    } else {
        format!("tri.{n}.log")
    }
}

/// `tri.log` devient `tri.1.log`, `tri.1.log` devient `tri.2.log`… ; au-delà de `keep`,
/// supprimé.
fn rotate(dir: &Path, keep: usize) {
    let _ = std::fs::remove_file(dir.join(file_name(keep.max(1))));
    if keep == 0 {
        let _ = std::fs::remove_file(dir.join(FILE));
        return;
    }
    for n in (0..keep).rev() {
        let from = dir.join(file_name(n));
        if from.exists() {
            let _ = std::fs::rename(&from, dir.join(file_name(n + 1)));
        }
    }
}

/// L'ancien `crash.log` (avant le 10/11/2026) : repris en une entrée, puis supprimé.
fn import_old_crashes(dir: &Path, settings: &LogSettings) {
    let old = dir.join(OLD_CRASH_FILE);
    let Ok(text) = std::fs::read_to_string(&old) else {
        return;
    };
    let entry = entry(
        id_of(NEXT.fetch_add(1, Ordering::Relaxed)),
        Level::Crash,
        "crash",
        "Earlier crashes (crash.log)",
        &text,
    );
    if append(dir, settings, &entry).is_ok() {
        let _ = std::fs::remove_file(old);
    }
}

/// Relit une ligne d'en-tête : `date heure NIVEAU [source] #id message`.
fn parse_header(line: &str) -> Option<Entry> {
    let bytes = line.as_bytes();
    if line.len() < 24 || !bytes[0].is_ascii_digit() || bytes[10] != b' ' || bytes[23] != b' ' {
        return None;
    }
    let time = line.get(..23)?.to_string();
    let rest = &line[24..];
    let (tag, rest) = rest.split_once(' ')?;
    let level = Level::parse(tag)?;
    let rest = rest.strip_prefix('[')?;
    let (source, rest) = rest.split_once("] ")?;
    let rest = rest.strip_prefix('#')?;
    let (id, message) = rest.split_once(' ').unwrap_or((rest, ""));
    Some(Entry {
        id: id.to_string(),
        time,
        level,
        source: source.to_string(),
        message: message.to_string(),
        detail: String::new(),
    })
}

/// Relit le texte d'un journal, dans l'ordre du fichier. Une ligne qui n'est pas un
/// en-tête complète le détail de l'entrée précédente.
pub fn parse(text: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    for line in text.lines() {
        if let Some(entry) = parse_header(line) {
            entries.push(entry);
            continue;
        }
        let line = line.strip_prefix("  ").unwrap_or(line);
        match entries.last_mut() {
            Some(last) => {
                if !last.detail.is_empty() {
                    last.detail.push('\n');
                }
                last.detail.push_str(line);
            }
            None if !line.trim().is_empty() => entries.push(Entry {
                id: String::new(),
                time: String::new(),
                level: Level::Info,
                source: String::new(),
                message: line.to_string(),
                detail: String::new(),
            }),
            None => {}
        }
    }
    entries
}

/// Toutes les entrées du journal de `dir` (fichiers anciens compris), **les plus récentes
/// d'abord**.
pub fn read(dir: &Path) -> Vec<Entry> {
    let mut names: Vec<(usize, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let n = if name == FILE {
                0
            } else {
                name.strip_prefix("tri.")?
                    .strip_suffix(".log")?
                    .parse()
                    .ok()?
            };
            Some((n, e.path()))
        })
        .collect();
    // Du plus ancien (numéro le plus grand) au plus récent.
    names.sort_by_key(|a| std::cmp::Reverse(a.0));
    let mut entries: Vec<Entry> = names
        .into_iter()
        .filter_map(|(_, path)| std::fs::read(path).ok())
        .flat_map(|bytes| parse(&String::from_utf8_lossy(&bytes)))
        .collect();
    entries.reverse();
    entries
}

/// Les `count` dernières entrées de `dir`, dans l'ordre du fichier, en texte : à joindre
/// à un signalement.
pub fn tail(dir: &Path, count: usize) -> String {
    let mut entries = read(dir);
    entries.truncate(count);
    entries.iter().rev().map(Entry::to_text).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, level: Level, message: &str, detail: &str) -> Entry {
        let mut entry = entry(id.into(), level, "develop", message, detail);
        entry.time = "2026-11-10 15:03:12.345".into();
        entry
    }

    #[test]
    fn an_entry_is_one_header_line_and_its_indented_detail() {
        let entry = sample(
            "42-7",
            Level::Error,
            "darktable failed on DSCF0001.RAF",
            "exit code: 1\nstderr:\nbad xmp",
        );
        assert_eq!(
            entry.to_text(),
            "2026-11-10 15:03:12.345 ERROR [develop] #42-7 darktable failed on DSCF0001.RAF\n  exit code: 1\n  stderr:\n  bad xmp\n"
        );
        assert_eq!(parse(&entry.to_text()), vec![entry]);
    }

    #[test]
    fn a_message_on_several_lines_keeps_the_first_as_message() {
        let entry = sample("1-1", Level::Warning, "first\nsecond", "detail");
        assert_eq!(entry.message, "first");
        assert_eq!(entry.detail, "second\ndetail");
    }

    #[test]
    fn lines_that_are_not_headers_extend_the_previous_detail() {
        let text = "stray line\n2026-11-10 15:03:12.345 INFO [gui] #1-1 started\nfree text\n";
        let entries = parse(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "stray line");
        assert_eq!(entries[1].detail, "free text");
    }

    #[test]
    fn the_log_rotates_and_keeps_the_configured_number_of_files() {
        let dir = tempfile::tempdir().unwrap();
        let settings = LogSettings {
            max_kb: 1,
            keep: 2,
            trace: false,
        };
        let long = "x".repeat(600);
        for n in 0..6 {
            append(
                dir.path(),
                &settings,
                &sample(&format!("1-{n}"), Level::Error, &long, ""),
            )
            .unwrap();
        }
        let mut files: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(files, ["tri.1.log", "tri.2.log", "tri.log"]);
        let ids: Vec<String> = read(dir.path()).into_iter().map(|e| e.id).collect();
        assert_eq!(ids, ["1-5", "1-4", "1-3"], "newest first, oldest dropped");
    }

    #[test]
    fn tail_gives_the_last_entries_in_file_order() {
        let dir = tempfile::tempdir().unwrap();
        for n in 0..4 {
            append(
                dir.path(),
                &LogSettings::default(),
                &sample(&format!("1-{n}"), Level::Info, &format!("m{n}"), ""),
            )
            .unwrap();
        }
        let tail = tail(dir.path(), 2);
        assert!(tail.find("m2").unwrap() < tail.find("m3").unwrap());
        assert!(!tail.contains("m1"));
    }

    #[test]
    fn writing_goes_to_the_thread_directory_and_trace_stays_off_by_default() {
        let dir = tempfile::tempdir().unwrap();
        set_thread_dir(Some(dir.path().to_path_buf()));
        let id = error("export", "failed", "full detail").unwrap();
        trace("gui", || "not written".into());
        set_thread_dir(None);
        let entries = read(dir.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, id);
        assert_eq!(entries[0].detail, "full detail");
        assert!(last_number() > 0);
    }

    #[test]
    fn the_old_crash_log_is_taken_in() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(OLD_CRASH_FILE), "=== plantage ===\nboom\n").unwrap();
        import_old_crashes(dir.path(), &LogSettings::default());
        assert!(!dir.path().join(OLD_CRASH_FILE).exists());
        let entries = read(dir.path());
        assert_eq!(entries[0].level, Level::Crash);
        assert!(entries[0].detail.contains("boom"));
    }
}

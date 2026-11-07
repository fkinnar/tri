//! Journal des plantages (19/10/2026) : un plantage sur macOS n'avait laissé qu'un rapport
//! du système sans le message d'erreur (une erreur Rust pendant le dessin de la fenêtre,
//! appelé depuis AppKit, arrête le programme net, et le message du terminal s'était
//! perdu). À toute erreur Rust (`panic`), le message, l'endroit (fichier, ligne), le fil
//! et la pile d'appels sont ajoutés à `crash.log` dans le dossier de données, avant le
//! comportement habituel (message sur la sortie d'erreur, arrêt).

use std::io::Write;
use std::path::{Path, PathBuf};

/// Nom du journal, dans le dossier de données.
pub const FILE: &str = "crash.log";

/// Un rapport : date (secondes depuis 1970, sans dépendance de date), version, fil,
/// endroit, message et pile.
pub fn report(thread: &str, location: &str, message: &str, backtrace: &str) -> String {
    let when = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    format!(
        "=== plantage (horodatage {when}, tri {}, {}) ===\nfil : {thread}\nendroit : {location}\nmessage : {message}\npile :\n{backtrace}\n\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
    )
}

/// Ajoute `text` au journal de `dir` (créé au besoin).
pub fn append(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(text.as_bytes())?;
    Ok(path)
}

/// Installe le crochet : journal dans `dir`, puis le comportement habituel.
pub fn install(dir: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(message non textuel)".into());
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let thread = std::thread::current()
            .name()
            .unwrap_or("(sans nom)")
            .to_string();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        if let Ok(path) = append(&dir, &report(&thread, &location, &message, &backtrace)) {
            eprintln!("tri : plantage consigné dans {}", path.display());
        }
        previous(info);
    }));
}

/// Dernière erreur de la carte graphique, pas encore montrée ([`gpu_error`],
/// [`take_gpu_error`]).
static GPU_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Une erreur de la carte graphique (mémoire pleine…), qui plantait `tri` avant le
/// 07/11/2026 (le gestionnaire de wgpu par défaut arrête le programme) : consignée dans le
/// journal de `dir` s'il y en a un, puis gardée pour la barre d'état. L'image en cause
/// n'est pas dessinée, le reste continue.
pub fn gpu_error(dir: Option<&Path>, message: &str) {
    eprintln!("tri : erreur de la carte graphique : {message}");
    if let Some(dir) = dir {
        let text = report("(carte graphique, sans plantage)", "wgpu", message, "");
        let _ = append(dir, &text);
    }
    if let Ok(mut last) = GPU_ERROR.lock() {
        *last = Some(message.to_string());
    }
}

/// L'erreur de la carte graphique à montrer, une fois.
pub fn take_gpu_error() -> Option<String> {
    GPU_ERROR.lock().ok()?.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graphics_error_is_logged_and_shown_once() {
        let dir = tempfile::tempdir().unwrap();
        gpu_error(Some(dir.path()), "Out of Memory");
        let log = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(log.contains("Out of Memory") && log.contains("sans plantage"));
        assert_eq!(take_gpu_error().as_deref(), Some("Out of Memory"));
        assert_eq!(take_gpu_error(), None, "une fois");
    }

    #[test]
    fn reports_are_appended_with_their_location_and_message() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("données");
        append(
            &data,
            &report("main", "app.rs:12:5", "index hors limites", "pile"),
        )
        .unwrap();
        append(&data, &report("tri-watch", "watch.rs:3:1", "autre", "pile")).unwrap();
        let log = std::fs::read_to_string(data.join(FILE)).unwrap();
        assert_eq!(
            log.matches("=== plantage").count(),
            2,
            "ajoutés, pas remplacés"
        );
        assert!(log.contains("endroit : app.rs:12:5"));
        assert!(log.contains("message : index hors limites"));
        assert!(log.contains("fil : tri-watch"));
    }
}

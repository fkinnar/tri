//! Accès disque sous surveillance, pour le développement : trouver ce qui lit ou écrit le
//! disque depuis le fil de l'interface, et simuler un disque lent.
//!
//! Idée du 19/11/2026 : sur un montage lent (iCloud sous Linux, NAS), chaque accès disque
//! resté sur le fil de l'interface fige la fenêtre ; sur un SSD, le même accès ne se voit
//! pas. Plutôt que de les découvrir un par un sur la machine de l'utilisateur, deux outils,
//! sans effet en usage normal :
//!
//! - `TRI_IO_WATCH=1` : chaque accès fait depuis le fil de l'interface
//!   ([`mark_interface_thread`]) est signalé sur la sortie d'erreur, une fois par endroit
//!   du code (les fonctions de `tri` et de son interface en cause), puis compté ;
//!   [`report`] donne le bilan (à la fermeture) ;
//! - `TRI_SLOW_IO_MS=<ms>` : chaque accès attend ce délai, dans tous les fils, comme sur
//!   un montage lent.
//!
//! Les accès sont déclarés par [`access`] aux endroits qui lisent ou écrivent les données
//! d'un dossier (cache, données de tri, sidecars) : pas chaque appel au système de fichiers,
//! mais ceux qui peuvent viser un support lent.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

thread_local! {
    /// Vrai dans le fil de l'interface ([`mark_interface_thread`]).
    static INTERFACE: Cell<bool> = const { Cell::new(false) };
}

/// Ce fil est celui de l'interface : ses accès disque sont surveillés (`TRI_IO_WATCH`).
pub fn mark_interface_thread() {
    INTERFACE.with(|i| i.set(true));
}

/// Vrai si `TRI_IO_WATCH` est posé (non vide, autre que `0`).
pub fn watching() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("TRI_IO_WATCH").is_ok_and(|v| !v.is_empty() && v != "0"))
}

/// Délai de chaque accès (`TRI_SLOW_IO_MS`), s'il est posé.
pub fn slow() -> Option<Duration> {
    static SLOW: OnceLock<Option<Duration>> = OnceLock::new();
    *SLOW.get_or_init(|| {
        std::env::var("TRI_SLOW_IO_MS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|&ms| ms > 0)
            .map(Duration::from_millis)
    })
}

/// Endroits du code déjà signalés, et combien d'accès chacun.
fn seen() -> &'static Mutex<HashMap<String, (String, u64)>> {
    static SEEN: OnceLock<Mutex<HashMap<String, (String, u64)>>> = OnceLock::new();
    SEEN.get_or_init(Default::default)
}

/// Un accès au disque : `what` (« lecture du cache »…) sur `path`. Attend le délai de
/// `TRI_SLOW_IO_MS` ; depuis le fil de l'interface, le signale (`TRI_IO_WATCH`).
pub fn access(what: &str, path: &Path) {
    if let Some(delay) = slow() {
        std::thread::sleep(delay);
    }
    if !watching() || !INTERFACE.with(Cell::get) {
        return;
    }
    let trace = std::backtrace::Backtrace::force_capture().to_string();
    let place = culprits(&trace);
    let mut seen = seen().lock().unwrap_or_else(|e| e.into_inner());
    let entry = seen
        .entry(place.clone())
        .or_insert_with(|| (what.to_string(), 0));
    entry.1 += 1;
    if entry.1 == 1 {
        eprintln!(
            "[io-watch] fil de l'interface ({}) : {what} {} ← {place}",
            std::thread::current().name().unwrap_or("?"),
            path.display()
        );
        // Rien de reconnu : la pile telle quelle, pour comprendre.
        if place == "?" {
            for line in trace.lines().take(40) {
                eprintln!("[io-watch]   {line}");
            }
        }
    }
}

/// Les fonctions de `tri` en cause dans `trace` (pile d'appels), de la plus proche à la
/// plus lointaine, sans celles de ce module, avec leur ligne quand la pile la donne
/// (compilé avec `debug = "line-tables-only"`), jusqu'à la première de l'interface.
fn culprits(trace: &str) -> String {
    let mut frames: Vec<(String, Option<String>)> = Vec::new();
    for line in trace.lines().map(str::trim) {
        if let Some(at) = line.strip_prefix("at ") {
            if let Some(last) = frames.last_mut()
                && last.1.is_none()
            {
                last.1 = Some(short_place(at));
            }
        } else if let Some((_, name)) = line.split_once(": ") {
            frames.push((normalize(name), None));
        }
    }
    // Une fonction de `tri` : son nom complet (sans numéros de ligne), ou son fichier dans
    // le dépôt (avec : les noms y sont courts), ni la bibliothèque standard ni un crate.
    let ours = |(name, at): &(String, Option<String>)| match at {
        Some(at) => !at.starts_with('/') && !at.contains("io_watch.rs"),
        None => {
            (name.starts_with("tri::") || name.starts_with("tri_gui::"))
                && !name.starts_with("tri::io_watch::")
        }
    };
    // Jusqu'à la première fonction de l'interface : c'est elle qu'il faut changer.
    let mut names: Vec<String> = Vec::new();
    for (name, at) in frames.into_iter().filter(ours) {
        let interface =
            name.starts_with("tri_gui::") || at.as_deref().is_some_and(|a| a.starts_with("gui/"));
        names.push(match at {
            Some(at) => format!("{name} ({at})"),
            None => name,
        });
        if interface || names.len() >= 8 {
            break;
        }
    }
    if names.is_empty() {
        "?".into()
    } else {
        names.join(" ← ")
    }
}

/// Nom d'une fonction de la pile, sans son empreinte (`::h0123…`) ni les chevrons d'une
/// méthode (`<tri_gui::app::TriApp>::ui` → `tri_gui::app::TriApp::ui`).
fn normalize(name: &str) -> String {
    let name = name.rsplit_once("::h").map_or(name, |(n, h)| {
        if h.chars().all(|c| c.is_ascii_hexdigit()) {
            n
        } else {
            name
        }
    });
    match name.strip_prefix('<') {
        Some(rest) if !rest.contains(" as ") => rest.replacen(">::", "::", 1),
        Some(rest) => rest.to_string(),
        None => name.to_string(),
    }
}

/// `…/tri/gui/src/app.rs:123:5` → `gui/src/app.rs:123` ; un fichier hors du dépôt
/// (bibliothèque standard, crates) reste absolu, donc écarté.
fn short_place(at: &str) -> String {
    let at = at.rsplit_once(':').map_or(at, |(a, _)| a);
    let foreign = ["/rustc/", "/.cargo/", "/.rustup/"];
    if foreign.iter().any(|f| at.contains(f)) {
        return at.to_string();
    }
    match at.rfind("/src/") {
        Some(i) => {
            let crate_start = at[..i].rfind('/').map_or(0, |j| j + 1);
            at[crate_start..].to_string()
        }
        None => at.to_string(),
    }
}

/// Bilan des accès faits depuis le fil de l'interface : (endroit, sorte, nombre), le plus
/// fréquent d'abord.
pub fn report() -> Vec<(String, String, u64)> {
    let seen = seen().lock().unwrap_or_else(|e| e.into_inner());
    let mut all: Vec<(String, String, u64)> = seen
        .iter()
        .map(|(place, (what, n))| (place.clone(), what.clone(), *n))
        .collect();
    all.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_culprits_are_the_nearest_tri_functions() {
        let trace = "   0: std::backtrace::Backtrace::force_capture\n\
                     \x20  1: tri::io_watch::access::h0123\n\
                     \x20  2: <tri::cache::Cache>::get_bytes\n\
                     \x20            at /Volumes/x/tri/src/cache.rs:766:20\n\
                     \x20  3: tri::preview::signature::habcd\n\
                     \x20  4: <tri_gui::app::TriApp>::drain_engine\n\
                     \x20            at /Volumes/x/tri/gui/src/app.rs:8398:31\n\
                     \x20  5: eframe::run\n";
        assert_eq!(
            culprits(trace),
            "tri::cache::Cache::get_bytes (tri/src/cache.rs:766) ← tri::preview::signature \
             ← tri_gui::app::TriApp::drain_engine (gui/src/app.rs:8398)"
        );
        assert_eq!(culprits("   0: std::rt::lang_start\n"), "?");
    }

    #[test]
    fn other_threads_are_not_reported() {
        // Ce fil n'est pas celui de l'interface : rien de compté, quel que soit le réglage.
        access("lecture", Path::new("/x"));
        assert!(report().iter().all(|(_, _, n)| *n > 0));
    }
}

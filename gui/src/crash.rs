//! Plantages (19/10/2026) : un plantage sur macOS n'avait laissé qu'un rapport du
//! système sans le message d'erreur (une erreur Rust pendant le dessin de la fenêtre,
//! appelé depuis AppKit, arrête le programme net, et le message du terminal s'était
//! perdu). À toute erreur Rust (`panic`), le message, l'endroit (fichier, ligne), le fil
//! et la pile d'appels vont au journal de l'application (`tri.log`, [`tri::log`] ; un
//! `crash.log` à part avant le 10/11/2026), avant le comportement habituel (message sur
//! la sortie d'erreur, arrêt).

use tri::log::Level;

/// Le détail d'un plantage : fil, endroit, pile.
pub fn detail(thread: &str, location: &str, backtrace: &str) -> String {
    format!(
        "tri {}, {}\nthread: {thread}\nlocation: {location}\nbacktrace:\n{backtrace}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
    )
}

/// Installe le crochet : journal, puis le comportement habituel.
pub fn install() {
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
        let detail = detail(&thread, &location, &backtrace);
        if tri::log::write(Level::Crash, "panic", &message, &detail).is_some()
            && let Some(path) = tri::log::path()
        {
            eprintln!("tri : plantage consigné dans {}", path.display());
        }
        previous(info);
    }));
}

thread_local! {
    /// Dernière erreur de la carte graphique, pas encore montrée ([`gpu_error`],
    /// [`take_gpu_error`]). Par fil : wgpu la signale sur le fil qui dessine, celui de
    /// l'interface (et chaque test a ainsi la sienne).
    static GPU_ERROR: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    /// Dernière erreur consignée dans le journal, et si le terminal a déjà été prévenu :
    /// une rafale (une texture ratée fait échouer chaque image ensuite) n'y laisse que la
    /// première (retour du 07/11/2026 : « des messages d'insulte dans le terminal »).
    static GPU_LOGGED: std::cell::Cell<(Option<std::time::Instant>, bool)> =
        const { std::cell::Cell::new((None, false)) };
}

/// Durée d'une rafale d'erreurs de la carte graphique : après la première, les suivantes
/// ne vont plus au journal.
const GPU_BURST: std::time::Duration = std::time::Duration::from_secs(5);

/// Une erreur de la carte graphique (mémoire pleine…), qui plantait `tri` avant le
/// 07/11/2026 (le gestionnaire de wgpu par défaut arrête le programme) : consignée dans le
/// journal, puis gardée pour la barre d'état. L'image en cause n'est pas dessinée, le
/// reste continue.
pub fn gpu_error(message: &str) {
    let now = std::time::Instant::now();
    let (last, told) = GPU_LOGGED.with(|l| l.get());
    let burst = last.is_some_and(|t| now.duration_since(t) < GPU_BURST);
    GPU_LOGGED.with(|l| l.set((Some(now), told)));
    if !burst
        && tri::log::write(Level::Crash, "gpu", message, "(no crash)").is_some()
        && !told
        && let Some(path) = tri::log::path()
    {
        // Une ligne par session dans le terminal ; le détail est au journal.
        eprintln!(
            "tri : erreur de la carte graphique, détails dans {}",
            path.display()
        );
        GPU_LOGGED.with(|l| l.set((Some(now), true)));
    }
    GPU_ERROR.with(|last| *last.borrow_mut() = Some(message.to_string()));
}

/// L'erreur de la carte graphique à montrer, une fois.
pub fn take_gpu_error() -> Option<String> {
    GPU_ERROR.with(|last| last.borrow_mut().take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graphics_error_is_logged_and_shown_once() {
        let dir = tempfile::tempdir().unwrap();
        tri::log::set_thread_dir(Some(dir.path().to_path_buf()));
        gpu_error("Out of Memory");
        // La rafale qui suit (chaque image échoue sur la texture ratée) : pas au journal.
        gpu_error("Validation Error");
        tri::log::set_thread_dir(None);
        let entries = tri::log::read(dir.path());
        assert_eq!(entries.len(), 1, "une seule entrée par rafale");
        assert_eq!(entries[0].level, Level::Crash);
        assert_eq!(entries[0].message, "Out of Memory");
        assert_eq!(take_gpu_error().as_deref(), Some("Validation Error"));
        assert_eq!(take_gpu_error(), None, "une fois");
    }

    #[test]
    fn a_crash_detail_has_its_thread_location_and_stack() {
        let detail = detail("tri-watch", "app.rs:12:5", "pile");
        assert!(detail.contains("thread: tri-watch"));
        assert!(detail.contains("location: app.rs:12:5"));
        assert!(detail.ends_with("backtrace:\npile"));
    }
}

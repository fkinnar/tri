//! Trace minutée pour le diagnostic (`TRI_GUI_TRACE=1`) : chaque étape du chargement des
//! images (demande au moteur, image prête, décodage, affichage) sur la sortie d'erreur, avec
//! le temps écoulé depuis le démarrage. Sans la variable, ne coûte qu'un test.

use std::sync::OnceLock;
use std::time::Instant;

fn start() -> Option<Instant> {
    static START: OnceLock<Option<Instant>> = OnceLock::new();
    *START.get_or_init(|| std::env::var_os("TRI_GUI_TRACE").map(|_| Instant::now()))
}

/// Écrit `message()` si la trace est active (le message n'est construit que dans ce cas).
pub fn trace(message: impl FnOnce() -> String) {
    if let Some(start) = start() {
        eprintln!("[{:8.3} s] {}", start.elapsed().as_secs_f64(), message());
    }
}

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

/// Durée des étapes d'une image de l'interface (`TRI_GUI_TRACE`) : une image plus lente
/// que le seuil est tracée avec le détail de ses étapes, pour trouver ce qui bloque
/// l'interface (un accès disque sur un NAS, retour du 29/10/2026). Sans la trace, ne
/// mesure rien.
pub struct Steps {
    last: Option<Instant>,
    parts: Vec<(&'static str, f64)>,
}

impl Steps {
    /// Début de l'image.
    pub fn start() -> Steps {
        Steps {
            last: start().map(|_| Instant::now()),
            parts: Vec::new(),
        }
    }

    /// Fin de l'étape `name`.
    pub fn mark(&mut self, name: &'static str) {
        if let Some(last) = self.last {
            let now = Instant::now();
            self.parts
                .push((name, now.duration_since(last).as_secs_f64() * 1000.0));
            self.last = Some(now);
        }
    }

    /// Trace l'image si elle a duré plus de `slow_ms`.
    pub fn finish(self, slow_ms: u64) {
        let total: f64 = self.parts.iter().map(|(_, ms)| ms).sum();
        if self.last.is_none() || total < slow_ms as f64 {
            return;
        }
        let parts: Vec<String> = self
            .parts
            .iter()
            .filter(|(_, ms)| *ms >= 1.0)
            .map(|(name, ms)| format!("{name} {ms:.0}"))
            .collect();
        trace(|| format!("image lente : {total:.0} ms ({})", parts.join(", ")));
    }
}

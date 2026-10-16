//! Interface graphique de tri.
//!
//!   tri-gui <dossier>
//!
//! Variables de développement : `TRI_GUI_SCREENSHOT=<fichier.png>` (capture de la fenêtre
//! une fois les vignettes prêtes, puis fermeture), `TRI_GUI_SELECT=<n>` (image
//! sélectionnée au démarrage) et `TRI_GUI_ACTIONS=<id>,<id>...` (actions exécutées au
//! démarrage, ex. `core.app.help`).

mod actions;
mod app;
mod grid;
mod loader;
mod textures;
mod view;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage : tri-gui <dossier>");
        std::process::exit(2);
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("tri")
            .with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "tri",
        options,
        Box::new(move |cc| {
            // Fond sombre et neutre : la norme pour juger des photos.
            cc.egui_ctx.set_theme(eframe::egui::Theme::Dark);
            Ok(Box::new(app::TriApp::new(dir, None)))
        }),
    )
}

//! Interface graphique de tri.
//!
//! ```text
//! tri-gui [<dossier>] [--cache <dossier du cache>] [--config <fichier>]
//! ```
//!
//! Sans dossier, l'interface s'ouvre sur le panneau des dossiers.
//!
//! Réglages : fichier de configuration (`--config`, sinon l'emplacement par défaut, voir
//! `tri::settings` et `doc/config.example.toml`) ; `--cache` l'emporte sur lui.
//!
//! Variables de développement : `TRI_GUI_SCREENSHOT=<fichier.png>` (capture de la fenêtre
//! une fois les vignettes prêtes, puis fermeture), `TRI_GUI_SELECT=<n>` (image
//! sélectionnée au démarrage) et `TRI_GUI_ACTIONS=<id>,<id>...` (actions exécutées au
//! démarrage, ex. `core.app.help`).

mod actions;
mod app;
mod grid;
mod histogram;
mod loader;
mod marks;
mod meta;
mod offload;
mod settings;
mod textures;
mod trace;
mod view;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let mut args = std::env::args_os().skip(1);
    let (mut dir, mut cache, mut config) = (None, None, None);
    while let Some(arg) = args.next() {
        if arg == "--cache" {
            cache = args.next().map(PathBuf::from);
        } else if arg == "--config" {
            config = args.next().map(PathBuf::from);
        } else {
            dir = Some(PathBuf::from(arg));
        }
    }
    let loaded = tri::settings::load::<settings::AppSettings>(config.as_deref());
    if let Some(warning) = &loaded.warning {
        eprintln!("attention : {warning}");
    }
    let mut settings = loaded.settings;
    if cache.is_some() {
        settings.cache.dir = cache;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("tri")
            .with_inner_size(settings.gui.window),
        ..Default::default()
    };
    eframe::run_native(
        "tri",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_theme(settings.gui.theme);
            Ok(Box::new(app::TriApp::new(dir, settings, loaded.warning)))
        }),
    )
}

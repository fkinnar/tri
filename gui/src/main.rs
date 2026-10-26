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

// Windows, en release : application graphique, sans fenêtre de console derrière elle
// (paquet .msi, 21/10/2026). En debug, la console garde les messages.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// Texte traduit (voir `tri::t!`), catalogues de l'interface compris : ils sont ajoutés à
/// la première utilisation (tests unitaires compris, sans passer par `main`).
macro_rules! t {
    ($($arg:tt)*) => {{
        $crate::i18n::ensure();
        tri::t!($($arg)*)
    }};
}

mod actions;
mod app;
mod compare;
mod crash;
mod crop;
mod develop_batch;
mod export;
mod files;
mod grid;
mod grid_render;
mod histogram;
mod i18n;
mod loader;
mod markdown;
mod marks;
mod meta;
mod offload;
mod path_field;
mod settings;
mod settings_window;
mod stacks;
mod state;
mod textures;
mod trace;
mod view;

use std::path::PathBuf;

fn main() -> eframe::Result {
    trace::trace(|| "démarrage".into());
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
    // Langue du système pour les messages de lecture de la configuration, puis celle
    // qu'elle choisit.
    i18n::init(tri::i18n::LanguageSetting::Auto);
    let loaded = tri::settings::load::<settings::AppSettings>(config.as_deref());
    i18n::init(loaded.settings.i18n.language);
    trace::trace(|| "configuration lue".into());
    if let Some(warning) = &loaded.warning {
        eprintln!("{}", tri::t!("warning-prefix", message = warning));
    }
    let mut settings = loaded.settings;
    // Journal des plantages, dans le dossier de données (voir `crash.rs`).
    if let Some(data) = settings.engine().data_dir() {
        crash::install(data);
    }
    if cache.is_some() {
        settings.cache.dir = cache;
    }
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("tri")
        // Wayland : relie la fenêtre au fichier `tri.desktop` (icône, nom).
        .with_app_id("tri")
        .with_icon(app::window_icon())
        .with_inner_size(settings.gui.window);
    // Fenêtre du lancement précédent (issue n° 58) : taille, position, agrandie.
    if settings.gui.remember_display
        && let Some(data) = settings.engine().data_dir()
        && let Some(window) = state::load(&data).window
    {
        viewport = state::restore_window(viewport, &window);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    trace::trace(|| "ouverture de la fenêtre".into());
    eframe::run_native(
        "tri",
        options,
        Box::new(move |cc| {
            trace::trace(|| "fenêtre ouverte".into());
            settings.gui.theme.apply(&cc.egui_ctx, &settings.gui.colors);
            // Fichier de la fenêtre de réglages : celui lu, sinon celui demandé ou
            // l'emplacement par défaut (créé au premier changement).
            let path = loaded.path.or(config).or_else(tri::settings::default_path);
            Ok(Box::new(
                app::TriApp::new(dir, settings, loaded.warning).with_config(path),
            ))
        }),
    )
}

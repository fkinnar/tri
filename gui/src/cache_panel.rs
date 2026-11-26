//! Place du cache et vidage, en tête de la section « Cache » de la fenêtre de réglages
//! (issue n° 128, décidé le 09/11/2026) : la place par famille de niveaux et le total, un
//! bouton « Vider » par famille et « Tout vider », confirmés, en fond ; l'espace libéré
//! annoncé à la fin. Usages : vider les 1:1 (gros, vite recalculés) sans perdre les
//! vignettes ; vider les développés après une mise à jour de darktable.
//!
//! Le cache général seulement : celui gardé avec les photos (`.tri/cache`) suit son dossier.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Ui};
use tri::cache::{Cache, Level, Usage};

/// Une ligne : identifiant (catalogues `cache-row-<id>`) et ses niveaux.
fn rows() -> Vec<(&'static str, Vec<Level>)> {
    let developed = Level::ALL
        .into_iter()
        .filter(|l| matches!(l, Level::Developed(_)))
        .collect();
    vec![
        ("thumbnails", vec![Level::Thumbnail]),
        ("embedded", vec![Level::Embedded]),
        ("standard", vec![Level::Standard]),
        ("full", vec![Level::EmbeddedFull, Level::Full]),
        ("developed", developed),
        // Les palettes (#492) sont des analyses aussi.
        (
            "analysis",
            [Level::Exposure, Level::Sharpness, Level::Similarity]
                .into_iter()
                .chain(
                    Level::ALL
                        .into_iter()
                        .filter(|l| matches!(l, Level::Palette(_))),
                )
                .collect(),
        ),
    ]
}

/// Ce qu'on vide : une ligne ([`rows`]) ou tout.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Row(usize),
    All,
}

/// De quoi faire le ménage (#604) : ses réglages, le dossier de données et l'historique
/// des déchargements.
#[derive(Clone, Debug)]
pub struct PurgeContext {
    pub settings: tri::purge::PurgeSettings,
    pub data: Option<PathBuf>,
    pub history: Option<PathBuf>,
}

/// Bilan d'un ménage, fait ou à l'essai (`dry_run`).
pub fn purge_text(report: &tri::purge::Report, dry_run: bool) -> String {
    t!(
        if dry_run {
            "cache-purge-would"
        } else {
            "cache-purge-done"
        },
        history = report.history,
        size = size_text(report.cache.bytes),
        folders = report.folders
    )
}

/// Le panneau, gardé par la fenêtre de réglages.
pub struct CachePanel {
    cache: Cache,
    /// Place par ligne, une fois mesurée.
    usage: Option<Vec<Usage>>,
    measuring: Option<Receiver<Vec<Usage>>>,
    /// Vidage en cours : l'espace libéré, ou l'erreur.
    clearing: Option<Receiver<Result<Usage, String>>>,
    /// Demande de vidage à confirmer.
    confirm: Option<Target>,
    /// Bilan du dernier vidage (texte, erreur).
    message: Option<(String, bool)>,
    /// Ménage (#604) : de quoi le faire, l'essai ou le ménage en cours (vrai : à
    /// l'essai), et l'essai à confirmer.
    purge: Option<PurgeContext>,
    purging: Option<(bool, Receiver<tri::purge::Report>)>,
    purge_preview: Option<tri::purge::Report>,
}

impl CachePanel {
    pub fn new(cache: Cache) -> Self {
        CachePanel {
            cache,
            usage: None,
            measuring: None,
            clearing: None,
            confirm: None,
            message: None,
            purge: None,
            purging: None,
            purge_preview: None,
        }
    }

    /// Avec « Nettoyer » (#604).
    pub fn with_purge(mut self, purge: PurgeContext) -> Self {
        self.purge = Some(purge);
        self
    }

    /// Ménage en fond : à l'essai d'abord (ce qui partirait, à confirmer), puis pour de bon.
    fn start_purge(&mut self, ctx: &egui::Context, dry_run: bool) {
        let Some(p) = self.purge.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let cache = self.cache.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let report = tri::purge::run(
                &p.settings,
                Some(&cache),
                p.data.as_deref(),
                p.history.clone(),
                std::time::SystemTime::now(),
                dry_run,
            );
            if !dry_run && let Some(data) = &p.data {
                let _ = tri::purge::mark_done(data);
            }
            let _ = tx.send(report);
            ctx.request_repaint();
        });
        self.purging = Some((dry_run, rx));
        self.purge_preview = None;
        self.message = None;
    }

    /// Mesure la place de chaque ligne en fond (des dizaines de milliers de fichiers
    /// possibles).
    fn measure(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let cache = self.cache.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let usage = rows()
                .iter()
                .map(|(_, levels)| sum(&cache, levels))
                .collect();
            let _ = tx.send(usage);
            ctx.request_repaint();
        });
        self.measuring = Some(rx);
    }

    fn clear(&mut self, ctx: &egui::Context, target: Target) {
        let levels: Vec<Level> = match target {
            Target::Row(i) => rows()[i].1.clone(),
            Target::All => Level::ALL.to_vec(),
        };
        let (tx, rx) = mpsc::channel();
        let cache = self.cache.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let freed = sum(&cache, &levels);
            let result = levels
                .iter()
                .try_for_each(|&l| cache.clear(Some(l)))
                .map(|()| freed)
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.clearing = Some(rx);
        self.message = None;
    }

    /// Reçoit ce que les fils ont fini.
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.measuring
            && let Ok(usage) = rx.try_recv()
        {
            self.usage = Some(usage);
            self.measuring = None;
        }
        if let Some(rx) = &self.clearing
            && let Ok(result) = rx.try_recv()
        {
            self.clearing = None;
            self.message = Some(match result {
                Ok(freed) => (t!("cache-freed", size = size_text(freed.bytes)), false),
                Err(e) => (t!("cache-clear-failed", error = e), true),
            });
            self.measure(ctx);
        }
        if let Some((dry_run, rx)) = &self.purging
            && let Ok(report) = rx.try_recv()
        {
            let dry_run = *dry_run;
            self.purging = None;
            let error = !report.errors.is_empty();
            if dry_run && !report.is_empty() {
                self.purge_preview = Some(report);
            } else if report.is_empty() && !error {
                self.message = Some((t!("cache-purge-nothing"), false));
            } else {
                let mut text = purge_text(&report, dry_run);
                for e in &report.errors {
                    text.push('\n');
                    text.push_str(e);
                }
                self.message = Some((text, error));
                self.measure(ctx);
            }
        }
        if self.usage.is_none() && self.measuring.is_none() {
            self.measure(ctx);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        let busy = self.clearing.is_some();
        let rows = rows();
        let mut ask = None;
        let mut confirmed = None;
        let size_of = |i: Option<usize>, usage: &Option<Vec<Usage>>| match usage {
            Some(usage) => size_text(match i {
                Some(i) => usage[i].bytes,
                None => usage.iter().map(|u| u.bytes).sum(),
            }),
            None => "…".to_string(),
        };
        egui::Grid::new("cache_panel")
            .num_columns(3)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                for (i, (id, _)) in rows.iter().enumerate() {
                    ui.label(t!(&format!("cache-row-{id}")))
                        .on_hover_text(t!(&format!("cache-row-{id}-tip")));
                    ui.label(size_of(Some(i), &self.usage));
                    let target = Target::Row(i);
                    self.button(ui, target, busy, &mut ask, &mut confirmed);
                    ui.end_row();
                }
                ui.strong(t!("cache-total"));
                ui.strong(size_of(None, &self.usage));
                self.button(ui, Target::All, busy, &mut ask, &mut confirmed);
                ui.end_row();
            });
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(t!("cache-clearing"));
            });
        } else if let Some((text, error)) = &self.message {
            if *error {
                ui.colored_label(ui.visuals().error_fg_color, text);
            } else {
                ui.label(text);
            }
        }
        ui.weak(t!("cache-general-only"));
        // Ménage (#604) : ce qui partirait, puis confirmé.
        if self.purge.is_some() {
            ui.add_space(6.0);
            let mut start = None;
            match (&self.purging, &self.purge_preview) {
                (Some(_), _) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(t!("cache-purging"));
                    });
                }
                (None, Some(preview)) => {
                    ui.label(purge_text(preview, true));
                    ui.horizontal(|ui| {
                        if ui.button(t!("cache-purge")).clicked() {
                            start = Some(false);
                        }
                        if ui.button(t!("cache-clear-cancel")).clicked() {
                            self.purge_preview = None;
                        }
                    });
                }
                (None, None) => {
                    if ui
                        .add_enabled(!busy, egui::Button::new(t!("cache-purge-ask")))
                        .on_hover_text(t!("cache-purge-tip"))
                        .clicked()
                    {
                        start = Some(true);
                    }
                }
            }
            if let Some(dry_run) = start {
                self.start_purge(&ctx, dry_run);
            }
        }
        ui.add_space(8.0);
        if let Some(target) = ask {
            self.confirm = target;
        }
        if let Some(target) = confirmed {
            self.confirm = None;
            self.clear(&ctx, target);
        }
    }

    /// « Vider » (ou « Tout vider »), puis la confirmation à sa place.
    fn button(
        &self,
        ui: &mut Ui,
        target: Target,
        busy: bool,
        ask: &mut Option<Option<Target>>,
        confirmed: &mut Option<Target>,
    ) {
        if self.confirm == Some(target) {
            ui.horizontal(|ui| {
                if ui.button(t!("cache-clear-confirm")).clicked() {
                    *confirmed = Some(target);
                }
                if ui.button(t!("cache-clear-cancel")).clicked() {
                    *ask = Some(None);
                }
            });
            return;
        }
        let text = match target {
            Target::Row(_) => t!("cache-clear"),
            Target::All => t!("cache-clear-all"),
        };
        if ui.add_enabled(!busy, egui::Button::new(text)).clicked() {
            *ask = Some(Some(target));
        }
    }
}

/// Place de plusieurs niveaux (un niveau encore vide compte pour rien).
fn sum(cache: &Cache, levels: &[Level]) -> Usage {
    let mut total = Usage::default();
    for &level in levels {
        if let Ok(u) = cache.usage(level) {
            total.bytes += u.bytes;
            total.entries += u.entries;
        }
    }
    total
}

/// « 820 Mo », « 1,4 Go », « 3,2 Mo », « 48 Ko » (une décimale sous 10 Mo et en Go ; en
/// Ko sous 1 Mo, plutôt qu'un « 0,0 Mo » qui ferait croire à un niveau vide).
pub fn size_text(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let mb = bytes as f64 / MB;
    if mb >= 1024.0 {
        t!("cache-size-gb", n = tri::i18n::decimal(mb / 1024.0, 1))
    } else if mb >= 10.0 {
        t!("cache-size-mb", n = format!("{}", mb.round() as u64))
    } else if mb >= 1.0 {
        t!("cache-size-mb", n = tri::i18n::decimal(mb, 1))
    } else {
        t!(
            "cache-size-kb",
            n = format!("{}", (bytes as f64 / 1024.0).ceil() as u64)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_is_in_one_row() {
        let rows = rows();
        for level in Level::ALL {
            let n = rows.iter().filter(|(_, l)| l.contains(&level)).count();
            assert_eq!(n, 1, "{level:?}");
        }
    }

    #[test]
    fn sizes_read_well() {
        crate::i18n::ensure();
        let text = size_text(3 * 1024 * 1024 / 2);
        assert!(text.starts_with("1") && text.contains("5"), "{text}");
        assert!(size_text(820 * 1024 * 1024).starts_with("820"));
        assert!(size_text(3 * 1024 * 1024 * 1024).starts_with("3"));
        assert_eq!(size_text(48 * 1024), t!("cache-size-kb", n = "48"));
        assert_eq!(size_text(0), t!("cache-size-kb", n = "0"));
    }

    #[test]
    fn a_row_is_cleared_after_confirmation_and_the_rest_kept() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path().join("cache")).unwrap();
        let photo = dir.path().join("x.RAF");
        std::fs::write(&photo, b"raw").unwrap();
        let key = |level| tri::cache::Key::new(&photo, level, "p").unwrap();
        cache.put_bytes(&key(Level::Full), &[0; 1000]).unwrap();
        cache.put_bytes(&key(Level::Thumbnail), &[0; 10]).unwrap();
        let mut panel = CachePanel::new(cache.clone());
        let ctx = egui::Context::default();
        let full = rows().iter().position(|(id, _)| *id == "full").unwrap();
        panel.clear(&ctx, Target::Row(full));
        for _ in 0..500 {
            panel.poll(&ctx);
            if panel.clearing.is_none() && panel.measuring.is_none() && panel.usage.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let (text, error) = panel.message.clone().expect("done");
        assert!(!error, "{text}");
        assert_eq!(cache.usage(Level::Full).unwrap().entries, 0);
        assert_eq!(cache.usage(Level::Thumbnail).unwrap().entries, 1);
        let usage = panel.usage.as_ref().unwrap();
        assert_eq!(usage[full].entries, 0);
        assert_eq!(usage[0].entries, 1);
    }

    #[test]
    fn clean_up_shows_what_would_go_then_does_it() {
        // #604 : le cache d'un dossier disparu ; l'essai le montre sans rien toucher, puis
        // le ménage le supprime et retient la date.
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path().join("cache")).unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        std::fs::write(dir.path().join("autre"), b"x").unwrap();
        let photo = photos.join("x.RAF");
        std::fs::write(&photo, b"raw").unwrap();
        let key = tri::cache::Key::new(&photo, Level::Thumbnail, "p").unwrap();
        cache.put_bytes(&key, &[0; 100]).unwrap();
        std::fs::remove_dir_all(&photos).unwrap();
        let data = dir.path().join("data");
        let mut panel = CachePanel::new(cache.clone()).with_purge(PurgeContext {
            settings: tri::purge::PurgeSettings::default(),
            data: Some(data.clone()),
            history: None,
        });
        let ctx = egui::Context::default();
        let wait = |panel: &mut CachePanel| {
            for _ in 0..500 {
                panel.poll(&ctx);
                if panel.purging.is_none() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("jamais fini");
        };
        panel.start_purge(&ctx, true);
        wait(&mut panel);
        let preview = panel.purge_preview.clone().expect("à confirmer");
        assert_eq!(preview.cache.entries, 1);
        assert!(cache.contains(&key), "rien touché à l'essai");
        panel.start_purge(&ctx, false);
        wait(&mut panel);
        assert!(!cache.contains(&key));
        assert!(panel.message.as_ref().is_some_and(|(_, error)| !error));
        assert!(!tri::purge::due(
            &data,
            &tri::purge::PurgeSettings::default(),
            std::time::SystemTime::now()
        ));
    }
}

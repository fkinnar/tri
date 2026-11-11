//! Place du cache et vidage, en tête de la section « Cache » de la fenêtre de réglages
//! (issue n° 128, décidé le 09/11/2026) : la place par famille de niveaux et le total, un
//! bouton « Vider » par famille et « Tout vider », confirmés, en fond ; l'espace libéré
//! annoncé à la fin. Usages : vider les 1:1 (gros, vite recalculés) sans perdre les
//! vignettes ; vider les développés après une mise à jour de darktable.
//!
//! Le cache général seulement : celui gardé avec les photos (`.tri/cache`) suit son dossier.

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
        (
            "analysis",
            vec![Level::Exposure, Level::Sharpness, Level::Similarity],
        ),
    ]
}

/// Ce qu'on vide : une ligne ([`rows`]) ou tout.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Row(usize),
    All,
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
        }
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
}

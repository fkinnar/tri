//! Histogramme **après** les réglages de visualisation (issue n° 420, décidée le
//! 11/11/2026) : histogramme, pourcentages saturés / bouchés et zones de l'image telle
//! qu'on la voit, réglages compris, à côté des données réelles.
//!
//! - L'« après » est mesuré sur l'aperçu affiché, réglé comme à l'écran
//!   ([`tri::adjust::apply_bitmap`]), réduit à `[gui.histogram] after_side`, en fond : un
//!   passage de plus sur des pixels, jamais sur le fil de l'interface.
//! - Pendant le glissement d'un curseur, la dernière mesure est **décalée** tout de suite
//!   de l'écart d'exposition et de balance ([`tri::exposure::Exposure::shifted`]) ; la
//!   mesure complète (contraste, noir et blanc compris) suit.
//! - Le pourcentage saturé du **capteur** reste affiché à côté : ce qu'il a perdu est
//!   perdu, quel que soit le réglage.
//! - Les badges de la grille restent sur les données réelles, sauf
//!   `[gui.histogram] badges_follow_adjustments` : décalage de l'analyse réelle, sans
//!   repasser sur les pixels.

use super::*;
use tri::adjust::Layers;
use tri::exposure::SERIES;

/// Exposition d'une image réglée, et ce avec quoi elle a été mesurée.
pub(super) struct After {
    /// L'exposition mesurée.
    exposure: Arc<Exposure>,
    /// Ses réglages au moment de la mesure (`None` : aucun).
    pub(super) layers: Option<Layers>,
    /// Le niveau mesuré (l'aperçu affiché).
    level: Level,
}

/// Une mesure finie : l'image, le niveau, les réglages appliqués, l'exposition (`None` :
/// aperçu pas (encore) dans le cache).
pub(super) type AfterDone = (PathBuf, Level, Option<Layers>, Option<Exposure>);

/// Décalage en IL, par série (rouge, vert, bleu, luminance), de l'exposition et de la
/// balance de `layers`.
fn shift_of(layers: Option<&Layers>) -> [f32; SERIES] {
    let Some(total) = layers.map(Layers::total) else {
        return [0.0; SERIES];
    };
    let e = total.exposure;
    let (r, b) = (total.gains.red.max(1e-3), total.gains.blue.max(1e-3));
    [
        e + r.log2(),
        e,
        e + b.log2(),
        e + (0.2126 * r + 0.7152 + 0.0722 * b).log2(),
    ]
}

/// Écart de décalage de `from` à `to`.
fn shift_between(from: Option<&Layers>, to: Option<&Layers>) -> [f32; SERIES] {
    let (a, b) = (shift_of(from), shift_of(to));
    std::array::from_fn(|i| b[i] - a[i])
}

impl TriApp {
    /// Vrai si l'histogramme de `path` se montre après les réglages : le réglage
    /// `[gui.histogram] after_auto` (case de la section Réglages, bouton du grand
    /// histogramme, `e`), pour une image réglée (sans réglage, l'après est le réel).
    pub(super) fn histogram_shows_after(&self, path: &Path) -> bool {
        self.gui.histogram.after_auto
            && crate::adjust_view::layers_for(&self.adjust_map, path, Level::Standard)
                .is_some_and(|l| !l.is_none())
    }

    /// Exposition à montrer pour l'image `index` : l'« après » si elle est demandée et
    /// prête, avec alors l'exposition réelle à côté (capteur) ; sinon la réelle.
    pub(super) fn shown_exposure(
        &mut self,
        index: usize,
    ) -> Option<(Arc<Exposure>, Option<Arc<Exposure>>)> {
        let real = self.exposure_of(index)?;
        let path = self.files.get(index)?.path.clone();
        if !self.histogram_shows_after(&path) {
            return Some((real, None));
        }
        match self.after_exposure_of(index) {
            Some(after) => Some((after, Some(real))),
            None => Some((real, None)),
        }
    }

    /// Niveau mesuré pour l'« après » de `index` : l'aperçu affiché (plein écran : celui à
    /// l'écran, ramené à sa taille d'aperçu ; grille : celui de la vignette).
    fn after_level(&mut self, index: usize) -> Level {
        let file = &self.files[index];
        let raw = file.kind == MediaKind::Raw;
        let shown = if self.mode == Mode::View && self.last_shown.0 == index {
            self.last_shown.1
        } else {
            Some(self.grid_level(index))
        };
        match shown {
            Some(Level::Full) => Level::Standard,
            Some(Level::EmbeddedFull) => Level::Embedded,
            Some(level @ (Level::Standard | Level::Embedded | Level::Developed(_))) => level,
            _ if raw => Level::Embedded,
            _ => Level::Standard,
        }
    }

    /// L'« après » de `index` : la dernière mesure, décalée de ce qui a changé depuis
    /// (exposition, balance) ; une nouvelle mesure est lancée quand les réglages ou
    /// l'aperçu ont changé. `None` tant qu'aucune mesure n'est arrivée.
    fn after_exposure_of(&mut self, index: usize) -> Option<Arc<Exposure>> {
        while let Ok((path, level, layers, exposure)) = self.after_read.1.try_recv() {
            self.after_pending.remove(&path);
            match exposure {
                Some(e) => {
                    self.after_failed.remove(&path);
                    self.after_masks.remove(&path);
                    self.afters.insert(
                        path,
                        After {
                            exposure: Arc::new(e),
                            layers,
                            level,
                        },
                    );
                }
                None => {
                    self.after_failed.insert(path, Instant::now());
                }
            }
        }
        let path = self.files.get(index)?.path.clone();
        let level = self.after_level(index);
        let current = crate::adjust_view::layers_for(&self.adjust_map, &path, level);
        let stale = self
            .afters
            .get(&path)
            .is_none_or(|a| a.level != level || a.layers != current);
        let retry = self
            .after_failed
            .get(&path)
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(self.gui.grid.render_poll_ms));
        if stale && retry && !self.after_pending.contains(&path) {
            self.measure_after(index, level);
        }
        let after = self.afters.get(&path).filter(|a| a.level == level)?;
        if after.layers == current {
            return Some(after.exposure.clone());
        }
        // Pendant le glissement : la dernière mesure, décalée tout de suite.
        let shift = shift_between(after.layers.as_ref(), current.as_ref());
        let settings = self.engine.as_ref()?.preview.exposure;
        Some(Arc::new(after.exposure.shifted(shift, &settings)))
    }

    /// Lance en fond la mesure de l'« après » de `index` au niveau `level`.
    fn measure_after(&mut self, index: usize, level: Level) {
        let Some(engine) = self.engine.as_ref() else {
            return;
        };
        let file = self.files[index].clone();
        let Ok(key) = preview::key(&file, level, &engine.preview) else {
            return;
        };
        // L'aperçu pas encore calculé (grille d'un dossier qui s'ouvre) : la vignette, même
        // image, en attendant.
        let thumbnail = preview::key(&file, Level::Thumbnail, &engine.preview).ok();
        let cache = engine.cache.clone();
        let settings = engine.preview.exposure;
        let side = self.gui.histogram.after_side.max(64);
        let map = self.adjust_map.clone();
        let tx = self.after_read.0.clone();
        let repaint = self.repaint.clone();
        self.after_pending.insert(file.path.clone());
        crate::trace::trace(|| {
            format!(
                "après réglages : mesure de {} ({level:?})",
                file.path.display()
            )
        });
        std::thread::spawn(move || {
            // Les réglages lus au moment de la mesure : ceux qui ont vraiment été appliqués.
            let layers = crate::adjust_view::layers_for(&map, &file.path, level);
            let bitmap = cache
                .get_fit(&key, side)
                .or_else(|| thumbnail.and_then(|k| cache.get(&k)));
            let exposure = bitmap.map(|b| {
                let mut b = b.fit(side);
                if let Some(l) = layers {
                    let transfer = tri::adjust::transfer_of(file.kind == MediaKind::Raw, level);
                    tri::adjust::apply_bitmap(&mut b, transfer, l.total(), l.mono);
                }
                Exposure::from_bitmap(&b, &settings)
            });
            crate::trace::trace(|| {
                format!(
                    "après réglages : {} mesuré ({})",
                    file.path.display(),
                    if exposure.is_some() {
                        "fait"
                    } else {
                        "aperçu absent"
                    }
                )
            });
            let _ = tx.send((file.path, level, layers, exposure));
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
    }

    /// Parts saturées / bouchées de `index` pour les badges de la grille, après son
    /// exposition et sa balance réglées (`[gui.histogram] badges_follow_adjustments`) :
    /// l'analyse réelle décalée. `None` : pas de réglage, ou rien à suivre.
    pub(super) fn adjusted_clip_fractions(&mut self, index: usize) -> Option<(f64, f64)> {
        if !self.gui.histogram.badges_follow_adjustments {
            return None;
        }
        let path = self.files.get(index)?.path.clone();
        let layers = crate::adjust_view::layers_for(&self.adjust_map, &path, Level::Standard)
            .filter(|l| !l.is_none())?;
        if let Some((done, fractions)) = self.clip_after.get(&path)
            && *done == layers
        {
            return Some(*fractions);
        }
        let e = self.exposures.get(&path).cloned().or_else(|| {
            let engine = self.engine.as_ref()?;
            preview::exposure(&engine.cache, &self.files[index], &engine.preview).map(Arc::new)
        })?;
        let settings = &self.engine.as_ref()?.preview.exposure;
        let shifted = e.shifted(shift_of(Some(&layers)), settings);
        let fractions = (shifted.over_fraction(), shifted.under_fraction());
        self.clip_after.insert(path, (layers, fractions));
        Some(fractions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shift_follows_exposure_and_balance() {
        let mut l = Layers::default();
        assert_eq!(shift_of(None), [0.0; SERIES]);
        assert_eq!(shift_of(Some(&l)), [0.0; SERIES]);
        l.correction.exposure = 1.0;
        l.correction.gains.red = 2.0;
        let s = shift_of(Some(&l));
        assert!(
            (s[0] - 2.0).abs() < 1e-5 && (s[1] - 1.0).abs() < 1e-5 && (s[2] - 1.0).abs() < 1e-5
        );
        let back = shift_between(Some(&l), None);
        assert!((back[0] + 2.0).abs() < 1e-5);
    }
}

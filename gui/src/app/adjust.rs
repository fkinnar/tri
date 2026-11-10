//! Réglages de visualisation dans l'interface (issue n° 137) : section « Réglages » du
//! panneau de droite, en onglets (base : exposition et balance ; style : décalage
//! créatif ; noir et blanc ; valeurs à reporter), synchronisée sur la sélection (un
//! curseur décale chaque image, un bouton recalcule chacune), copier / coller, harmoniser
//! la sélection, avant / après. Les réglages sont lus en fond dans les sidecars,
//! enregistrés dans `tri:Adjust` (annulables) et appliqués aux textures
//! ([`crate::adjust_view`]).

use super::*;
use crate::fine_slider::FineSlider;
use tri::adjust::Mode as LayerMode;
use tri::adjust::{Gains, Layers, Mixer, Modes, kelvin};

/// Réglages lus en fond : l'image, ce que son sidecar contient et la couleur de
/// l'éclairage de sa prise de vue (base des kelvins).
pub(super) type AdjustRead = (PathBuf, Option<Layers>, [f32; 3]);

/// Bornes des curseurs (limites du modèle, pas des réglages de l'utilisateur).
const EXPOSURE_RANGE: std::ops::RangeInclusive<f32> = -3.0..=3.0;
const KELVIN_RANGE: std::ops::RangeInclusive<f32> = 2000.0..=12000.0;
const KELVIN_TINT_RANGE: std::ops::RangeInclusive<f32> = -150.0..=150.0;
const WARMTH_RANGE: std::ops::RangeInclusive<f32> = -1.5..=1.5;
const TINT_RANGE: std::ops::RangeInclusive<f32> = -0.5..=0.5;
const MIXER_RANGE: std::ops::RangeInclusive<f32> = -100.0..=200.0;

/// Choix de la liste des balances.
#[derive(Clone, Copy, PartialEq)]
enum WhiteBalance {
    /// Telle que prise (réglage du boîtier).
    AsShot,
    /// Monde gris sur toute l'image.
    Auto,
    /// Un préréglage en kelvins.
    Preset(&'static str, f32),
}

impl TriApp {
    /// Réglages connus de `path` (`None` : aucun, ou pas encore lus : la lecture part en
    /// fond, sans jamais attendre le disque, comme les versions).
    pub(super) fn adjust_layers(&mut self, path: &Path) -> Option<Layers> {
        self.adjust_poll();
        if !self.adjust_known.contains(path) && self.adjust_pending.insert(path.to_path_buf()) {
            let (tx, target) = (self.adjust_read.0.clone(), path.to_path_buf());
            let kind = self
                .files
                .iter()
                .find(|f| f.path == path)
                .map_or(tri::media::MediaKind::Flat, |f| f.kind);
            tri::cache::defer_job(format!("adjust:{}", target.display()), move || {
                let layers = tri::adjust::read(&target).ok().flatten();
                let shot = tri::adjust::shot_white(&target, kind);
                let _ = tx.send((target, layers, shot));
            });
        }
        self.adjust_map
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .layers
            .get(path)
            .copied()
    }

    /// Réglages arrivés de la lecture en fond : retenus, et les images concernées
    /// rechargées.
    pub(super) fn adjust_poll(&mut self) {
        while let Ok((path, layers, shot)) = self.adjust_read.1.try_recv() {
            self.adjust_pending.remove(&path);
            self.adjust_shot.insert(path.clone(), shot);
            if !self.adjust_known.insert(path.clone()) {
                continue; // déjà connus (posés dans la session entre-temps)
            }
            if let Some(layers) = layers {
                self.adjust_map
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .layers
                    .insert(path.clone(), layers);
                self.drop_adjusted_textures(&path);
            }
        }
    }

    /// Dossier changé : réglages oubliés (relus à la demande), lectures en cours ignorées.
    pub(super) fn adjust_forget(&mut self) {
        self.adjust_known.clear();
        self.adjust_pending.clear();
        self.adjust_read = mpsc::channel();
        self.adjust_dirty = None;
        self.adjust_shot.clear();
        self.adjust_creative_memory.clear();
        self.adjust_mono_memory.clear();
        self.adjust_map
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .layers
            .clear();
    }

    /// Les textures de `path` sont refaites (réglages changés) : vignette et grandes
    /// images.
    pub(super) fn drop_adjusted_textures(&mut self, path: &Path) {
        let owned = path.to_path_buf();
        self.textures.remove(&owned);
        self.texture_levels.remove(&owned);
        let keys: Vec<(PathBuf, Level)> = self
            .big
            .keys()
            .filter(|(p, _)| p == path)
            .cloned()
            .collect();
        for key in keys {
            self.big.remove(&key);
        }
        // Le chargeur reçoit la liste à nouveau, même identique.
        self.wanted.clear();
    }

    /// Réglages de `path` changés à l'écran (curseur en cours) : montrés tout de suite,
    /// enregistrés quand on lâche ([`TriApp::adjust_commit`]).
    pub(super) fn adjust_live(&mut self, path: &Path, layers: Layers) {
        {
            let mut shared = self.adjust_map.write().unwrap_or_else(|e| e.into_inner());
            // Début du geste : l'état d'avant, pour reporter l'écart sur la sélection.
            if self.adjust_gesture.as_ref().is_none_or(|(p, _)| p != path) {
                let before = shared.layers.get(path).copied().unwrap_or_default();
                self.adjust_gesture = Some((path.to_path_buf(), before));
            }
            if layers.is_none() {
                shared.layers.remove(path);
            } else {
                shared.layers.insert(path.to_path_buf(), layers);
            }
        }
        self.adjust_known.insert(path.to_path_buf());
        self.drop_adjusted_textures(path);
        self.adjust_dirty = Some(path.to_path_buf());
    }

    /// Enregistre les réglages changés à l'écran, une fois le pointeur relâché : un pas de
    /// l'historique par geste, pas par image. Sélection synchronisée : le geste y est
    /// reporté, relativement à ce que chaque image a déjà ([`tri::adjust::propagate`]).
    pub(super) fn adjust_commit(&mut self, ctx: &egui::Context) {
        if self.adjust_dirty.is_none() || ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        let Some(path) = self.adjust_dirty.take() else {
            return;
        };
        let after = self.adjust_layers(&path).unwrap_or_default();
        let mut changes = vec![(path.clone(), Some(after))];
        if let Some((gesture, before)) = self.adjust_gesture.take()
            && gesture == path
            && before != after
        {
            let shot = self.adjust_shot_now(&path);
            for target in self.adjust_sync_targets(&path) {
                let current = self.adjust_layers_now(&target);
                let target_shot = self.adjust_shot_now(&target);
                let new = tri::adjust::propagate(&before, &after, shot, &current, target_shot);
                changes.push((target, Some(new)));
            }
        }
        self.save_adjustments(changes);
    }

    /// Écrit les réglages de plusieurs images (un pas de l'historique), et les montre.
    pub(super) fn save_adjustments(&mut self, changes: Vec<(PathBuf, Option<Layers>)>) {
        let convention = self.settings.xmp.sidecar;
        let mut done = Vec::new();
        for (path, layers) in &changes {
            match tri::adjust::write(path, layers.as_ref(), convention) {
                Ok(change) if change.before != change.after => done.push(change),
                Ok(_) => {}
                Err(e) => {
                    self.note_error = Some(t!(
                        "adjust-write-failed",
                        name = shown_name(path),
                        error = e.to_string()
                    ));
                }
            }
            {
                let mut shared = self.adjust_map.write().unwrap_or_else(|e| e.into_inner());
                match layers {
                    Some(l) if !l.is_none() => {
                        shared.layers.insert(path.clone(), *l);
                    }
                    _ => {
                        shared.layers.remove(path);
                    }
                }
            }
            self.adjust_known.insert(path.clone());
            self.drop_adjusted_textures(path);
        }
        if !done.is_empty() {
            self.history.push(tri::history::Step::Adjust(done));
        }
    }

    /// Après avoir défait ou refait un pas : les réglages des images touchées sont relus.
    pub(super) fn adjust_reread(&mut self, images: &[PathBuf]) {
        for path in images {
            if !self.adjust_known.contains(path) {
                continue;
            }
            let layers = tri::adjust::read(path).ok().flatten();
            {
                let mut shared = self.adjust_map.write().unwrap_or_else(|e| e.into_inner());
                match layers {
                    Some(l) => shared.layers.insert(path.clone(), l),
                    None => shared.layers.remove(path),
                };
            }
            self.drop_adjusted_textures(path);
        }
    }

    /// Avant / après : toutes les images sans leurs réglages, ou avec.
    pub(super) fn toggle_before_after(&mut self) {
        let paths: Vec<PathBuf> = {
            let mut shared = self.adjust_map.write().unwrap_or_else(|e| e.into_inner());
            shared.original = !shared.original;
            shared.layers.keys().cloned().collect()
        };
        for path in paths {
            self.drop_adjusted_textures(&path);
        }
    }

    /// Vrai en « avant ».
    pub(super) fn showing_original(&self) -> bool {
        self.adjust_map
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .original
    }

    /// La vignette de `path` sans réglage, base des mesures (JPEG : sRGB).
    pub(super) fn adjust_base(&self, path: &Path) -> Option<tri::adjust::Base> {
        let engine = self.engine.as_ref()?;
        let file = self.files.iter().find(|f| f.path == path)?;
        let key = preview::key(file, Level::Thumbnail, &engine.preview).ok()?;
        Some(tri::adjust::Base {
            bitmap: engine.cache.get(&key)?,
            transfer: tri::adjust::Transfer::Srgb,
        })
    }

    /// Couleur de l'éclairage de la prise de vue de `path` pour l'affichage, sans attendre
    /// le disque (D65 tant qu'elle n'est pas lue, ou pour une image plate).
    fn adjust_shot_shown(&self, path: &Path) -> [f32; 3] {
        self.adjust_shot.get(path).copied().unwrap_or(kelvin::D65)
    }

    /// La même, lue tout de suite s'il le faut : pour un calcul demandé (bouton, geste
    /// reporté sur la sélection).
    fn adjust_shot_now(&mut self, path: &Path) -> [f32; 3] {
        if let Some(shot) = self.adjust_shot.get(path) {
            return *shot;
        }
        let kind = self
            .files
            .iter()
            .find(|f| f.path == path)
            .map_or(tri::media::MediaKind::Flat, |f| f.kind);
        let shot = tri::adjust::shot_white(path, kind);
        self.adjust_shot.insert(path.to_path_buf(), shot);
        shot
    }

    /// Réglages de `path` à jour pour un calcul : ceux de la session, sinon son sidecar.
    fn adjust_layers_now(&self, path: &Path) -> Layers {
        if self.adjust_known.contains(path) {
            return self
                .adjust_map
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .layers
                .get(path)
                .copied()
                .unwrap_or_default();
        }
        tri::adjust::read(path).ok().flatten().unwrap_or_default()
    }

    /// Images synchronisées avec l'image courante `current` (sans elle) : la sélection,
    /// si « Synchroniser » est coché et qu'elle a plusieurs images.
    fn adjust_sync_targets(&self, current: &Path) -> Vec<PathBuf> {
        if !self.adjust_sync || self.selection.visible(&self.files) < 2 {
            return Vec::new();
        }
        self.selection
            .targets(&self.files, self.selected)
            .into_iter()
            .filter(|p| p != current)
            .collect()
    }

    /// Un bouton (auto, préréglage, réinitialiser) : `compute` calcule les réglages de
    /// l'image courante (`current` : ses réglages à l'écran) et de chaque image
    /// synchronisée, **chacune d'après les siens** (`None` : sa vignette n'est pas prête) ;
    /// un seul pas de l'historique.
    fn adjust_each(
        &mut self,
        path: &Path,
        current: Layers,
        compute: impl Fn(&Self, &Path, Layers, [f32; 3]) -> Option<Layers>,
    ) {
        let mut items = vec![(path.to_path_buf(), current)];
        for target in self.adjust_sync_targets(path) {
            let layers = self.adjust_layers_now(&target);
            items.push((target, layers));
        }
        let mut changes = Vec::new();
        let mut missing = 0;
        for (image, layers) in items {
            let shot = self.adjust_shot_now(&image);
            match compute(self, &image, layers, shot) {
                Some(new) => changes.push((image, Some(new))),
                None => missing += 1,
            }
        }
        self.adjust_dirty = None;
        self.adjust_gesture = None;
        self.save_adjustments(changes);
        if missing > 0 {
            self.notice = Some((t!("adjust-missing", missing = missing), false));
        }
    }

    /// Exposition automatique de `layers` (médiane vers `[adjust] auto_exposure_target`),
    /// mesurée sur la vignette de `path`.
    fn auto_exposure_of(&self, path: &Path, mut layers: Layers) -> Option<Layers> {
        let base = self.adjust_base(path)?;
        let ev = tri::adjust::auto_exposure(
            &base,
            layers.correction.gains,
            self.settings.adjust.auto_exposure_target,
        );
        layers.correction.exposure = ev.clamp(*EXPOSURE_RANGE.start(), *EXPOSURE_RANGE.end());
        Some(layers)
    }

    /// Balance auto de `layers` : monde gris sur la vignette de `path`.
    fn auto_balance_of(&self, path: &Path, mut layers: Layers) -> Option<Layers> {
        let base = self.adjust_base(path)?;
        let cast = tri::adjust::measure(&base.bitmap, base.transfer, None).cast;
        layers.correction.gains = tri::adjust::neutral(cast);
        Some(layers)
    }

    /// Zone de la pipette autour de `point` (en fractions de l'image) : un carré de
    /// `[gui] adjust_eyedropper_size`.
    pub(super) fn adjust_point_zone(&self, point: egui::Pos2) -> tri::adjust::Zone {
        let half = (self.gui.adjust_eyedropper_size / 2.0).clamp(0.001, 0.5);
        tri::adjust::Zone {
            x: (point.x - half).clamp(0.0, 1.0 - 2.0 * half),
            y: (point.y - half).clamp(0.0, 1.0 - 2.0 * half),
            w: 2.0 * half,
            h: 2.0 * half,
        }
    }

    /// Pipette : la balance de la correction rend neutre `zone` (un point cliqué : une zone
    /// grise ; une zone tracée : sa moyenne, « auto » sur cette partie de l'image), mesurée
    /// sur la vignette sans réglage. Images synchronisées : leur balance est harmonisée
    /// sur l'image courante (la zone n'existe que dans celle-ci).
    pub(super) fn adjust_pick(&mut self, path: &Path, zone: tri::adjust::Zone) {
        let Some(base) = self.adjust_base(path) else {
            self.notice = Some((t!("adjust-apply-wait"), false));
            return;
        };
        let cast = tri::adjust::measure(&base.bitmap, base.transfer, Some(zone)).cast;
        let mut layers = self.adjust_layers_now(path);
        layers.correction.gains = tri::adjust::neutral(cast);
        let mut changes = vec![(path.to_path_buf(), Some(layers))];
        let modes = Modes {
            exposure: LayerMode::Keep,
            white_balance: LayerMode::Harmonise,
            creative: LayerMode::Keep,
            mono: LayerMode::Keep,
        };
        let mut missing = 0;
        for target in self.adjust_sync_targets(path) {
            let Some(target_base) = self.adjust_base(&target) else {
                missing += 1;
                continue;
            };
            let current = self.adjust_layers_now(&target);
            let new = tri::adjust::apply_to(&layers, &base, &current, &target_base, modes);
            changes.push((target, Some(new)));
        }
        self.save_adjustments(changes);
        if missing > 0 {
            self.notice = Some((t!("adjust-missing", missing = missing), false));
        }
    }

    /// « Harmoniser la sélection » : l'exposition et la balance des autres images
    /// sélectionnées alignées sur l'image courante, sans toucher au style ni au noir et
    /// blanc (un seul pas de l'historique).
    pub(super) fn adjust_apply_to_selection(&mut self) {
        let Some(file) = self.files.get(self.selected).cloned() else {
            return;
        };
        let targets: Vec<PathBuf> = self
            .selection
            .targets(&self.files, self.selected)
            .into_iter()
            .filter(|p| *p != file.path)
            .collect();
        if targets.is_empty() {
            self.notice = Some((t!("adjust-apply-none"), false));
            return;
        }
        let modes = Modes {
            exposure: LayerMode::Harmonise,
            white_balance: LayerMode::Harmonise,
            creative: LayerMode::Keep,
            mono: LayerMode::Keep,
        };
        let reference = self.adjust_layers_now(&file.path);
        let Some(reference_base) = self.adjust_base(&file.path) else {
            self.notice = Some((t!("adjust-apply-wait"), false));
            return;
        };
        self.adjust_to(&reference, &reference_base, targets, modes);
    }

    /// Les réglages `reference` (image de vignette `reference_base`) vers `targets`, selon
    /// `modes`, en un pas ; le bilan dans la barre d'état.
    fn adjust_to(
        &mut self,
        reference: &Layers,
        reference_base: &tri::adjust::Base,
        targets: Vec<PathBuf>,
        modes: Modes,
    ) {
        let mut changes = Vec::new();
        let mut missing = 0;
        for target in targets {
            let Some(base) = self.adjust_base(&target) else {
                missing += 1;
                continue;
            };
            let current = self.adjust_layers_now(&target);
            let layers = tri::adjust::apply_to(reference, reference_base, &current, &base, modes);
            changes.push((target, Some(layers)));
        }
        let n = changes.len();
        self.save_adjustments(changes);
        self.notice = Some((
            if missing > 0 {
                t!("adjust-applied-missing", n = n, missing = missing)
            } else {
                t!("adjust-applied", n = n)
            },
            false,
        ));
    }

    /// Copier les réglages de l'image courante (avec sa vignette, pour harmoniser au
    /// collage, même dans un autre dossier).
    pub(super) fn adjust_copy(&mut self) {
        let Some(file) = self.files.get(self.selected).cloned() else {
            return;
        };
        let Some(base) = self.adjust_base(&file.path) else {
            self.notice = Some((t!("adjust-apply-wait"), false));
            return;
        };
        let layers = self.adjust_layers_now(&file.path);
        self.adjust_clipboard = Some(AdjustClip { layers, base });
        self.notice = Some((t!("adjust-copied", name = shown_name(&file.path)), false));
    }

    /// Coller les réglages copiés sur l'image courante et les images synchronisées :
    /// exposition et balance harmonisées sur l'image copiée, style et noir et blanc
    /// copiés.
    pub(super) fn adjust_paste(&mut self) {
        let Some(clip) = self.adjust_clipboard.clone() else {
            self.notice = Some((t!("adjust-paste-empty"), false));
            return;
        };
        let Some(file) = self.files.get(self.selected).cloned() else {
            return;
        };
        let mut targets = vec![file.path.clone()];
        targets.extend(self.adjust_sync_targets(&file.path));
        self.adjust_to(&clip.layers, &clip.base, targets, Modes::default());
    }

    /// Section « Réglages » du panneau de droite (retours du 10/11/2026) : la case
    /// « Synchroniser », quatre onglets (base, style, noir et blanc, valeurs), puis les
    /// boutons communs.
    pub(super) fn adjust_section(&mut self, ui: &mut Ui) {
        let Some(file) = self.files.get(self.selected).cloned() else {
            ui.weak(t!("panel-no-image"));
            return;
        };
        if tri::developments::get(&file.path).is_some()
            || (self.mode == Mode::View && self.source_for(&file) == Source::Developed)
        {
            // Les réglages valent avant le développement par un autre logiciel.
            ui.weak(t!("adjust-developed"));
            return;
        }
        let before = self.adjust_layers(&file.path).unwrap_or_default();
        let mut layers = before;
        let shot = self.adjust_shot_shown(&file.path);
        let mut pending = Pending::default();
        // Synchroniser : grisée avec une seule image, cochée par défaut sinon.
        let n = self.selection.visible(&self.files);
        ui.add_enabled_ui(n > 1, |ui| {
            let mut on = self.adjust_sync && n > 1;
            let label = if n > 1 {
                t!("adjust-sync", n = n)
            } else {
                t!("adjust-sync-none")
            };
            if ui
                .checkbox(&mut on, label)
                .on_hover_text(t!("adjust-sync-tip"))
                .on_disabled_hover_text(t!("adjust-sync-tip"))
                .changed()
            {
                self.adjust_sync = on;
            }
        });
        ui.add_space(6.0);
        // Onglets ; un point sur ceux qui changent l'image (un réglage caché se voit).
        ui.horizontal_wrapped(|ui| {
            let tabs = [
                (
                    AdjustTab::Base,
                    "adjust-tab-base",
                    !layers.correction.is_none(),
                ),
                (
                    AdjustTab::Style,
                    "adjust-tab-style",
                    layers.creative.is_some(),
                ),
                (AdjustTab::Mono, "adjust-tab-mono", layers.mono.is_some()),
                (AdjustTab::Values, "adjust-tab-values", false),
            ];
            for (tab, key, active) in tabs {
                let text = if active {
                    format!("{} •", t!(key))
                } else {
                    t!(key)
                };
                if ui.selectable_label(self.adjust_tab == tab, text).clicked() {
                    self.adjust_tab = tab;
                }
            }
        });
        ui.separator();
        ui.add_space(4.0);
        match self.adjust_tab {
            AdjustTab::Base => {
                let presets = self.settings.adjust.white_balance_presets();
                base_tab(
                    ui,
                    shot,
                    &presets,
                    self.adjust_eyedropper,
                    &mut layers,
                    &mut pending,
                );
            }
            AdjustTab::Style => {
                let mut on = layers.creative.is_some();
                if ui
                    .checkbox(&mut on, t!("adjust-creative"))
                    .on_hover_text(t!("adjust-creative-tip"))
                    .changed()
                {
                    if on {
                        layers.creative = Some(
                            self.adjust_creative_memory
                                .remove(&file.path)
                                .unwrap_or_default(),
                        );
                    } else if let Some(c) = layers.creative.take() {
                        self.adjust_creative_memory.insert(file.path.clone(), c);
                    }
                }
                ui.add_space(6.0);
                style_tab(ui, &mut layers);
            }
            AdjustTab::Mono => {
                let mut on = layers.mono.is_some();
                if ui.checkbox(&mut on, t!("adjust-mono")).changed() {
                    if on {
                        layers.mono = Some(
                            self.adjust_mono_memory
                                .remove(&file.path)
                                .unwrap_or_default(),
                        );
                    } else if let Some(m) = layers.mono.take() {
                        self.adjust_mono_memory.insert(file.path.clone(), m);
                    }
                }
                ui.add_space(6.0);
                mono_tab(ui, &mut layers);
            }
            AdjustTab::Values => values_tab(ui, &layers, shot),
        }
        ui.add_space(6.0);
        ui.separator();
        let (mut copy, mut paste, mut harmonise, mut toggle, mut reset) =
            (false, false, false, false, false);
        ui.horizontal_wrapped(|ui| {
            copy = ui
                .button(t!("adjust-copy"))
                .on_hover_text(t!("adjust-copy-tip"))
                .clicked();
            paste = ui
                .add_enabled(
                    self.adjust_clipboard.is_some(),
                    egui::Button::new(t!("adjust-paste")),
                )
                .on_hover_text(t!("adjust-paste-tip"))
                .on_disabled_hover_text(t!("adjust-paste-tip"))
                .clicked();
            harmonise = ui
                .add_enabled(n > 1, egui::Button::new(t!("adjust-harmonise")))
                .on_hover_text(t!("adjust-harmonise-tip"))
                .on_disabled_hover_text(t!("adjust-harmonise-tip"))
                .clicked();
        });
        ui.horizontal_wrapped(|ui| {
            toggle = ui
                .selectable_label(self.showing_original(), t!("adjust-before"))
                .on_hover_text(t!("adjust-before-tip"))
                .clicked();
            reset = ui
                .add_enabled(!layers.is_none(), egui::Button::new(t!("adjust-reset")))
                .clicked();
        });
        // Curseurs, cases, listes : montrés tout de suite, enregistrés (et reportés sur la
        // sélection) au relâchement.
        if layers != before {
            self.adjust_live(&file.path, layers);
        }
        if pending.eyedropper {
            self.adjust_eyedropper = !self.adjust_eyedropper;
            if self.adjust_eyedropper && self.mode == Mode::Grid {
                self.open_view();
            }
        }
        // Boutons : recalculés sur chaque image.
        if pending.auto_exposure {
            self.adjust_each(&file.path, layers, |app, p, l, _| {
                app.auto_exposure_of(p, l)
            });
        }
        match pending.white_balance {
            Some(WhiteBalance::AsShot) => self.adjust_each(&file.path, layers, |_, _, mut l, _| {
                l.correction.gains = Gains::NONE;
                Some(l)
            }),
            Some(WhiteBalance::Preset(_, k)) => {
                self.adjust_each(&file.path, layers, |_, _, mut l, shot| {
                    l.correction.gains = kelvin::gains(shot, k, 0.0);
                    Some(l)
                })
            }
            Some(WhiteBalance::Auto) => {
                self.adjust_each(&file.path, layers, |app, p, l, _| app.auto_balance_of(p, l))
            }
            None => {}
        }
        if reset {
            self.adjust_each(&file.path, layers, |_, _, _, _| Some(Layers::default()));
        }
        if toggle {
            self.toggle_before_after();
        }
        if copy || paste || harmonise {
            // Ce qui vient d'être réglé d'abord.
            self.adjust_commit(ui.ctx());
        }
        if copy {
            self.adjust_copy();
        }
        if paste {
            self.adjust_paste();
        }
        if harmonise {
            self.adjust_apply_to_selection();
        }
    }
}

/// Onglet de la section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum AdjustTab {
    /// Exposition et balance (correction).
    #[default]
    Base,
    /// Décalage créatif.
    Style,
    /// Noir et blanc.
    Mono,
    /// Les valeurs, à reporter.
    Values,
}

/// Réglages copiés : ceux de l'image et sa vignette, pour harmoniser au collage.
#[derive(Clone)]
pub(super) struct AdjustClip {
    layers: Layers,
    base: tri::adjust::Base,
}

/// Ce qu'un onglet demande, fait après coup (calculs sur chaque image).
#[derive(Default)]
struct Pending {
    auto_exposure: bool,
    white_balance: Option<WhiteBalance>,
    eyedropper: bool,
}

/// Largeur du champ de valeur à droite des curseurs (tous alignés).
const FIELD_WIDTH: f32 = 76.0;

/// Ligne de titre d'un réglage : le nom à gauche, `right` (auto, préréglage…) à droite.
/// Rend l'identifiant du nom, qui nomme le curseur et son champ.
fn title_row(ui: &mut Ui, text: String, right: impl FnOnce(&mut Ui)) -> egui::Id {
    ui.horizontal(|ui| {
        let id = ui.label(text).id;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
        id
    })
    .inner
}

/// Le curseur d'un réglage, sur toute la largeur du panneau, et sa valeur à taper à
/// droite ; nommés par `name`. `steps` : grand cran (glisser) et petit (Maj + glisser).
#[allow(clippy::too_many_arguments)]
fn setting_slider(
    ui: &mut Ui,
    name: egui::Id,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    steps: (f32, f32),
    decimals: usize,
    suffix: &str,
    logarithmic: bool,
) -> egui::Response {
    let response = ui
        .horizontal(|ui| {
            let width =
                (ui.available_width() - FIELD_WIDTH - ui.spacing().item_spacing.x).max(40.0);
            let slider = ui
                .add(
                    FineSlider::new(value, range.clone())
                        .steps(steps.0, steps.1)
                        .logarithmic(logarithmic)
                        .width(width),
                )
                .labelled_by(name);
            let field = ui
                .add_sized(
                    [FIELD_WIDTH, ui.spacing().interact_size.y],
                    egui::DragValue::new(value)
                        .range(range)
                        .speed(steps.1)
                        .fixed_decimals(decimals)
                        .suffix(suffix),
                )
                .labelled_by(name)
                .on_hover_text(t!("adjust-type-value"));
            slider.union(field)
        })
        .inner;
    ui.add_space(8.0);
    response
}

/// Onglet « Base » : exposition, température, teinte.
fn base_tab(
    ui: &mut Ui,
    shot: [f32; 3],
    presets: &[(&'static str, f32)],
    eyedropper: bool,
    layers: &mut Layers,
    pending: &mut Pending,
) {
    let ev = t!("adjust-ev-suffix");
    let name = title_row(ui, t!("adjust-exposure"), |ui| {
        pending.auto_exposure = ui
            .button(t!("adjust-auto"))
            .on_hover_text(t!("adjust-auto-exposure-tip"))
            .clicked();
    });
    let r = setting_slider(
        ui,
        name,
        &mut layers.correction.exposure,
        EXPOSURE_RANGE,
        (0.1, 0.01),
        2,
        &ev,
        false,
    );
    if r.double_clicked_slider(ui) {
        layers.correction.exposure = 0.0;
    }
    // Balance en kelvins et teinte (présentation des gains relatifs).
    let (k0, t0) = kelvin::of_gains(shot, layers.correction.gains);
    let (mut k, mut tint) = (k0.round(), t0.round());
    let current = wb_choice(shot, layers.correction.gains, presets);
    let name = title_row(ui, t!("adjust-temperature"), |ui| {
        // De droite à gauche : la pipette, puis la liste.
        pending.eyedropper = ui
            .selectable_label(eyedropper, t!("adjust-eyedropper"))
            .on_hover_text(t!("adjust-eyedropper-tip"))
            .clicked();
        egui::ComboBox::from_id_salt("adjust_wb_preset")
            .selected_text(current.map_or_else(|| t!("adjust-wb-custom"), wb_name))
            .show_ui(ui, |ui| {
                let mut choices = vec![WhiteBalance::AsShot, WhiteBalance::Auto];
                choices.extend(presets.iter().map(|&(id, k)| WhiteBalance::Preset(id, k)));
                for choice in choices {
                    if ui
                        .selectable_label(current == Some(choice), wb_name(choice))
                        .clicked()
                    {
                        pending.white_balance = Some(choice);
                    }
                }
            })
            .response
            .on_hover_text(t!("adjust-wb-tip"));
    });
    let rk = setting_slider(ui, name, &mut k, KELVIN_RANGE, (50.0, 1.0), 0, " K", true);
    let name = title_row(ui, t!("adjust-tint"), |_| {});
    let rt = setting_slider(
        ui,
        name,
        &mut tint,
        KELVIN_TINT_RANGE,
        (5.0, 1.0),
        0,
        "",
        false,
    );
    if rk.double_clicked_slider(ui) || rt.double_clicked_slider(ui) {
        // Double-clic : la balance de la prise de vue.
        layers.correction.gains = Gains::NONE;
    } else if (rk.changed() || rt.changed())
        && ((k - k0.round()).abs() > 0.5 || (tint - t0.round()).abs() > 0.5)
    {
        layers.correction.gains = kelvin::gains(shot, k, tint);
    }
}

/// Onglet « Style » : le décalage créatif, s'il est activé.
fn style_tab(ui: &mut Ui, layers: &mut Layers) {
    let Some(mut creative) = layers.creative else {
        return;
    };
    let ev = t!("adjust-ev-suffix");
    let name = title_row(ui, t!("adjust-exposure"), |_| {});
    if setting_slider(
        ui,
        name,
        &mut creative.exposure,
        EXPOSURE_RANGE,
        (0.1, 0.01),
        2,
        &ev,
        false,
    )
    .double_clicked_slider(ui)
    {
        creative.exposure = 0.0;
    }
    let (mut warmth, mut tint) = (creative.gains.warmth(), creative.gains.tint());
    let name = title_row(ui, t!("adjust-warmth"), |_| {});
    let rw = setting_slider(
        ui,
        name,
        &mut warmth,
        WARMTH_RANGE,
        (0.1, 0.01),
        2,
        "",
        false,
    );
    let name = title_row(ui, t!("adjust-creative-tint"), |_| {});
    let rt = setting_slider(ui, name, &mut tint, TINT_RANGE, (0.05, 0.01), 2, "", false);
    if rw.double_clicked_slider(ui) {
        warmth = 0.0;
    }
    if rt.double_clicked_slider(ui) {
        tint = 0.0;
    }
    let gains = Gains::from_warmth_tint(warmth, tint);
    if (gains.red - creative.gains.red).abs() > 1e-5
        || (gains.blue - creative.gains.blue).abs() > 1e-5
    {
        creative.gains = gains;
    }
    layers.creative = Some(creative);
}

/// Onglet « N&B » : le filtre (préréglage), le mélange, « Normalisé ».
fn mono_tab(ui: &mut Ui, layers: &mut Layers) {
    let Some(mut mixer) = layers.mono else {
        return;
    };
    let current = mixer.preset_name();
    title_row(ui, t!("adjust-filter"), |ui| {
        egui::ComboBox::from_id_salt("adjust_mono_preset")
            .selected_text(current.map_or_else(|| t!("adjust-mono-custom"), mixer_name))
            .show_ui(ui, |ui| {
                for p in Mixer::PRESETS {
                    if ui
                        .selectable_label(current == Some(p), mixer_name(p))
                        .clicked()
                        && let Some(m) = Mixer::preset(p)
                    {
                        mixer = Mixer {
                            normalise: mixer.normalise,
                            ..m
                        };
                    }
                }
            });
    });
    ui.add_space(6.0);
    let neutral = Mixer::default().parts();
    for (i, key) in ["adjust-red", "adjust-green", "adjust-blue"]
        .into_iter()
        .enumerate()
    {
        let mut percent = mixer.parts()[i] * 100.0;
        let name = title_row(ui, t!(key), |_| {});
        let r = setting_slider(
            ui,
            name,
            &mut percent,
            MIXER_RANGE,
            (5.0, 1.0),
            0,
            " %",
            false,
        );
        let wanted = if r.double_clicked_slider(ui) {
            Some(neutral[i])
        } else if r.changed() {
            Some(percent / 100.0)
        } else {
            None
        };
        if let Some(v) = wanted {
            mixer = if mixer.normalise {
                // Le total reste à 100 % : les deux autres suivent.
                mixer.set_locked(i, v)
            } else {
                let mut parts = mixer.parts();
                parts[i] = v;
                mixer.with_parts(parts)
            };
        }
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .checkbox(&mut mixer.normalise, t!("adjust-mono-normalise"))
            .on_hover_text(t!("adjust-mono-normalise-tip"))
            .changed()
            && mixer.normalise
        {
            mixer = mixer.rescaled();
        }
        ui.weak(t!(
            "adjust-mono-total",
            total = format!("{:.0}", mixer.sum() * 100.0)
        ));
    });
    layers.mono = Some(mixer);
}

/// Onglet « Valeurs » : un tableau à reporter dans un logiciel de développement, comme
/// les EXIF (clic droit sur une ligne : la copier ; « Tout copier »).
fn values_tab(ui: &mut Ui, layers: &Layers, shot: [f32; 3]) {
    let rows = adjust_rows(layers, shot);
    let all = rows
        .iter()
        .map(|(label, value)| format!("{label} : {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    egui::Grid::new("adjust_values")
        .num_columns(2)
        .spacing(vec2(10.0, 6.0))
        .show(ui, |ui| {
            for (label, value) in &rows {
                let a = ui
                    .add(egui::Label::new(egui::RichText::new(label).weak()).sense(Sense::click()));
                let b = ui.add(egui::Label::new(value).wrap().sense(Sense::click()));
                a.union(b).context_menu(|ui| {
                    if ui.button(t!("metadata-menu-copy")).clicked() {
                        ui.ctx().copy_text(format!("{label} : {value}"));
                        ui.close();
                    }
                    if ui.button(t!("metadata-menu-copy-all")).clicked() {
                        ui.ctx().copy_text(all.clone());
                        ui.close();
                    }
                });
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    if ui.button(t!("adjust-copy-all")).clicked() {
        ui.ctx().copy_text(all);
    }
}

/// Lignes de l'onglet « Valeurs » : (libellé, valeur).
pub(super) fn adjust_rows(layers: &Layers, shot: [f32; 3]) -> Vec<(String, String)> {
    let r = tri::adjust::report(layers, shot);
    let ev = t!("adjust-ev-suffix");
    let signed = |v: f32| {
        let text = tri::i18n::decimal(v as f64, 2);
        if v >= 0.0 { format!("+{text}") } else { text }
    };
    let kelvins = |(k, tint): (f32, f32)| {
        format!("{k:.0} K, {} {tint:+.0}", t!("adjust-tint").to_lowercase())
    };
    let mut rows = vec![
        (
            t!("adjust-value-exposure"),
            format!("{}{ev}", signed(r.exposure)),
        ),
        (
            t!("adjust-value-correction"),
            format!("{}{ev}", signed(r.correction_exposure)),
        ),
    ];
    if let Some(c) = layers.creative {
        rows.push((
            t!("adjust-value-creative"),
            format!("{}{ev}", signed(c.exposure)),
        ));
    }
    rows.push((t!("adjust-value-balance"), kelvins(r.kelvin)));
    rows.push((t!("adjust-value-shot"), kelvins(r.shot_kelvin)));
    if let Some(c) = layers.creative {
        rows.push((
            t!("adjust-value-creative-balance"),
            t!(
                "adjust-value-warmth-tint",
                warmth = signed(c.gains.warmth()),
                tint = signed(c.gains.tint())
            ),
        ));
    }
    if let Some([red, green, blue]) = r.mixer {
        rows.push((
            t!("adjust-value-mono"),
            format!("{red:.0} / {green:.0} / {blue:.0} %"),
        ));
    }
    rows
}

/// Balance de la liste qui correspond à `gains` (prise de vue `shot`), s'il y en a une.
fn wb_choice(
    shot: [f32; 3],
    gains: Gains,
    presets: &[(&'static str, f32)],
) -> Option<WhiteBalance> {
    if gains.is_none() {
        return Some(WhiteBalance::AsShot);
    }
    let (k, tint) = kelvin::of_gains(shot, gains);
    presets
        .iter()
        .find(|&&(_, pk)| (k - pk).abs() < 10.0 && tint.abs() < 1.0)
        .map(|&(id, pk)| WhiteBalance::Preset(id, pk))
}

/// Nom affiché d'une balance de la liste.
fn wb_name(choice: WhiteBalance) -> String {
    match choice {
        WhiteBalance::AsShot => t!("adjust-wb-as-shot"),
        WhiteBalance::Auto => t!("adjust-wb-auto"),
        WhiteBalance::Preset(id, k) => t!(
            match id {
                "daylight" => "adjust-wb-daylight",
                "cloudy" => "adjust-wb-cloudy",
                "shade" => "adjust-wb-shade",
                "tungsten" => "adjust-wb-tungsten",
                "fluorescent" => "adjust-wb-fluorescent",
                _ => "adjust-wb-flash",
            },
            kelvin = format!("{k:.0}")
        ),
    }
}

/// Nom affiché d'un préréglage de noir et blanc.
fn mixer_name(preset: &str) -> String {
    t!(match preset {
        "red" => "adjust-mono-red",
        "orange" => "adjust-mono-orange",
        "yellow" => "adjust-mono-yellow",
        "green" => "adjust-mono-green",
        "blue" => "adjust-mono-blue",
        _ => "adjust-mono-neutral",
    })
}

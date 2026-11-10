//! Réglages de visualisation dans l'interface (issue n° 137) : section « Réglages » du
//! panneau de droite (exposition, balance, décalage créatif, noir et blanc), avant /
//! après, « Appliquer à la sélection » (corrections harmonisées sur l'image courante,
//! décalage créatif et noir et blanc copiés), rapport des valeurs à reporter. Les réglages
//! sont lus en fond dans les sidecars, enregistrés dans `tri:Adjust` (annulables) et
//! appliqués aux textures ([`crate::adjust_view`]).

use super::*;
use tri::adjust::{Gains, Layers, Mixer, kelvin};

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
    fn adjust_live(&mut self, path: &Path, layers: Layers) {
        {
            let mut shared = self.adjust_map.write().unwrap_or_else(|e| e.into_inner());
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
    /// l'historique par geste, pas par image.
    pub(super) fn adjust_commit(&mut self, ctx: &egui::Context) {
        if self.adjust_dirty.is_none() || ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        let Some(path) = self.adjust_dirty.take() else {
            return;
        };
        let layers = self.adjust_layers(&path);
        self.save_adjustments(vec![(path, layers)]);
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

    /// Balance « auto » : la balance de la correction rend la dominante de l'image neutre
    /// (monde gris, sur la vignette).
    fn adjust_neutralise(&mut self, path: &Path, mut layers: Layers) -> Option<Layers> {
        let base = self.adjust_base(path)?;
        let cast = tri::adjust::measure(&base.bitmap, base.transfer, None).cast;
        layers.correction.gains = tri::adjust::neutral(cast);
        Some(layers)
    }

    /// Couleur de l'éclairage de la prise de vue de `path` (D65 tant qu'elle n'est pas
    /// lue, ou pour une image plate).
    fn adjust_shot_of(&self, path: &Path) -> [f32; 3] {
        self.adjust_shot.get(path).copied().unwrap_or(kelvin::D65)
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
    /// sur la vignette sans réglage.
    pub(super) fn adjust_pick(&mut self, path: &Path, zone: tri::adjust::Zone) {
        let Some(base) = self.adjust_base(path) else {
            self.notice = Some((t!("adjust-apply-wait"), false));
            return;
        };
        let cast = tri::adjust::measure(&base.bitmap, base.transfer, Some(zone)).cast;
        let mut layers = self.adjust_layers(path).unwrap_or_default();
        layers.correction.gains = tri::adjust::neutral(cast);
        self.save_adjustments(vec![(path.to_path_buf(), Some(layers))]);
    }

    /// Exposition automatique de la correction (médiane vers `[adjust]
    /// auto_exposure_target`), mesurée sur la vignette.
    fn adjust_auto_exposure(&mut self, path: &Path, mut layers: Layers) -> Option<Layers> {
        let base = self.adjust_base(path)?;
        let ev = tri::adjust::auto_exposure(
            &base,
            layers.correction.gains,
            self.settings.adjust.auto_exposure_target,
        );
        layers.correction.exposure = ev.clamp(*EXPOSURE_RANGE.start(), *EXPOSURE_RANGE.end());
        Some(layers)
    }

    /// « Appliquer à la sélection » : les réglages de l'image courante partent vers les
    /// autres images sélectionnées, corrections harmonisées (un seul pas de l'historique).
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
        let reference = self.adjust_layers(&file.path).unwrap_or_default();
        let Some(reference_base) = self.adjust_base(&file.path) else {
            self.notice = Some((t!("adjust-apply-wait"), false));
            return;
        };
        let mut changes = Vec::new();
        let mut missing = 0;
        for target in targets {
            let Some(base) = self.adjust_base(&target) else {
                missing += 1;
                continue;
            };
            let current = tri::adjust::read(&target)
                .ok()
                .flatten()
                .unwrap_or_default();
            let layers = tri::adjust::apply_to(
                &reference,
                &reference_base,
                &current,
                &base,
                tri::adjust::Modes::default(),
            );
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

    /// Section « Réglages » du panneau de droite, pour l'image courante.
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
        let mut edited = false;
        let slider = |ui: &mut Ui,
                      label: String,
                      value: &mut f32,
                      range: std::ops::RangeInclusive<f32>,
                      suffix: &str,
                      default: f32| {
            // Une ligne de la grille : libellé, curseur.
            let name = ui.label(label).id;
            let response = labelled_slider(ui, name, value, range, 0.01, 2, suffix);
            ui.end_row();
            if response.double_clicked_slider(ui) {
                *value = default;
                return true;
            }
            response.changed()
        };
        let ev = t!("adjust-ev-suffix");
        let shot = self.adjust_shot_of(&file.path);
        let mut auto_exposure = false;
        let mut white_balance = None;
        // Correction.
        ui.label(egui::RichText::new(t!("adjust-correction")).strong());
        egui::Grid::new("adjust_correction")
            .num_columns(2)
            .show(ui, |ui| {
                let name = ui.label(t!("adjust-exposure")).id;
                let response = ui
                    .horizontal(|ui| {
                        let r = labelled_slider(
                            ui,
                            name,
                            &mut layers.correction.exposure,
                            EXPOSURE_RANGE,
                            0.01,
                            2,
                            &ev,
                        );
                        auto_exposure = ui
                            .small_button(t!("adjust-auto"))
                            .on_hover_text(t!("adjust-auto-exposure-tip"))
                            .clicked();
                        r
                    })
                    .inner;
                ui.end_row();
                if response.double_clicked_slider(ui) {
                    layers.correction.exposure = 0.0;
                    edited = true;
                } else if response.changed() {
                    edited = true;
                }
                // Balance en kelvins et teinte (présentation des gains relatifs).
                let (k0, t0) = kelvin::of_gains(shot, layers.correction.gains);
                let (shot_k, shot_t) = kelvin::of_gains(shot, Gains::NONE);
                let (mut k, mut tint) = (k0.round(), t0.round());
                let name = ui.label(t!("adjust-temperature")).id;
                let rk = labelled_slider_with(ui, name, &mut k, KELVIN_RANGE, 1.0, 0, " K", true);
                ui.end_row();
                let name = ui.label(t!("adjust-tint")).id;
                let rt = labelled_slider(ui, name, &mut tint, KELVIN_TINT_RANGE, 1.0, 0, "");
                ui.end_row();
                let mut wb = rk.changed() || rt.changed();
                if rk.double_clicked_slider(ui) || rt.double_clicked_slider(ui) {
                    // Double-clic : la balance de la prise de vue.
                    (k, tint) = (shot_k, shot_t);
                    layers.correction.gains = Gains::NONE;
                    edited = true;
                    wb = false;
                }
                if wb && ((k - k0.round()).abs() > 0.5 || (tint - t0.round()).abs() > 0.5) {
                    layers.correction.gains = kelvin::gains(shot, k, tint);
                    edited = true;
                }
            });
        ui.horizontal_wrapped(|ui| {
            let presets = self.settings.adjust.white_balance_presets();
            let current = wb_choice(shot, layers.correction.gains, &presets);
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
                            white_balance = Some(choice);
                        }
                    }
                })
                .response
                .on_hover_text(t!("adjust-wb-tip"));
            // Pipette : dans la grille, ouvre le plein écran, où l'image est en grand.
            if ui
                .selectable_label(self.adjust_eyedropper, t!("adjust-eyedropper"))
                .on_hover_text(t!("adjust-eyedropper-tip"))
                .clicked()
            {
                self.adjust_eyedropper = !self.adjust_eyedropper;
                if self.adjust_eyedropper && self.mode == Mode::Grid {
                    self.open_view();
                }
            }
        });
        ui.add_space(4.0);
        // Décalage créatif : une case, comme le noir et blanc.
        let mut creative_on = layers.creative.is_some();
        if ui
            .checkbox(&mut creative_on, t!("adjust-creative"))
            .on_hover_text(t!("adjust-creative-tip"))
            .changed()
        {
            if creative_on {
                layers.creative = Some(
                    self.adjust_creative_memory
                        .remove(&file.path)
                        .unwrap_or_default(),
                );
            } else if let Some(c) = layers.creative.take() {
                self.adjust_creative_memory.insert(file.path.clone(), c);
            }
            edited = true;
        }
        if let Some(mut creative) = layers.creative {
            egui::Grid::new("adjust_creative")
                .num_columns(2)
                .show(ui, |ui| {
                    edited |= slider(
                        ui,
                        t!("adjust-exposure"),
                        &mut creative.exposure,
                        EXPOSURE_RANGE,
                        &ev,
                        0.0,
                    );
                    let (mut cw, mut ct) = (creative.gains.warmth(), creative.gains.tint());
                    if slider(ui, t!("adjust-warmth"), &mut cw, WARMTH_RANGE, "", 0.0)
                        | slider(ui, t!("adjust-creative-tint"), &mut ct, TINT_RANGE, "", 0.0)
                    {
                        creative.gains = Gains::from_warmth_tint(cw, ct);
                        edited = true;
                    }
                });
            layers.creative = Some(creative);
        }
        ui.add_space(4.0);
        // Noir et blanc.
        let mut mono = layers.mono.is_some();
        if ui.checkbox(&mut mono, t!("adjust-mono")).changed() {
            if mono {
                layers.mono = Some(
                    self.adjust_mono_memory
                        .remove(&file.path)
                        .unwrap_or_default(),
                );
            } else if let Some(m) = layers.mono.take() {
                self.adjust_mono_memory.insert(file.path.clone(), m);
            }
            edited = true;
        }
        if let Some(mut mixer) = layers.mono {
            // Préréglages en boutons, visibles d'un coup d'œil (retour du 10/11/2026).
            let current = mixer.preset_name();
            ui.horizontal_wrapped(|ui| {
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
                        edited = true;
                    }
                }
            });
            let neutral = Mixer::default();
            let mut percent = [mixer.red * 100.0, mixer.green * 100.0, mixer.blue * 100.0];
            let defaults = [neutral.red, neutral.green, neutral.blue].map(|v| v * 100.0);
            egui::Grid::new("adjust_mixer")
                .num_columns(2)
                .show(ui, |ui| {
                    for (i, key) in ["adjust-red", "adjust-green", "adjust-blue"]
                        .into_iter()
                        .enumerate()
                    {
                        let name = ui.label(t!(key)).id;
                        let response =
                            labelled_slider(ui, name, &mut percent[i], MIXER_RANGE, 1.0, 0, " %");
                        ui.end_row();
                        if response.double_clicked_slider(ui) {
                            percent[i] = defaults[i];
                            edited = true;
                        } else if response.changed() {
                            edited = true;
                        }
                    }
                });
            mixer.red = percent[0] / 100.0;
            mixer.green = percent[1] / 100.0;
            mixer.blue = percent[2] / 100.0;
            ui.horizontal_wrapped(|ui| {
                if ui
                    .checkbox(&mut mixer.normalise, t!("adjust-mono-normalise"))
                    .on_hover_text(t!("adjust-mono-normalise-tip"))
                    .changed()
                {
                    edited = true;
                }
                ui.weak(t!(
                    "adjust-mono-total",
                    total = format!("{:.0}", mixer.sum() * 100.0)
                ));
            });
            layers.mono = Some(mixer);
        }
        ui.add_space(6.0);
        // Rapport : les valeurs à reporter dans un logiciel de développement.
        if !layers.is_none() {
            let report = adjust_report_text(&layers, shot);
            ui.weak(&report);
            if ui
                .small_button(t!("adjust-copy-report"))
                .on_hover_text(t!("adjust-copy-report-tip"))
                .clicked()
            {
                ui.ctx().copy_text(report);
            }
            ui.add_space(4.0);
        }
        let mut reset = false;
        let mut apply = false;
        let mut toggle = false;
        ui.horizontal_wrapped(|ui| {
            let others = self.selection.visible(&self.files) > 1;
            apply = ui
                .add_enabled(others, egui::Button::new(t!("adjust-apply")))
                .on_hover_text(t!("adjust-apply-tip"))
                .on_disabled_hover_text(t!("adjust-apply-tip"))
                .clicked();
            toggle = ui
                .selectable_label(self.showing_original(), t!("adjust-before"))
                .on_hover_text(t!("adjust-before-tip"))
                .clicked();
            reset = ui
                .add_enabled(!layers.is_none(), egui::Button::new(t!("adjust-reset")))
                .clicked();
        });
        let mut button = false;
        match white_balance {
            Some(WhiteBalance::AsShot) => {
                layers.correction.gains = Gains::NONE;
                (edited, button) = (true, true);
            }
            Some(WhiteBalance::Preset(_, k)) => {
                layers.correction.gains = kelvin::gains(shot, k, 0.0);
                (edited, button) = (true, true);
            }
            Some(WhiteBalance::Auto) => match self.adjust_neutralise(&file.path, layers) {
                Some(n) => {
                    layers = n;
                    (edited, button) = (true, true);
                }
                None => self.notice = Some((t!("adjust-apply-wait"), false)),
            },
            None => {}
        }
        if auto_exposure {
            match self.adjust_auto_exposure(&file.path, layers) {
                Some(n) => {
                    layers = n;
                    (edited, button) = (true, true);
                }
                None => self.notice = Some((t!("adjust-apply-wait"), false)),
            }
        }
        if reset {
            layers = Layers::default();
            edited = true;
        }
        if edited && layers != before {
            self.adjust_live(&file.path, layers);
            // Un bouton : enregistré tout de suite ; un curseur : au relâchement.
            if button || reset {
                self.adjust_commit(ui.ctx());
            }
        }
        if toggle {
            self.toggle_before_after();
        }
        if apply {
            // Ce qui vient d'être réglé d'abord.
            self.adjust_commit(ui.ctx());
            self.adjust_apply_to_selection();
        }
    }
}

/// Un curseur et sa valeur modifiable, tous deux nommés par le libellé `name` (lecteurs
/// d'écran, tests) : le champ de valeur d'un `Slider` d'egui n'a pas de nom à lui. Un clic
/// sur la valeur permet de la taper.
fn labelled_slider(
    ui: &mut Ui,
    name: egui::Id,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
    decimals: usize,
    suffix: &str,
) -> egui::Response {
    labelled_slider_with(ui, name, value, range, step, decimals, suffix, false)
}

/// [`labelled_slider`], en échelle logarithmique au besoin (kelvins).
#[allow(clippy::too_many_arguments)]
fn labelled_slider_with(
    ui: &mut Ui,
    name: egui::Id,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
    decimals: usize,
    suffix: &str,
    logarithmic: bool,
) -> egui::Response {
    ui.horizontal(|ui| {
        let slider = ui
            .add(
                egui::Slider::new(value, range.clone())
                    .step_by(step)
                    .logarithmic(logarithmic)
                    .show_value(false),
            )
            .labelled_by(name);
        let field = ui
            .add(
                egui::DragValue::new(value)
                    .range(range)
                    .speed(step)
                    .fixed_decimals(decimals)
                    .suffix(suffix),
            )
            .labelled_by(name)
            .on_hover_text(t!("adjust-type-value"));
        slider.union(field)
    })
    .inner
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

/// Le rapport de `layers`, à reporter à la main (et à copier).
pub(super) fn adjust_report_text(layers: &Layers, shot: [f32; 3]) -> String {
    let r = tri::adjust::report(layers, shot);
    let signed = |v: f32| {
        let text = tri::i18n::decimal(v as f64, 2);
        if v >= 0.0 { format!("+{text}") } else { text }
    };
    let mut lines = vec![t!(
        "adjust-report-exposure",
        total = signed(r.exposure),
        correction = signed(r.correction_exposure),
        creative = signed(r.creative_exposure)
    )];
    if !r.gains.is_none() {
        lines.push(t!(
            "adjust-report-balance",
            kelvin = format!("{:.0}", r.kelvin.0),
            tint = format!("{:+.0}", r.kelvin.1),
            shot_kelvin = format!("{:.0}", r.shot_kelvin.0),
            shot_tint = format!("{:+.0}", r.shot_kelvin.1)
        ));
    }
    if let Some([red, green, blue]) = r.mixer {
        lines.push(t!(
            "adjust-report-mono",
            red = format!("{red:.0}"),
            green = format!("{green:.0}"),
            blue = format!("{blue:.0}")
        ));
    }
    lines.join("\n")
}

//! Réglages de visualisation dans l'interface (issue n° 137) : section « Réglages » du
//! panneau de droite (exposition, balance, décalage créatif, noir et blanc), avant /
//! après, « Appliquer à la sélection » (corrections harmonisées sur l'image courante,
//! décalage créatif et noir et blanc copiés), rapport des valeurs à reporter. Les réglages
//! sont lus en fond dans les sidecars, enregistrés dans `tri:Adjust` (annulables) et
//! appliqués aux textures ([`crate::adjust_view`]).

use super::*;
use tri::adjust::{Gains, Layers, Mixer};

/// Réglages lus en fond : l'image et ce que son sidecar contient.
pub(super) type AdjustRead = (PathBuf, Option<Layers>);

/// Bornes des curseurs.
const EXPOSURE_RANGE: std::ops::RangeInclusive<f32> = -3.0..=3.0;
const WARMTH_RANGE: std::ops::RangeInclusive<f32> = -1.5..=1.5;
const TINT_RANGE: std::ops::RangeInclusive<f32> = -0.5..=0.5;
const MIXER_RANGE: std::ops::RangeInclusive<f32> = -100.0..=200.0;

/// Préréglages du noir et blanc proposés, dans l'ordre de la liste.
const MIXER_PRESETS: [&str; 6] = ["neutral", "red", "orange", "yellow", "green", "blue"];

impl TriApp {
    /// Réglages connus de `path` (`None` : aucun, ou pas encore lus : la lecture part en
    /// fond, sans jamais attendre le disque, comme les versions).
    pub(super) fn adjust_layers(&mut self, path: &Path) -> Option<Layers> {
        self.adjust_poll();
        if !self.adjust_known.contains(path) && self.adjust_pending.insert(path.to_path_buf()) {
            let (tx, target) = (self.adjust_read.0.clone(), path.to_path_buf());
            tri::cache::defer_job(format!("adjust:{}", target.display()), move || {
                let layers = tri::adjust::read(&target).ok().flatten();
                let _ = tx.send((target, layers));
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
        while let Ok((path, layers)) = self.adjust_read.1.try_recv() {
            self.adjust_pending.remove(&path);
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

    /// « Neutraliser » : la balance de la correction rend la dominante de l'image neutre
    /// (monde gris, sur la vignette).
    fn adjust_neutralise(&mut self, path: &Path, mut layers: Layers) -> Option<Layers> {
        let base = self.adjust_base(path)?;
        let cast = tri::adjust::measure(&base.bitmap, base.transfer, None).cast;
        layers.correction.gains = tri::adjust::neutral(cast);
        Some(layers)
    }

    /// Pipette : la balance de la correction rend neutre la zone autour de `point` (en
    /// fractions de l'image), mesurée sur la vignette sans réglage.
    pub(super) fn adjust_pick(&mut self, path: &Path, point: egui::Pos2) {
        let Some(base) = self.adjust_base(path) else {
            self.notice = Some((t!("adjust-apply-wait"), false));
            return;
        };
        let half = self.gui.adjust_eyedropper_size / 2.0;
        let zone = tri::adjust::Zone {
            x: (point.x - half).clamp(0.0, 1.0 - 2.0 * half),
            y: (point.y - half).clamp(0.0, 1.0 - 2.0 * half),
            w: 2.0 * half,
            h: 2.0 * half,
        };
        let cast = tri::adjust::measure(&base.bitmap, base.transfer, Some(zone)).cast;
        let mut layers = self.adjust_layers(path).unwrap_or_default();
        layers.correction.gains = tri::adjust::neutral(cast);
        self.save_adjustments(vec![(path.to_path_buf(), Some(layers))]);
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
        // Correction.
        ui.label(egui::RichText::new(t!("adjust-correction")).strong());
        egui::Grid::new("adjust_correction")
            .num_columns(2)
            .show(ui, |ui| {
                edited |= slider(
                    ui,
                    t!("adjust-exposure"),
                    &mut layers.correction.exposure,
                    EXPOSURE_RANGE,
                    &ev,
                    0.0,
                );
                let (mut warmth, mut tint) = (
                    layers.correction.gains.warmth(),
                    layers.correction.gains.tint(),
                );
                let wb = slider(ui, t!("adjust-warmth"), &mut warmth, WARMTH_RANGE, "", 0.0)
                    | slider(ui, t!("adjust-tint"), &mut tint, TINT_RANGE, "", 0.0);
                if wb {
                    layers.correction.gains = Gains::from_warmth_tint(warmth, tint);
                    edited = true;
                }
            });
        let mut neutralise = false;
        ui.horizontal_wrapped(|ui| {
            neutralise = ui
                .button(t!("adjust-neutral"))
                .on_hover_text(t!("adjust-neutral-tip"))
                .clicked();
            // Pipette : en plein écran, là où l'on voit l'image en grand.
            if self.mode == Mode::View
                && ui
                    .selectable_label(self.adjust_eyedropper, t!("adjust-eyedropper"))
                    .on_hover_text(t!("adjust-eyedropper-tip"))
                    .clicked()
            {
                self.adjust_eyedropper = !self.adjust_eyedropper;
            }
        });
        ui.add_space(4.0);
        // Décalage créatif.
        ui.label(egui::RichText::new(t!("adjust-creative")).strong())
            .on_hover_text(t!("adjust-creative-tip"));
        egui::Grid::new("adjust_creative")
            .num_columns(2)
            .show(ui, |ui| {
                edited |= slider(
                    ui,
                    t!("adjust-exposure"),
                    &mut layers.creative.exposure,
                    EXPOSURE_RANGE,
                    &ev,
                    0.0,
                );
                let (mut cw, mut ct) =
                    (layers.creative.gains.warmth(), layers.creative.gains.tint());
                if slider(ui, t!("adjust-warmth"), &mut cw, WARMTH_RANGE, "", 0.0)
                    | slider(ui, t!("adjust-tint"), &mut ct, TINT_RANGE, "", 0.0)
                {
                    layers.creative.gains = Gains::from_warmth_tint(cw, ct);
                    edited = true;
                }
            });
        ui.add_space(4.0);
        // Noir et blanc.
        let mut mono = layers.mono.is_some();
        if ui.checkbox(&mut mono, t!("adjust-mono")).changed() {
            layers.mono = mono.then(Mixer::default);
            edited = true;
        }
        if let Some(mut mixer) = layers.mono {
            let current = MIXER_PRESETS
                .iter()
                .find(|p| Mixer::preset(p) == Some(mixer))
                .map_or_else(|| t!("adjust-mono-custom"), |p| mixer_name(p));
            egui::ComboBox::from_id_salt("adjust_mono_preset")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for p in MIXER_PRESETS {
                        if ui.selectable_label(false, mixer_name(p)).clicked()
                            && let Some(m) = Mixer::preset(p)
                        {
                            mixer = m;
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
            mixer = Mixer {
                red: percent[0] / 100.0,
                green: percent[1] / 100.0,
                blue: percent[2] / 100.0,
            };
            layers.mono = Some(mixer);
        }
        ui.add_space(6.0);
        // Rapport : les valeurs à reporter dans un logiciel de développement.
        if !layers.is_none() {
            let report = adjust_report_text(&layers);
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
        if neutralise && let Some(n) = self.adjust_neutralise(&file.path, layers) {
            layers = n;
            edited = true;
        }
        if reset {
            layers = Layers::default();
            edited = true;
        }
        if edited && layers != before {
            self.adjust_live(&file.path, layers);
            // Un bouton : enregistré tout de suite ; un curseur : au relâchement.
            if neutralise || reset {
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
/// d'écran, tests) : le champ de valeur d'un `Slider` d'egui n'a pas de nom à lui.
fn labelled_slider(
    ui: &mut Ui,
    name: egui::Id,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f64,
    decimals: usize,
    suffix: &str,
) -> egui::Response {
    ui.horizontal(|ui| {
        let slider = ui
            .add(
                egui::Slider::new(value, range.clone())
                    .step_by(step)
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
            .labelled_by(name);
        slider.union(field)
    })
    .inner
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
pub(super) fn adjust_report_text(layers: &Layers) -> String {
    let r = tri::adjust::report(layers);
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
            warmth = signed(r.warmth),
            tint = signed(r.tint)
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

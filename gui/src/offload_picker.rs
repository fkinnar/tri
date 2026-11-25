//! Choisir les photos d'une séance à décharger (#507) : une galerie des images de la carte,
//! une coche sur chacune, « Tout sélectionner », et la photo en grand comme l'Aperçu de
//! macOS (ajustée, pas de 1:1). Idée de l'utilisateur (25/11/2026) : plus simple que la
//! grille de `tri`, qu'il aurait fallu ouvrir sur la carte puis rendre telle quelle.
//! Les vignettes sont les JPEG embarqués, lus en fond sur la carte, sans cache.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Color32, ColorImage, Key, Sense, TextureHandle, vec2};
use tri::cache::Level;
use tri::media::{MediaFile, MediaKind};
use tri::offload::Item;

/// Une photo de la séance : une image, ou une paire RAW+JPEG cochée d'un bloc.
struct Photo {
    /// Fichiers sur la carte (la paire entière).
    files: Vec<PathBuf>,
    /// Celui qu'on montre : le JPEG d'une paire (plus rapide), sinon l'image.
    shown: MediaFile,
    name: String,
    /// Heure de prise de vue (« 14:32 »).
    time: String,
    size: u64,
}

/// Ce que devient la boîte.
pub enum Outcome {
    /// Encore ouverte.
    Open,
    /// Validée : les fichiers des photos cochées.
    Done(HashSet<PathBuf>),
    /// Fermée sans rien changer.
    Cancelled,
}

/// La galerie d'une séance.
pub struct Picker {
    day: String,
    photos: Vec<Photo>,
    pub(crate) checked: Vec<bool>,
    cursor: usize,
    /// Côté des vignettes (points), réglé par le curseur.
    side: f32,
    /// Photo montrée en grand.
    pub(crate) enlarged: Option<usize>,
    thumbs: Vec<Option<TextureHandle>>,
    rx: Receiver<(usize, ColorImage)>,
    big: Option<(usize, Option<TextureHandle>)>,
    big_rx: Option<Receiver<(usize, ColorImage)>>,
    cancel: Arc<AtomicBool>,
    /// Colonnes de la dernière image dessinée (flèches haut / bas).
    columns: usize,
    /// Départ d'une plage (Maj+clic, Maj+flèches) : la dernière photo touchée.
    anchor: usize,
    /// Mode visuel (`v`, Maj+Espace, comme la grille) : son départ ; les déplacements
    /// cochent la plage parcourue.
    pub(crate) visual: Option<usize>,
}

/// Réglages des images de la galerie : vignettes de 320 px, grand à 1600 px.
fn settings() -> tri::preview::Settings {
    let mut s = tri::preview::Settings::default();
    s.sizes.thumbnail = 320;
    s.sizes.standard = 1600;
    s
}

fn to_image(bitmap: &tri::bitmap::Bitmap) -> ColorImage {
    ColorImage::from_rgb(
        [bitmap.width as usize, bitmap.height as usize],
        &bitmap.data,
    )
}

impl Drop for Picker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Picker {
    /// Galerie des images `items` (une séance) du jour `day` ; `chosen` : les fichiers déjà
    /// choisis (rouverte), sinon tout est coché.
    pub fn new(
        day: &str,
        items: &[&Item],
        chosen: Option<&HashSet<PathBuf>>,
        ctx: &egui::Context,
    ) -> Picker {
        let mut photos: Vec<Photo> = Vec::new();
        for item in items {
            let stem = item.source.with_extension("");
            match photos
                .iter_mut()
                .find(|p| p.files[0].with_extension("") == stem)
            {
                Some(photo) => {
                    photo.files.push(item.source.clone());
                    photo.size += item.size;
                    if item.kind == MediaKind::Flat {
                        photo.shown = MediaFile {
                            path: item.source.clone(),
                            kind: item.kind,
                        };
                    }
                }
                None => photos.push(Photo {
                    files: vec![item.source.clone()],
                    shown: MediaFile {
                        path: item.source.clone(),
                        kind: item.kind,
                    },
                    name: stem
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    time: chrono::DateTime::<chrono::Local>::from(item.modified)
                        .format("%H:%M")
                        .to_string(),
                    size: item.size,
                }),
            }
        }
        let checked = photos
            .iter()
            .map(|p| chosen.is_none_or(|c| p.files.iter().any(|f| c.contains(f))))
            .collect();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let shown: Vec<MediaFile> = photos.iter().map(|p| p.shown.clone()).collect();
        let (stop, repaint) = (cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let s = settings();
            for (i, file) in shown.iter().enumerate() {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                if let Ok(bitmap) = tri::preview::compute(file, Level::Thumbnail, &s, &stop)
                    && tx.send((i, to_image(&bitmap))).is_ok()
                {
                    repaint.request_repaint();
                }
            }
        });
        Picker {
            day: day.to_string(),
            thumbs: (0..photos.len()).map(|_| None).collect(),
            photos,
            checked,
            cursor: 0,
            side: 160.0,
            enlarged: None,
            rx,
            big: None,
            big_rx: None,
            cancel,
            columns: 1,
            anchor: 0,
            visual: None,
        }
    }

    /// Coche les photos de `a` à `b` (dans un sens ou l'autre).
    fn check_range(&mut self, a: usize, b: usize) {
        let (lo, hi) = (a.min(b), a.max(b));
        for c in &mut self.checked[lo..=hi.min(self.photos.len().saturating_sub(1))] {
            *c = true;
        }
    }

    /// Le curseur va en `to` : en mode visuel ou avec Maj (`extend`), la plage parcourue
    /// est cochée ; sinon il devient le départ des plages.
    fn move_to(&mut self, to: usize, extend: bool) {
        self.cursor = to;
        if let Some(start) = self.visual {
            self.check_range(start, to);
        } else if extend {
            self.check_range(self.anchor, to);
        } else {
            self.anchor = to;
        }
    }

    /// Fichiers des photos cochées.
    fn chosen(&self) -> HashSet<PathBuf> {
        self.photos
            .iter()
            .zip(&self.checked)
            .filter(|(_, on)| **on)
            .flat_map(|(p, _)| p.files.iter().cloned())
            .collect()
    }

    /// Montre la photo `i` en grand (lue en fond).
    fn enlarge(&mut self, i: usize, ctx: &egui::Context) {
        self.enlarged = Some(i);
        if self.big.as_ref().is_some_and(|(b, _)| *b == i) {
            return;
        }
        self.big = Some((i, None));
        let file = self.photos[i].shown.clone();
        let (tx, rx) = mpsc::channel();
        let (stop, repaint) = (self.cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let level = match file.kind {
                MediaKind::Raw => Level::Embedded,
                MediaKind::Flat => Level::Standard,
            };
            if let Ok(bitmap) = tri::preview::compute(&file, level, &settings(), &stop) {
                let _ = tx.send((i, to_image(&bitmap)));
                repaint.request_repaint();
            }
        });
        self.big_rx = Some(rx);
    }

    fn receive(&mut self, ctx: &egui::Context) {
        for (i, image) in self.rx.try_iter() {
            let name = format!("offload-pick-{}-{i}", self.day);
            self.thumbs[i] = Some(ctx.load_texture(name, image, Default::default()));
        }
        if let Some(rx) = &self.big_rx {
            for (i, image) in rx.try_iter() {
                if let Some((b, texture)) = &mut self.big
                    && *b == i
                {
                    *texture =
                        Some(ctx.load_texture("offload-pick-big", image, Default::default()));
                }
            }
        }
    }

    /// Clavier, comme la grille : flèches (ou hjkl), Maj + flèches étendent ; Espace coche
    /// ou décoche, `v` / Maj+Espace le mode visuel ; Cmd/Ctrl+A tout cocher, avec Maj tout
    /// décocher ; Entrée agrandit ; Échap sort du visuel, de la vue en grand, puis ferme
    /// (vrai : fermer sans rien changer).
    fn keys(&mut self, ctx: &egui::Context) -> bool {
        let n = self.photos.len();
        let mut close = false;
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            let shift = modifiers.shift;
            let command = modifiers.command;
            let step = if self.enlarged.is_some() {
                0
            } else {
                self.columns.max(1)
            };
            let last = n.saturating_sub(1);
            let moved = match key {
                Key::ArrowLeft | Key::H if !command => Some(self.cursor.saturating_sub(1)),
                Key::ArrowRight | Key::L if !command => Some((self.cursor + 1).min(last)),
                Key::ArrowUp | Key::K if !command && step > 0 => {
                    Some(self.cursor.saturating_sub(step))
                }
                Key::ArrowDown | Key::J if !command && step > 0 => {
                    Some((self.cursor + step).min(last))
                }
                _ => None,
            };
            if n == 0 {
                close |= key == Key::Escape;
                continue;
            }
            if let Some(to) = moved {
                self.move_to(to, shift);
                if self.enlarged.is_some() {
                    self.enlarge(self.cursor, ctx);
                }
                continue;
            }
            match key {
                Key::Space if shift => self.toggle_visual(),
                Key::Space => self.checked[self.cursor] = !self.checked[self.cursor],
                Key::V if !command => self.toggle_visual(),
                Key::A if command => {
                    let on = !shift;
                    self.checked.iter_mut().for_each(|c| *c = on);
                }
                Key::Enter => match self.enlarged {
                    Some(_) => self.enlarged = None,
                    None => self.enlarge(self.cursor, ctx),
                },
                Key::Escape => {
                    if self.visual.is_some() || self.enlarged.is_some() {
                        // Pris ici : la boîte ne se ferme pas avec.
                        ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
                    }
                    if self.visual.is_some() {
                        self.visual = None;
                    } else if self.enlarged.is_some() {
                        self.enlarged = None;
                    } else {
                        close = true;
                    }
                }
                _ => {}
            }
        }
        close
    }

    /// Entre dans le mode visuel (la photo courante cochée) ou en sort.
    fn toggle_visual(&mut self) {
        self.visual = match self.visual {
            Some(_) => None,
            None => {
                self.checked[self.cursor] = true;
                self.anchor = self.cursor;
                Some(self.cursor)
            }
        };
    }

    /// Dessine la boîte.
    pub fn show(&mut self, ctx: &egui::Context, accent: Color32) -> Outcome {
        self.receive(ctx);
        if self.keys(ctx) {
            return Outcome::Cancelled;
        }
        let (mut done, mut cancel) = (false, false);
        let chosen = self.checked.iter().filter(|c| **c).count();
        let bytes: u64 = self
            .photos
            .iter()
            .zip(&self.checked)
            .filter(|(_, on)| **on)
            .map(|(p, _)| p.size)
            .sum();
        let screen = ctx.content_rect().size();
        let width = (screen.x * 0.8).clamp(360.0, 1400.0);
        let height = (screen.y * 0.7).clamp(240.0, 1000.0);
        let modal = crate::app::modal_at("offload_picker", crate::app::CENTRED)
            .title(t!("offload-pick-title", day = self.day.clone()))
            .width(Some(width))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(t!("offload-pick-all")).clicked() {
                        self.checked.iter_mut().for_each(|c| *c = true);
                    }
                    if ui.button(t!("offload-pick-none")).clicked() {
                        self.checked.iter_mut().for_each(|c| *c = false);
                    }
                    ui.label(t!(
                        "offload-pick-count",
                        n = chosen,
                        total = self.photos.len(),
                        size = crate::cache_panel::size_text(bytes)
                    ));
                    if self.enlarged.is_none() {
                        ui.separator();
                        ui.add(
                            egui::Slider::new(&mut self.side, 80.0..=360.0)
                                .show_value(false)
                                .text(t!("offload-pick-size")),
                        );
                    }
                });
                ui.add_space(4.0);
                match self.enlarged {
                    Some(i) => self.big_view(ui, i, vec2(width, height), accent),
                    None => self.gallery(ui, ctx, vec2(width, height), accent),
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    done = ui.button(t!("offload-pick-ok")).clicked();
                    cancel = ui.button(t!("cancel")).clicked();
                });
            });
        if done {
            Outcome::Done(self.chosen())
        } else if cancel || modal.should_close() {
            Outcome::Cancelled
        } else {
            Outcome::Open
        }
    }

    /// La coche d'une photo, en haut à gauche de `rect` ; vrai si cliquée.
    fn check(&mut self, ui: &mut egui::Ui, i: usize, rect: egui::Rect, accent: Color32) -> bool {
        let r = (self.side.min(rect.width()) * 0.09).clamp(8.0, 14.0);
        let centre = rect.left_top() + vec2(r + 4.0, r + 4.0);
        let hit = egui::Rect::from_center_size(centre, vec2(2.0 * r + 6.0, 2.0 * r + 6.0));
        let id = ui.scope_id().with(("offload_pick_check", i));
        let response = ui
            .interact(hit, id, Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let label = self.photos[i].name.clone();
        let on = self.checked[i];
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, &label));
        let painter = ui.painter();
        if on {
            crate::marks::badge_with_glyph(painter, centre, r, true, accent, Color32::WHITE);
        } else {
            painter.circle_filled(centre, r, Color32::from_black_alpha(120));
            crate::marks::neutral_ring(painter, centre, r, Color32::WHITE);
        }
        response.clicked()
    }

    fn gallery(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        size: egui::Vec2,
        accent: Color32,
    ) {
        let side = self.side;
        let gap = 8.0;
        let label_h = ui.text_style_height(&egui::TextStyle::Small) + 4.0;
        self.columns = (((size.x + gap) / (side + gap)).floor() as usize).max(1);
        let mut open = None;
        egui::ScrollArea::vertical()
            .max_height(size.y)
            .min_scrolled_height(size.y)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(size.x);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(gap, gap);
                    for i in 0..self.photos.len() {
                        let (cell, response) =
                            ui.allocate_exact_size(vec2(side, side + label_h), Sense::click());
                        let image = egui::Rect::from_min_size(cell.min, vec2(side, side));
                        let painter = ui.painter();
                        painter.rect_filled(image, 4.0, ui.visuals().extreme_bg_color);
                        match &self.thumbs[i] {
                            Some(t) => {
                                let [w, h] = t.size();
                                let s = (side / w as f32).min(side / h as f32);
                                let r = egui::Rect::from_center_size(
                                    image.center(),
                                    vec2(w as f32 * s, h as f32 * s),
                                );
                                painter.image(
                                    t.id(),
                                    r,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    if self.checked[i] {
                                        Color32::WHITE
                                    } else {
                                        Color32::from_gray(110)
                                    },
                                );
                            }
                            None => {
                                egui::Spinner::new().paint_at(
                                    ui,
                                    egui::Rect::from_center_size(image.center(), vec2(16.0, 16.0)),
                                );
                            }
                        }
                        let photo = &self.photos[i];
                        ui.painter().text(
                            egui::pos2(cell.left(), image.bottom() + 2.0),
                            egui::Align2::LEFT_TOP,
                            format!("{}  {}", photo.name, photo.time),
                            egui::TextStyle::Small.resolve(ui.style()),
                            ui.visuals().text_color(),
                        );
                        if i == self.cursor {
                            ui.painter().rect_stroke(
                                image,
                                4.0,
                                egui::Stroke::new(2.0, accent),
                                egui::StrokeKind::Outside,
                            );
                        }
                        let checked = self.check(ui, i, image, accent);
                        let modifiers = ui.input(|input| input.modifiers);
                        if checked {
                            self.checked[i] = !self.checked[i];
                            self.cursor = i;
                            self.anchor = i;
                        } else if response.double_clicked() {
                            self.cursor = i;
                            open = Some(i);
                        } else if response.clicked() {
                            // Cmd/Ctrl+clic : bascule ; Maj+clic : la plage ; clic seul : le
                            // curseur (il ne décoche pas tout, comme le Finder : les photos
                            // partent toutes cochées).
                            if modifiers.command {
                                self.checked[i] = !self.checked[i];
                                self.cursor = i;
                                self.anchor = i;
                            } else {
                                self.move_to(i, modifiers.shift);
                            }
                        }
                    }
                });
            });
        if let Some(i) = open {
            self.enlarge(i, ctx);
        }
    }

    fn big_view(&mut self, ui: &mut egui::Ui, i: usize, size: egui::Vec2, accent: Color32) {
        let (area, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 4.0, ui.visuals().extreme_bg_color);
        let texture = self
            .big
            .as_ref()
            .filter(|(b, _)| *b == i)
            .and_then(|(_, t)| t.clone())
            .or_else(|| self.thumbs[i].clone());
        match texture {
            Some(t) => {
                let [w, h] = t.size();
                let s = (area.width() / w as f32).min(area.height() / h as f32);
                let r =
                    egui::Rect::from_center_size(area.center(), vec2(w as f32 * s, h as f32 * s));
                painter.image(
                    t.id(),
                    r,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => egui::Spinner::new().paint_at(
                ui,
                egui::Rect::from_center_size(area.center(), vec2(24.0, 24.0)),
            ),
        }
        if self.check(ui, i, area, accent) {
            self.checked[i] = !self.checked[i];
        } else if response.double_clicked() {
            self.enlarged = None;
        }
        let photo = &self.photos[i];
        ui.horizontal(|ui| {
            ui.label(format!(
                "{}  {}  ({}/{})",
                photo.name,
                photo.time,
                i + 1,
                self.photos.len()
            ));
        });
    }
}

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
use tri::offload::{Cut, CutReason, Item, Plan};

/// Une photo de la séance : une image, ou une paire RAW+JPEG cochée d'un bloc.
struct Photo {
    /// Fichiers sur la carte (la paire entière).
    files: Vec<PathBuf>,
    /// Celui qu'on montre : le JPEG d'une paire (plus rapide), sinon l'image.
    shown: MediaFile,
    name: String,
    /// Heure de prise de vue (« 14:32 »).
    time: String,
    /// Heure de prise de vue (la plus ancienne de la paire).
    at: std::time::SystemTime,
    size: u64,
    /// Pour chaque fichier de `files` : son type et s'il est déjà déchargé (d'après
    /// l'historique ou la destination).
    copied: Vec<(MediaKind, bool)>,
}

impl Photo {
    /// Déjà copiée : tous les fichiers que la copie prendrait le sont (avec « RAW
    /// seulement », le RAW d'une paire suffit ; retour du 26/11/2026 : des RAW copiés
    /// seuls laissaient leur photo cochée sans rien dire).
    fn copied(&self, raw_only: bool) -> bool {
        let has_raw = self.copied.iter().any(|(k, _)| *k == MediaKind::Raw);
        self.copied
            .iter()
            .filter(|(k, _)| !(raw_only && has_raw) || *k == MediaKind::Raw)
            .all(|(_, known)| *known)
    }

    /// En partie copiée : un de ses fichiers l'est, pas tous ceux à copier.
    fn partly(&self, raw_only: bool) -> bool {
        !self.copied(raw_only) && self.copied.iter().any(|(_, known)| *known)
    }
}

/// Outil de la galerie de toute la carte (#600, idée de l'utilisateur : une table de
/// montage) ; Alt donne l'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    /// Cocher, agrandir (les gestes de la galerie d'une séance).
    Select,
    /// Couper entre deux photos.
    Scissors,
    /// Recoller une séance à la précédente (clic sur son bandeau).
    Glue,
}

/// Ce que devient la boîte.
pub enum Outcome {
    /// Encore ouverte.
    Open,
    /// Validée : les fichiers des photos cochées, si les photos déjà déchargées sont
    /// exclues (la case de la galerie, celle de la boîte de déchargement), et les
    /// coupures en séances (galerie de toute la carte, #600).
    Done(HashSet<PathBuf>, bool, Option<Vec<Cut>>),
    /// Fermée sans rien changer.
    Cancelled,
}

/// La galerie d'une séance, ou de toute la carte coupée en séances (#600).
pub struct Picker {
    day: String,
    /// Titre de la boîte.
    title: String,
    /// Galerie de toute la carte (#600) : pour chaque photo, si une séance commence là
    /// (et pourquoi) ; `None` : galerie d'une séance, sans coupures.
    pub(crate) starts: Option<Vec<Option<CutReason>>>,
    /// Places des photos dans la galerie à la dernière image (flèches haut / bas, d'une
    /// ligne à l'autre malgré les bandeaux des séances).
    places: Vec<egui::Pos2>,
    /// Faire défiler jusqu'au curseur (déplacé au clavier).
    follow: bool,
    /// Outil de la galerie de toute la carte.
    pub(crate) tool: Tool,
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
    /// Photos déjà déchargées exclues (retour du 25/11/2026) : décochées et pas
    /// cochables ; décochée, elles se cochent comme les autres (recopiées).
    pub(crate) exclude_known: bool,
    /// « RAW seulement » de la boîte : ce que la copie prendra de chaque photo.
    raw_only: bool,
    /// Cache du moteur (images de la galerie), s'il y en a un.
    gallery: Option<Gallery>,
}

/// Cache et réglages d'aperçu du moteur, où la galerie range ses images (#603).
pub type Gallery = (tri::cache::Cache, tri::preview::Settings);

/// L'image de `file` au niveau `level` : dans le cache du moteur s'il y en a un (lue, ou
/// calculée et rangée : la même carte rouverte va plus vite, et la séance déchargée la
/// retrouve), sinon calculée aux réglages de la galerie.
fn load(
    file: &MediaFile,
    level: Level,
    gallery: Option<&Gallery>,
    stop: &AtomicBool,
) -> Option<tri::bitmap::Bitmap> {
    let Some((cache, s)) = gallery else {
        return tri::preview::compute(file, level, &settings(), stop).ok();
    };
    let key = tri::preview::key(file, level, s).ok();
    if let Some(bitmap) = key.as_ref().and_then(|k| cache.get(k)) {
        return Some(bitmap);
    }
    let bitmap = tri::preview::compute(file, level, s, stop).ok()?;
    if let Some(key) = &key {
        let _ = cache.put(key, &bitmap);
    }
    Some(bitmap)
}

/// Réglages des images de la galerie sans cache : vignettes de 320 px, grand à 1600 px.
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
        exclude_known: bool,
        raw_only: bool,
        gallery: Option<Gallery>,
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
                    photo.copied.push((item.kind, item.known));
                    photo.at = photo.at.min(item.modified);
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
                    at: item.modified,
                    copied: vec![(item.kind, item.known)],
                }),
            }
        }
        let checked = photos
            .iter()
            .map(|p| {
                !(exclude_known && p.copied(raw_only))
                    && chosen.is_none_or(|c| p.files.iter().any(|f| c.contains(f)))
            })
            .collect();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let shown: Vec<MediaFile> = photos.iter().map(|p| p.shown.clone()).collect();
        let (stop, repaint, cached) = (cancel.clone(), ctx.clone(), gallery.clone());
        std::thread::spawn(move || {
            for (i, file) in shown.iter().enumerate() {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(bitmap) = load(file, Level::Thumbnail, cached.as_ref(), &stop)
                    && tx.send((i, to_image(&bitmap))).is_ok()
                {
                    repaint.request_repaint();
                }
            }
        });
        Picker {
            day: day.to_string(),
            title: t!("offload-pick-title", day = day.to_string()),
            starts: None,
            places: Vec::new(),
            follow: false,
            tool: Tool::Select,
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
            exclude_known,
            raw_only,
            gallery,
        }
    }

    /// Galerie de toute la carte `plan` coupée à `cuts` (« Séances… », #600) : chaque
    /// séance commence sur une ligne, sous un bandeau ; on coupe et on recoud ici.
    /// `chosen` : les fichiers déjà choisis, sinon tout est coché.
    pub fn sessions(
        plan: &Plan,
        cuts: &[Cut],
        chosen: Option<&HashSet<PathBuf>>,
        exclude_known: bool,
        raw_only: bool,
        gallery: Option<Gallery>,
        ctx: &egui::Context,
    ) -> Picker {
        let shots = plan.shots();
        let items: Vec<&Item> = shots
            .iter()
            .flat_map(|s| s.items.iter().map(|&i| &plan.items[i]))
            .collect();
        let mut picker = Picker::new(
            "card",
            &items,
            chosen,
            exclude_known,
            raw_only,
            gallery,
            ctx,
        );
        picker.title = t!("offload-cuts-title");
        let mut starts = vec![None; picker.photos.len()];
        for cut in cuts {
            if let Some(i) = picker
                .photos
                .iter()
                .position(|p| p.files.contains(&cut.first))
                && i > 0
            {
                starts[i] = Some(cut.reason);
            }
        }
        picker.starts = Some(starts);
        picker
    }

    /// Commence une séance à la photo `i`, ou recoud les deux séances si elle en
    /// commence déjà une (pas avant la première photo).
    pub(crate) fn toggle_cut(&mut self, i: usize) {
        if let Some(starts) = &mut self.starts
            && i > 0
            && i < starts.len()
        {
            starts[i] = match starts[i] {
                Some(_) => None,
                None => Some(CutReason::Chosen),
            };
        }
    }

    /// Les coupures de la galerie de toute la carte.
    pub(crate) fn cuts(&self) -> Option<Vec<Cut>> {
        let starts = self.starts.as_ref()?;
        Some(
            starts
                .iter()
                .enumerate()
                .filter_map(|(i, reason)| {
                    let photo = &self.photos[i];
                    reason.map(|reason| Cut {
                        first: photo.files[0].clone(),
                        at: photo.at,
                        reason,
                    })
                })
                .collect(),
        )
    }

    /// Vrai si la photo `i` ne peut pas être cochée (déjà déchargée, exclue).
    fn locked(&self, i: usize) -> bool {
        self.exclude_known && self.photos[i].copied(self.raw_only)
    }

    /// Coche ou décoche la photo `i` (une photo exclue reste décochée).
    fn set(&mut self, i: usize, on: bool) {
        self.checked[i] = on && !self.locked(i);
    }

    /// Exclut les photos déjà déchargées (décochées) ou les reprend (cochées).
    fn exclude(&mut self, on: bool) {
        self.exclude_known = on;
        for i in 0..self.photos.len() {
            if self.photos[i].copied(self.raw_only) {
                self.checked[i] = !on;
            }
        }
    }

    /// Coche les photos de `a` à `b` (dans un sens ou l'autre).
    fn check_range(&mut self, a: usize, b: usize) {
        let (lo, hi) = (a.min(b), a.max(b));
        for i in lo..=hi.min(self.photos.len().saturating_sub(1)) {
            self.set(i, true);
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
        let (stop, repaint, cached) = (self.cancel.clone(), ctx.clone(), self.gallery.clone());
        std::thread::spawn(move || {
            let level = match file.kind {
                MediaKind::Raw => Level::Embedded,
                MediaKind::Flat => Level::Standard,
            };
            if let Some(bitmap) = load(&file, level, cached.as_ref(), &stop) {
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
                Key::ArrowUp | Key::K if !command && step > 0 => Some(
                    self.vertical(false)
                        .unwrap_or(self.cursor.saturating_sub(step)),
                ),
                Key::ArrowDown | Key::J if !command && step > 0 => Some(
                    self.vertical(true)
                        .unwrap_or((self.cursor + step).min(last)),
                ),
                _ => None,
            };
            if n == 0 {
                close |= key == Key::Escape;
                continue;
            }
            if let Some(to) = moved {
                self.move_to(to, shift);
                self.follow = true;
                if self.enlarged.is_some() {
                    self.enlarge(self.cursor, ctx);
                }
                continue;
            }
            match key {
                Key::Space if shift => self.toggle_visual(),
                Key::Space => self.set(self.cursor, !self.checked[self.cursor]),
                Key::V if !command => self.toggle_visual(),
                Key::C if !command && self.enlarged.is_none() => self.toggle_cut(self.cursor),
                Key::A if command => {
                    let on = !shift;
                    (0..self.photos.len()).for_each(|i| self.set(i, on));
                }
                Key::Enter => match self.enlarged {
                    Some(_) => self.enlarged = None,
                    None => self.enlarge(self.cursor, ctx),
                },
                Key::Escape if self.tool != Tool::Select => {
                    // Pris ici : la boîte ne se ferme pas avec.
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
                    self.tool = Tool::Select;
                }
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

    /// La photo de la ligne du dessous (`down`) ou du dessus la plus proche du curseur
    /// en largeur, d'après les places de la dernière image (`None` : pas encore dessinée,
    /// ou pas de ligne là).
    fn vertical(&self, down: bool) -> Option<usize> {
        let here = *self.places.get(self.cursor)?;
        let row = self
            .places
            .iter()
            .map(|p| p.y)
            .filter(|&y| if down { y > here.y } else { y < here.y })
            .reduce(|a, b| if down { a.min(b) } else { a.max(b) })?;
        self.places
            .iter()
            .enumerate()
            .filter(|(_, p)| p.y == row)
            .min_by(|(_, a), (_, b)| (a.x - here.x).abs().total_cmp(&(b.x - here.x).abs()))
            .map(|(i, _)| i)
    }

    /// Entre dans le mode visuel (la photo courante cochée) ou en sort.
    fn toggle_visual(&mut self) {
        self.visual = match self.visual {
            Some(_) => None,
            None => {
                self.set(self.cursor, true);
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
            .title(self.title.clone())
            .width(Some(width))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Table de montage (#600) : ciseaux et colle, toujours visibles.
                    if self.starts.is_some() && self.enlarged.is_none() {
                        let size = ui.spacing().interact_size.y + 4.0;
                        for (tool, label, tip) in [
                            (
                                Tool::Scissors,
                                t!("offload-tool-scissors"),
                                t!("offload-tool-scissors-tip"),
                            ),
                            (
                                Tool::Glue,
                                t!("offload-tool-glue"),
                                t!("offload-tool-glue-tip"),
                            ),
                        ] {
                            let on = self.tool == tool;
                            let (rect, response) =
                                crate::marks::icon_toggle(ui, size, &label, &tip, on);
                            let color = if on {
                                accent
                            } else {
                                ui.visuals().text_color()
                            };
                            draw_tool(ui.painter(), rect.shrink(3.0), tool, color);
                            if response.clicked() {
                                self.tool = if on { Tool::Select } else { tool };
                            }
                        }
                        ui.separator();
                    }
                    if ui.button(t!("offload-pick-all")).clicked() {
                        (0..self.photos.len()).for_each(|i| self.set(i, true));
                    }
                    if ui.button(t!("offload-pick-none")).clicked() {
                        self.checked.iter_mut().for_each(|c| *c = false);
                    }
                    let known = self
                        .photos
                        .iter()
                        .filter(|p| p.copied(self.raw_only))
                        .count();
                    if known > 0 {
                        let mut on = self.exclude_known;
                        if ui
                            .checkbox(&mut on, t!("offload-pick-exclude", n = known))
                            .on_hover_text(t!("offload-pick-exclude-tip"))
                            .changed()
                        {
                            self.exclude(on);
                        }
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
            Outcome::Done(self.chosen(), self.exclude_known, self.cuts())
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

    /// Ciseaux ou colle (#600) : entre deux photos d'une même séance, un clic coupe ; sur
    /// le bandeau d'une séance (pas la première), un clic la recoud à la précédente. Alt
    /// donne l'autre outil. Le pointeur devient l'outil, la coupure ou le bandeau visé
    /// s'allume. Renvoie (photo, une séance y commence) pour un clic.
    fn tool_zones(
        &self,
        ui: &mut egui::Ui,
        grid: &Grid,
        origin: egui::Vec2,
        image: egui::Vec2,
        header_h: f32,
        accent: Color32,
    ) -> Option<(usize, bool)> {
        let starts = self.starts.as_deref()?;
        let alt = ui.input(|i| i.modifiers.alt);
        let tool = match (self.tool, alt) {
            (Tool::Scissors, false) | (Tool::Glue, true) => Tool::Scissors,
            _ => Tool::Glue,
        };
        let reach = 12.0;
        let mut clicked = None;
        let stroke = egui::Stroke::new(3.0, accent);
        for (i, &start) in starts.iter().enumerate().skip(1) {
            let zone = match (tool, start) {
                (Tool::Scissors, None) => {
                    let left = grid.cells[i].x + origin.x;
                    let top = grid.cells[i].y + origin.y;
                    egui::Rect::from_min_max(
                        egui::pos2(
                            left - (left - (grid.cells[i - 1].x + origin.x + image.x))
                                .clamp(0.0, 8.0)
                                - reach,
                            top,
                        ),
                        egui::pos2(left + reach, top + image.y),
                    )
                }
                (Tool::Glue, Some(_)) => {
                    let Some(&(_, y)) = grid.headers.iter().find(|(first, _)| *first == i) else {
                        continue;
                    };
                    egui::Rect::from_min_size(
                        egui::pos2(origin.x + grid.left, origin.y + y),
                        vec2(grid.width, header_h),
                    )
                }
                _ => continue,
            };
            let id = ui.scope_id().with(("offload_tool_zone", i));
            let response = ui.interact(zone, id, Sense::click());
            let label = match tool {
                Tool::Scissors => t!("offload-tool-cut-at", name = self.photos[i].name.clone()),
                _ => t!(
                    "offload-tool-join",
                    n = grid.headers.iter().position(|(f, _)| *f == i).unwrap_or(0) + 1
                ),
            };
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
            if response.hovered() {
                match tool {
                    Tool::Scissors => {
                        let x = grid.cells[i].x + origin.x - 4.0;
                        ui.painter().vline(x, zone.y_range().expand(4.0), stroke);
                    }
                    _ => {
                        ui.painter()
                            .rect_stroke(zone, 4.0, stroke, egui::StrokeKind::Inside);
                    }
                }
            }
            if response.clicked() {
                clicked = Some((i, tool == Tool::Scissors));
            }
        }
        // Le pointeur devient l'outil, au-dessus de tout.
        if let Some(pointer) = ui.ctx().pointer_hover_pos()
            && ui.clip_rect().contains(pointer)
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::unique("offload_tool_pointer"),
            ));
            let rect = egui::Rect::from_center_size(pointer, vec2(22.0, 22.0));
            painter.circle_filled(pointer, 13.0, Color32::from_black_alpha(140));
            draw_tool(&painter, rect, tool, Color32::WHITE);
        }
        clicked
    }

    /// Bandeau d'une séance de la galerie de toute la carte : son numéro, ses heures, son
    /// nombre de photos (cochées), et pourquoi elle commence là.
    fn header(&self, first: usize, number: usize) -> String {
        let starts = self.starts.as_deref().unwrap_or_default();
        let end = (first + 1..self.photos.len())
            .find(|&i| starts[i].is_some())
            .unwrap_or(self.photos.len());
        let chosen = self.checked[first..end].iter().filter(|c| **c).count();
        let reason = match starts[first] {
            _ if first == 0 => String::new(),
            Some(CutReason::Day) => t!("offload-cut-day"),
            Some(CutReason::Pause(d)) => {
                let minutes = d.as_secs() / 60;
                t!(
                    "offload-cut-pause",
                    time = format!("{} h {:02}", minutes / 60, minutes % 60)
                )
            }
            Some(CutReason::Chosen) | None => t!("offload-cut-chosen"),
        };
        let day = chrono::DateTime::<chrono::Local>::from(self.photos[first].at)
            .format("%Y-%m-%d")
            .to_string();
        let header = t!(
            "offload-cuts-header",
            n = number,
            day = day,
            start = self.photos[first].time.clone(),
            end = self.photos[end - 1].time.clone(),
            chosen = chosen,
            count = end - first
        );
        if reason.is_empty() {
            header
        } else {
            format!("{header} · {reason}")
        }
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
        let header_h = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
        let grid = layout(
            self.photos.len(),
            self.starts.as_deref(),
            size.x,
            vec2(side, side + label_h),
            gap,
            header_h,
        );
        self.columns = grid.columns;
        let mut open = None;
        let mut cut = None;
        // Ciseaux ou colle : (photo, séance qui y commence ou non).
        let mut edit: Option<(usize, bool)> = None;
        let follow = std::mem::take(&mut self.follow);
        let tooling = self.starts.is_some() && self.tool != Tool::Select;
        egui::ScrollArea::vertical()
            .max_height(size.y)
            .min_scrolled_height(size.y)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (area, _) = ui.allocate_exact_size(vec2(size.x, grid.height), Sense::hover());
                let origin = area.min.to_vec2();
                for (k, &(first, y)) in grid.headers.iter().enumerate() {
                    let band = egui::Rect::from_min_size(
                        egui::pos2(area.left() + grid.left, area.top() + y),
                        vec2(grid.width, header_h),
                    );
                    ui.painter()
                        .rect_filled(band, 4.0, ui.visuals().faint_bg_color);
                    ui.painter().text(
                        band.left_center() + vec2(8.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        self.header(first, k + 1),
                        egui::TextStyle::Body.resolve(ui.style()),
                        ui.visuals().strong_text_color(),
                    );
                }
                self.places.clone_from(&grid.cells);
                for i in 0..self.photos.len() {
                    let cell = egui::Rect::from_min_size(
                        grid.cells[i] + origin,
                        vec2(side, side + label_h),
                    );
                    let id = ui.scope_id().with(("offload_pick_cell", i));
                    let response = ui.interact(cell, id, Sense::click());
                    if follow && i == self.cursor {
                        ui.scroll_to_rect(cell, None);
                    }
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
                    let state = if photo.copied(self.raw_only) {
                        Some(t!("offload-pick-known"))
                    } else if photo.partly(self.raw_only) {
                        Some(t!("offload-pick-partly"))
                    } else {
                        None
                    };
                    let caption = match state {
                        Some(state) => format!("{}  {} · {state}", photo.name, photo.time),
                        None => format!("{}  {}", photo.name, photo.time),
                    };
                    ui.painter().text(
                        egui::pos2(cell.left(), image.bottom() + 2.0),
                        egui::Align2::LEFT_TOP,
                        caption,
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
                    if tooling {
                        continue;
                    }
                    let checked = self.check(ui, i, image, accent);
                    let modifiers = ui.input(|input| input.modifiers);
                    if checked {
                        self.set(i, !self.checked[i]);
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
                            self.set(i, !self.checked[i]);
                            self.cursor = i;
                            self.anchor = i;
                        } else {
                            self.move_to(i, modifiers.shift);
                        }
                    }
                    // Galerie de toute la carte : couper ou recoudre au clic droit.
                    if let Some(starts) = &self.starts
                        && i > 0
                    {
                        let label = if starts[i].is_some() {
                            t!("offload-cut-remove")
                        } else {
                            t!("offload-cut-here")
                        };
                        response.context_menu(|ui| {
                            if ui.button(label).clicked() {
                                cut = Some(i);
                                ui.close();
                            }
                        });
                    }
                }
                if tooling {
                    edit = self.tool_zones(ui, &grid, origin, vec2(side, side), header_h, accent);
                }
            });
        if let Some(i) = cut {
            self.toggle_cut(i);
        }
        if let Some((i, on)) = edit
            && let Some(starts) = &mut self.starts
        {
            starts[i] = on.then_some(CutReason::Chosen);
        }
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
            self.set(i, !self.checked[i]);
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

/// L'icône d'un outil de la table de montage.
fn draw_tool(painter: &egui::Painter, rect: egui::Rect, tool: Tool, color: Color32) {
    match tool {
        Tool::Scissors => crate::marks::scissors(painter, rect, color),
        _ => crate::marks::glue(painter, rect, color),
    }
}

/// Places des vignettes de la galerie (#600, #601) : en lignes centrées dans la largeur,
/// chaque séance (galerie de toute la carte) sur une nouvelle ligne, sous son bandeau.
struct Grid {
    columns: usize,
    /// Marge à gauche des lignes (centrées), et leur largeur.
    left: f32,
    width: f32,
    /// Coin de chaque vignette, depuis le haut à gauche de la galerie.
    cells: Vec<egui::Pos2>,
    /// Bandeaux : (première photo de la séance, hauteur).
    headers: Vec<(usize, f32)>,
    height: f32,
}

fn layout(
    n: usize,
    starts: Option<&[Option<CutReason>]>,
    width: f32,
    cell: egui::Vec2,
    gap: f32,
    header_h: f32,
) -> Grid {
    let columns = (((width + gap) / (cell.x + gap)).floor() as usize).max(1);
    // Moins de photos qu'une ligne : centrées quand même (une seule séance).
    let used = match starts {
        Some(_) => columns,
        None => columns.min(n.max(1)),
    };
    let row_w = used as f32 * cell.x + (used - 1) as f32 * gap;
    let left = ((width - row_w) / 2.0).max(0.0);
    let (mut cells, mut headers) = (Vec::with_capacity(n), Vec::new());
    let (mut y, mut col) = (0.0, 0);
    for i in 0..n {
        let starts_here = starts.is_some_and(|s| i == 0 || s[i].is_some());
        if starts_here {
            if col > 0 {
                y += cell.y + gap;
                col = 0;
            }
            headers.push((i, y));
            y += header_h + gap;
        }
        cells.push(egui::pos2(left + col as f32 * (cell.x + gap), y));
        col += 1;
        if col == columns && i + 1 < n {
            col = 0;
            y += cell.y + gap;
        }
    }
    let height = if n == 0 { 0.0 } else { y + cell.y };
    Grid {
        columns,
        left,
        width: row_w,
        cells,
        headers,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, known: bool) -> Item {
        let kind = if name.ends_with(".RAF") {
            MediaKind::Raw
        } else {
            MediaKind::Flat
        };
        Item {
            source: PathBuf::from(format!("/carte/{name}")),
            size: 10,
            modified: std::time::SystemTime::UNIX_EPOCH,
            kind,
            known,
        }
    }

    #[test]
    fn photos_already_copied_are_left_out_unless_asked() {
        // Retour du 25/11/2026 : la galerie cochait aussi les photos déjà déchargées.
        crate::i18n::ensure();
        let items = [item("A.RAF", true), item("B.RAF", false)];
        let refs: Vec<&Item> = items.iter().collect();
        let ctx = egui::Context::default();
        let mut p = Picker::new("2026-11-25", &refs, None, true, false, None, &ctx);
        assert_eq!(p.checked, [false, true], "déjà copiée : décochée");
        // Exclue : on ne peut pas la cocher, ni seule ni avec toutes.
        p.set(0, true);
        p.check_range(0, 1);
        assert_eq!(p.checked, [false, true]);
        // Plus exclue : cochée, et cochable.
        p.exclude(false);
        assert_eq!(p.checked, [true, true]);
        p.set(0, false);
        p.set(0, true);
        assert!(p.checked[0]);
        // De nouveau exclue : décochée.
        p.exclude(true);
        assert_eq!(p.checked, [false, true]);
        p.cancel.store(true, Ordering::Relaxed);
    }

    #[test]
    fn a_pair_whose_raw_alone_was_copied_depends_on_raw_only() {
        // Retour du 26/11/2026 : 7 RAW copiés seuls (« RAW seulement »), leurs photos
        // restaient cochées sans rien dire. Avec « RAW seulement », elles sont copiées ;
        // sans, il reste leur JPEG : « en partie copiée », cochée.
        crate::i18n::ensure();
        let items = [
            item("A.RAF", true),
            item("A.JPG", false),
            item("B.RAF", false),
        ];
        let refs: Vec<&Item> = items.iter().collect();
        let ctx = egui::Context::default();
        let raw_only = Picker::new("2026-11-25", &refs, None, true, true, None, &ctx);
        assert_eq!(raw_only.checked, [false, true]);
        assert!(raw_only.photos[0].copied(true));
        raw_only.cancel.store(true, Ordering::Relaxed);
        let all = Picker::new("2026-11-25", &refs, None, true, false, None, &ctx);
        assert_eq!(all.checked, [true, true], "son JPEG reste à copier");
        assert!(all.photos[0].partly(false));
        assert!(!all.photos[1].partly(false));
        all.cancel.store(true, Ordering::Relaxed);
    }

    #[test]
    fn the_gallery_is_centred_and_each_session_starts_a_row() {
        // #601 : centrée ; #600 : une séance par ligne, sous son bandeau.
        let cell = vec2(100.0, 120.0);
        let one = layout(2, None, 1000.0, cell, 10.0, 20.0);
        assert_eq!(one.columns, 9);
        assert_eq!(
            one.cells[0].x,
            (1000.0 - 210.0) / 2.0,
            "deux photos, centrées"
        );
        let starts = [None, None, None, Some(CutReason::Chosen), None];
        let grid = layout(5, Some(&starts), 340.0, cell, 10.0, 20.0);
        assert_eq!(grid.columns, 3);
        assert_eq!(grid.headers, [(0, 0.0), (3, 160.0)]);
        let ys: Vec<f32> = grid.cells.iter().map(|c| c.y).collect();
        assert_eq!(ys, [30.0, 30.0, 30.0, 190.0, 190.0]);
        assert_eq!(
            grid.cells[3].x, grid.cells[0].x,
            "la séance repart à gauche"
        );
        assert_eq!(grid.height, 310.0);
    }

    #[test]
    fn cuts_are_placed_and_removed_in_the_whole_card_gallery() {
        use std::time::{Duration, UNIX_EPOCH};
        crate::i18n::ensure();
        let at = |m: u64| UNIX_EPOCH + Duration::from_secs(1_800_000_000 + m * 60);
        let mut items = vec![
            item("B.RAF", false),
            item("A.RAF", false),
            item("A.JPG", false),
            item("C.RAF", false),
        ];
        for (item, m) in items.iter_mut().zip([10, 0, 0, 200]) {
            item.modified = at(m);
        }
        let plan = Plan {
            source: PathBuf::from("/carte"),
            items,
            ignored: 0,
        };
        let cuts = plan.proposed_cuts(&tri::offload::OffloadSettings::default());
        let ctx = egui::Context::default();
        let mut p = Picker::sessions(&plan, &cuts, None, true, false, None, &ctx);
        let names: Vec<&str> = p.photos.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            ["A", "B", "C"],
            "dans l'ordre de prise de vue, paire d'un bloc"
        );
        assert!(matches!(
            p.starts.as_deref(),
            Some([None, None, Some(CutReason::Pause(_))])
        ));
        // Couper avant B, recoudre avant C ; pas de coupure avant la première.
        p.toggle_cut(1);
        p.toggle_cut(2);
        p.toggle_cut(0);
        let cuts = p.cuts().unwrap();
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].first, PathBuf::from("/carte/B.RAF"));
        assert_eq!(cuts[0].reason, CutReason::Chosen);
        let sessions = plan.sessions_at(&cuts, 4);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[1].items, [0, 3]);
        // La galerie d'une séance n'a pas de coupures.
        let refs: Vec<&Item> = plan.items.iter().collect();
        let mut one = Picker::new("2026-11-25", &refs, None, true, false, None, &ctx);
        one.toggle_cut(1);
        assert!(one.cuts().is_none());
        p.cancel.store(true, Ordering::Relaxed);
        one.cancel.store(true, Ordering::Relaxed);
    }

    #[test]
    fn gallery_thumbnails_are_cached_and_follow_the_copy() {
        // Idée de l'utilisateur (26/11/2026) : les vignettes de la galerie servent ensuite
        // à la séance déchargée, et la même carte rouverte va plus vite.
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("carte");
        std::fs::create_dir_all(&card).unwrap();
        let photo = card.join("IMG0.jpg");
        image::RgbImage::from_pixel(64, 48, image::Rgb([90, 120, 150]))
            .save(&photo)
            .unwrap();
        let cache = tri::cache::Cache::open(dir.path().join("cache")).unwrap();
        let preview = tri::preview::Settings::default();
        let gallery = (cache.clone(), preview);
        let file = |path: &std::path::Path| MediaFile {
            path: path.to_path_buf(),
            kind: MediaKind::Flat,
        };
        let stop = AtomicBool::new(false);
        assert!(load(&file(&photo), Level::Thumbnail, Some(&gallery), &stop).is_some());
        let on_card = tri::preview::key(&file(&photo), Level::Thumbnail, &preview).unwrap();
        assert!(cache.contains(&on_card), "rangée dans le cache du moteur");
        // Le déchargement fait suivre l'entrée, comme la boîte.
        let history = tri::offload::History::in_memory();
        let plan = tri::offload::plan(&card, &history).unwrap();
        let dest = dir.path().join("seance");
        tri::offload::run(
            &plan,
            &dest,
            &tri::offload::OffloadSettings::default(),
            &mut tri::offload::History::in_memory(),
            &stop,
            |_| {},
            |source, copy| {
                tri::preview::carry(
                    &cache,
                    &preview,
                    &[(source.to_path_buf(), copy.to_path_buf())],
                    true,
                );
            },
            |_| false,
        );
        let copied =
            tri::preview::key(&file(&dest.join("IMG0.jpg")), Level::Thumbnail, &preview).unwrap();
        assert!(cache.contains(&copied), "la séance la retrouve");
        assert!(cache.contains(&on_card), "la carte la garde");
    }
}

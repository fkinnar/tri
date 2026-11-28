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

use crate::actions::{Action, Keymap};
use eframe::egui::{self, Color32, ColorImage, Sense, TextureHandle, vec2};
use tri::cache::Level;
use tri::media::{MediaFile, MediaKind};
use tri::offload::{CutReason, Item, Plan, Session};

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

/// Une séance rendue par la galerie de toute la carte : les fichiers de ses photos, et
/// pourquoi elle commence là ([`Plan::sessions_of`]).
pub type Part = (Vec<PathBuf>, Option<CutReason>);

/// Les séances de la galerie de toute la carte (#600) : celle de chaque photo, et
/// pourquoi chacune commence. Une séance n'est pas forcément d'un seul tenant (photos
/// regroupées à la main) ; la galerie les montre l'une après l'autre, chacune à l'heure de
/// sa première photo.
#[derive(Clone, Debug)]
pub(crate) struct Split {
    of: Vec<usize>,
    reasons: Vec<Option<CutReason>>,
}

/// Ce que devient la boîte.
pub enum Outcome {
    /// Encore ouverte.
    Open,
    /// Validée : les fichiers des photos cochées, si les photos déjà déchargées sont
    /// exclues (la case de la galerie, celle de la boîte de déchargement), et les séances
    /// (galerie de toute la carte, #600 : pour [`Plan::sessions_of`]).
    Done(HashSet<PathBuf>, bool, Option<Vec<Part>>),
    /// Fermée sans rien changer.
    Cancelled,
}

/// La galerie d'une séance, ou de toute la carte coupée en séances (#600).
pub struct Picker {
    day: String,
    /// Titre de la boîte.
    title: String,
    /// Galerie de toute la carte (#600) : ses séances ; `None` : galerie d'une séance,
    /// sans coupures.
    pub(crate) split: Option<Split>,
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
    pub(crate) side: f32,
    /// Photo montrée en grand.
    pub(crate) enlarged: Option<usize>,
    thumbs: Vec<Option<TextureHandle>>,
    rx: Receiver<(usize, ColorImage)>,
    big: Option<(usize, Option<TextureHandle>)>,
    big_rx: Option<Receiver<(usize, ColorImage)>>,
    cancel: Arc<AtomicBool>,
    /// Colonnes de la dernière image dessinée (flèches haut / bas).
    pub(crate) columns: usize,
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
    /// Raccourcis de l'application (`keys.toml`), dans le contexte de la galerie.
    keymap: Keymap,
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
            split: None,
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
            keymap: gallery_keys(Keymap::from_profiles(&[crate::actions::Profile::Classic])),
        }
    }

    /// Avec les raccourcis de l'application (`keys.toml`, `[keys]`).
    pub fn with_keys(mut self, keymap: Keymap) -> Picker {
        self.keymap = gallery_keys(keymap);
        self
    }

    /// Galerie de toute la carte `plan` rangée en `sessions` (« Séances… », #600) : chaque
    /// séance commence sur une ligne, sous un bandeau ; on coupe, recoud et regroupe ici.
    /// `chosen` : les fichiers déjà choisis, sinon tout est coché.
    pub fn sessions(
        plan: &Plan,
        sessions: &[Session],
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
        let session_of: std::collections::HashMap<&PathBuf, usize> = sessions
            .iter()
            .enumerate()
            .flat_map(|(k, s)| s.items.iter().map(move |&i| (&plan.items[i].source, k)))
            .collect();
        let mut reasons: Vec<Option<CutReason>> = sessions.iter().map(|s| s.reason).collect();
        let of = picker
            .photos
            .iter()
            .map(|p| {
                p.files
                    .iter()
                    .find_map(|f| session_of.get(f).copied())
                    .unwrap_or_else(|| {
                        // Absente des séances (ne devrait pas arriver) : la sienne.
                        reasons.push(Some(CutReason::Chosen));
                        reasons.len() - 1
                    })
            })
            .collect();
        picker.split = Some(Split { of, reasons });
        picker
    }

    /// L'ordre des photos dans la galerie : séance après séance (chacune à sa première
    /// photo), dans l'ordre de prise de vue dans chacune.
    pub(crate) fn order(&self) -> Vec<usize> {
        let n = self.photos.len();
        let mut order: Vec<usize> = (0..n).collect();
        if let Some(split) = &self.split {
            let mut first = vec![usize::MAX; split.reasons.len()];
            for (i, &id) in split.of.iter().enumerate() {
                first[id] = first[id].min(i);
            }
            order.sort_by_key(|&i| (first[split.of[i]], i));
        }
        order
    }

    /// Pour chaque place de `order`, si une séance y commence (galerie de toute la carte).
    fn begins(&self, order: &[usize]) -> Option<Vec<bool>> {
        let split = self.split.as_ref()?;
        Some(
            (0..order.len())
                .map(|p| p == 0 || split.of[order[p]] != split.of[order[p - 1]])
                .collect(),
        )
    }

    /// Pour chaque photo, si une séance y commence (sauf la première de la galerie) et
    /// pourquoi.
    #[cfg(test)]
    pub(crate) fn starts(&self) -> Option<Vec<Option<CutReason>>> {
        let split = self.split.as_ref()?;
        let order = self.order();
        let begins = self.begins(&order)?;
        let mut starts = vec![None; order.len()];
        for (p, &i) in order.iter().enumerate().skip(1) {
            if begins[p] {
                starts[i] = Some(split.reasons[split.of[i]].unwrap_or(CutReason::Chosen));
            }
        }
        Some(starts)
    }

    /// Commence une séance à la photo `i` (avec les photos de sa séance prises après
    /// elle), ou, si elle en commence déjà une, recoud cette séance à celle d'avant dans
    /// la galerie.
    pub(crate) fn toggle_cut(&mut self, i: usize) {
        let order = self.order();
        let Some(split) = &mut self.split else {
            return;
        };
        let Some(p) = order.iter().position(|&k| k == i) else {
            return;
        };
        let id = split.of[i];
        let first = (0..split.of.len()).find(|&k| split.of[k] == id);
        if first == Some(i) {
            if p == 0 {
                return;
            }
            let before = split.of[order[p - 1]];
            split
                .of
                .iter_mut()
                .filter(|k| **k == id)
                .for_each(|k| *k = before);
        } else {
            split.reasons.push(Some(CutReason::Chosen));
            let new = split.reasons.len() - 1;
            for k in i..split.of.len() {
                if split.of[k] == id {
                    split.of[k] = new;
                }
            }
        }
    }

    /// Les photos cochées forment une nouvelle séance (#600 : les portraits de deux
    /// personnes pris en alternance) ; puis toutes repartent cochées (les coches ont servi
    /// à choisir). Rien si aucune ou toutes sont cochées.
    pub(crate) fn group_checked(&mut self) {
        let n = self.photos.len();
        let chosen = self.checked.iter().filter(|c| **c).count();
        let Some(split) = &mut self.split else {
            return;
        };
        if chosen == 0 || chosen == n {
            return;
        }
        split.reasons.push(Some(CutReason::Grouped));
        let new = split.reasons.len() - 1;
        for (k, on) in self.checked.iter().enumerate() {
            if *on {
                split.of[k] = new;
            }
        }
        (0..n).for_each(|k| self.set(k, true));
    }

    /// Les séances de la galerie de toute la carte, pour [`Plan::sessions_of`].
    pub(crate) fn parts(&self) -> Option<Vec<Part>> {
        let split = self.split.as_ref()?;
        let order = self.order();
        let begins = self.begins(&order)?;
        let mut parts: Vec<Part> = Vec::new();
        for (p, &i) in order.iter().enumerate() {
            if begins[p] {
                parts.push((Vec::new(), split.reasons[split.of[i]]));
            }
            if let Some(part) = parts.last_mut() {
                part.0.extend(self.photos[i].files.iter().cloned());
            }
        }
        Some(parts)
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

    /// Coche les photos de `a` à `b` (dans un sens ou l'autre, dans l'ordre de la
    /// galerie).
    fn check_range(&mut self, a: usize, b: usize) {
        let order = self.order();
        let place = |i: usize| order.iter().position(|&k| k == i).unwrap_or(i);
        let (pa, pb) = (place(a), place(b));
        let (lo, hi) = (pa.min(pb), pa.max(pb));
        for &i in order.iter().take(hi + 1).skip(lo) {
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

    /// Un clic sur la photo `i`, comme dans un gestionnaire de fichiers (retour du
    /// 28/11/2026 : Maj+clic ne se voyait pas, tout part coché) : seul, elle seule
    /// cochée ; Cmd/Ctrl+clic, elle basculée ; Maj+clic, la plage depuis le dernier clic
    /// seule cochée ; Cmd/Ctrl+Maj+clic, la plage ajoutée. En mode visuel, le clic
    /// étend la plage du visuel.
    fn click(&mut self, i: usize, modifiers: egui::Modifiers) {
        let add = modifiers.command || modifiers.ctrl;
        if self.visual.is_some() {
            self.move_to(i, false);
            return;
        }
        if modifiers.shift {
            if !add {
                (0..self.photos.len()).for_each(|k| self.set(k, false));
            }
            self.check_range(self.anchor, i);
            self.cursor = i;
            return;
        }
        if add {
            self.set(i, !self.checked[i]);
        } else {
            (0..self.photos.len()).for_each(|k| self.set(k, false));
            self.set(i, true);
        }
        self.cursor = i;
        self.anchor = i;
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

    /// Clavier, par les actions de `keys.toml` comme la grille (jamais de touche en dur) :
    /// images précédente / suivante / dessus / dessous, première / dernière ; avec Maj, la
    /// plage parcourue est cochée ; cocher (`core.select.toggle`), mode visuel
    /// (`core.select.visual`), tout cocher / décocher, agrandir (`core.view.open`),
    /// couper (`core.offload.cut`) ; retour (`core.view.back`) : sort de l'outil, du
    /// visuel, de la vue en grand, puis ferme (vrai : fermer sans rien changer).
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
            let lookup = |m: egui::Modifiers| {
                crate::actions::Chord::from_event(key, m).and_then(|c| self.keymap.chord_action(c))
            };
            let nav = |a: Option<Action>| {
                a.filter(|a| {
                    matches!(
                        a,
                        Action::NavPrev
                            | Action::NavNext
                            | Action::NavUp
                            | Action::NavDown
                            | Action::NavFirst
                            | Action::NavLast
                    )
                })
            };
            // Maj + un déplacement : il coche la plage (Maj+flèche a un autre sens ailleurs).
            let unshifted = egui::Modifiers {
                shift: false,
                ..modifiers
            };
            let (action, extend) = match nav(lookup(unshifted)) {
                Some(a) if modifiers.shift && nav(lookup(modifiers)).is_none() => (Some(a), true),
                _ => (lookup(modifiers), false),
            };
            let Some(action) = action else {
                continue;
            };
            if n == 0 {
                close |= action == Action::Back;
                continue;
            }
            let step = if self.enlarged.is_some() {
                0
            } else {
                self.columns.max(1)
            };
            let last = n - 1;
            // Gauche / droite dans l'ordre de la galerie (séances regroupées).
            let order = self.order();
            let here = order.iter().position(|&k| k == self.cursor).unwrap_or(0);
            let moved = match action {
                Action::NavPrev => order.get(here.saturating_sub(1)).copied(),
                Action::NavNext => order.get((here + 1).min(last)).copied(),
                Action::NavFirst => order.first().copied(),
                Action::NavLast => order.last().copied(),
                Action::NavUp if step > 0 => Some(
                    self.vertical(false)
                        .unwrap_or(self.cursor.saturating_sub(step)),
                ),
                Action::NavDown if step > 0 => Some(
                    self.vertical(true)
                        .unwrap_or((self.cursor + step).min(last)),
                ),
                _ => None,
            };
            if let Some(to) = moved {
                self.move_to(to, extend);
                self.follow = true;
                if self.enlarged.is_some() {
                    self.enlarge(self.cursor, ctx);
                }
                continue;
            }
            // Pris ici : la boîte ne se ferme pas avec.
            let consume = |ctx: &egui::Context| {
                ctx.input_mut(|i| i.consume_key(modifiers, key));
            };
            match action {
                Action::SelectVisual => self.toggle_visual(),
                Action::SelectToggle => self.set(self.cursor, !self.checked[self.cursor]),
                Action::OffloadCut if self.enlarged.is_none() => self.toggle_cut(self.cursor),
                Action::SelectAll => (0..n).for_each(|i| self.set(i, true)),
                Action::SelectClear => (0..n).for_each(|i| self.set(i, false)),
                Action::Open => match self.enlarged {
                    Some(_) => self.enlarged = None,
                    None => self.enlarge(self.cursor, ctx),
                },
                Action::Back if self.tool != Tool::Select => {
                    consume(ctx);
                    self.tool = Tool::Select;
                }
                Action::Back => {
                    if self.visual.is_some() || self.enlarged.is_some() {
                        consume(ctx);
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
                    if self.split.is_some() && self.enlarged.is_none() {
                        let size = ui.spacing().interact_size.y + 4.0;
                        for (tool, label, tip) in [
                            (
                                Tool::Scissors,
                                t!("offload-tool-scissors"),
                                t!("offload-tool-scissors-tip", key = inverse_key()),
                            ),
                            (
                                Tool::Glue,
                                t!("offload-tool-glue"),
                                t!("offload-tool-glue-tip", key = inverse_key()),
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
                        // Les photos cochées en une séance à part (#600), toujours à sa
                        // place, grisé quand il n'y a rien à grouper.
                        let chosen = self.checked.iter().filter(|c| **c).count();
                        if ui
                            .add_enabled(
                                chosen > 0 && chosen < self.photos.len(),
                                egui::Button::new(t!("offload-group")),
                            )
                            .on_hover_text(t!("offload-group-tip"))
                            .on_disabled_hover_text(t!("offload-group-tip"))
                            .clicked()
                        {
                            self.group_checked();
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
            Outcome::Done(self.chosen(), self.exclude_known, self.parts())
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
    /// le bandeau d'une séance (pas la première), un clic la recoud à la précédente. Alt ou
    /// Ctrl (Cmd sur macOS) donne l'autre outil. Le pointeur devient l'outil, la coupure ou le bandeau visé
    /// s'allume. Renvoie (photo, une séance y commence) pour un clic.
    #[allow(clippy::too_many_arguments)]
    fn tool_zones(
        &self,
        ui: &mut egui::Ui,
        grid: &Grid,
        order: &[usize],
        origin: egui::Vec2,
        image: egui::Vec2,
        header_h: f32,
        accent: Color32,
    ) -> Option<usize> {
        let begins = self.begins(order)?;
        // Alt, ou Ctrl (Cmd sur macOS) : sous Linux Mint (Cinnamon), Alt+clic déplace la
        // fenêtre et n'arrive jamais à `tri` (retour du 26/11/2026).
        let alt = ui.input(|i| i.modifiers.alt || i.modifiers.command || i.modifiers.ctrl);
        let tool = match (self.tool, alt) {
            (Tool::Scissors, false) | (Tool::Glue, true) => Tool::Scissors,
            _ => Tool::Glue,
        };
        let reach = 12.0;
        let mut clicked = None;
        let stroke = egui::Stroke::new(3.0, accent);
        // Zones : (photo, zone, trait de la coupure visée (x), libellé).
        let mut zones: Vec<(usize, egui::Rect, Option<f32>, String)> = Vec::new();
        for (p, &begin) in begins.iter().enumerate().skip(1) {
            let i = order[p];
            match (tool, begin) {
                (Tool::Scissors, false) => {
                    let (here, before) = (grid.cells[p] + origin, grid.cells[p - 1] + origin);
                    let name = self.photos[i].name.clone();
                    // Devant la photo (l'écart avec la précédente, sur la même ligne).
                    let gap = (here.x - (before.x + image.x)).clamp(0.0, 8.0);
                    zones.push((
                        i,
                        egui::Rect::from_min_max(
                            egui::pos2(here.x - gap - reach, here.y),
                            egui::pos2(here.x + reach, here.y + image.y),
                        ),
                        Some(here.x - gap / 2.0),
                        t!("offload-tool-cut-at", name = name),
                    ));
                    // En tête de ligne : aussi après la dernière photo de la ligne d'avant
                    // (retour du 26/11/2026).
                    if here.y != before.y {
                        let right = before.x + image.x;
                        zones.push((
                            i,
                            egui::Rect::from_min_max(
                                egui::pos2(right - reach, before.y),
                                egui::pos2(right + 8.0 + reach, before.y + image.y),
                            ),
                            Some(right + 4.0),
                            t!(
                                "offload-tool-cut-after",
                                name = self.photos[order[p - 1]].name.clone()
                            ),
                        ));
                    }
                }
                (Tool::Glue, true) => {
                    let Some(k) = grid.headers.iter().position(|(first, _)| *first == p) else {
                        continue;
                    };
                    zones.push((
                        i,
                        egui::Rect::from_min_size(
                            egui::pos2(origin.x + grid.left, origin.y + grid.headers[k].1),
                            vec2(grid.width, header_h),
                        ),
                        None,
                        t!("offload-tool-join", n = k + 1),
                    ));
                }
                _ => {}
            }
        }
        for (n, (i, zone, line, label)) in zones.into_iter().enumerate() {
            let id = ui.scope_id().with(("offload_tool_zone", n));
            let response = ui.interact(zone, id, Sense::click());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
            if response.hovered() {
                match line {
                    Some(x) => {
                        ui.painter().vline(x, zone.y_range().expand(4.0), stroke);
                    }
                    None => {
                        ui.painter()
                            .rect_stroke(zone, 4.0, stroke, egui::StrokeKind::Inside);
                    }
                }
            }
            if response.clicked() {
                clicked = Some(i);
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
    /// nombre de photos (cochées), et pourquoi elle commence là. `first` : sa place dans
    /// `order`.
    fn header(&self, order: &[usize], begins: &[bool], first: usize, number: usize) -> String {
        let end = (first + 1..order.len())
            .find(|&p| begins[p])
            .unwrap_or(order.len());
        let photos = &order[first..end];
        let chosen = photos.iter().filter(|&&i| self.checked[i]).count();
        let reason = self
            .split
            .as_ref()
            .and_then(|s| s.reasons[s.of[order[first]]]);
        let reason = match reason {
            _ if first == 0 => String::new(),
            Some(CutReason::Day) => t!("offload-cut-day"),
            Some(CutReason::Pause(d)) => {
                let minutes = d.as_secs() / 60;
                t!(
                    "offload-cut-pause",
                    time = format!("{} h {:02}", minutes / 60, minutes % 60)
                )
            }
            Some(CutReason::Grouped) => t!("offload-cut-grouped"),
            Some(CutReason::Chosen) | None => t!("offload-cut-chosen"),
        };
        let (a, b) = (
            &self.photos[photos[0]],
            &self.photos[photos[photos.len() - 1]],
        );
        let day = chrono::DateTime::<chrono::Local>::from(a.at)
            .format("%Y-%m-%d")
            .to_string();
        let header = t!(
            "offload-cuts-header",
            n = number,
            day = day,
            start = a.time.clone(),
            end = b.time.clone(),
            chosen = chosen,
            count = photos.len()
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
        let order = self.order();
        let begins = self.begins(&order);
        let grid = layout(
            self.photos.len(),
            begins.as_deref(),
            size.x,
            vec2(side, side + label_h),
            gap,
            header_h,
        );
        self.columns = grid.columns;
        let mut open = None;
        let mut cut = None;
        // Ciseaux ou colle : la photo où couper ou recoudre.
        let mut edit: Option<usize> = None;
        let follow = std::mem::take(&mut self.follow);
        let tooling = self.split.is_some() && self.tool != Tool::Select;
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
                        self.header(&order, begins.as_deref().unwrap_or_default(), first, k + 1),
                        egui::TextStyle::Body.resolve(ui.style()),
                        ui.visuals().strong_text_color(),
                    );
                }
                // Places par photo (flèches haut / bas).
                self.places = vec![egui::Pos2::ZERO; order.len()];
                for (p, &i) in order.iter().enumerate() {
                    self.places[i] = grid.cells[p];
                }
                for (p, &i) in order.iter().enumerate() {
                    let cell = egui::Rect::from_min_size(
                        grid.cells[p] + origin,
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
                        self.click(i, modifiers);
                    }
                    // Galerie de toute la carte : couper ou recoudre au clic droit.
                    if let Some(begins) = &begins
                        && p > 0
                    {
                        let label = if begins[p] {
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
                    edit = self.tool_zones(
                        ui,
                        &grid,
                        &order,
                        origin,
                        vec2(side, side),
                        header_h,
                        accent,
                    );
                }
            });
        if let Some(i) = cut {
            self.toggle_cut(i);
        }
        if let Some(i) = edit {
            self.toggle_cut(i);
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

/// La table des raccourcis dans le contexte de la galerie : ses touches propres
/// (`core.offload.cut`) l'emportent.
fn gallery_keys(mut keymap: Keymap) -> Keymap {
    keymap.set_focus(crate::actions::Scope::Offload);
    keymap
}

/// Les touches qui donnent l'autre outil, pour les bulles d'aide.
fn inverse_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "Alt / Cmd"
    } else {
        "Alt / Ctrl"
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
    pub(crate) columns: usize,
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
    starts: Option<&[bool]>,
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
        let starts_here = starts.is_some_and(|s| i == 0 || s[i]);
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
        let starts = [true, false, false, true, false];
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
        let sessions = plan.sessions_at(&cuts, 4);
        let mut p = Picker::sessions(&plan, &sessions, None, true, false, None, &ctx);
        let names: Vec<&str> = p.photos.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            ["A", "B", "C"],
            "dans l'ordre de prise de vue, paire d'un bloc"
        );
        assert!(matches!(
            p.starts().as_deref(),
            Some([None, None, Some(CutReason::Pause(_))])
        ));
        // Couper avant B, recoudre avant C ; pas de coupure avant la première.
        p.toggle_cut(1);
        p.toggle_cut(2);
        p.toggle_cut(0);
        assert_eq!(p.starts(), Some(vec![None, Some(CutReason::Chosen), None]));
        let sessions = plan.sessions_of(&p.parts().unwrap(), 4);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[1].items, [0, 3]);
        assert_eq!(sessions[1].reason, Some(CutReason::Chosen));
        // Regrouper A et C (qui ne se suivent pas) : une séance à part, placée à A ; B
        // reste seule. Puis tout repart coché.
        p.checked = vec![true, false, true];
        p.group_checked();
        assert_eq!(p.checked, [true, true, true]);
        assert_eq!(p.order(), [0, 2, 1], "A et C d'abord, à l'heure de A");
        let sessions = plan.sessions_of(&p.parts().unwrap(), 4);
        let items: Vec<&[usize]> = sessions.iter().map(|s| s.items.as_slice()).collect();
        assert_eq!(items, [&[1, 2, 3][..], &[0]]);
        // Couper dans la séance regroupée (avant C) : C seule, à son heure, après B ; la
        // recoudre la joint à la séance d'avant dans la galerie (B).
        p.toggle_cut(2);
        assert_eq!(p.order(), [0, 1, 2]);
        assert_eq!(plan.sessions_of(&p.parts().unwrap(), 4).len(), 3);
        p.toggle_cut(2);
        let sessions = plan.sessions_of(&p.parts().unwrap(), 4);
        let items: Vec<&[usize]> = sessions.iter().map(|s| s.items.as_slice()).collect();
        assert_eq!(items, [&[1, 2][..], &[0, 3]]);
        // La galerie d'une séance n'a pas de coupures.
        let refs: Vec<&Item> = plan.items.iter().collect();
        let mut one = Picker::new("2026-11-25", &refs, None, true, false, None, &ctx);
        one.toggle_cut(1);
        assert!(one.parts().is_none());
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

    #[test]
    fn the_gallery_keys_come_from_the_keymap() {
        // Retour du 26/11/2026 : aucun raccourci en dur. `core.offload.cut` changé dans
        // `[keys]` : la nouvelle touche coupe, l'ancienne plus.
        use std::time::{Duration, UNIX_EPOCH};
        crate::i18n::ensure();
        let mut items = vec![item("A.RAF", false), item("B.RAF", false)];
        for (k, item) in items.iter_mut().enumerate() {
            item.modified = UNIX_EPOCH + Duration::from_secs(1_800_000_000 + k as u64 * 60);
        }
        let plan = Plan {
            source: PathBuf::from("/carte"),
            items,
            ignored: 0,
        };
        let ctx = egui::Context::default();
        let sessions = plan.sessions_at(&[], 4);
        let overrides = std::collections::BTreeMap::from([(
            "core.offload.cut".to_string(),
            vec!["x".to_string()],
        )]);
        let (keys, warnings) =
            Keymap::from_profiles(&[crate::actions::Profile::Classic]).with_overrides(&overrides);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut p =
            Picker::sessions(&plan, &sessions, None, true, false, None, &ctx).with_keys(keys);
        p.cursor = 1;
        let press = |p: &mut Picker, key: egui::Key| {
            let input = egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            };
            ctx.begin_pass(input);
            let close = p.keys(&ctx);
            let mut output = ctx.end_pass();
            output.textures_delta.clear();
            close
        };
        press(&mut p, egui::Key::C);
        assert_eq!(p.starts(), Some(vec![None, None]), "c ne coupe plus");
        press(&mut p, egui::Key::X);
        assert_eq!(p.starts(), Some(vec![None, Some(CutReason::Chosen)]));
        // Espace (core.select.toggle) décoche, Échap (core.view.back) ferme.
        press(&mut p, egui::Key::Space);
        assert_eq!(p.checked, [true, false]);
        assert!(press(&mut p, egui::Key::Escape));
        p.cancel.store(true, Ordering::Relaxed);
    }

    #[test]
    fn clicks_in_the_gallery_select_like_a_file_manager() {
        // Retour du 28/11/2026 : Ctrl+clic et Maj+clic « ne marchaient pas » (tout part
        // coché : Maj+clic ne changeait rien). Clic : elle seule ; Cmd/Ctrl+clic : elle
        // en plus ; Maj+clic : la plage seule ; Cmd/Ctrl+Maj+clic : la plage en plus.
        use egui::Modifiers;
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;
        crate::i18n::ensure();
        let items: Vec<Item> = (0..6)
            .map(|k| item(&format!("IMG{k}.RAF"), false))
            .collect();
        let refs: Vec<&Item> = items.iter().collect();
        let ctx = egui::Context::default();
        let picker = Picker::new("2026-11-28", &refs, None, true, false, None, &ctx);
        let mut h = Harness::builder()
            .with_size(vec2(1400.0, 1000.0))
            .build_ui_state(
                |ui, p: &mut Picker| {
                    p.show(ui.ctx(), Color32::BLUE);
                },
                picker,
            );
        h.run_steps(3);
        // Au milieu de la vignette, pas sur sa coche (en haut à gauche).
        let click = |h: &mut Harness<'_, Picker>, k: usize, m: Modifiers| {
            let check = h.get_by_label(&format!("IMG{k}")).rect();
            let pos = check.center() + vec2(40.0, 40.0);
            h.event(egui::Event::ModifiersChanged(m));
            h.event(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                h.event(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: m,
                });
            }
            h.run_steps(2);
            h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
            h.run_steps(1);
            h.state().checked.clone()
        };
        let (o, x) = (true, false);
        assert_eq!(h.state().checked, [o; 6], "toutes cochées au départ");
        assert_eq!(click(&mut h, 1, Modifiers::NONE), [x, o, x, x, x, x]);
        assert_eq!(click(&mut h, 3, Modifiers::COMMAND), [x, o, x, o, x, x]);
        assert_eq!(click(&mut h, 1, Modifiers::CTRL), [x, x, x, o, x, x]);
        // Plage depuis le dernier clic (la 2e photo).
        assert_eq!(click(&mut h, 4, Modifiers::SHIFT), [x, o, o, o, o, x]);
        assert_eq!(click(&mut h, 2, Modifiers::SHIFT), [x, o, o, x, x, x]);
        assert_eq!(click(&mut h, 5, Modifiers::COMMAND), [x, o, o, x, x, o]);
        assert_eq!(
            click(&mut h, 3, Modifiers::COMMAND | Modifiers::SHIFT),
            [x, o, o, o, o, o],
            "la plage de la 6e à la 4e ajoutée"
        );
        assert_eq!(h.state().cursor, 3);
        h.state().cancel.store(true, Ordering::Relaxed);
    }
}

//! Palettes de couleurs (#492, idée de l'utilisateur, 15/11/2026 : construire des
//! moodboards à partir de ses photos et d'images de référence).
//!
//! Chaque image reçoit une **palette** ([`Palette`]) : ses couleurs principales, de 5 à 8,
//! chacune avec sa **part** de l'image. Calculée une fois sur une petite version de
//! l'image, gardée dans le cache d'analyse (donnée dérivée, pas une note de
//! l'utilisateur : rien dans le XMP), comme les empreintes de [`crate::similarity`].
//!
//! - **Extraction** ([`extract`]) : k-moyennes dans l'espace **OKLab** (perceptuel : des
//!   écarts égaux s'y voient à peu près égaux), initialisées sans hasard (même image, même
//!   palette), couleurs trop proches fusionnées, miettes retirées.
//! - **Distance** entre deux palettes ([`distance`]) : le coût de transport des parts de
//!   l'une vers l'autre (« Earth Mover's Distance »), approché en servant d'abord les
//!   couples les plus proches ; instantané sur des milliers de palettes.
//! - **Part d'une couleur** ([`Palette::share_near`]) : « contient assez de ce bleu ».
//!
//! Trois **sources** sont prévues (décision de l'utilisateur, 21/11/2026) : le JPEG
//! boîtier, les données brutes de LibRaw et les rendus finis ; le choix de la source et le
//! cache viennent ensuite, ce module ne voit qu'une image.
//!
//! Les images arrivent dans l'espace de travail de `tri` ([`crate::colour`] : primaires
//! Rec.2020, courbe sRGB) ; OKLab est calculé depuis la lumière linéaire.

use serde::{Deserialize, Serialize};

use crate::bitmap::Bitmap;
use crate::colour;

/// Version du calcul (dans la clé du cache : la changer recalcule tout).
pub const VERSION: u32 = 2;

/// Source d'une palette (décision de l'utilisateur, 21/11/2026) : trois usages, une
/// palette par image et par source dans le cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// JPEG boîtier (embarqué dans le RAW, ou le JPEG lui-même) : la vignette.
    Camera,
    /// Données brutes de LibRaw (rendu RAW de `tri`, sans traitement) ; une image plate :
    /// l'image elle-même, comme [`Source::Camera`].
    Raw,
    /// Rendu fini d'un logiciel de développement (un développement de la liste).
    Developed(crate::develop::Engine),
}

/// Source choisie dans l'interface ou en ligne de commande (sur le modèle du rendu
/// affiché, retour du 21/11/2026) : JPEG boîtier, un logiciel de développement (tous ses
/// développements, clones compris), ou les données brutes. Dans un fichier :
/// `"camera"`, `"raw"`, ou l'identifiant du logiciel (`"darktable"`…).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum SourceKind {
    /// JPEG boîtier.
    #[default]
    Camera,
    /// Données brutes de LibRaw.
    Raw,
    /// Les développements de ce logiciel.
    Developed(crate::develop::Engine),
    /// Comme la couverture de la grille : le rendu de chaque développement, et pour une
    /// photo sans développement, son JPEG boîtier.
    Cover,
}

impl SourceKind {
    /// Tous, dans l'ordre de l'interface (comme le rendu affiché : JPEG boîtier, les
    /// logiciels, RAW).
    pub fn all() -> Vec<SourceKind> {
        std::iter::once(SourceKind::Camera)
            .chain(std::iter::once(SourceKind::Cover))
            .chain(crate::develop::Engine::ALL.map(SourceKind::Developed))
            .chain(std::iter::once(SourceKind::Raw))
            .collect()
    }

    /// Nom dans la ligne de commande et le fichier de configuration.
    pub fn id(self) -> &'static str {
        match self {
            SourceKind::Camera => "camera",
            SourceKind::Raw => "raw",
            SourceKind::Cover => "cover",
            SourceKind::Developed(engine) => engine.id(),
        }
    }

    /// Lit [`SourceKind::id`] ; `"developed"` (avant le choix par logiciel) : darktable.
    pub fn parse(text: &str) -> Option<SourceKind> {
        if text == "developed" {
            return Some(SourceKind::Developed(crate::develop::Engine::Darktable));
        }
        Self::all().into_iter().find(|k| k.id() == text)
    }

    /// Source de la palette de `file` pour ce choix : une image plate n'a pas de données
    /// brutes (son image) ; un logiciel : seulement ses développements (`None` sinon).
    pub fn source_for(self, file: &crate::media::MediaFile) -> Option<Source> {
        match self {
            SourceKind::Camera => Some(Source::Camera),
            SourceKind::Raw if file.kind == crate::media::MediaKind::Raw => Some(Source::Raw),
            SourceKind::Raw => Some(Source::Camera),
            SourceKind::Developed(engine) => crate::developments::get(&file.path)
                .filter(|d| d.engine == engine)
                .map(|_| Source::Developed(engine)),
            // Un développement : son rendu ; une photo qui en a : rien (ses développements
            // portent ses palettes) ; sinon son JPEG boîtier.
            SourceKind::Cover => match crate::developments::get(&file.path) {
                Some(d) => Some(Source::Developed(d.engine)),
                None if crate::developments::of(&file.path).is_empty() => Some(Source::Camera),
                None => None,
            },
        }
    }
}

/// Écrit (état d'affichage) par son identifiant.
impl From<SourceKind> for String {
    fn from(kind: SourceKind) -> String {
        kind.id().to_string()
    }
}

/// Relu de son identifiant ; inconnu (logiciel retiré) : le JPEG boîtier, sans perdre le
/// reste de l'état d'affichage.
impl From<String> for SourceKind {
    fn from(text: String) -> SourceKind {
        SourceKind::parse(&text).unwrap_or_default()
    }
}

/// Réglages (section `[palette]` du fichier de configuration).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaletteSettings {
    /// Couleurs cherchées dans une image (au plus ; moins après fusion des proches).
    pub colors: usize,
    /// Côté de la grille de points lus dans l'image (`side × side` au plus) : la palette
    /// ne dépend pas des détails, une petite image suffit.
    pub side: u32,
    /// Passes des k-moyennes.
    pub iterations: u32,
    /// Deux couleurs plus proches que cette distance OKLab sont fusionnées (0,03 : à
    /// peine distinguables côte à côte).
    pub merge: f32,
    /// Part minimale d'une couleur (0 à 1) : en deçà, retirée (miettes, bruit).
    pub min_share: f32,
    /// Poids de la teinte et de la saturation (a et b d'OKLab) face à la clarté dans la
    /// distance entre palettes (retour du 21/11/2026 : à poids égal, une photo brune au
    /// fond sombre semblait aussi proche d'une photo violette au fond sombre que les autres
    /// violettes : la clarté l'emportait).
    pub chroma_weight: f32,
    /// « Palettes proches » : seuil de départ du curseur (distance, voir [`distance`]).
    pub similar_threshold: f32,
    /// « Palettes proches » : seuil au bout du curseur.
    pub similar_max: f32,
    /// « Contient cette couleur » : tolérance de départ (distance d'une couleur de la
    /// palette à la couleur cherchée, comptée comme [`distance`]).
    pub colour_radius: f32,
    /// « Contient cette couleur » : tolérance au bout du curseur.
    pub colour_radius_max: f32,
    /// « Contient cette couleur » : les couleurs de la palette plus petites que cette part
    /// de l'image (0 à 1) ne comptent pas (miettes).
    pub colour_min_share: f32,
    /// Chroma (OKLab) sous lequel une couleur est un gris : une image dont toutes les
    /// couleurs le sont est en noir et blanc, et les recherches ne la mélangent pas aux
    /// images en couleur.
    pub mono_chroma: f32,
    /// Chroma au plus d'une image monochrome teintée (sépia, virage).
    pub mono_toned_chroma: f32,
    /// Empiler par palette : distance entre palettes ramenée à l'échelle des empreintes
    /// (divisée par cette valeur, bornée à 1), sur laquelle le curseur des piles est calé.
    pub stack_scale: f32,
    /// Pipette : côté de la zone moyennée autour du point visé, en part du grand côté de
    /// l'image (montrée par le collimateur de la loupe).
    pub pick_area: f32,
    /// Écart de teintes au plus (degrés) d'une image monochrome teintée : ses couleurs
    /// suivent une seule teinte.
    pub mono_hue_spread: f32,
}

impl Default for PaletteSettings {
    fn default() -> Self {
        Self {
            colors: 6,
            side: 96,
            iterations: 12,
            merge: 0.03,
            min_share: 0.02,
            chroma_weight: 2.0,
            similar_threshold: 0.12,
            similar_max: 0.5,
            colour_radius: 0.1,
            colour_radius_max: 0.4,
            colour_min_share: 0.02,
            mono_chroma: 0.03,
            mono_toned_chroma: 0.08,
            stack_scale: 0.5,
            pick_area: 0.02,
            mono_hue_spread: 15.0,
        }
    }
}

/// Une couleur de la palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swatch {
    /// La couleur, en OKLab (L de 0 à 1, a et b autour de 0).
    pub oklab: [f32; 3],
    /// Sa part de l'image (0 à 1 ; les parts d'une palette font 1).
    pub share: f32,
}

impl Swatch {
    /// La couleur en sRGB 8 bits (pour l'écrire, `#rrggbb`), écrêtée au gamut sRGB.
    pub fn srgb(&self) -> [u8; 3] {
        let linear = oklab_to_linear_srgb(self.oklab);
        linear.map(|c| (encode_srgb(c.clamp(0.0, 1.0)) * 255.0).round() as u8)
    }

    /// La couleur dans l'espace de travail (8 bits, comme les images du cache) : à
    /// afficher comme elles.
    pub fn working(&self) -> [u8; 3] {
        let linear = colour::srgb_linear_to_working(oklab_to_linear_srgb(self.oklab));
        linear.map(|c| (encode_srgb(c.clamp(0.0, 1.0)) * 255.0).round() as u8)
    }

    /// Teinte (degrés, 0 à 360) et chroma (0 : gris) : sa place sur la roue chromatique.
    pub fn hue_chroma(&self) -> (f32, f32) {
        let [_, a, b] = self.oklab;
        (b.atan2(a).to_degrees().rem_euclid(360.0), a.hypot(b))
    }
}

/// Palette d'une image : ses couleurs, de la plus présente à la moins présente.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Palette {
    /// Les couleurs, parts décroissantes.
    pub swatches: Vec<Swatch>,
    /// Couverture fine des couleurs, pour « contient cette couleur » ([`Coverage`]).
    pub coverage: Coverage,
}

/// Cases de clarté de [`Coverage`] (L de 0 à 1, pas de 0,04).
const COVER_L: u32 = 25;
/// Cases de a et de b (de -0,32 à 0,32, pas de 0,016 : ×2 pour la saturation, ~0,03).
const COVER_AB: u32 = 40;
/// Bornes de a et b dans [`Coverage`] (un sRGB très saturé atteint ~0,32).
const COVER_AB_MAX: f32 = 0.32;

/// Couverture fine des couleurs d'une image : ses points d'échantillonnage comptés par
/// petites cases OKLab. Retour du 21/11/2026 : la palette (6 couleurs moyennes) fondait
/// la peau dans l'ombre dans une couleur bien moins saturée ; la couleur pipetée sur la
/// photo ne retrouvait pas la photo. « Contient cette couleur » mesure ici la vraie part
/// des pixels proches ([`Coverage::reach`], [`Coverage::share_within`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Cases occupées (numéro, nombre de points), dans l'ordre des numéros.
    bins: Vec<(u16, u16)>,
    /// Nombre total de points.
    total: u32,
}

impl Coverage {
    /// Couverture des points `samples` (OKLab).
    pub fn from_samples(samples: &[[f32; 3]]) -> Coverage {
        let mut counts: std::collections::BTreeMap<u16, u32> = Default::default();
        for &p in samples {
            *counts.entry(cover_bin(p)).or_default() += 1;
        }
        Coverage {
            bins: counts
                .into_iter()
                .map(|(bin, n)| (bin, n.min(u32::from(u16::MAX)) as u16))
                .collect(),
            total: samples.len() as u32,
        }
    }

    /// Vrai sans aucun point (palette d'une ancienne version, image vide).
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Part des pixels à moins de `radius` de `oklab` (teinte et saturation comptées
    /// `chroma_weight` fois).
    pub fn share_within(&self, oklab: [f32; 3], radius: f32, chroma_weight: f32) -> f32 {
        let near: u32 = self
            .bins
            .iter()
            .filter(|(bin, _)| {
                weighted_distance(cover_centre(*bin), oklab, chroma_weight) <= radius
            })
            .map(|&(_, n)| u32::from(n))
            .sum();
        near as f32 / self.total.max(1) as f32
    }

    /// Distance à `oklab` en deçà de laquelle se trouve au moins `min_share` de l'image
    /// (les miettes plus petites ne comptent pas) ; `None` sans point.
    pub fn reach(&self, oklab: [f32; 3], chroma_weight: f32, min_share: f32) -> Option<f32> {
        if self.is_empty() {
            return None;
        }
        let mut near: Vec<(f32, u16)> = self
            .bins
            .iter()
            .map(|&(bin, n)| {
                (
                    weighted_distance(cover_centre(bin), oklab, chroma_weight),
                    n,
                )
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let wanted = (min_share.max(0.0) * self.total as f32).max(1.0);
        let mut seen = 0.0;
        for (d, n) in near {
            seen += f32::from(n);
            if seen >= wanted {
                return Some(d);
            }
        }
        None
    }
}

/// Case de [`Coverage`] d'une couleur OKLab.
fn cover_bin([l, a, b]: [f32; 3]) -> u16 {
    let index = |v: f32, lo: f32, hi: f32, n: u32| {
        (((v - lo) / (hi - lo) * n as f32).floor().max(0.0) as u32).min(n - 1)
    };
    let l = index(l, 0.0, 1.0, COVER_L);
    let a = index(a, -COVER_AB_MAX, COVER_AB_MAX, COVER_AB);
    let b = index(b, -COVER_AB_MAX, COVER_AB_MAX, COVER_AB);
    ((l * COVER_AB + a) * COVER_AB + b) as u16
}

/// Centre d'une case de [`Coverage`].
fn cover_centre(bin: u16) -> [f32; 3] {
    let bin = u32::from(bin);
    let (b, a, l) = (
        bin % COVER_AB,
        bin / COVER_AB % COVER_AB,
        bin / COVER_AB / COVER_AB,
    );
    let ab = |i: u32| -COVER_AB_MAX + (i as f32 + 0.5) * 2.0 * COVER_AB_MAX / COVER_AB as f32;
    [(l as f32 + 0.5) / COVER_L as f32, ab(a), ab(b)]
}

impl Palette {
    /// Part de l'image dont la couleur est à moins de `radius` de `oklab` (distance OKLab,
    /// teinte et saturation comptées `chroma_weight` fois, comme [`distance`]) : « contient
    /// assez de ce bleu ».
    pub fn share_near(&self, oklab: [f32; 3], radius: f32, chroma_weight: f32) -> f32 {
        if !self.coverage.is_empty() {
            return self.coverage.share_within(oklab, radius, chroma_weight);
        }
        self.swatches
            .iter()
            .filter(|s| weighted_distance(s.oklab, oklab, chroma_weight) <= radius)
            .map(|s| s.share)
            .sum()
    }

    /// Vrai si l'image est monochrome : en noir et blanc (toutes ses couleurs, sauf les
    /// miettes sous `min_share`, ont un chroma sous `mono.chroma`), ou **teintée** (sépia,
    /// virage : les couleurs plus saturées restent sous `mono.toned_chroma` et leurs
    /// teintes dans `mono.hue_spread` degrés, alignées sur une seule teinte). Les recherches
    /// ne mélangent pas monochromes et couleur (retours du 21/11/2026).
    pub fn is_monochrome(&self, mono: Monochrome, min_share: f32) -> bool {
        let mut hues = Vec::new();
        for s in self.swatches.iter().filter(|s| s.share >= min_share) {
            let (hue, chroma) = s.hue_chroma();
            if chroma < mono.chroma {
                continue;
            }
            if chroma >= mono.toned_chroma {
                return false;
            }
            hues.push(hue);
        }
        hue_range(&mut hues) <= mono.hue_spread
    }

    /// Distance de la couleur de la palette la plus proche de `oklab` (comptée comme
    /// [`distance`]), parmi celles d'au moins `min_share` de l'image ; `None` s'il n'y en a
    /// aucune. « Contient cette couleur » : plus elle est petite, plus l'image la contient.
    pub fn nearest(&self, oklab: [f32; 3], chroma_weight: f32, min_share: f32) -> Option<f32> {
        // La vraie part des pixels proches (couverture), sinon les couleurs de la palette.
        if !self.coverage.is_empty() {
            return self.coverage.reach(oklab, chroma_weight, min_share);
        }
        self.swatches
            .iter()
            .filter(|s| s.share >= min_share)
            .map(|s| weighted_distance(s.oklab, oklab, chroma_weight))
            .min_by(f32::total_cmp)
    }

    /// Format binaire du cache d'analyse : version, nombre de couleurs, puis pour chacune
    /// L, a, b et sa part (f32).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(5 + self.swatches.len() * 16);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.push(self.swatches.len() as u8);
        for s in &self.swatches {
            for v in s.oklab.iter().chain([&s.share]) {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        // Puis la couverture : nombre de points, nombre de cases, (case, points) en u16.
        out.extend_from_slice(&self.coverage.total.to_le_bytes());
        out.extend_from_slice(&(self.coverage.bins.len() as u32).to_le_bytes());
        for &(bin, n) in &self.coverage.bins {
            out.extend_from_slice(&bin.to_le_bytes());
            out.extend_from_slice(&n.to_le_bytes());
        }
        out
    }

    /// Relit [`Palette::to_bytes`] ; `None` si l'entrée est illisible ou d'une autre
    /// version.
    pub fn from_bytes(bytes: &[u8]) -> Option<Palette> {
        let version = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?);
        if version != VERSION {
            return None;
        }
        let n = *bytes.get(4)? as usize;
        let body = bytes.get(5..5 + n * 16)?;
        let f = |i: usize| f32::from_le_bytes(body[i * 4..i * 4 + 4].try_into().unwrap());
        let swatches = (0..n)
            .map(|k| Swatch {
                oklab: [f(k * 4), f(k * 4 + 1), f(k * 4 + 2)],
                share: f(k * 4 + 3),
            })
            .collect();
        let rest = bytes.get(5 + n * 16..)?;
        let u32_at = |i: usize| Some(u32::from_le_bytes(rest.get(i..i + 4)?.try_into().ok()?));
        let total = u32_at(0)?;
        let count = u32_at(4)? as usize;
        let body = rest.get(8..8 + count * 4)?;
        let u16_at = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]);
        let bins = (0..count)
            .map(|k| (u16_at(k * 4), u16_at(k * 4 + 2)))
            .collect();
        Some(Palette {
            swatches,
            coverage: Coverage { bins, total },
        })
    }
}

/// Palette de `bitmap` (espace de travail de `tri`).
pub fn extract(bitmap: &Bitmap, settings: &PaletteSettings) -> Palette {
    let samples = sample(bitmap, settings.side.max(1));
    let k = settings.colors.clamp(1, 16);
    if samples.is_empty() {
        return Palette::default();
    }
    let mut centres = initial_centres(&samples, k);
    let mut members = vec![0usize; samples.len()];
    for _ in 0..settings.iterations.max(1) {
        for (m, p) in members.iter_mut().zip(&samples) {
            *m = nearest(&centres, *p);
        }
        let mut sums = vec![[0.0f64; 4]; centres.len()];
        for (m, p) in members.iter().zip(&samples) {
            let s = &mut sums[*m];
            s[0] += f64::from(p[0]);
            s[1] += f64::from(p[1]);
            s[2] += f64::from(p[2]);
            s[3] += 1.0;
        }
        let mut moved = false;
        for (c, s) in centres.iter_mut().zip(&sums) {
            if s[3] > 0.0 {
                let next = [
                    (s[0] / s[3]) as f32,
                    (s[1] / s[3]) as f32,
                    (s[2] / s[3]) as f32,
                ];
                moved |= oklab_distance(next, *c) > 1e-5;
                *c = next;
            }
        }
        if !moved {
            break;
        }
    }
    let mut counts = vec![0usize; centres.len()];
    for p in &samples {
        counts[nearest(&centres, *p)] += 1;
    }
    let total = samples.len() as f32;
    let mut swatches: Vec<Swatch> = centres
        .into_iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|(oklab, n)| Swatch {
            oklab,
            share: n as f32 / total,
        })
        .collect();
    merge_close(&mut swatches, settings.merge);
    swatches.retain(|s| s.share >= settings.min_share);
    let kept: f32 = swatches.iter().map(|s| s.share).sum();
    if kept > 0.0 {
        for s in &mut swatches {
            s.share /= kept;
        }
    }
    swatches.sort_by(|a, b| b.share.total_cmp(&a.share));
    Palette {
        swatches,
        coverage: Coverage::from_samples(&samples),
    }
}

/// Distance entre deux palettes (0 : identiques) : coût de transport de leurs parts, en
/// servant d'abord les couples de couleurs les plus proches (approche gloutonne de
/// l'« Earth Mover's Distance », exacte dans les cas simples et toujours au moins aussi
/// grande que la vraie). Distances entre couleurs dans OKLab, teinte et saturation (a, b)
/// comptées `chroma_weight` fois (1 : OKLab tel quel). Une palette vide : distance
/// infinie.
pub fn distance(a: &Palette, b: &Palette, chroma_weight: f32) -> f32 {
    if a.swatches.is_empty() || b.swatches.is_empty() {
        return f32::INFINITY;
    }
    let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
    for (i, x) in a.swatches.iter().enumerate() {
        for (j, y) in b.swatches.iter().enumerate() {
            pairs.push((weighted_distance(x.oklab, y.oklab, chroma_weight), i, j));
        }
    }
    pairs.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut left_a: Vec<f32> = a.swatches.iter().map(|s| s.share).collect();
    let mut left_b: Vec<f32> = b.swatches.iter().map(|s| s.share).collect();
    let mut cost = 0.0;
    for (d, i, j) in pairs {
        let flow = left_a[i].min(left_b[j]);
        if flow > 0.0 {
            cost += flow * d;
            left_a[i] -= flow;
            left_b[j] -= flow;
        }
    }
    cost
}

/// « Palettes proches » (#492) : les éléments de `items` dont la palette est à moins de
/// `threshold` de la référence, du plus proche au plus lointain (ordre gardé à distance
/// égale). `distance` dit, pour chacun : `None` s'il n'a pas de palette possible (une photo
/// sans rendu fini) — retiré ; `Some(None)` si elle n'est pas encore calculée — **gardé, à
/// la fin**, sans quoi elle ne serait jamais calculée (le calcul en fond ne voit que la
/// liste affichée) ; `Some(Some(d))` sinon.
pub fn rank_similar<T>(
    items: Vec<T>,
    distance: impl Fn(&T) -> Option<Option<f32>>,
    threshold: f32,
) -> Vec<T> {
    let mut known: Vec<(f32, T)> = Vec::new();
    let mut unknown: Vec<T> = Vec::new();
    for item in items {
        match distance(&item) {
            None => {}
            Some(None) => unknown.push(item),
            Some(Some(d)) if d <= threshold => known.push((d, item)),
            Some(Some(_)) => {}
        }
    }
    known.sort_by(|a, b| a.0.total_cmp(&b.0));
    known.into_iter().map(|(_, t)| t).chain(unknown).collect()
}

/// Seuils de « monochrome » (voir [`Palette::is_monochrome`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Monochrome {
    /// Chroma sous lequel une couleur est un gris.
    pub chroma: f32,
    /// Chroma au plus d'une image teintée (sépia).
    pub toned_chroma: f32,
    /// Écart de teintes au plus d'une image teintée (degrés).
    pub hue_spread: f32,
}

impl PaletteSettings {
    /// Les seuils de « monochrome » de ces réglages.
    pub fn monochrome(&self) -> Monochrome {
        Monochrome {
            chroma: self.mono_chroma,
            toned_chroma: self.mono_toned_chroma,
            hue_spread: self.mono_hue_spread,
        }
    }
}

/// Étendue de teintes (degrés) sur le cercle : le plus petit arc qui les contient toutes
/// (0 sans teinte ou pour une seule).
fn hue_range(hues: &mut [f32]) -> f32 {
    if hues.len() < 2 {
        return 0.0;
    }
    hues.sort_by(f32::total_cmp);
    let mut largest_gap = 360.0 - (hues[hues.len() - 1] - hues[0]);
    for w in hues.windows(2) {
        largest_gap = largest_gap.max(w[1] - w[0]);
    }
    360.0 - largest_gap
}

/// Distance dans OKLab, a et b comptés `chroma_weight` fois.
pub fn weighted_distance(a: [f32; 3], b: [f32; 3], chroma_weight: f32) -> f32 {
    let w = chroma_weight;
    ((a[0] - b[0]).powi(2) + (w * (a[1] - b[1])).powi(2) + (w * (a[2] - b[2])).powi(2)).sqrt()
}

/// Distance euclidienne dans OKLab.
pub fn oklab_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// La couleur `oklab` ramenée dans le gamut sRGB en gardant sa clarté et sa teinte : son
/// chroma réduit juste assez (une couleur trop saturée pour l'écran, coupée composante par
/// composante, changeait de teinte et faisait des taches au bord de la roue du sélecteur).
pub fn gamut_map(oklab: [f32; 3]) -> [f32; 3] {
    let inside = |c: [f32; 3]| {
        oklab_to_linear_srgb(c)
            .iter()
            .all(|v| (-1e-4..=1.0 + 1e-4).contains(v))
    };
    if inside(oklab) {
        return oklab;
    }
    let [l, a, b] = oklab;
    let l = l.clamp(0.0, 1.0);
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..20 {
        let mid = (lo + hi) / 2.0;
        if inside([l, a * mid, b * mid]) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    [l, a * lo, b * lo]
}

/// Liens entre images voisines pour **empiler par palette** (#492, idée de l'utilisateur,
/// 21/11/2026 : séparer les séries, un éclairage, un fond, plutôt que les rafales) : la
/// distance de leurs palettes ramenée à l'échelle des empreintes (`settings.stack_scale`),
/// avec l'écart de temps de prise de vue (`times`, de leurs empreintes). Une palette
/// inconnue : pas de lien (coupé, comme une empreinte manquante).
pub fn stack_links(
    palettes: &[Option<&Palette>],
    times: &[Option<i64>],
    settings: &PaletteSettings,
) -> Vec<Option<crate::similarity::Link>> {
    palettes
        .windows(2)
        .enumerate()
        .map(|(i, w)| {
            let (a, b) = (w[0]?, w[1]?);
            let d = distance(a, b, settings.chroma_weight);
            let gap_s = match (
                times.get(i).copied().flatten(),
                times.get(i + 1).copied().flatten(),
            ) {
                (Some(x), Some(y)) => Some((y - x).abs() as f32),
                _ => None,
            };
            Some(crate::similarity::Link {
                distance: (d / settings.stack_scale.max(1e-6)).min(1.0),
                gap_s,
            })
        })
        .collect()
}

/// Clé du seuil « palettes proches » retenu par dossier.
const SIMILAR_KEY: &str = "palette-similar";
/// Clé de la tolérance « contient cette couleur » retenue par dossier.
const COLOUR_KEY: &str = "palette-colour";

/// Seuil « palettes proches » retenu pour le dossier `dir` (comme les autres curseurs).
pub fn folder_threshold(data: Option<&std::path::Path>, dir: &std::path::Path) -> Option<f32> {
    crate::store::value(data, dir, SIMILAR_KEY)
}

/// Retient le seuil « palettes proches » pour `dir` ; `None` : l'oublie (valeur de départ).
pub fn remember_threshold(
    data: Option<&std::path::Path>,
    dir: &std::path::Path,
    value: Option<f32>,
) -> std::io::Result<()> {
    crate::store::set_value(data, dir, SIMILAR_KEY, value)
}

/// Tolérance « contient cette couleur » retenue pour le dossier `dir`.
pub fn folder_radius(data: Option<&std::path::Path>, dir: &std::path::Path) -> Option<f32> {
    crate::store::value(data, dir, COLOUR_KEY)
}

/// Retient la tolérance « contient cette couleur » pour `dir` ; `None` : l'oublie.
pub fn remember_radius(
    data: Option<&std::path::Path>,
    dir: &std::path::Path,
    value: Option<f32>,
) -> std::io::Result<()> {
    crate::store::set_value(data, dir, COLOUR_KEY, value)
}

/// Un pixel 8 bits de l'espace de travail, en OKLab.
pub fn working_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
    let linear = rgb.map(|c| decode_srgb(f32::from(c) / 255.0));
    linear_srgb_to_oklab(colour::working_to_srgb_linear(linear))
}

/// Un pixel sRGB 8 bits (une couleur tapée `#rrggbb`), en OKLab.
pub fn srgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
    linear_srgb_to_oklab(rgb.map(|c| decode_srgb(f32::from(c) / 255.0)))
}

/// Points de l'image, en OKLab : une grille d'au plus `side × side`.
fn sample(bitmap: &Bitmap, side: u32) -> Vec<[f32; 3]> {
    let (w, h) = (bitmap.width, bitmap.height);
    if w == 0 || h == 0 || bitmap.data.len() < (w * h * 3) as usize {
        return Vec::new();
    }
    let step = (w.max(h) / side).max(1);
    let mut out = Vec::with_capacity(((w / step + 1) * (h / step + 1)) as usize);
    for y in (step / 2..h).step_by(step as usize) {
        for x in (step / 2..w).step_by(step as usize) {
            let i = ((y * w + x) * 3) as usize;
            out.push(working_to_oklab([
                bitmap.data[i],
                bitmap.data[i + 1],
                bitmap.data[i + 2],
            ]));
        }
    }
    out
}

/// Centres de départ, sans hasard : le point le plus proche de la moyenne, puis à chaque
/// fois le point au milieu (en cumul) des distances au carré aux centres déjà pris
/// (k-moyennes++ sans tirage : même image, même palette).
fn initial_centres(samples: &[[f32; 3]], k: usize) -> Vec<[f32; 3]> {
    let n = samples.len() as f32;
    let mean = samples.iter().fold([0.0f32; 3], |m, p| {
        [m[0] + p[0] / n, m[1] + p[1] / n, m[2] + p[2] / n]
    });
    let first = samples
        .iter()
        .min_by(|a, b| oklab_distance(**a, mean).total_cmp(&oklab_distance(**b, mean)))
        .copied()
        .unwrap_or(mean);
    let mut centres = vec![first];
    let mut nearest_sq: Vec<f32> = samples
        .iter()
        .map(|p| oklab_distance(*p, first).powi(2))
        .collect();
    while centres.len() < k {
        let total: f64 = nearest_sq.iter().map(|d| f64::from(*d)).sum();
        if total <= 1e-12 {
            break;
        }
        let half = total / 2.0;
        let mut acc = 0.0;
        let mut chosen = samples.len() - 1;
        for (i, d) in nearest_sq.iter().enumerate() {
            acc += f64::from(*d);
            if acc >= half {
                chosen = i;
                break;
            }
        }
        let c = samples[chosen];
        centres.push(c);
        for (d, p) in nearest_sq.iter_mut().zip(samples) {
            *d = d.min(oklab_distance(*p, c).powi(2));
        }
    }
    centres
}

fn nearest(centres: &[[f32; 3]], p: [f32; 3]) -> usize {
    let mut best = (f32::INFINITY, 0);
    for (i, c) in centres.iter().enumerate() {
        let d = (c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2) + (c[2] - p[2]).powi(2);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// Fusionne les couleurs plus proches que `radius` (moyenne pondérée par leurs parts).
fn merge_close(swatches: &mut Vec<Swatch>, radius: f32) {
    loop {
        let mut closest: Option<(f32, usize, usize)> = None;
        for i in 0..swatches.len() {
            for j in i + 1..swatches.len() {
                let d = oklab_distance(swatches[i].oklab, swatches[j].oklab);
                if d < radius && closest.is_none_or(|c| d < c.0) {
                    closest = Some((d, i, j));
                }
            }
        }
        let Some((_, i, j)) = closest else {
            return;
        };
        let (a, b) = (swatches[i], swatches[j]);
        let share = a.share + b.share;
        swatches[i] = Swatch {
            oklab: std::array::from_fn(|c| (a.oklab[c] * a.share + b.oklab[c] * b.share) / share),
            share,
        };
        swatches.remove(j);
    }
}

fn decode_srgb(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn encode_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB linéaire → OKLab (Björn Ottosson, 2020).
fn linear_srgb_to_oklab([r, g, b]: [f32; 3]) -> [f32; 3] {
    let l = 0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// OKLab → sRGB linéaire (valeurs hors de 0..1 hors du gamut sRGB).
fn oklab_to_linear_srgb([l, a, b]: [f32; 3]) -> [f32; 3] {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l, m, s) = (l_.powi(3), m_.powi(3), s_.powi(3));
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une image faite de bandes verticales de couleurs (espace de travail), de largeurs
    /// données.
    fn bands(colors: &[([u8; 3], u32)]) -> Bitmap {
        let width: u32 = colors.iter().map(|(_, w)| w).sum();
        let height = 40;
        let mut data = Vec::new();
        for _ in 0..height {
            for (c, w) in colors {
                for _ in 0..*w {
                    data.extend_from_slice(c);
                }
            }
        }
        Bitmap {
            width,
            height,
            data,
        }
    }

    #[test]
    fn oklab_round_trips_and_white_is_l_one() {
        let white = srgb_to_oklab([255, 255, 255]);
        assert!((white[0] - 1.0).abs() < 1e-3 && white[1].abs() < 1e-3);
        for c in [[200, 30, 40], [10, 120, 230], [128, 128, 128]] {
            let s = Swatch {
                oklab: srgb_to_oklab(c),
                share: 1.0,
            };
            let back = s.srgb();
            for k in 0..3 {
                assert!(back[k].abs_diff(c[k]) <= 1, "{c:?} → {back:?}");
            }
        }
    }

    #[test]
    fn two_colours_give_two_swatches_with_their_shares() {
        // Trois quarts de rouge, un quart de bleu (dans l'espace de travail).
        let img = bands(&[([200, 40, 40], 120), ([40, 60, 200], 40)]);
        let p = extract(&img, &PaletteSettings::default());
        assert_eq!(p.swatches.len(), 2, "{p:?}");
        assert!((p.swatches[0].share - 0.75).abs() < 0.03, "{p:?}");
        let red = working_to_oklab([200, 40, 40]);
        assert!(oklab_distance(p.swatches[0].oklab, red) < 0.01);
        let sum: f32 = p.swatches.iter().map(|s| s.share).sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn same_image_same_palette() {
        let img = bands(&[
            ([200, 40, 40], 50),
            ([40, 160, 60], 30),
            ([40, 60, 200], 20),
            ([230, 220, 90], 10),
        ]);
        let s = PaletteSettings::default();
        assert_eq!(extract(&img, &s), extract(&img, &s));
    }

    #[test]
    fn tiny_specks_and_near_duplicates_are_dropped() {
        // Deux rouges indiscernables fusionnent ; un point vert de 1 % disparaît.
        let img = bands(&[([200, 40, 40], 60), ([201, 41, 40], 39), ([40, 200, 40], 1)]);
        let p = extract(&img, &PaletteSettings::default());
        assert_eq!(p.swatches.len(), 1, "{p:?}");
        assert!((p.swatches[0].share - 1.0).abs() < 1e-4);
    }

    #[test]
    fn closer_palettes_have_smaller_distances() {
        let s = PaletteSettings::default();
        let a = extract(&bands(&[([200, 40, 40], 80), ([40, 60, 200], 20)]), &s);
        let near = extract(&bands(&[([190, 50, 45], 80), ([40, 60, 200], 20)]), &s);
        let far = extract(&bands(&[([40, 160, 60], 80), ([230, 220, 90], 20)]), &s);
        let w = s.chroma_weight;
        assert_eq!(distance(&a, &a, w), 0.0);
        assert!(distance(&a, &near, w) < distance(&a, &far, w));
        assert!((distance(&a, &near, w) - distance(&near, &a, w)).abs() < 1e-6);
        assert!(distance(&a, &Palette::default(), w).is_infinite());
    }

    #[test]
    fn hue_counts_more_than_lightness_for_dark_backgrounds() {
        // Retour du 21/11/2026 : référence au fond bleu nuit ; une photo violette au fond
        // sombre doit être plus proche qu'une photo brune au fond sombre, nettement.
        let s = PaletteSettings::default();
        let reference = extract(
            &bands(&[([3, 37, 106], 62), ([3, 13, 50], 18), ([36, 87, 195], 20)]),
            &s,
        );
        let purple = extract(
            &bands(&[([26, 10, 74], 50), ([176, 64, 224], 30), ([5, 8, 48], 20)]),
            &s,
        );
        let brown = extract(
            &bands(&[([26, 21, 18], 55), ([58, 42, 34], 25), ([128, 96, 80], 20)]),
            &s,
        );
        let gap = |w: f32| distance(&reference, &brown, w) - distance(&reference, &purple, w);
        assert!(gap(s.chroma_weight) > 0.0, "la violette d'abord");
        assert!(
            gap(s.chroma_weight) > 2.0 * gap(1.0),
            "l'écart se creuse : {} contre {}",
            gap(s.chroma_weight),
            gap(1.0)
        );
    }

    #[test]
    fn similar_palettes_are_kept_under_the_threshold_closest_first() {
        // a : 0,05 ; b : sans palette possible ; c : 0,01 ; d : pas encore calculée ; e : 0,2.
        let items = ["a", "b", "c", "d", "e"];
        let distance = |s: &&str| match *s {
            "a" => Some(Some(0.05)),
            "b" => None,
            "c" => Some(Some(0.01)),
            "d" => Some(None),
            _ => Some(Some(0.2)),
        };
        assert_eq!(rank_similar(items.to_vec(), distance, 0.1), ["c", "a", "d"]);
    }

    #[test]
    fn grey_and_toned_palettes_are_monochrome_a_warm_colour_one_is_not() {
        let s = PaletteSettings::default();
        let mono = s.monochrome();
        let from = |hexes: &[(&str, f32)]| Palette {
            coverage: Default::default(),
            swatches: hexes
                .iter()
                .map(|(h, share)| {
                    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
                    Swatch {
                        oklab: srgb_to_oklab([v(1), v(3), v(5)]),
                        share: *share,
                    }
                })
                .collect(),
        };
        let grey = from(&[("#e6e6e6", 0.6), ("#1e1e1e", 0.4)]);
        assert!(grey.is_monochrome(mono, s.colour_min_share));
        let speck = from(&[("#e6e6e6", 0.99), ("#c81e1e", 0.01)]);
        assert!(
            speck.is_monochrome(mono, s.colour_min_share),
            "miette ignorée"
        );
        // Sépia : teintes de 64 à 78°.
        let sepia = from(&[
            ("#2b2118", 0.3),
            ("#5e4a36", 0.3),
            ("#a88b6a", 0.2),
            ("#e0cdb0", 0.2),
        ]);
        assert!(sepia.is_monochrome(mono, s.colour_min_share), "sépia");
        // Série brune en couleur (retour du 21/11/2026) : teintes de 32 à 58°.
        let brown = from(&[
            ("#151414", 0.51),
            ("#272221", 0.14),
            ("#655346", 0.11),
            ("#433831", 0.09),
            ("#d8a79d", 0.08),
            ("#a67e66", 0.07),
        ]);
        assert!(
            !brown.is_monochrome(mono, s.colour_min_share),
            "brune en couleur"
        );
        let blue = from(&[("#e6e6e6", 0.6), ("#283cc8", 0.4)]);
        assert!(!blue.is_monochrome(mono, s.colour_min_share));
        assert_eq!(hue_range(&mut [350.0, 10.0]), 20.0, "autour de 0°");
    }

    #[test]
    fn gamut_mapping_keeps_lightness_and_hue() {
        let wild = [0.6, 0.3, -0.3];
        let mapped = gamut_map(wild);
        assert_eq!(mapped[0], 0.6);
        let (h0, h1) = (wild[2].atan2(wild[1]), mapped[2].atan2(mapped[1]));
        assert!((h0 - h1).abs() < 1e-4);
        assert!(
            oklab_to_linear_srgb(mapped)
                .iter()
                .all(|v| (-1e-3..=1.001).contains(v))
        );
        let grey = [0.5, 0.0, 0.0];
        assert_eq!(gamut_map(grey), grey, "déjà dedans");
    }

    #[test]
    fn stack_links_scale_palette_distances_and_cut_on_unknown() {
        let s = PaletteSettings::default();
        let blue = extract(&bands(&[([30, 60, 200], 10)]), &s);
        let red = extract(&bands(&[([220, 40, 40], 10)]), &s);
        let links = stack_links(
            &[Some(&blue), Some(&blue), Some(&red), None],
            &[Some(0), Some(2), Some(100), None],
            &s,
        );
        assert_eq!(links.len(), 3);
        let l0 = links[0].unwrap();
        assert_eq!((l0.distance, l0.gap_s), (0.0, Some(2.0)));
        let l1 = links[1].unwrap();
        assert!(l1.distance > 0.3 && l1.distance <= 1.0, "{l1:?}");
        assert_eq!(l1.gap_s, Some(98.0));
        assert!(links[2].is_none(), "palette inconnue : coupé");
    }

    #[test]
    fn nearest_colour_ignores_specks() {
        let p = Palette {
            coverage: Default::default(),
            swatches: vec![
                Swatch {
                    oklab: [0.5, 0.1, 0.0],
                    share: 0.9,
                },
                Swatch {
                    oklab: [0.5, -0.1, 0.0],
                    share: 0.01,
                },
            ],
        };
        let target = [0.5, -0.1, 0.0];
        assert_eq!(p.nearest(target, 1.0, 0.0), Some(0.0));
        assert!(
            (p.nearest(target, 1.0, 0.02).unwrap() - 0.2).abs() < 1e-6,
            "la miette ignorée"
        );
        assert_eq!(Palette::default().nearest(target, 1.0, 0.0), None);
    }

    #[test]
    fn share_near_finds_a_colour() {
        let p = extract(
            &bands(&[([200, 40, 40], 70), ([40, 60, 200], 30)]),
            &PaletteSettings::default(),
        );
        let blue = working_to_oklab([45, 65, 195]);
        assert!((p.share_near(blue, 0.05, 1.0) - 0.3).abs() < 0.03);
        assert_eq!(p.share_near(srgb_to_oklab([0, 255, 0]), 0.05, 2.0), 0.0);
    }

    #[test]
    fn a_colour_picked_on_the_photo_finds_the_photo() {
        // Retour du 21/11/2026 (vidéo) : la peau dans l'ombre, orangée et saturée, était
        // fondue dans une couleur moyenne de la palette, bien moins saturée ; pipetée sur
        // la photo, elle ne retrouvait pas la photo. La couverture compte les vrais pixels.
        let skin_shadow = [121u8, 66, 24];
        let image = bands(&[
            ([20, 18, 17], 50),
            ([150, 110, 85], 20),
            ([170, 130, 105], 10),
            (skin_shadow, 5),
            ([90, 80, 72], 15),
        ]);
        let s = PaletteSettings::default();
        let palette = extract(&image, &s);
        let picked = working_to_oklab(skin_shadow);
        let reach = palette
            .nearest(picked, s.chroma_weight, s.colour_min_share)
            .unwrap();
        assert!(reach < 0.04, "trouvée : {reach}");
        assert!(
            (palette.share_near(picked, 0.05, s.chroma_weight) - 0.05).abs() < 0.02,
            "5 % de l'image"
        );
        // Une couleur absente reste loin ; une miette sous la part minimale ne compte pas.
        let blue = working_to_oklab([30, 60, 200]);
        assert!(palette.nearest(blue, s.chroma_weight, 0.02).unwrap() > 0.3);
        assert!(palette.nearest(picked, s.chroma_weight, 0.2).unwrap() > reach);
    }

    #[test]
    fn coverage_bins_round_trip_their_centres() {
        for oklab in [[0.5, 0.0, 0.0], [0.9, 0.2, -0.25], [0.1, -0.3, 0.31]] {
            let c = cover_centre(cover_bin(oklab));
            assert!(weighted_distance(c, oklab, 2.0) < 0.04, "{oklab:?} → {c:?}");
        }
    }

    #[test]
    fn bytes_round_trip_and_reject_other_versions() {
        let p = extract(
            &bands(&[([200, 40, 40], 70), ([40, 60, 200], 30)]),
            &PaletteSettings::default(),
        );
        assert_eq!(Palette::from_bytes(&p.to_bytes()), Some(p.clone()));
        let mut bytes = p.to_bytes();
        bytes[0] ^= 0xff;
        assert_eq!(Palette::from_bytes(&bytes), None);
        assert_eq!(Palette::from_bytes(&bytes[..3]), None);
    }

    #[test]
    fn hue_places_red_near_zero_and_grey_has_no_chroma() {
        let red = Swatch {
            oklab: srgb_to_oklab([255, 0, 0]),
            share: 1.0,
        };
        let (h, c) = red.hue_chroma();
        assert!(h < 40.0 || h > 340.0, "{h}");
        assert!(c > 0.2);
        let grey = Swatch {
            oklab: srgb_to_oklab([128, 128, 128]),
            share: 1.0,
        };
        assert!(grey.hue_chroma().1 < 1e-3);
    }
}

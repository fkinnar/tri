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

impl Source {
    /// Identifiant (index des palettes, #561) : `camera`, `raw`, ou celui du logiciel.
    pub fn id(self) -> &'static str {
        match self {
            Source::Camera => "camera",
            Source::Raw => "raw",
            Source::Developed(engine) => engine.id(),
        }
    }

    /// Source d'après son identifiant ([`Source::id`]).
    pub fn parse(id: &str) -> Option<Source> {
        match id {
            "camera" => Some(Source::Camera),
            "raw" => Some(Source::Raw),
            id => crate::develop::Engine::from_id(id).map(Source::Developed),
        }
    }
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
    /// « Contient cette couleur », d'après les vrais pixels : part minimale de l'image proche
    /// de la couleur (0 à 1). Plus petite que `colour_min_share` : un détail qu'on pipette
    /// (un reflet) suffit (retour du 21/11/2026).
    pub colour_coverage: f32,
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
    /// Contient ces couleurs, recherche hiérarchique : poids de chaque couleur face à la
    /// précédente (0,4 : la deuxième compte 0,4 fois la première, la troisième 0,16 fois… ;
    /// mesuré le 23/11/2026 : à 0,6, la deuxième couleur pesait encore trop).
    pub colour_hierarchy: f32,
    /// Contient ces couleurs, recherche hiérarchique : poids de la proximité (distance
    /// rapportée à la tolérance) face à la présence (part de l'image), qui décide d'abord.
    pub colour_proximity: f32,
    /// Couleur moyenne : poids de la clarté face à la teinte et la saturation (comptées
    /// `chroma_weight` fois) ; petit, une photo sombre à dominante bleue reste bleue.
    pub mean_lightness: f32,
    /// Contient ces couleurs, recherche hiérarchique : tolérance élargie à chaque rang, en
    /// part de celle de la première (0,25 : la deuxième ×1,25, la troisième ×1,5…).
    pub colour_widen: f32,
    /// Couleurs perçues (palette en grand, expérience) : part de la teinte de l'entourage
    /// retirée de chaque pixel (adaptation de l'œil à la lumière dominante).
    pub perceived_adaptation: f32,
    /// Couleurs perçues : taille de l'entourage, en part du grand côté de l'image.
    pub perceived_surround: f32,
    /// Couleurs perçues : poids des zones qui tranchent sur leur entourage.
    pub perceived_salience: f32,
    /// Couleurs perçues : saturation rendue après l'adaptation (1 : telle quelle), pour
    /// `perceived_saturation_colours` couleurs.
    pub perceived_saturation: f32,
    /// Couleurs perçues : nombre de couleurs auquel vaut `perceived_saturation`.
    pub perceived_saturation_colours: usize,
    /// Couleurs perçues : pente de la saturation selon le nombre de couleurs (le surplus
    /// au-delà de 1 varie comme (référence / couleurs) ^ pente ; 0 : la même partout).
    pub perceived_saturation_falloff: f32,
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
            colour_coverage: 0.005,
            mono_chroma: 0.03,
            mono_toned_chroma: 0.08,
            stack_scale: 0.5,
            pick_area: 0.02,
            mono_hue_spread: 15.0,
            colour_hierarchy: 0.4,
            colour_proximity: 0.25,
            mean_lightness: 0.3,
            colour_widen: 0.25,
            perceived_adaptation: 0.8,
            perceived_surround: 0.2,
            perceived_salience: 1.0,
            perceived_saturation: 1.4,
            perceived_saturation_colours: 4,
            perceived_saturation_falloff: 0.5,
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

    /// Part des pixels dont le chroma (saturation OKLab) dépasse `chroma`.
    pub fn share_chroma_above(&self, chroma: f32) -> f32 {
        let above: u32 = self
            .bins
            .iter()
            .filter(|(bin, _)| {
                let [_, a, b] = cover_centre(*bin);
                a.hypot(b) > chroma
            })
            .map(|&(_, n)| u32::from(n))
            .sum();
        above as f32 / self.total.max(1) as f32
    }

    /// Part des pixels **sûrement** plus saturés que `chroma` : leur case entière l'est
    /// (son point le plus proche du gris). Retour du 22/11/2026 : un gris de JPEG à peine
    /// teinté (chroma ~0,026) tombait dans une case de centre 0,034 ; jugée sur les
    /// centres, une photo en noir et blanc passait pour colorée.
    pub fn share_surely_above(&self, chroma: f32) -> f32 {
        let step = 2.0 * COVER_AB_MAX / COVER_AB as f32;
        let nearest_to_zero = |c: f32| (c.abs() - step / 2.0).max(0.0);
        let above: u32 = self
            .bins
            .iter()
            .filter(|(bin, _)| {
                let [_, a, b] = cover_centre(*bin);
                nearest_to_zero(a).hypot(nearest_to_zero(b)) > chroma
            })
            .map(|&(_, n)| u32::from(n))
            .sum();
        above as f32 / self.total.max(1) as f32
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

    /// Vrai si l'image est en **noir et blanc** pur : toutes ses couleurs (et ses vrais
    /// pixels, sauf les miettes sous `min_share`) ont un chroma sous `mono.chroma`. Une
    /// image teintée (sépia, virage) ne l'est pas.
    pub fn is_grey(&self, mono: Monochrome, min_share: f32) -> bool {
        if !self.coverage.is_empty() {
            return self.coverage.share_surely_above(mono.chroma) <= min_share;
        }
        self.swatches
            .iter()
            .filter(|s| s.share >= min_share)
            .all(|s| s.hue_chroma().1 < mono.chroma)
    }

    /// Vrai si l'image est monochrome : en noir et blanc (toutes ses couleurs, sauf les
    /// miettes sous `min_share`, ont un chroma sous `mono.chroma`), ou **teintée** (sépia,
    /// virage : les couleurs plus saturées restent sous `mono.toned_chroma` et leurs
    /// teintes dans `mono.hue_spread` degrés, alignées sur une seule teinte). Les recherches
    /// ne mélangent pas monochromes et couleur (retours du 21/11/2026).
    pub fn is_monochrome(&self, mono: Monochrome, min_share: f32) -> bool {
        // Les vrais pixels d'abord (retour du 21/11/2026, vidéo) : un portrait chaud et peu
        // saturé, sur fond sombre, avait toutes ses couleurs moyennes sous le seuil des
        // teintées, alignées ; jugé « sépia », il était écarté d'une recherche de couleur
        // pipetée sur lui. Plus de `min_share` des pixels au-delà de ce seuil : en couleur.
        if !self.coverage.is_empty()
            && self.coverage.share_chroma_above(mono.toned_chroma) > min_share
        {
            return false;
        }
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

    /// Couleur moyenne de l'image (OKLab) : celles de la palette, pondérées par leur part ;
    /// `None` sans couleur.
    pub fn mean(&self) -> Option<[f32; 3]> {
        let total: f32 = self.swatches.iter().map(|s| s.share).sum();
        if total <= 0.0 {
            return None;
        }
        let mut m = [0.0; 3];
        for s in &self.swatches {
            for (k, v) in m.iter_mut().enumerate() {
                *v += s.oklab[k] * s.share / total;
            }
        }
        Some(m)
    }

    /// « Contient ces couleurs » (#492) : coût de la palette face à `query`, plus petit
    /// pour une image qui répond mieux ; `None` si l'une des couleurs manque (plus loin que
    /// la tolérance, voir [`Palette::nearest`]).
    ///
    /// Recherche absolue : la moyenne des distances (une couleur seule : sa distance).
    /// Hiérarchique : chaque couleur compte `query.hierarchy` fois la précédente, et sa
    /// présence (part de l'image à moins de la tolérance) s'y ajoute : l'image où la
    /// 1ʳᵉ couleur est la plus proche et la plus présente passe devant.
    pub fn colours_cost(
        &self,
        query: &ColourQuery,
        chroma_weight: f32,
        min_share: f32,
    ) -> Option<f32> {
        // Couleur moyenne : la teinte et la saturation surtout, la clarté peu (une photo à
        // dominante bleue mais sombre reste « bleue en moyenne »).
        if let Some(lightness) = query.mean {
            let wanted = *query.colours.first()?;
            let m = self.mean()?;
            let d = ((lightness * (m[0] - wanted[0])).powi(2)
                + (chroma_weight * (m[1] - wanted[1])).powi(2)
                + (chroma_weight * (m[2] - wanted[2])).powi(2))
            .sqrt();
            return (d <= query.radius).then_some(d);
        }
        let (mut sum, mut weights, mut weight) = (0.0, 0.0, 1.0);
        for (rank, &oklab) in query.colours.iter().enumerate() {
            // Hiérarchique : la tolérance s'élargit de rang en rang (idée de l'utilisateur,
            // 23/11/2026 : la première couleur plus précise que la dernière).
            let radius = match query.hierarchy {
                Some(_) => query.radius * (1.0 + query.widen * rank as f32),
                None => query.radius,
            };
            let d = self.nearest(oklab, chroma_weight, min_share)?;
            if d > radius {
                return None;
            }
            // Hiérarchique : la présence d'abord (mesuré le 23/11/2026 sur 300 photos :
            // la proximité, ajoutée à part égale, l'emportait, et l'ordre changeait à
            // peine), la proximité (rapportée à la tolérance) pour départager.
            let cost = match query.hierarchy {
                None => d,
                Some(_) => {
                    let share = self.share_near(oklab, radius, chroma_weight);
                    (1.0 - share) + query.proximity * d / radius.max(1e-6)
                }
            };
            sum += weight * cost;
            weights += weight;
            if let Some(h) = query.hierarchy {
                weight *= h.clamp(0.0, 1.0);
            }
        }
        (weights > 0.0).then(|| sum / weights)
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
    let (centres, counts) = cluster(&samples, k, settings.iterations);
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
    rank_similar_by(items, distance, threshold, false)
}

/// [`rank_similar`], du plus lointain au plus proche avec `descending` (tri inversé) ;
/// ceux pas encore calculés toujours à la fin.
pub fn rank_similar_by<T>(
    items: Vec<T>,
    distance: impl Fn(&T) -> Option<Option<f32>>,
    threshold: f32,
    descending: bool,
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
    if descending {
        known.reverse();
    }
    known.into_iter().map(|(_, t)| t).chain(unknown).collect()
}

/// Comme [`rank_similar`], sans trier (#492, 23/11/2026 : le filtre filtre, le tri trie) :
/// les éléments gardés restent dans leur ordre, ceux dont la palette n'est pas encore
/// calculée à leur place.
pub fn keep_similar<T>(
    items: Vec<T>,
    distance: impl Fn(&T) -> Option<Option<f32>>,
    threshold: f32,
) -> Vec<T> {
    items
        .into_iter()
        .filter(|item| match distance(item) {
            None => false,
            Some(None) => true,
            Some(Some(d)) => d <= threshold,
        })
        .collect()
}

/// Lit une couleur `#rrggbb` (ou `rrggbb`) ; `None` sinon.
pub fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// « Contient ces couleurs » (#492) : les couleurs cherchées (OKLab, la plus importante
/// d'abord), la tolérance, et `hierarchy` : `None` pour une recherche absolue (toutes
/// pèsent pareil), sinon le poids de chacune face à la précédente.
#[derive(Clone, Debug, PartialEq)]
pub struct ColourQuery {
    /// Couleurs cherchées, en OKLab, la plus importante d'abord.
    pub colours: Vec<[f32; 3]>,
    /// Tolérance : distance au plus de chaque couleur (voir [`Palette::nearest`]).
    pub radius: f32,
    /// `None` : recherche absolue ; sinon poids de chaque couleur face à la précédente.
    pub hierarchy: Option<f32>,
    /// Recherche hiérarchique : poids de la proximité face à la présence.
    pub proximity: f32,
    /// Recherche hiérarchique : la tolérance de chaque couleur, plus large de cette part
    /// de celle de la première à chaque rang (0,25 : la 2ᵉ ×1,25, la 3ᵉ ×1,5…).
    pub widen: f32,
    /// Recherche de la couleur **moyenne** (#492, 23/11/2026) : `Some(poids de la
    /// clarté)`, la première couleur seule comparée à la moyenne de l'image (voir
    /// [`Palette::mean`]) ; `None` sinon.
    pub mean: Option<f32>,
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

/// K-moyennes déterministes sur `samples` (OKLab) : `k` centres au plus, et le nombre de
/// points de chacun.
fn cluster(samples: &[[f32; 3]], k: usize, iterations: u32) -> (Vec<[f32; 3]>, Vec<usize>) {
    cluster_weighted(samples, None, k, iterations)
}

/// [`cluster`], chaque point pesant `weights[i]` dans le calcul des centres (`None` : 1).
fn cluster_weighted(
    samples: &[[f32; 3]],
    weights: Option<&[f32]>,
    k: usize,
    iterations: u32,
) -> (Vec<[f32; 3]>, Vec<usize>) {
    let mut centres = initial_centres(samples, k);
    let mut members = vec![0usize; samples.len()];
    for _ in 0..iterations.max(1) {
        for (m, p) in members.iter_mut().zip(samples) {
            *m = nearest(&centres, *p);
        }
        let mut sums = vec![[0.0f64; 4]; centres.len()];
        for (i, (m, p)) in members.iter().zip(samples).enumerate() {
            let w = f64::from(weights.map_or(1.0, |w| w[i]));
            let s = &mut sums[*m];
            s[0] += w * f64::from(p[0]);
            s[1] += w * f64::from(p[1]);
            s[2] += w * f64::from(p[2]);
            s[3] += w;
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
    for p in samples {
        counts[nearest(&centres, *p)] += 1;
    }
    (centres, counts)
}

/// Vue « par palette » (#492, idée de l'utilisateur, 23/11/2026) : l'image en `k`
/// couleurs, chaque pixel remplacé par la plus proche (OKLab) des `k` couleurs principales
/// de l'image (k-moyennes sans fusion ni miettes, comme [`extract`]) ; `k = 1` : sa
/// couleur moyenne, la dominante. Voir les rapports de masses colorées. Renvoie l'image
/// (même taille, espace de travail) et ses couleurs, les plus présentes d'abord (part
/// des pixels).
pub fn posterize(bitmap: &Bitmap, k: usize, settings: &PaletteSettings) -> (Bitmap, Vec<Swatch>) {
    posterize_with(bitmap, k, settings, false)
}

/// [`posterize`], en **couleurs perçues** avec `perceived` (expérience du 23/11/2026, idée
/// de l'utilisateur : sous une lumière violette, un visage beige rosé paraît orange) :
/// chaque pixel est vu après adaptation à la lumière de son entourage (une part
/// `perceived_adaptation` de la teinte moyenne d'un voisinage de `perceived_surround` du
/// grand côté, retirée en OKLab), et les zones qui tranchent sur leur entourage pèsent
/// plus (`perceived_salience`) dans le choix des couleurs. `k = 0` : l'image perçue,
/// sans réduction.
pub fn posterize_with(
    bitmap: &Bitmap,
    k: usize,
    settings: &PaletteSettings,
    perceived: bool,
) -> (Bitmap, Vec<Swatch>) {
    if perceived {
        return posterize_perceived(bitmap, k, settings);
    }
    let (w, h) = (bitmap.width, bitmap.height);
    let n = (w * h) as usize;
    let samples = sample(bitmap, settings.side.max(1));
    if samples.is_empty() || bitmap.data.len() < n * 3 {
        return (bitmap.clone(), Vec::new());
    }
    let centres = if k <= 1 {
        let m = samples.len() as f32;
        vec![samples.iter().fold([0.0f32; 3], |a, p| {
            [a[0] + p[0] / m, a[1] + p[1] / m, a[2] + p[2] / m]
        })]
    } else {
        cluster(&samples, k.min(16), settings.iterations).0
    };
    let colours: Vec<[u8; 3]> = centres
        .iter()
        .map(|&oklab| Swatch { oklab, share: 0.0 }.working())
        .collect();
    // Chaque pixel : sa couleur la plus proche ; un pixel déjà vu (même valeur) n'est pas
    // reconverti (les aplats, les ciels).
    let mut memo: std::collections::HashMap<[u8; 3], u8> = std::collections::HashMap::new();
    let mut counts = vec![0usize; centres.len()];
    let mut data = Vec::with_capacity(n * 3);
    for &rgb in bitmap.data[..n * 3].as_chunks::<3>().0 {
        let i = *memo
            .entry(rgb)
            .or_insert_with(|| nearest(&centres, working_to_oklab(rgb)) as u8);
        counts[i as usize] += 1;
        data.extend_from_slice(&colours[i as usize]);
    }
    let mut swatches: Vec<Swatch> = centres
        .into_iter()
        .zip(counts)
        .filter(|(_, c)| *c > 0)
        .map(|(oklab, c)| Swatch {
            oklab,
            share: c as f32 / n as f32,
        })
        .collect();
    swatches.sort_by(|a, b| b.share.total_cmp(&a.share));
    (
        Bitmap {
            width: w,
            height: h,
            data,
        },
        swatches,
    )
}

/// [`posterize_with`] en couleurs perçues.
fn posterize_perceived(
    bitmap: &Bitmap,
    k: usize,
    settings: &PaletteSettings,
) -> (Bitmap, Vec<Swatch>) {
    let (w, h) = (bitmap.width as usize, bitmap.height as usize);
    let n = w * h;
    if n == 0 || bitmap.data.len() < n * 3 {
        return (bitmap.clone(), Vec::new());
    }
    // L'image en OKLab (un pixel déjà vu n'est pas reconverti).
    let mut memo: std::collections::HashMap<[u8; 3], [f32; 3]> = std::collections::HashMap::new();
    let lab: Vec<[f32; 3]> = bitmap.data[..n * 3]
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&rgb| *memo.entry(rgb).or_insert_with(|| working_to_oklab(rgb)))
        .collect();
    // La teinte de l'entourage : a et b floutés (moyenne d'un carré, deux passes).
    let radius = ((w.max(h) as f32 * settings.perceived_surround.clamp(0.0, 1.0)) as usize).max(1);
    let a: Vec<f32> = lab.iter().map(|p| p[1]).collect();
    let b: Vec<f32> = lab.iter().map(|p| p[2]).collect();
    let around_a = box_blur(&box_blur(&a, w, h, radius), w, h, radius);
    let around_b = box_blur(&box_blur(&b, w, h, radius), w, h, radius);
    let adapt = settings.perceived_adaptation.clamp(0.0, 1.0);
    // Retirer la lumière dominante ôte aussi de la couleur à tout le reste : rendue par
    // `perceived_saturation` (retour du 23/11/2026 : « infiniment plus naturel », mais
    // terne).
    // Dégressive (retour du 23/11/2026) : moins de couleurs, chacune plus moyenne, plus
    // terne ; `perceived_saturation` vaut pour `perceived_saturation_colours` couleurs,
    // et le surplus varie comme (référence / k) ^ `perceived_saturation_falloff` ;
    // l'image entière (k = 0) n'est pas touchée (retour du 23/11/2026).
    let reference = settings.perceived_saturation_colours.max(1) as f32;
    let extra = settings.perceived_saturation - 1.0;
    let saturation = if k == 0 {
        1.0
    } else {
        (1.0 + extra * (reference / k as f32).powf(settings.perceived_saturation_falloff.max(0.0)))
            .max(0.0)
    };
    let perceived: Vec<[f32; 3]> = (0..n)
        .map(|i| {
            [
                lab[i][0],
                saturation * (lab[i][1] - adapt * around_a[i]),
                saturation * (lab[i][2] - adapt * around_b[i]),
            ]
        })
        .collect();
    // Ce qui tranche sur l'entourage pèse plus (l'écart de teinte, en part d'un écart
    // franc de 0,1).
    let weight: Vec<f32> = (0..n)
        .map(|i| {
            let d = (lab[i][1] - around_a[i]).hypot(lab[i][2] - around_b[i]);
            1.0 + settings.perceived_salience.max(0.0) * d / 0.1
        })
        .collect();
    let to_bitmap = |colours: &[[f32; 3]]| -> Vec<u8> {
        let mut data = Vec::with_capacity(n * 3);
        for c in colours {
            data.extend_from_slice(
                &Swatch {
                    oklab: *c,
                    share: 0.0,
                }
                .working(),
            );
        }
        data
    };
    // Les points de la grille (comme `sample`), avec leur poids.
    let step = (w.max(h) / settings.side.max(1) as usize).max(1);
    let (mut points, mut weights) = (Vec::new(), Vec::new());
    for y in (step / 2..h).step_by(step) {
        for x in (step / 2..w).step_by(step) {
            points.push(perceived[y * w + x]);
            weights.push(weight[y * w + x]);
        }
    }
    // L'image perçue entière (k = 0) : sa palette, du nombre de couleurs des palettes
    // (retour du 23/11/2026 : à droite, rien ne venait), l'image intacte.
    if k == 0 {
        let centres = cluster_weighted(
            &points,
            Some(&weights),
            settings.colors.clamp(1, 16),
            settings.iterations,
        )
        .0;
        let mut counts = vec![0usize; centres.len()];
        for p in &perceived {
            counts[nearest(&centres, *p)] += 1;
        }
        let mut swatches: Vec<Swatch> = centres
            .into_iter()
            .zip(counts)
            .filter(|(_, c)| *c > 0)
            .map(|(oklab, c)| Swatch {
                oklab,
                share: c as f32 / n as f32,
            })
            .collect();
        swatches.sort_by(|a, b| b.share.total_cmp(&a.share));
        return (
            Bitmap {
                width: bitmap.width,
                height: bitmap.height,
                data: to_bitmap(&perceived),
            },
            swatches,
        );
    }
    let centres = if k <= 1 {
        let total: f32 = weights.iter().sum::<f32>().max(1e-6);
        let mut m = [0.0f32; 3];
        for (p, wt) in points.iter().zip(&weights) {
            for c in 0..3 {
                m[c] += p[c] * wt / total;
            }
        }
        vec![m]
    } else {
        cluster_weighted(&points, Some(&weights), k.min(16), settings.iterations).0
    };
    let mut counts = vec![0usize; centres.len()];
    let mut out = Vec::with_capacity(n);
    for p in &perceived {
        let i = nearest(&centres, *p);
        counts[i] += 1;
        out.push(centres[i]);
    }
    let data = to_bitmap(&out);
    let mut swatches: Vec<Swatch> = centres
        .into_iter()
        .zip(counts)
        .filter(|(_, c)| *c > 0)
        .map(|(oklab, c)| Swatch {
            oklab,
            share: c as f32 / n as f32,
        })
        .collect();
    swatches.sort_by(|a, b| b.share.total_cmp(&a.share));
    (
        Bitmap {
            width: bitmap.width,
            height: bitmap.height,
            data,
        },
        swatches,
    )
}

/// Moyenne de `values` (une grille `w × h`) sur un carré de côté `2 × radius + 1`, bords
/// compris (en deux passes, lignes puis colonnes, par sommes glissantes).
fn box_blur(values: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let mut rows = vec![0.0f32; w * h];
    for y in 0..h {
        let line = &values[y * w..(y + 1) * w];
        let mut prefix = vec![0.0f64; w + 1];
        for x in 0..w {
            prefix[x + 1] = prefix[x] + f64::from(line[x]);
        }
        for x in 0..w {
            let (lo, hi) = (x.saturating_sub(radius), (x + radius + 1).min(w));
            rows[y * w + x] = ((prefix[hi] - prefix[lo]) / (hi - lo) as f64) as f32;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for x in 0..w {
        let mut prefix = vec![0.0f64; h + 1];
        for y in 0..h {
            prefix[y + 1] = prefix[y] + f64::from(rows[y * w + x]);
        }
        for y in 0..h {
            let (lo, hi) = (y.saturating_sub(radius), (y + radius + 1).min(h));
            out[y * w + x] = ((prefix[hi] - prefix[lo]) / (hi - lo) as f64) as f32;
        }
    }
    out
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
    fn a_warm_portrait_is_not_taken_for_a_toned_monochrome() {
        // Retour du 21/11/2026 (vidéo) : fond sombre, peau chaude peu saturée en moyenne ;
        // la peau dans l'ombre, plus saturée, en fait une photo en couleur. Un vrai virage
        // (sépia léger partout) reste monochrome.
        let s = PaletteSettings {
            colors: 3,
            ..PaletteSettings::default()
        };
        let mono = s.monochrome();
        let portrait = extract(
            &bands(&[
                ([20, 18, 17], 60),
                ([130, 115, 105], 25),
                ([121, 80, 50], 6),
                ([90, 82, 76], 9),
            ]),
            &s,
        );
        // Ses couleurs moyennes seules la diraient virée (le cas de la vidéo)…
        let averages_only = Palette {
            coverage: Coverage::default(),
            ..portrait.clone()
        };
        assert!(
            averages_only.is_monochrome(mono, s.colour_min_share),
            "{portrait:?}"
        );
        // … ses vrais pixels, non.
        assert!(!portrait.is_monochrome(mono, s.colour_min_share));
        let sepia = extract(
            &bands(&[
                ([40, 34, 28], 40),
                ([110, 95, 80], 30),
                ([180, 160, 140], 30),
            ]),
            &s,
        );
        assert!(sepia.is_monochrome(mono, s.colour_min_share));
    }

    #[test]
    fn grey_is_black_and_white_and_a_toning_is_not() {
        let s = PaletteSettings::default();
        let grey = extract(&bands(&[([30, 30, 30], 50), ([200, 200, 200], 50)]), &s);
        assert!(grey.is_grey(s.monochrome(), s.colour_min_share));
        let toned = extract(&bands(&[([60, 48, 38], 50), ([150, 118, 95], 50)]), &s);
        assert!(!toned.is_grey(s.monochrome(), s.colour_min_share));
    }

    #[test]
    fn a_small_highlight_is_found_with_the_colour_coverage() {
        // Retour du 21/11/2026 (vidéo) : un reflet orangé pipeté sur une photo violette ne
        // la retrouvait pas : moins de 2 % de l'image.
        let s = PaletteSettings::default();
        let highlight = [250u8, 170, 120];
        let image = bands(&[([60, 20, 160], 99), (highlight, 1)]);
        let palette = extract(&image, &s);
        let picked = working_to_oklab(highlight);
        assert!(
            palette
                .nearest(picked, s.chroma_weight, s.colour_min_share)
                .unwrap()
                > s.colour_radius,
            "à 2 %, pas trouvé"
        );
        assert!(
            palette
                .nearest(picked, s.chroma_weight, s.colour_coverage)
                .unwrap()
                < 0.04,
            "à 0,5 %, trouvé"
        );
    }

    #[test]
    fn keeping_similar_items_keeps_their_order() {
        let items = vec![
            ("a", Some(Some(0.3))),
            ("b", None),
            ("c", Some(None)),
            ("d", Some(Some(0.1))),
            ("e", Some(Some(0.9))),
        ];
        let kept = keep_similar(items.clone(), |i| i.1, 0.5);
        assert_eq!(
            kept.iter().map(|i| i.0).collect::<Vec<_>>(),
            ["a", "c", "d"]
        );
        let ranked = rank_similar(items, |i| i.1, 0.5);
        assert_eq!(
            ranked.iter().map(|i| i.0).collect::<Vec<_>>(),
            ["d", "a", "c"]
        );
    }

    #[test]
    fn the_average_colour_finds_the_colour_cast_only() {
        // Idée de l'utilisateur (23/11/2026) : une photo à dominante bleue avec un peu de
        // peau sort d'une recherche de la peau ; en couleur moyenne, non.
        let s = PaletteSettings::default();
        let (blue, skin) = ([40u8, 70, 150], [210u8, 160, 130]);
        let blue_cast = extract(&bands(&[(blue, 85), (skin, 15)]), &s);
        let portrait = extract(&bands(&[(skin, 85), (blue, 15)]), &s);
        let query = |c: [u8; 3], mean| ColourQuery {
            colours: vec![working_to_oklab(c)],
            radius: s.colour_radius,
            hierarchy: None,
            proximity: s.colour_proximity,
            widen: s.colour_widen,
            mean,
        };
        let cost =
            |p: &Palette, q: &ColourQuery| p.colours_cost(q, s.chroma_weight, s.colour_coverage);
        let ordinary = query(skin, None);
        assert!(cost(&blue_cast, &ordinary).is_some() && cost(&portrait, &ordinary).is_some());
        let mean = Some(s.mean_lightness);
        assert!(
            cost(&blue_cast, &query(skin, mean)).is_none(),
            "pas à dominante chair"
        );
        assert!(cost(&portrait, &query(skin, mean)).is_some());
        assert!(cost(&blue_cast, &query(blue, mean)).is_some());
        assert!(cost(&portrait, &query(blue, mean)).is_none());
        // Plus sombre, la même dominante (la clarté compte peu ; très sombre, un bleu est
        // aussi moins saturé en OKLab : la tolérance s'élargit au curseur).
        let dark = extract(&bands(&[([30u8, 50, 110], 100)]), &s);
        assert!(cost(&dark, &query([45u8, 75, 150], mean)).is_some());
        // La recherche ordinaire, qui compte la clarté pleinement, l'écarte.
        assert!(cost(&dark, &query([45u8, 75, 150], None)).is_none());
    }

    #[test]
    fn a_hierarchical_search_widens_the_tolerance_rank_by_rank() {
        // Idée de l'utilisateur (23/11/2026) : la première couleur plus précise que la
        // dernière.
        let s = PaletteSettings::default();
        let (red, blue) = ([200u8, 30, 30], [30u8, 40, 200]);
        let palette = extract(&bands(&[(red, 60), (blue, 40)]), &s);
        let red = working_to_oklab(red);
        // Un bleu un peu à côté : à 1,1 fois la tolérance.
        let mut off = working_to_oklab(blue);
        off[0] += 0.08;
        let d = palette
            .nearest(off, s.chroma_weight, s.colour_coverage)
            .unwrap();
        let radius = d / 1.1;
        let query = |hierarchy| ColourQuery {
            colours: vec![red, off],
            radius,
            hierarchy,
            proximity: s.colour_proximity,
            widen: s.colour_widen,
            mean: None,
        };
        let cost = |q: &ColourQuery| palette.colours_cost(q, s.chroma_weight, s.colour_coverage);
        assert!(cost(&query(None)).is_none(), "absolue : trop loin");
        assert!(
            cost(&query(Some(s.colour_hierarchy))).is_some(),
            "2ᵉ rang : ×1,25"
        );
        // Dans l'autre ordre, le bleu en premier garde la tolérance du curseur.
        let reversed = ColourQuery {
            colours: vec![off, red],
            ..query(Some(s.colour_hierarchy))
        };
        assert!(cost(&reversed).is_none());
    }

    #[test]
    fn posterize_shows_the_image_in_k_colours() {
        // #492 (23/11/2026) : l'image en k couleurs ; 1, sa couleur moyenne.
        let s = PaletteSettings::default();
        let image = bands(&[
            ([200u8, 30, 30], 60),
            ([30u8, 40, 200], 30),
            ([240u8, 240, 230], 10),
        ]);
        let distinct = |b: &Bitmap| {
            let mut set = std::collections::HashSet::new();
            for px in b.data.as_chunks::<3>().0 {
                set.insert(*px);
            }
            set.len()
        };
        for k in [1usize, 2, 3] {
            let (out, swatches) = posterize(&image, k, &s);
            assert_eq!((out.width, out.height), (image.width, image.height));
            assert_eq!(distinct(&out), k, "k = {k}");
            assert_eq!(swatches.len(), k);
            let total: f32 = swatches.iter().map(|w| w.share).sum();
            assert!((total - 1.0).abs() < 1e-4);
            assert!(swatches.windows(2).all(|w| w[0].share >= w[1].share));
        }
        // 3 couleurs : les vraies, rouge d'abord (60 %).
        let (_, swatches) = posterize(&image, 3, &s);
        assert!((swatches[0].share - 0.6).abs() < 0.05);
        // 1 : la moyenne, entre le rouge et le bleu.
        let (_, mean) = posterize(&image, 1, &s);
        let [r, _, b] = mean[0].srgb();
        assert!(r > 80 && b > 40, "{:?}", mean[0].srgb());
    }

    #[test]
    fn perceived_colours_remove_the_dominant_light() {
        // Expérience du 23/11/2026 : sous une lumière violette, un visage beige rosé paraît
        // orange. En couleurs perçues, la petite zone tire vers l'orange (moins de bleu :
        // b plus grand), et le fond violet perd de sa couleur.
        let s = PaletteSettings::default();
        let (violet, face) = ([60u8, 20, 150], [158u8, 123, 163]);
        let image = bands(&[(violet, 85), (face, 15)]);
        let (_, raw) = posterize(&image, 2, &s);
        let (_, seen) = posterize_with(&image, 2, &s, true);
        let small = |w: &[Swatch]| *w.iter().min_by(|a, b| a.share.total_cmp(&b.share)).unwrap();
        let large = |w: &[Swatch]| *w.iter().max_by(|a, b| a.share.total_cmp(&b.share)).unwrap();
        assert!(
            small(&seen).oklab[2] > small(&raw).oklab[2] + 0.05,
            "{raw:?} {seen:?}"
        );
        let chroma = |w: Swatch| w.hue_chroma().1;
        assert!(chroma(large(&seen)) < chroma(large(&raw)));
        // Perçues, sans réduction (k = 0) : l'image entière, adaptée.
        let (whole, palette) = posterize_with(&image, 0, &s, true);
        assert!(
            !palette.is_empty() && palette.len() <= s.colors,
            "sa palette perçue"
        );
        assert_eq!((whole.width, whole.height), (image.width, image.height));
    }

    #[test]
    fn several_colours_are_all_required() {
        // #492 : plusieurs couleurs cherchées ensemble ; une seule qui manque écarte.
        let s = PaletteSettings::default();
        let (red, blue, green) = ([200u8, 30, 30], [30u8, 40, 200], [30u8, 180, 40]);
        let palette = extract(&bands(&[(red, 60), (blue, 40)]), &s);
        let query = |colours: &[[u8; 3]], hierarchy| ColourQuery {
            colours: colours.iter().map(|&c| working_to_oklab(c)).collect(),
            radius: s.colour_radius,
            hierarchy,
            proximity: s.colour_proximity,
            widen: s.colour_widen,
            mean: None,
        };
        let cost = |q: &ColourQuery| palette.colours_cost(q, s.chroma_weight, s.colour_coverage);
        assert!(cost(&query(&[red, blue], None)).is_some());
        assert!(
            cost(&query(&[red, green], None)).is_none(),
            "le vert manque"
        );
        // Une couleur seule : sa distance, comme avant.
        assert_eq!(
            cost(&query(&[red], None)),
            palette.nearest(working_to_oklab(red), s.chroma_weight, s.colour_coverage)
        );
    }

    #[test]
    fn a_hierarchical_search_puts_the_first_colour_first() {
        // #492 : recherche hiérarchique, l'image où la 1ʳᵉ couleur est la plus présente
        // passe devant ; absolue, l'ordre des couleurs ne compte pas.
        let s = PaletteSettings::default();
        let (red, blue) = ([200u8, 30, 30], [30u8, 40, 200]);
        let mostly_red = extract(&bands(&[(red, 80), (blue, 20)]), &s);
        let mostly_blue = extract(&bands(&[(red, 20), (blue, 80)]), &s);
        let query = |colours: &[[u8; 3]], hierarchy| ColourQuery {
            colours: colours.iter().map(|&c| working_to_oklab(c)).collect(),
            radius: s.colour_radius,
            hierarchy,
            proximity: s.colour_proximity,
            widen: s.colour_widen,
            mean: None,
        };
        let cost = |p: &Palette, q: &ColourQuery| {
            p.colours_cost(q, s.chroma_weight, s.colour_coverage)
                .unwrap()
        };
        let red_first = query(&[red, blue], Some(s.colour_hierarchy));
        assert!(cost(&mostly_red, &red_first) < cost(&mostly_blue, &red_first));
        let blue_first = query(&[blue, red], Some(s.colour_hierarchy));
        assert!(cost(&mostly_blue, &blue_first) < cost(&mostly_red, &blue_first));
        let absolute = query(&[red, blue], None);
        let reversed = query(&[blue, red], None);
        for p in [&mostly_red, &mostly_blue] {
            assert!((cost(p, &absolute) - cost(p, &reversed)).abs() < 1e-6);
        }
    }

    #[test]
    fn a_barely_tinted_jpeg_grey_is_still_black_and_white() {
        // Retour du 22/11/2026 : les photos en noir et blanc sortaient d'une recherche de
        // couleur peu saturée ; leur gris à peine teinté tombait dans une case colorée.
        let s = PaletteSettings::default();
        let samples = vec![[0.5f32, 0.02, 0.017]; 100];
        let p = Palette {
            swatches: vec![Swatch {
                oklab: [0.5, 0.02, 0.017],
                share: 1.0,
            }],
            coverage: Coverage::from_samples(&samples),
        };
        let [_, a, b] = cover_centre(cover_bin(samples[0]));
        assert!(
            a.hypot(b) > s.mono_chroma,
            "centre de case au-dessus du seuil"
        );
        assert!(p.is_grey(s.monochrome(), s.colour_min_share));
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

//! Réglages de visualisation (issue n° 137) : exposition, balance des blancs et noir et
//! blanc par mélange des canaux, **pour comparer et trier une série**, pas pour développer.
//!
//! - Les réglages d'une image sont des **couches** ([`Layers`]) : une *correction* qui
//!   ramène l'image à une base commune (exposition harmonisée, balance neutre), calculée
//!   pour chaque image ; un *décalage créatif*, le même pour toute la série ; un *noir et
//!   blanc* (mélangeur). Refaire l'une ne perd pas les autres.
//! - **Appliquer à d'autres images** ([`apply_to`]) : chaque couche part avec son mode
//!   naturel ([`Modes::default`]) — correction d'exposition et de balance *harmonisées*
//!   (l'écart est calculé pour chaque image pour ressembler à la référence), décalage
//!   créatif et noir et blanc *copiés*. Chaque mode peut être forcé ([`Mode`]).
//! - **Où** : sur un RAW, exposition et balance sur les données du capteur
//!   ([`crate::raw::RenderOptions`], exact, hautes lumières récupérables) ; sur un JPEG
//!   (boîtier, vignette, image plate), une approximation sur l'image ([`apply_bitmap`] :
//!   le brûlé ne revient pas). Le mélangeur est exact partout (opération sur le RVB).
//! - **Mesures** ([`Measure`]) : luminance médiane (en linéaire) et dominante moyenne
//!   (rouge / vert, bleu / vert), sur l'image telle qu'elle est montrée, réglages compris :
//!   sur une image en noir et blanc, l'harmonisation travaille sur le noir et blanc.
//!
//! Le moteur ne fait que calculer ; l'enregistrement (sidecar XMP, espace `tri:`) et
//! l'interface viennent ensuite. Voir `doc/en/adjust.md`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::bitmap::Bitmap;
use crate::xmp::{self, Convention};

pub mod kelvin;

/// Section `[adjust]` du fichier de configuration : réglages de visualisation.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AdjustSettings {
    /// Exposition automatique : luminance médiane visée, en lumière linéaire (0,18 : le
    /// gris moyen des posemètres).
    pub auto_exposure_target: f32,
    /// Exposition automatique : compensation (IL), qui relève ensemble la médiane visée et
    /// le niveau des hautes lumières (celui-ci jamais au-delà de 0,95) ; retour du
    /// 10/11/2026 : « généralement un peu sombre ».
    pub auto_exposure_bias: f32,
    /// Exposition automatique : centile des hautes lumières (0,99 : le 1 % le plus clair).
    pub auto_exposure_highlights: f32,
    /// Exposition automatique : niveau (lumière linéaire) que ces hautes lumières ne
    /// dépassent pas en éclaircissant, et sous lequel elles ne descendent pas en
    /// assombrissant.
    pub auto_exposure_highlight_target: f32,
    /// Balance auto : un pixel compte comme « presque neutre » si l'écart entre ses
    /// canaux reste sous cette part du plus fort (0,2 : 20 %).
    pub auto_balance_chroma: f32,
    /// Balance auto : passes (ce qui est neutre change avec la correction).
    pub auto_balance_passes: usize,
    /// Préréglages de balance des blancs, en kelvins (teinte nulle).
    pub daylight: f32,
    /// Voir `daylight`.
    pub cloudy: f32,
    /// Voir `daylight`.
    pub shade: f32,
    /// Voir `daylight`.
    pub tungsten: f32,
    /// Voir `daylight`.
    pub fluorescent: f32,
    /// Voir `daylight`.
    pub flash: f32,
}

impl Default for AdjustSettings {
    fn default() -> Self {
        AdjustSettings {
            auto_exposure_target: 0.18,
            auto_exposure_bias: 0.5,
            auto_exposure_highlights: 0.99,
            auto_exposure_highlight_target: 0.6,
            auto_balance_chroma: 0.2,
            auto_balance_passes: 4,
            daylight: 5500.0,
            cloudy: 6500.0,
            shade: 7500.0,
            tungsten: 2850.0,
            fluorescent: 3800.0,
            flash: 5500.0,
        }
    }
}

impl AdjustSettings {
    /// Préréglages de balance en kelvins, dans l'ordre de la liste : (identifiant,
    /// kelvins).
    pub fn white_balance_presets(&self) -> [(&'static str, f32); 6] {
        [
            ("daylight", self.daylight),
            ("cloudy", self.cloudy),
            ("shade", self.shade),
            ("tungsten", self.tungsten),
            ("fluorescent", self.fluorescent),
            ("flash", self.flash),
        ]
    }
}

/// Gains rouge et bleu de la balance des blancs, relatifs à la balance de base de l'image
/// (celle de la prise de vue) : `1.0` partout, rien. Le vert sert de référence.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gains {
    /// Gain du rouge.
    pub red: f32,
    /// Gain du bleu.
    pub blue: f32,
}

impl Default for Gains {
    fn default() -> Self {
        Gains {
            red: 1.0,
            blue: 1.0,
        }
    }
}

impl Gains {
    /// Aucun changement.
    pub const NONE: Gains = Gains {
        red: 1.0,
        blue: 1.0,
    };

    /// Les deux balances l'une après l'autre.
    pub fn then(self, other: Gains) -> Gains {
        Gains {
            red: self.red * other.red,
            blue: self.blue * other.blue,
        }
    }

    /// L'inverse (annule `self`).
    pub fn inverse(self) -> Gains {
        Gains {
            red: 1.0 / self.red,
            blue: 1.0 / self.blue,
        }
    }

    /// Vrai si rien ne change (à un millième près).
    pub fn is_none(self) -> bool {
        (self.red - 1.0).abs() < 1e-3 && (self.blue - 1.0).abs() < 1e-3
    }

    /// Écart en « plus chaud / plus froid », en IL : log₂(rouge / bleu). Positif : plus
    /// chaud. Pour le décalage créatif, relatif ; la correction se montre en kelvins
    /// ([`kelvin`]).
    pub fn warmth(self) -> f32 {
        (self.red / self.blue).log2()
    }

    /// Écart vers le magenta (positif) ou le vert (négatif), en IL : rouge et bleu
    /// ensemble face au vert.
    pub fn tint(self) -> f32 {
        (self.red * self.blue).log2() / 2.0
    }

    /// Gains à partir d'un écart de chaleur et de teinte (inverse de
    /// [`Gains::warmth`] / [`Gains::tint`]) : les curseurs de l'interface.
    pub fn from_warmth_tint(warmth: f32, tint: f32) -> Gains {
        Gains {
            red: (tint + warmth / 2.0).exp2(),
            blue: (tint - warmth / 2.0).exp2(),
        }
    }
}

/// Une couche d'exposition et de balance (et, pour le style, de contraste et de
/// saturation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Adjust {
    /// Correction d'exposition (IL).
    pub exposure: f32,
    /// Balance des blancs.
    pub gains: Gains,
    /// Contraste, de -1 à 1 (retour du 10/11/2026, dans le style) : une courbe en lumière
    /// linéaire pivotée sur le gris moyen, appliquée à la luminance (teinte gardée).
    #[serde(skip_serializing_if = "is_zero")]
    pub contrast: f32,
    /// Saturation, de -1 (gris) à 1, autour de la luminance.
    #[serde(skip_serializing_if = "is_zero")]
    pub saturation: f32,
}

fn is_zero(v: &f32) -> bool {
    v.abs() < 1e-6
}

/// Pivot du contraste : le gris moyen, qui ne bouge pas.
const CONTRAST_PIVOT: f32 = 0.18;
/// Force du contraste : à 1, les écarts au gris moyen (en IL) sont multipliés par
/// 1 + cette valeur ; à -1, par 1 - cette valeur.
const CONTRAST_STRENGTH: f32 = 0.6;

impl Adjust {
    /// Les deux couches l'une après l'autre (expositions, contrastes et saturations
    /// ajoutés, gains multipliés).
    pub fn then(self, other: Adjust) -> Adjust {
        Adjust {
            exposure: self.exposure + other.exposure,
            gains: self.gains.then(other.gains),
            contrast: self.contrast + other.contrast,
            saturation: self.saturation + other.saturation,
        }
    }

    /// Vrai si la couche ne change rien.
    pub fn is_none(self) -> bool {
        self.exposure.abs() < 1e-3
            && self.gains.is_none()
            && self.contrast.abs() < 1e-3
            && self.saturation.abs() < 1e-3
    }
}

/// Mélange des canaux pour le noir et blanc : part du rouge, du vert et du bleu (1.0 =
/// 100 %). La somme n'est pas imposée (au-dessus de 1, l'image s'éclaircit).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mixer {
    /// Part du rouge.
    pub red: f32,
    /// Part du vert.
    pub green: f32,
    /// Part du bleu.
    pub blue: f32,
    /// Normalisé : les parts sont ramenées à une somme de 100 % à l'application (la
    /// luminosité ne bouge pas quand on change le mélange). Par défaut ; écrit seulement
    /// quand il est coupé.
    #[serde(skip_serializing_if = "is_true")]
    pub normalise: bool,
}

fn is_true(v: &bool) -> bool {
    *v
}

impl Default for Mixer {
    /// La luminance (Rec. 709) : un noir et blanc « neutre », normalisé.
    fn default() -> Self {
        Mixer {
            red: 0.2126,
            green: 0.7152,
            blue: 0.0722,
            normalise: true,
        }
    }
}

impl Mixer {
    /// Préréglages façon filtres de couleur devant l'objectif (noir et blanc argentique).
    pub fn preset(name: &str) -> Option<Mixer> {
        let (red, green, blue) = match name {
            "neutral" => return Some(Mixer::default()),
            "red" => (0.9, 0.2, -0.1),
            "orange" => (0.6, 0.4, 0.0),
            "yellow" => (0.45, 0.5, 0.05),
            "green" => (0.1, 0.8, 0.1),
            "blue" => (0.0, 0.3, 0.7),
            _ => return None,
        };
        Some(Mixer {
            red,
            green,
            blue,
            normalise: true,
        })
    }

    /// Préréglages proposés, dans l'ordre.
    pub const PRESETS: [&'static str; 6] = ["neutral", "red", "orange", "yellow", "green", "blue"];

    /// Somme des parts.
    pub fn sum(self) -> f32 {
        self.red + self.green + self.blue
    }

    /// Les parts appliquées (rouge, vert, bleu) : ramenées à 100 % si normalisé (une somme
    /// presque nulle reste telle quelle).
    pub fn weights(self) -> [f32; 3] {
        let sum = self.sum();
        let k = if self.normalise && sum.abs() > 1e-3 {
            1.0 / sum
        } else {
            1.0
        };
        [self.red * k, self.green * k, self.blue * k]
    }

    /// Même mélange (aux arrondis près), normalisé ou non.
    pub fn same_mix(self, other: Mixer) -> bool {
        let close = |a: f32, b: f32| (a - b).abs() < 5e-4;
        close(self.red, other.red) && close(self.green, other.green) && close(self.blue, other.blue)
    }

    /// Bornes de chaque part (-100 % à 200 %).
    pub const RANGE: (f32, f32) = (-1.0, 2.0);

    /// Les parts (rouge, vert, bleu).
    pub fn parts(self) -> [f32; 3] {
        [self.red, self.green, self.blue]
    }

    /// Avec ces parts.
    pub fn with_parts(self, [red, green, blue]: [f32; 3]) -> Mixer {
        Mixer {
            red,
            green,
            blue,
            ..self
        }
    }

    /// Ramené à 100 % (cocher « normalisé » sur un mélange qui n'y est pas), proportions
    /// gardées ; une somme presque nulle reste telle quelle.
    pub fn rescaled(self) -> Mixer {
        let sum = self.sum();
        if sum.abs() < 1e-3 {
            return self;
        }
        self.with_parts(self.parts().map(|p| p / sum))
    }

    /// La part `channel` (0 rouge, 1 vert, 2 bleu) mise à `value` **sans changer le
    /// total** (mode normalisé, retour du 10/11/2026) : l'écart est pris sur les deux autres
    /// au prorata de leurs parts (moitié chacune si elles sont nulles ou de signes
    /// opposés). Aux bornes ([`Mixer::RANGE`]), la part demandée s'arrête là où les autres
    /// ne peuvent plus suivre.
    pub fn set_locked(self, channel: usize, value: f32) -> Mixer {
        let parts = self.parts();
        let (lo, hi) = Self::RANGE;
        let (i, j) = match channel {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let total = self.sum();
        let place = |v: f32| -> Option<[f32; 3]> {
            let (oi, oj) = (parts[i], parts[j]);
            let rest = total - v;
            let s = oi + oj;
            let (ni, nj) = if oi * oj >= 0.0 && s.abs() > 1e-4 {
                (oi / s * rest, oj / s * rest)
            } else {
                let d = (rest - s) / 2.0;
                (oi + d, oj + d)
            };
            let ok = |x: f32| (lo - 1e-4..=hi + 1e-4).contains(&x);
            let mut out = parts;
            out[channel] = v;
            out[i] = ni;
            out[j] = nj;
            (ok(ni) && ok(nj)).then_some(out)
        };
        let value = value.clamp(lo, hi);
        if let Some(out) = place(value) {
            return self.with_parts(out);
        }
        // Le plus loin possible vers `value`.
        let (mut good, mut bad) = (parts[channel], value);
        if place(good).is_none() {
            return self;
        }
        for _ in 0..40 {
            let mid = (good + bad) / 2.0;
            if place(mid).is_some() {
                good = mid;
            } else {
                bad = mid;
            }
        }
        place(good).map_or(self, |out| self.with_parts(out))
    }

    /// Le préréglage qui a ce mélange, s'il y en a un.
    pub fn preset_name(self) -> Option<&'static str> {
        Self::PRESETS
            .into_iter()
            .find(|p| Mixer::preset(p).is_some_and(|m| m.same_mix(self)))
    }
}

/// Les réglages d'une image. Enregistrés dans son sidecar XMP, en JSON dans
/// `tri:Adjust` ([`read`], [`write`]) ; illisibles par les autres logiciels (assumé :
/// leurs réglages sont internes et propres à chacun).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layers {
    /// Correction : base commune (exposition harmonisée, balance neutre), propre à
    /// l'image.
    pub correction: Adjust,
    /// Décalage créatif, le même pour la série, s'il y en a un (case de l'interface,
    /// comme le noir et blanc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creative: Option<Adjust>,
    /// Noir et blanc, s'il y en a un.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mono: Option<Mixer>,
}

impl Layers {
    /// Exposition et balance au total (correction puis décalage créatif).
    pub fn total(&self) -> Adjust {
        self.correction.then(self.creative.unwrap_or_default())
    }

    /// Vrai si l'image n'a aucun réglage.
    pub fn is_none(&self) -> bool {
        self.correction.is_none() && self.creative.is_none() && self.mono.is_none()
    }

    /// Éléments de `tri:Adjust` : un seul, en JSON ; aucun sans réglage.
    pub fn items(layers: Option<&Layers>) -> Vec<String> {
        layers
            .filter(|l| !l.is_none())
            .map(|l| serde_json::to_string(l).expect("réglages sérialisables"))
            .into_iter()
            .collect()
    }

    /// Depuis les éléments de `tri:Adjust` (le premier lisible).
    pub fn from_items(items: &[String]) -> Option<Layers> {
        items
            .iter()
            .find_map(|i| serde_json::from_str::<Layers>(i).ok())
            .filter(|l| !l.is_none())
    }
}

/// Réglages de `image`, lus dans son sidecar (aucun sans sidecar).
pub fn read(image: &Path) -> Result<Option<Layers>, xmp::XmpError> {
    Ok(Layers::from_items(&xmp::read_list_of(image, xmp::ADJUST)?))
}

/// Écrit les réglages de `image` dans son sidecar (créé selon `default` s'il le faut ;
/// aucun réglage : la liste est retirée). Seul `tri:Adjust` change. Renvoie les éléments
/// d'avant et d'après, pour l'historique ([`crate::history::Step::Adjust`]).
pub fn write(
    image: &Path,
    layers: Option<&Layers>,
    default: Convention,
) -> Result<crate::history::AdjustChange, xmp::XmpError> {
    let before = xmp::read_list_of(image, xmp::ADJUST)?;
    let after = Layers::items(layers);
    if before != after {
        xmp::write_list(image, xmp::ADJUST, &after, default)?;
    }
    Ok(crate::history::AdjustChange {
        image: PathBuf::from(image),
        before,
        after,
    })
}

/// Couleur de l'éclairage de la prise de vue de `image` (sRGB linéaire, vert = 1), base
/// de ses kelvins : lue par LibRaw pour un RAW (dès l'ouverture, sans décompresser),
/// [`kelvin::D65`] pour une image plate ou un RAW sans balance connue.
pub fn shot_white(image: &Path, kind: crate::media::MediaKind) -> [f32; 3] {
    if kind != crate::media::MediaKind::Raw {
        return kelvin::D65;
    }
    crate::raw::RawFile::open(image)
        .ok()
        .and_then(|raw| raw.as_shot_white())
        .unwrap_or(kelvin::D65)
}

/// Courbe d'une image 8 bits selon sa source : un rendu LibRaw (aperçu standard ou 1:1 d'un
/// RAW) est en BT.709, tout le reste (JPEG boîtier, vignette, image plate) en sRGB.
pub fn transfer_of(raw: bool, level: crate::cache::Level) -> Transfer {
    use crate::cache::Level;
    if raw && matches!(level, Level::Standard | Level::Full) {
        Transfer::Bt709
    } else {
        Transfer::Srgb
    }
}

/// Comment une couche part vers une autre image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// La valeur telle quelle (+0,7 IL, mêmes gains).
    Absolute,
    /// Le même écart ajouté à ce que l'image a déjà.
    Offset,
    /// L'écart calculé pour que l'image ressemble à la référence.
    Harmonise,
    /// Rien : l'image garde la sienne.
    Keep,
}

/// Le mode de chaque couche (voir [`apply_to`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modes {
    /// Exposition de la correction.
    pub exposure: Mode,
    /// Balance de la correction.
    pub white_balance: Mode,
    /// Décalage créatif (exposition et balance).
    pub creative: Mode,
    /// Noir et blanc ([`Mode::Absolute`] : le même mélange ; [`Mode::Keep`] : celui de
    /// l'image ; les autres modes valent [`Mode::Absolute`]).
    pub mono: Mode,
}

impl Default for Modes {
    /// « Appliquer à la sélection » intelligent : corrections harmonisées, décalage
    /// créatif et noir et blanc copiés.
    fn default() -> Self {
        Modes {
            exposure: Mode::Harmonise,
            white_balance: Mode::Harmonise,
            creative: Mode::Absolute,
            mono: Mode::Absolute,
        }
    }
}

/// Courbe de tonalité d'une image 8 bits : pour revenir à la lumière linéaire, où
/// l'exposition est un simple facteur. Se tromper de courbe fausse la mesure dans les
/// ombres (essai du 10/11/2026 : +1,18 IL au lieu de +1 sur une photo sombre).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    /// sRGB : JPEG du boîtier, images plates.
    Srgb,
    /// BT.709 : rendus de LibRaw (`OutputSpace::PREVIEW`, gamma 0,45 et pente 4,5).
    Bt709,
}

impl Transfer {
    /// Valeur codée (0..1) vers linéaire.
    fn decode(self, c: f32) -> f32 {
        match self {
            Transfer::Srgb if c <= 0.04045 => c / 12.92,
            Transfer::Srgb => ((c + 0.055) / 1.055).powf(2.4),
            Transfer::Bt709 if c < 0.081 => c / 4.5,
            Transfer::Bt709 => ((c + 0.099) / 1.099).powf(1.0 / 0.45),
        }
    }

    /// Linéaire (0..1) vers valeur codée.
    fn encode(self, c: f32) -> f32 {
        match self {
            Transfer::Srgb if c <= 0.003_130_8 => c * 12.92,
            Transfer::Srgb => 1.055 * c.powf(1.0 / 2.4) - 0.055,
            Transfer::Bt709 if c < 0.018 => c * 4.5,
            Transfer::Bt709 => 1.099 * c.powf(0.45) - 0.099,
        }
    }

    /// Table 8 bits vers linéaire.
    fn to_linear(self) -> [f32; 256] {
        std::array::from_fn(|i| self.decode(i as f32 / 255.0))
    }

    /// Table linéaire (0..1, par pas de 1 / 4095) vers 8 bits.
    fn to_coded(self) -> Vec<u8> {
        (0..OUT_STEPS)
            .map(|i| {
                let c = self.encode(i as f32 / (OUT_STEPS - 1) as f32);
                (c * 255.0).round().clamp(0.0, 255.0) as u8
            })
            .collect()
    }
}

/// Une image sans réglage et sa courbe : la base des mesures.
#[derive(Clone, Debug, PartialEq)]
pub struct Base {
    /// Les pixels.
    pub bitmap: Bitmap,
    /// Leur courbe.
    pub transfer: Transfer,
}

/// Pas de la table linéaire vers 8 bits.
const OUT_STEPS: usize = 4096;

fn encode(table: &[u8], v: f32) -> u8 {
    let i = (v.clamp(0.0, 1.0) * (OUT_STEPS - 1) as f32).round() as usize;
    table[i]
}

/// Applique `adjust` (exposition et balance) puis `mono` à une image 8 bits, en passant
/// par le linéaire. Sur un JPEG, c'est une approximation : sa courbe de tonalité est déjà
/// appliquée et ce qui est brûlé ne revient pas. Sur un rendu RAW déjà réglé par LibRaw,
/// passer seulement le reste (exposition au-delà de ses bornes, noir et blanc).
pub fn apply_bitmap(bitmap: &mut Bitmap, transfer: Transfer, adjust: Adjust, mono: Option<Mixer>) {
    if adjust.is_none() && mono.is_none() {
        return;
    }
    let lin = transfer.to_linear();
    let out = transfer.to_coded();
    let k = adjust.exposure.exp2();
    let (kr, kg, kb) = (k * adjust.gains.red, k, k * adjust.gains.blue);
    let mono = mono.map(Mixer::weights);
    let saturation = 1.0 + adjust.saturation.clamp(-1.0, 1.0);
    let colour = (saturation - 1.0).abs() > 1e-3;
    let contrast = Contrast::new(adjust.contrast);
    for px in bitmap.data.as_chunks_mut::<3>().0 {
        let mut r = lin[px[0] as usize] * kr;
        let mut g = lin[px[1] as usize] * kg;
        let mut b = lin[px[2] as usize] * kb;
        match mono {
            Some([mr, mg, mb]) => {
                let y = mr * r + mg * g + mb * b;
                let y = encode(&out, y * contrast.factor(y));
                *px = [y, y, y];
            }
            None => {
                let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                if colour {
                    r = (y + (r - y) * saturation).max(0.0);
                    g = (y + (g - y) * saturation).max(0.0);
                    b = (y + (b - y) * saturation).max(0.0);
                }
                let f = contrast.factor(y);
                px[0] = encode(&out, r * f);
                px[1] = encode(&out, g * f);
                px[2] = encode(&out, b * f);
            }
        }
    }
}

/// Le contraste d'une couche : le facteur à appliquer à un pixel de luminance `y`
/// (lumière linéaire), tiré d'une table (une puissance par pixel coûterait trop sur les
/// grandes images).
struct Contrast {
    exponent: f32,
    table: Option<Vec<f32>>,
}

impl Contrast {
    /// Cases de la table, sur la luminance 0..1.
    const STEPS: usize = 4096;

    fn new(contrast: f32) -> Contrast {
        let exponent = 1.0 + CONTRAST_STRENGTH * contrast.clamp(-1.0, 1.0);
        let table = ((exponent - 1.0).abs() > 1e-3).then(|| {
            (0..Self::STEPS)
                .map(|i| Self::exact(exponent, (i as f32 + 0.5) / Self::STEPS as f32))
                .collect()
        });
        Contrast { exponent, table }
    }

    /// `y' / y` pour la courbe `y' = pivot × (y / pivot)^exposant`.
    fn exact(exponent: f32, y: f32) -> f32 {
        if y <= 1e-6 {
            return if exponent > 1.0 { 0.0 } else { 1.0 };
        }
        CONTRAST_PIVOT * (y / CONTRAST_PIVOT).powf(exponent) / y
    }

    fn factor(&self, y: f32) -> f32 {
        match &self.table {
            None => 1.0,
            Some(_) if y >= 1.0 => Self::exact(self.exponent, y),
            Some(table) => table[((y.max(0.0) * Self::STEPS as f32) as usize).min(Self::STEPS - 1)],
        }
    }
}

/// Une zone de l'image (en fractions de sa largeur et de sa hauteur, 0..1) : la pipette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    /// Bord gauche.
    pub x: f32,
    /// Bord haut.
    pub y: f32,
    /// Largeur.
    pub w: f32,
    /// Hauteur.
    pub h: f32,
}

/// Ce qu'on mesure d'une image pour l'harmoniser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// Luminance médiane (Rec. 709, linéaire, 0..1).
    pub luminance: f32,
    /// Dominante : moyennes linéaires rouge / vert et bleu / vert, sur les pixels ni
    /// bouchés ni brûlés.
    pub cast: [f32; 2],
}

/// Pixels écartés de la dominante : trop sombres (bruit) ou presque brûlés (un canal
/// écrêté fausse le rapport).
const CAST_RANGE: (u8, u8) = (8, 247);

/// Mesure `bitmap`, de courbe `transfer` (sur `zone` seulement pour la dominante, s'il y
/// en a une).
pub fn measure(bitmap: &Bitmap, transfer: Transfer, zone: Option<Zone>) -> Measure {
    let lin = transfer.to_linear();
    let mut histogram = vec![0u32; 4096];
    let (mut sr, mut sg, mut sb) = (0.0f64, 0.0f64, 0.0f64);
    let (w, h) = (bitmap.width as usize, bitmap.height as usize);
    let (x0, y0, x1, y1) = match zone {
        Some(z) => (
            ((z.x * w as f32) as usize).min(w),
            ((z.y * h as f32) as usize).min(h),
            (((z.x + z.w) * w as f32).ceil() as usize).clamp(1, w),
            (((z.y + z.h) * h as f32).ceil() as usize).clamp(1, h),
        ),
        None => (0, 0, w, h),
    };
    for (i, px) in bitmap.data.as_chunks::<3>().0.iter().enumerate() {
        let (r, g, b) = (
            lin[px[0] as usize],
            lin[px[1] as usize],
            lin[px[2] as usize],
        );
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        histogram[((y * 4095.0).round() as usize).min(4095)] += 1;
        let (x, row) = (i % w, i / w);
        if x >= x0 && x < x1 && row >= y0 && row < y1 {
            let lo = px.iter().min().copied().unwrap_or(0);
            let hi = px.iter().max().copied().unwrap_or(0);
            if lo >= CAST_RANGE.0 && hi <= CAST_RANGE.1 {
                sr += f64::from(r);
                sg += f64::from(g);
                sb += f64::from(b);
            }
        }
    }
    let total: u32 = histogram.iter().sum();
    let mut seen = 0;
    let mut median = 0;
    for (i, n) in histogram.iter().enumerate() {
        seen += n;
        if seen * 2 >= total {
            median = i;
            break;
        }
    }
    let cast = if sg > 0.0 {
        [(sr / sg) as f32, (sb / sg) as f32]
    } else {
        [1.0, 1.0]
    };
    Measure {
        // Une image toute noire : une médiane minuscule plutôt que 0 (log₂).
        luminance: (median as f32 / 4095.0).max(1.0 / 4095.0),
        cast,
    }
}

/// Gains qui neutralisent la dominante `cast` (pipette sur une zone grise, ou « monde
/// gris » sur toute l'image).
pub fn neutral(cast: [f32; 2]) -> Gains {
    Gains {
        red: 1.0 / cast[0].max(1e-3),
        blue: 1.0 / cast[1].max(1e-3),
    }
}

/// Balance auto sur les **pixels presque neutres** (retour du 10/11/2026 : le « monde
/// gris », la moyenne de toute l'image, refroidissait un portrait au visage et au pull
/// chauds, R/V = 1,33). Ne comptent que les pixels peu saturés (`auto_balance_chroma`),
/// ni bouchés ni presque brûlés ; on les neutralise, puis on recommence
/// (`auto_balance_passes`), ce qui est neutre changeant avec la correction. Moins de 1 %
/// de pixels neutres : le monde gris, faute de mieux.
pub fn auto_balance(base: &Base, settings: &AdjustSettings) -> Gains {
    let lin = base.transfer.to_linear();
    let pixels = base.bitmap.data.as_chunks::<3>().0;
    let usable = pixels
        .iter()
        .filter(|px| {
            let (lo, hi) = (
                px.iter().min().copied().unwrap_or(0),
                px.iter().max().copied().unwrap_or(0),
            );
            lo >= CAST_RANGE.0 && hi <= CAST_RANGE.1
        })
        .count();
    let grey_world = || neutral(measure(&base.bitmap, base.transfer, None).cast);
    let chroma = settings.auto_balance_chroma.clamp(0.01, 1.0);
    let mut gains = Gains::NONE;
    for pass in 0..settings.auto_balance_passes.max(1) {
        let (mut sr, mut sg, mut sb, mut n) = (0.0f64, 0.0f64, 0.0f64, 0usize);
        for px in pixels {
            let (lo, hi) = (
                px.iter().min().copied().unwrap_or(0),
                px.iter().max().copied().unwrap_or(0),
            );
            if lo < CAST_RANGE.0 || hi > CAST_RANGE.1 {
                continue;
            }
            let r = lin[px[0] as usize] * gains.red;
            let g = lin[px[1] as usize];
            let b = lin[px[2] as usize] * gains.blue;
            let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
            if mx <= 0.0 || (mx - mn) / mx > chroma {
                continue;
            }
            sr += f64::from(r);
            sg += f64::from(g);
            sb += f64::from(b);
            n += 1;
        }
        if n * 100 < usable.max(1) || sg <= 0.0 {
            return if pass == 0 { grey_world() } else { gains };
        }
        gains = gains.then(neutral([(sr / sg) as f32, (sb / sg) as f32]));
    }
    gains
}

/// Mesure d'une image avec des réglages.
fn measure_adjusted(base: &Base, adjust: Adjust, mono: Option<Mixer>) -> Measure {
    let mut copy = base.bitmap.clone();
    apply_bitmap(&mut copy, base.transfer, adjust, mono);
    measure(&copy, base.transfer, None)
}

/// Les réglages de `reference` appliqués à une autre image, selon `modes`.
///
/// `reference_base` et `target_base` : les deux images **sans réglage**, à une petite
/// taille (une vignette suffit) ; `target` : les réglages qu'a déjà la cible.
///
/// Harmonisation de la balance : la cible prend la dominante de la référence corrigée
/// (sa correction de balance appliquée). Harmonisation de l'exposition : la luminance
/// médiane de la cible, avec tous ses nouveaux réglages, égale celle de la référence avec
/// les siens. L'exposition étant un simple facteur en linéaire, un seul calcul suffit
/// (aux pixels écrêtés près).
pub fn apply_to(
    reference: &Layers,
    reference_base: &Base,
    target: &Layers,
    target_base: &Base,
    modes: Modes,
) -> Layers {
    let mut out = *target;
    // Décalage créatif et noir et blanc d'abord : ils comptent dans la mesure.
    out.creative = match modes.creative {
        Mode::Absolute | Mode::Harmonise => reference.creative,
        Mode::Offset => match (target.creative, reference.creative) {
            (Some(t), Some(r)) => Some(t.then(r)),
            (t, r) => t.or(r),
        },
        Mode::Keep => target.creative,
    };
    if modes.mono != Mode::Keep {
        out.mono = reference.mono;
    }
    out.correction.gains = match modes.white_balance {
        Mode::Absolute => reference.correction.gains,
        Mode::Offset => target.correction.gains.then(reference.correction.gains),
        Mode::Keep => target.correction.gains,
        Mode::Harmonise => {
            let wanted = measure_adjusted(
                reference_base,
                Adjust {
                    exposure: 0.0,
                    gains: reference.correction.gains,
                    ..Adjust::default()
                },
                None,
            )
            .cast;
            let have = measure(&target_base.bitmap, target_base.transfer, None).cast;
            Gains {
                red: wanted[0] / have[0].max(1e-3),
                blue: wanted[1] / have[1].max(1e-3),
            }
        }
    };
    out.correction.exposure = match modes.exposure {
        Mode::Absolute => reference.correction.exposure,
        Mode::Offset => target.correction.exposure + reference.correction.exposure,
        Mode::Keep => target.correction.exposure,
        Mode::Harmonise => {
            let wanted = measure_adjusted(reference_base, reference.total(), reference.mono);
            let without = Layers {
                correction: Adjust {
                    exposure: 0.0,
                    gains: out.correction.gains,
                    ..Adjust::default()
                },
                ..out
            };
            let have = measure_adjusted(target_base, without.total(), without.mono);
            (wanted.luminance / have.luminance).log2()
        }
    };
    out
}

/// Un geste fait sur l'image de référence (réglages `before` → `after`, prise de vue
/// `reference_shot`) reporté sur une autre image de la sélection (`target`, prise de vue
/// `target_shot`), retours du 10/11/2026 : **seul ce qui a changé** part, et
/// **relativement** à ce que l'image a déjà :
///
/// - exposition (correction et style) : le même écart en IL ;
/// - température de la correction : le même rapport en kelvins (règle de 3, chacune dans
///   ses kelvins), teinte : le même écart ;
/// - balance du style : le même écart (gains) ;
/// - décalage créatif activé ou coupé, noir et blanc : copiés.
pub fn propagate(
    before: &Layers,
    after: &Layers,
    reference_shot: [f32; 3],
    target: &Layers,
    target_shot: [f32; 3],
) -> Layers {
    let mut out = *target;
    let (b, a) = (before.correction, after.correction);
    if (a.exposure - b.exposure).abs() > 1e-6 {
        out.correction.exposure = target.correction.exposure + a.exposure - b.exposure;
    }
    if a.gains != b.gains {
        let (kb, tb) = kelvin::of_gains(reference_shot, b.gains);
        let (ka, ta) = kelvin::of_gains(reference_shot, a.gains);
        let (k, t) = kelvin::of_gains(target_shot, target.correction.gains);
        let k = (k * ka / kb.max(1.0)).clamp(kelvin::KELVIN_MIN, kelvin::KELVIN_MAX);
        out.correction.gains = kelvin::gains(target_shot, k, t + ta - tb);
    }
    if before.creative != after.creative {
        out.creative = after.creative.map(|a| {
            let b = before.creative.unwrap_or_default();
            let mine = target.creative.unwrap_or_default();
            Adjust {
                exposure: mine.exposure + a.exposure - b.exposure,
                gains: mine.gains.then(b.gains.inverse()).then(a.gains),
                contrast: (mine.contrast + a.contrast - b.contrast).clamp(-1.0, 1.0),
                saturation: (mine.saturation + a.saturation - b.saturation).clamp(-1.0, 1.0),
            }
        });
    }
    if before.mono != after.mono {
        out.mono = after.mono;
    }
    out
}

/// Exposition automatique, **à priorité aux hautes lumières**, compensée de
/// `auto_exposure_bias` (les deux niveaux visés relevés d'autant) (retour du 10/11/2026 : sur
/// un portrait en low-key, pull et fond sombres, viser la médiane donnait +3,3 IL et un
/// visage brûlé) :
///
/// - visée : la luminance médiane de l'image (sans réglage, sa balance corrigée) vers
///   `auto_exposure_target` (0,18, le gris moyen des posemètres) ;
/// - pour éclaircir, on s'arrête quand les hautes lumières (le centile
///   `auto_exposure_highlights`) atteignent `auto_exposure_highlight_target` ;
/// - pour assombrir, on s'arrête quand elles y descendent (une scène de neige ne devient
///   pas grise).
pub fn auto_exposure(base: &Base, gains: Gains, settings: &AdjustSettings) -> f32 {
    let mut copy = base.bitmap.clone();
    apply_bitmap(
        &mut copy,
        base.transfer,
        Adjust {
            exposure: 0.0,
            gains,
            ..Adjust::default()
        },
        None,
    );
    let histogram = luminance_histogram(&copy, base.transfer);
    let median = percentile(&histogram, 0.5);
    let high = percentile(
        &histogram,
        settings.auto_exposure_highlights.clamp(0.5, 1.0),
    );
    let bias = settings.auto_exposure_bias.clamp(-3.0, 3.0).exp2();
    let wanted = (settings.auto_exposure_target.max(1e-4) * bias / median).log2();
    let ceiling = (settings.auto_exposure_highlight_target * bias).clamp(1e-3, HIGHLIGHT_MAX);
    let room = (ceiling / high).log2();
    if wanted > 0.0 {
        wanted.min(room.max(0.0))
    } else {
        wanted.max(room.min(0.0))
    }
}

/// Niveau des hautes lumières au plus après la compensation de l'exposition automatique
/// (sous le blanc : rien ne brûle).
const HIGHLIGHT_MAX: f32 = 0.95;

/// Histogramme de la luminance linéaire (Rec. 709) de `bitmap`, en 4096 cases.
fn luminance_histogram(bitmap: &Bitmap, transfer: Transfer) -> Vec<u32> {
    let lin = transfer.to_linear();
    let mut histogram = vec![0u32; 4096];
    for px in bitmap.data.as_chunks::<3>().0 {
        let y = 0.2126 * lin[px[0] as usize]
            + 0.7152 * lin[px[1] as usize]
            + 0.0722 * lin[px[2] as usize];
        histogram[((y * 4095.0).round() as usize).min(4095)] += 1;
    }
    histogram
}

/// Centile `p` (0..1) d'un histogramme de [`luminance_histogram`], en lumière linéaire
/// (jamais tout à fait 0 : il sert de diviseur).
fn percentile(histogram: &[u32], p: f32) -> f32 {
    let total: u64 = histogram.iter().map(|&n| u64::from(n)).sum();
    let wanted = (total as f64 * f64::from(p)).ceil().max(1.0) as u64;
    let mut seen = 0u64;
    for (i, &n) in histogram.iter().enumerate() {
        seen += u64::from(n);
        if seen >= wanted {
            return (i as f32 / 4095.0).max(1.0 / 4095.0);
        }
    }
    1.0
}

/// Le rapport d'une image : les valeurs à reporter à la main dans un logiciel de
/// développement (première étape, avant d'écrire dans leurs formats).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Report {
    /// Correction d'exposition (IL).
    pub correction_exposure: f32,
    /// Décalage créatif d'exposition (IL).
    pub creative_exposure: f32,
    /// Exposition totale à reporter (IL).
    pub exposure: f32,
    /// Balance de la prise de vue : kelvins et teinte ([`kelvin`]).
    pub shot_kelvin: (f32, f32),
    /// Balance totale (correction puis décalage créatif) : kelvins et teinte.
    pub kelvin: (f32, f32),
    /// Balance totale : chaleur (IL, log₂ rouge / bleu ; positif : plus chaud).
    pub warmth: f32,
    /// Balance totale : teinte (IL ; positif : magenta, négatif : vert).
    pub tint: f32,
    /// Gains totaux.
    pub gains: Gains,
    /// Mélange du noir et blanc appliqué, en pour cent (rouge, vert, bleu).
    pub mixer: Option<[f32; 3]>,
    /// Contraste et saturation du style, en pour cent.
    pub contrast: f32,
    /// Voir `contrast`.
    pub saturation: f32,
}

/// Le rapport de `layers`, pour une image prise sous un éclairage de couleur `shot`
/// ([`kelvin::D65`] sans données du boîtier).
pub fn report(layers: &Layers, shot: [f32; 3]) -> Report {
    let total = layers.total();
    Report {
        correction_exposure: layers.correction.exposure,
        creative_exposure: layers.creative.unwrap_or_default().exposure,
        exposure: total.exposure,
        shot_kelvin: kelvin::of_gains(shot, Gains::NONE),
        kelvin: kelvin::of_gains(shot, total.gains),
        warmth: total.gains.warmth(),
        tint: total.gains.tint(),
        gains: total.gains,
        mixer: layers.mono.map(|m| m.weights().map(|v| v * 100.0)),
        contrast: total.contrast * 100.0,
        saturation: total.saturation * 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(width: u32, height: u32, rgb: [u8; 3]) -> Bitmap {
        Bitmap {
            width,
            height,
            data: rgb.repeat((width * height) as usize),
        }
    }

    const SRGB: Transfer = Transfer::Srgb;

    fn base(bitmap: &Bitmap) -> Base {
        Base {
            bitmap: bitmap.clone(),
            transfer: SRGB,
        }
    }

    fn close(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn one_ev_doubles_the_linear_light() {
        let mut image = flat(2, 2, [60, 60, 60]);
        let before = measure(&image, SRGB, None).luminance;
        apply_bitmap(
            &mut image,
            SRGB,
            Adjust {
                exposure: 1.0,
                ..Adjust::default()
            },
            None,
        );
        let after = measure(&image, SRGB, None).luminance;
        assert!(close(after / before, 2.0, 0.05), "{before} → {after}");
    }

    #[test]
    fn nothing_to_apply_leaves_the_image_untouched() {
        let mut image = flat(2, 1, [10, 200, 30]);
        let copy = image.clone();
        apply_bitmap(&mut image, SRGB, Adjust::default(), None);
        assert_eq!(image, copy);
    }

    #[test]
    fn the_mixer_makes_grey_pixels_from_the_chosen_channels() {
        let mut image = flat(1, 1, [200, 40, 40]);
        let red = Mixer::preset("red").unwrap();
        apply_bitmap(&mut image, SRGB, Adjust::default(), Some(red));
        let [r, g, b] = [image.data[0], image.data[1], image.data[2]];
        assert!(r == g && g == b);
        let mut blue = flat(1, 1, [200, 40, 40]);
        apply_bitmap(&mut blue, SRGB, Adjust::default(), Mixer::preset("blue"));
        assert!(r > blue.data[0], "a red filter lightens red things");
    }

    #[test]
    fn each_curve_decodes_what_it_encodes() {
        for transfer in [Transfer::Srgb, Transfer::Bt709] {
            for c in [0.001f32, 0.01, 0.2, 0.9] {
                let back = transfer.decode(transfer.encode(c));
                assert!(close(back, c, 1e-4), "{transfer:?} {c} → {back}");
            }
        }
    }

    #[test]
    fn one_ev_doubles_the_light_in_a_bt709_render_too() {
        let mut image = flat(2, 2, [30, 30, 30]);
        let before = measure(&image, Transfer::Bt709, None).luminance;
        let up = Adjust {
            exposure: 1.0,
            ..Adjust::default()
        };
        apply_bitmap(&mut image, Transfer::Bt709, up, None);
        let after = measure(&image, Transfer::Bt709, None).luminance;
        assert!(close(after / before, 2.0, 0.1), "{before} → {after}");
    }

    #[test]
    fn warmth_and_tint_round_trip() {
        let gains = Gains::from_warmth_tint(0.4, -0.1);
        assert!(close(gains.warmth(), 0.4, 1e-4));
        assert!(close(gains.tint(), -0.1, 1e-4));
        assert!(gains.red > gains.blue, "warmer: more red");
    }

    #[test]
    fn the_eyedropper_neutralises_a_grey_zone() {
        // Moitié gauche grise mais bleutée, moitié droite rouge vif (ignorée).
        let mut image = flat(4, 2, [100, 110, 140]);
        for row in 0..2 {
            for x in 2..4 {
                let i = (row * 4 + x) * 3;
                image.data[i..i + 3].copy_from_slice(&[220, 30, 30]);
            }
        }
        let zone = Zone {
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 1.0,
        };
        let gains = neutral(measure(&image, SRGB, Some(zone)).cast);
        let mut fixed = image.clone();
        apply_bitmap(
            &mut fixed,
            SRGB,
            Adjust {
                exposure: 0.0,
                gains,
                ..Adjust::default()
            },
            None,
        );
        let [r, g, b] = [fixed.data[0], fixed.data[1], fixed.data[2]];
        assert!(r.abs_diff(g) <= 1 && b.abs_diff(g) <= 1, "{r} {g} {b}");
    }

    #[test]
    fn harmonising_brings_an_underexposed_image_to_the_reference() {
        // La référence, bien exposée ; la cible, la même scène à -1 IL.
        let reference = flat(8, 8, [140, 120, 100]);
        let mut under = reference.clone();
        apply_bitmap(
            &mut under,
            SRGB,
            Adjust {
                exposure: -1.0,
                ..Adjust::default()
            },
            None,
        );
        let ref_layers = Layers {
            creative: Some(Adjust {
                exposure: 0.3,
                ..Adjust::default()
            }),
            ..Layers::default()
        };
        let out = apply_to(
            &ref_layers,
            &base(&reference),
            &Layers::default(),
            &base(&under),
            Modes::default(),
        );
        assert!(close(out.correction.exposure, 1.0, 0.05), "{out:?}");
        assert_eq!(out.creative, ref_layers.creative, "creative offset copied");
        // Déjà bien exposée : rien.
        let same = apply_to(
            &ref_layers,
            &base(&reference),
            &Layers::default(),
            &base(&reference),
            Modes::default(),
        );
        assert!(close(same.correction.exposure, 0.0, 0.02), "{same:?}");
    }

    #[test]
    fn harmonising_the_balance_aligns_on_the_corrected_reference() {
        // La référence est neutralisée par sa correction ; la cible, plus chaude, doit
        // prendre la même dominante (neutre).
        let reference = flat(4, 4, [120, 120, 150]);
        let mut ref_layers = Layers::default();
        ref_layers.correction.gains = neutral(measure(&reference, SRGB, None).cast);
        let warm = flat(4, 4, [160, 120, 100]);
        let out = apply_to(
            &ref_layers,
            &base(&reference),
            &Layers::default(),
            &base(&warm),
            Modes::default(),
        );
        let mut fixed = warm.clone();
        apply_bitmap(&mut fixed, SRGB, out.correction, None);
        let [r, g, b] = [fixed.data[0], fixed.data[1], fixed.data[2]];
        assert!(r.abs_diff(g) <= 2 && b.abs_diff(g) <= 2, "{r} {g} {b}");
    }

    #[test]
    fn harmonising_a_black_and_white_series_measures_the_black_and_white() {
        // Un mélange qui assombrit le rouge : la cible, rouge, est mesurée après lui.
        let reference = flat(4, 4, [120, 120, 120]);
        let red = flat(4, 4, [200, 60, 60]);
        let ref_layers = Layers {
            mono: Mixer::preset("blue"),
            ..Layers::default()
        };
        let out = apply_to(
            &ref_layers,
            &base(&reference),
            &Layers::default(),
            &base(&red),
            Modes {
                white_balance: Mode::Keep,
                ..Modes::default()
            },
        );
        let mut a = reference.clone();
        apply_bitmap(&mut a, SRGB, ref_layers.total(), ref_layers.mono);
        let mut b = red.clone();
        apply_bitmap(&mut b, SRGB, out.total(), out.mono);
        assert!(
            a.data[0].abs_diff(b.data[0]) <= 2,
            "{} {}",
            a.data[0],
            b.data[0]
        );
    }

    #[test]
    fn modes_can_be_forced() {
        let image = flat(2, 2, [100, 100, 100]);
        let mut reference = Layers::default();
        reference.correction.exposure = 0.5;
        let mut target = Layers::default();
        target.correction.exposure = 0.25;
        let modes = |exposure| Modes {
            exposure,
            white_balance: Mode::Keep,
            ..Modes::default()
        };
        let image = base(&image);
        let apply = |m| apply_to(&reference, &image, &target, &image, modes(m));
        assert_eq!(apply(Mode::Absolute).correction.exposure, 0.5);
        assert_eq!(apply(Mode::Offset).correction.exposure, 0.75);
        assert_eq!(apply(Mode::Keep).correction.exposure, 0.25);
    }

    #[test]
    fn adjustments_are_kept_in_the_sidecar_and_undone() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0001.RAF");
        std::fs::write(&image, b"raw").unwrap();
        let conv = Convention::Darktable;
        assert_eq!(read(&image).unwrap(), None);
        let layers = Layers {
            correction: Adjust {
                exposure: 0.7,
                gains: Gains::from_warmth_tint(0.2, 0.0),
                ..Adjust::default()
            },
            mono: Mixer::preset("orange"),
            ..Layers::default()
        };
        let change = write(&image, Some(&layers), conv).unwrap();
        assert_eq!(read(&image).unwrap(), Some(layers));
        let mut history = crate::history::History::new(10);
        history.push(crate::history::Step::Adjust(vec![change]));
        history.undo(conv).unwrap();
        assert_eq!(read(&image).unwrap(), None, "undone");
        history.redo(conv).unwrap();
        assert_eq!(read(&image).unwrap(), Some(layers), "redone");
        // Aucun réglage : la liste est retirée.
        write(&image, None, conv).unwrap();
        assert_eq!(read(&image).unwrap(), None);
    }

    #[test]
    fn the_report_splits_correction_and_creative() {
        let layers = Layers {
            correction: Adjust {
                exposure: 1.0,
                gains: Gains::from_warmth_tint(-0.2, 0.0),
                ..Adjust::default()
            },
            creative: Some(Adjust {
                exposure: 0.3,
                gains: Gains::from_warmth_tint(0.1, 0.0),
                ..Adjust::default()
            }),
            mono: None,
        };
        let r = report(&layers, kelvin::D65);
        assert!(close(r.exposure, 1.3, 1e-5));
        assert!(close(r.warmth, -0.1, 1e-4));
        assert_eq!(r.mixer, None);
        assert!((r.shot_kelvin.0 - 6504.0).abs() < 60.0, "{r:?}");
        assert!(
            r.kelvin.0 < r.shot_kelvin.0,
            "cooler overall: fewer kelvins"
        );
    }

    #[test]
    fn a_normalised_mix_keeps_the_brightness() {
        let mut mixer = Mixer::preset("orange").unwrap();
        mixer.red *= 2.0;
        let w = mixer.weights();
        assert!(close(w.iter().sum::<f32>(), 1.0, 1e-5), "{w:?}");
        mixer.normalise = false;
        assert!(mixer.weights()[0] > 1.0);
        assert_eq!(
            Mixer::preset("yellow").unwrap().preset_name(),
            Some("yellow")
        );
        assert!(mixer.preset_name().is_none());
        // Écrit seulement quand il est coupé, relu à l'identique.
        let json = serde_json::to_string(&mixer).unwrap();
        assert!(json.contains("normalise"), "{json}");
        assert_eq!(serde_json::from_str::<Mixer>(&json).unwrap(), mixer);
        let on = Mixer::default();
        let json = serde_json::to_string(&on).unwrap();
        assert!(!json.contains("normalise"), "{json}");
        assert_eq!(serde_json::from_str::<Mixer>(&json).unwrap(), on);
    }

    #[test]
    fn a_locked_mix_keeps_its_total_and_the_ratio_of_the_others() {
        let orange = Mixer::preset("orange").unwrap(); // 60 / 40 / 0
        let m = orange.set_locked(0, 0.7);
        assert!(close(m.red, 0.7, 1e-5) && close(m.green, 0.3, 1e-5) && close(m.blue, 0.0, 1e-5));
        let yellow = Mixer::preset("yellow").unwrap(); // 45 / 50 / 5
        let m = yellow.set_locked(0, 0.55);
        assert!(close(m.sum(), 1.0, 1e-5));
        assert!(close(m.green / m.blue, 10.0, 1e-3), "{m:?}");
        // Signes opposés (filtre rouge : -10 % de bleu) : moitié chacun.
        let red = Mixer::preset("red").unwrap(); // 90 / 20 / -10
        let m = red.set_locked(1, 0.4);
        assert!(
            close(m.red, 0.8, 1e-5) && close(m.blue, -0.2, 1e-5),
            "{m:?}"
        );
        // Aux bornes : s'arrête là où les autres ne suivent plus.
        let m = Mixer::default().set_locked(0, 2.0);
        assert!(close(m.sum(), 1.0, 1e-4), "{m:?}");
        assert!(
            m.green >= Mixer::RANGE.0 - 1e-3 && m.blue >= Mixer::RANGE.0 - 1e-3,
            "{m:?}"
        );
        let free = Mixer {
            normalise: false,
            ..Mixer::preset("orange").unwrap()
        }
        .with_parts([1.0, 1.0, 0.0]);
        assert!(close(free.rescaled().sum(), 1.0, 1e-6));
    }

    #[test]
    fn a_gesture_moves_the_other_images_relatively() {
        let shot = kelvin::D65;
        let mut a = Layers::default();
        a.correction.exposure = 0.3;
        a.correction.gains = kelvin::gains(shot, 5000.0, 0.0);
        let mut b = Layers::default();
        b.correction.exposure = 1.0;
        b.correction.gains = kelvin::gains(shot, 6000.0, 10.0);
        // Exposition : le même écart.
        let mut after = a;
        after.correction.exposure = 0.5;
        let out = propagate(&a, &after, shot, &b, shot);
        assert!(close(out.correction.exposure, 1.2, 1e-5), "{out:?}");
        assert_eq!(
            out.correction.gains, b.correction.gains,
            "balance untouched"
        );
        // Température : règle de 3 ; teinte : même écart.
        let mut after = a;
        after.correction.gains = kelvin::gains(shot, 5500.0, 4.0);
        let out = propagate(&a, &after, shot, &b, shot);
        let (k, t) = kelvin::of_gains(shot, out.correction.gains);
        assert!(
            (k - 6600.0).abs() < 40.0 && (t - 14.0).abs() < 1.0,
            "{k} {t}"
        );
        assert_eq!(out.correction.exposure, 1.0, "exposure untouched");
        // Noir et blanc : copié.
        let mut after = a;
        after.mono = Mixer::preset("green");
        assert_eq!(propagate(&a, &after, shot, &b, shot).mono, after.mono);
    }

    #[test]
    fn auto_exposure_brings_the_median_to_mid_grey() {
        // Sans compensation : la médiane au gris moyen.
        let settings = AdjustSettings {
            auto_exposure_bias: 0.0,
            ..AdjustSettings::default()
        };
        let dark = flat(4, 4, [40, 40, 40]);
        let ev = auto_exposure(&base(&dark), Gains::NONE, &settings);
        let mut fixed = dark.clone();
        apply_bitmap(
            &mut fixed,
            SRGB,
            Adjust {
                exposure: ev,
                ..Adjust::default()
            },
            None,
        );
        let y = measure(&fixed, SRGB, None).luminance;
        assert!(ev > 0.0 && close(y, 0.18, 0.01), "{ev} {y}");
        // Compensée : d'autant plus claire.
        let biased = auto_exposure(&base(&dark), Gains::NONE, &AdjustSettings::default());
        assert!(close(biased - ev, 0.5, 0.05), "{ev} {biased}");
    }

    #[test]
    fn auto_exposure_keeps_the_highlights_of_a_low_key_portrait() {
        // Retour du 10/11/2026 : fond et pull sombres (90 %), visage clair (10 %) ; la
        // médiane seule aurait éclairci de plus de 3 IL et brûlé le visage.
        let mut image = flat(10, 10, [45, 45, 45]);
        for px in image.data.as_chunks_mut::<3>().0.iter_mut().take(10) {
            *px = [185, 185, 185];
        }
        let settings = AdjustSettings::default();
        let ceiling = (settings.auto_exposure_highlight_target
            * settings.auto_exposure_bias.exp2())
        .min(HIGHLIGHT_MAX);
        let ev = auto_exposure(&base(&image), Gains::NONE, &settings);
        let face = SRGB.decode(185.0 / 255.0);
        assert!((0.0..1.0).contains(&ev), "{ev}");
        assert!(close(face * ev.exp2(), ceiling, 0.02), "{ev}");
        // Neige : très claire partout, assombrie jusqu'aux hautes lumières à la cible.
        let snow = flat(4, 4, [250, 250, 250]);
        let ev = auto_exposure(&base(&snow), Gains::NONE, &settings);
        let white = SRGB.decode(250.0 / 255.0);
        assert!(ev < 0.0 && close(white * ev.exp2(), ceiling, 0.02), "{ev}");
        // Sans compensation : les niveaux de la configuration tels quels.
        let plain = AdjustSettings {
            auto_exposure_bias: 0.0,
            ..settings
        };
        let ev = auto_exposure(&base(&image), Gains::NONE, &plain);
        assert!(
            close(face * ev.exp2(), plain.auto_exposure_highlight_target, 0.02),
            "{ev}"
        );
    }

    #[test]
    fn contrast_keeps_mid_grey_and_saturation_keeps_greys() {
        let style = |contrast: f32, saturation: f32| Adjust {
            contrast,
            saturation,
            ..Adjust::default()
        };
        // Le gris moyen ne bouge pas ; plus de contraste : les ombres plus sombres, les
        // clairs plus clairs.
        let grey = SRGB.encode(CONTRAST_PIVOT);
        let pivot = (grey * 255.0).round() as u8;
        let mut image = Bitmap {
            width: 3,
            height: 1,
            data: vec![40, 40, 40, pivot, pivot, pivot, 200, 200, 200],
        };
        apply_bitmap(&mut image, SRGB, style(0.5, 0.0), None);
        assert!(
            image.data[0] < 40 && image.data[6] > 200,
            "{:?}",
            image.data
        );
        assert!(image.data[3].abs_diff(pivot) <= 1, "{:?}", image.data);
        // Saturation : un gris reste gris, une couleur s'éloigne ou s'approche du gris.
        let mut grey = flat(1, 1, [120, 120, 120]);
        apply_bitmap(&mut grey, SRGB, style(0.0, 1.0), None);
        assert_eq!(grey.data, vec![120, 120, 120]);
        let spread = |b: &Bitmap| b.data.iter().max().unwrap() - b.data.iter().min().unwrap();
        let red = flat(1, 1, [180, 90, 80]);
        let mut more = red.clone();
        apply_bitmap(&mut more, SRGB, style(0.0, 0.5), None);
        let mut none = red.clone();
        apply_bitmap(&mut none, SRGB, style(0.0, -1.0), None);
        assert!(
            spread(&more) > spread(&red) && spread(&none) <= 1,
            "{more:?} {none:?}"
        );
        // Ancien sidecar : sans les champs, zéro ; zéro, pas écrit.
        let json = serde_json::to_string(&Adjust::default()).unwrap();
        assert!(!json.contains("contrast"), "{json}");
    }

    #[test]
    fn auto_balance_neutralises_the_greys_not_the_skin() {
        // Retour du 10/11/2026 : un mur gris un peu bleuté (70 %) et un visage chaud
        // (30 %) ; le monde gris aurait refroidi l'image, la balance auto rend le mur gris.
        let mut image = flat(10, 10, [118, 120, 128]);
        for px in image.data.as_chunks_mut::<3>().0.iter_mut().take(30) {
            *px = [210, 150, 120];
        }
        let gains = auto_balance(&base(&image), &AdjustSettings::default());
        let mut fixed = image.clone();
        apply_bitmap(
            &mut fixed,
            SRGB,
            Adjust {
                exposure: 0.0,
                gains,
                ..Adjust::default()
            },
            None,
        );
        let wall = &fixed.data[fixed.data.len() - 3..];
        assert!(
            wall[0].abs_diff(wall[1]) <= 1 && wall[2].abs_diff(wall[1]) <= 1,
            "{wall:?}"
        );
        let world = neutral(measure(&image, SRGB, None).cast);
        assert!(world.warmth() < gains.warmth() - 0.1, "grey world: colder");
        // Rien de neutre : le monde gris.
        let red = flat(4, 4, [200, 60, 60]);
        let g = auto_balance(&base(&red), &AdjustSettings::default());
        assert_eq!(g, neutral(measure(&red, SRGB, None).cast));
    }

    #[test]
    fn a_creative_offset_without_values_is_kept() {
        // Case cochée, curseurs à zéro : gardé (l'interface montre la case cochée).
        let layers = Layers {
            creative: Some(Adjust::default()),
            ..Layers::default()
        };
        assert!(!layers.is_none());
        let items = Layers::items(Some(&layers));
        assert_eq!(Layers::from_items(&items), Some(layers));
        // Ancien format : décalage créatif sans Option.
        let old = r#"{"correction":{"exposure":0.5},"creative":{"exposure":0.25}}"#;
        let read = Layers::from_items(&[old.to_string()]).unwrap();
        assert_eq!(read.creative.unwrap().exposure, 0.25);
    }
}

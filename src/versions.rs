//! Versions d'une photo (décidé le 19/10/2026 avec l'utilisateur) : des **recadrages**
//! (story 9:16, publication 4:5…) gardés comme des *instructions*, jamais comme des copies.
//! Corriger le développement suffit : toutes les versions suivent à l'export.
//!
//! - On recadre **ce qu'on voit** ([`Source`]) : le développé de darktable (par-dessus son
//!   éventuel recadrage), le rendu RAW de LibRaw, ou le JPEG boîtier.
//! - Une version a un nom, un cadre ([`Rect`], en proportions de l'image vue, redressée),
//!   un rapport ([`Ratio`]) et, en option, un profil d'export (qui pré-remplit la boîte).
//! - Stockage : le sidecar XMP de la photo, liste `tri:Versions` (un objet JSON par
//!   élément, [`crate::xmp::VERSIONS`]) ; les autres logiciels l'ignorent, rien d'autre
//!   n'est touché. Une version illisible (écrite par une version plus récente de `tri`)
//!   est gardée telle quelle.
//!
//! La géométrie du cadre (rapports, orientation, poignées) est ici, testée sans interface :
//! l'interface ne fait que la dessiner. Voir `doc/en/versions.md`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::xmp::{self, Convention};

/// Image recadrée : celle qu'on regardait.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// JPEG boîtier (aperçu embarqué).
    #[default]
    Embedded,
    /// Rendu de base par LibRaw.
    Raw,
    /// Développé par darktable.
    Developed,
}

/// Rapport largeur / hauteur du cadre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ratio {
    /// Libre.
    Free,
    /// Celui de l'image ; `flipped` : tourné d'un quart (portrait ↔ paysage).
    Original {
        /// Orientation inverse de celle de l'image.
        flipped: bool,
    },
    /// Fixe, `w:h` (4:5 portrait, 5:4 paysage…).
    Fixed(u32, u32),
}

impl Ratio {
    /// Rapports proposés (dans l'ordre des touches 1 à 7), en paysage pour les fixes ;
    /// [`Ratio::rotated`] les met en portrait.
    pub const PRESETS: [Ratio; 7] = [
        Ratio::Free,
        Ratio::Original { flipped: false },
        Ratio::Fixed(1, 1),
        Ratio::Fixed(5, 4),
        Ratio::Fixed(3, 2),
        Ratio::Fixed(16, 9),
        Ratio::Fixed(4, 3),
    ];

    /// Largeur / hauteur en pixels pour une image de rapport `aspect` ; `None` : libre.
    pub fn value(self, aspect: f64) -> Option<f64> {
        match self {
            Ratio::Free => None,
            Ratio::Original { flipped: false } => Some(aspect),
            Ratio::Original { flipped: true } => Some(1.0 / aspect),
            Ratio::Fixed(w, h) => Some(f64::from(w.max(1)) / f64::from(h.max(1))),
        }
    }

    /// Orientation inversée (portrait ↔ paysage) ; libre et 1:1 inchangés.
    pub fn rotated(self) -> Ratio {
        match self {
            Ratio::Free => Ratio::Free,
            Ratio::Original { flipped } => Ratio::Original { flipped: !flipped },
            Ratio::Fixed(w, h) => Ratio::Fixed(h, w),
        }
    }

    /// Même rapport, sans tenir compte de l'orientation (pour allumer le bon bouton).
    pub fn same_shape(self, other: Ratio) -> bool {
        self == other || self == other.rotated()
    }

    /// Libellé : « libre », « original », « 4:5 »… (stocké tel quel dans le XMP).
    pub fn label(self) -> String {
        match self {
            Ratio::Free => "free".into(),
            Ratio::Original { flipped: false } => "original".into(),
            Ratio::Original { flipped: true } => "original-rotated".into(),
            Ratio::Fixed(w, h) => format!("{w}:{h}"),
        }
    }

    /// Lit un libellé de [`Ratio::label`].
    pub fn parse(text: &str) -> Option<Ratio> {
        match text.trim() {
            "free" => Some(Ratio::Free),
            "original" => Some(Ratio::Original { flipped: false }),
            "original-rotated" => Some(Ratio::Original { flipped: true }),
            other => {
                let (w, h) = other.split_once(':')?;
                let (w, h) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
                (w > 0 && h > 0).then_some(Ratio::Fixed(w, h))
            }
        }
    }
}

impl Serialize for Ratio {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.label())
    }
}

impl<'de> Deserialize<'de> for Ratio {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Ratio::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("rapport : {text}")))
    }
}

/// Cadre, en proportions de l'image vue (redressée) : origine en haut à gauche, tout entre
/// 0 et 1. Stocké `[x, y, largeur, hauteur]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f64; 4]", into = "[f64; 4]")]
pub struct Rect {
    /// Bord gauche.
    pub x: f64,
    /// Bord haut.
    pub y: f64,
    /// Largeur.
    pub w: f64,
    /// Hauteur.
    pub h: f64,
}

impl From<[f64; 4]> for Rect {
    fn from([x, y, w, h]: [f64; 4]) -> Self {
        Rect { x, y, w, h }.clamped()
    }
}

impl From<Rect> for [f64; 4] {
    fn from(r: Rect) -> Self {
        // Arrondi à 1e-5 : fichier lisible, précision bien au-delà du pixel.
        let round = |v: f64| (v * 1e5).round() / 1e5;
        [round(r.x), round(r.y), round(r.w), round(r.h)]
    }
}

/// Plus petit côté d'un cadre, en proportion de l'image (pour ne pas le perdre).
pub const MIN_SIDE: f64 = 0.02;

impl Rect {
    /// L'image entière.
    pub const FULL: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };

    /// Ramené dans l'image, côtés d'au moins [`MIN_SIDE`].
    pub fn clamped(self) -> Rect {
        let w = self.w.clamp(MIN_SIDE, 1.0);
        let h = self.h.clamp(MIN_SIDE, 1.0);
        Rect {
            x: self.x.clamp(0.0, 1.0 - w),
            y: self.y.clamp(0.0, 1.0 - h),
            w,
            h,
        }
    }

    /// Le plus grand cadre de rapport `ratio`, centré dans une image de rapport `aspect`
    /// (largeur / hauteur en pixels).
    pub fn fit(ratio: Ratio, aspect: f64) -> Rect {
        let Some(r) = ratio.value(aspect) else {
            return Rect::FULL;
        };
        // En proportions, largeur / hauteur = r / aspect.
        let k = r / aspect;
        let (w, h) = if k >= 1.0 { (1.0, 1.0 / k) } else { (k, 1.0) };
        Rect {
            x: (1.0 - w) / 2.0,
            y: (1.0 - h) / 2.0,
            w,
            h,
        }
    }

    /// Le même cadre au rapport `ratio`, autour du même centre, aussi grand que possible
    /// sans dépasser l'aire actuelle ni l'image (changement de rapport ou d'orientation).
    pub fn with_ratio(self, ratio: Ratio, aspect: f64) -> Rect {
        let Some(r) = ratio.value(aspect) else {
            return self;
        };
        let k = r / aspect;
        let area = self.w * self.h;
        let (mut w, mut h) = ((area * k).sqrt(), (area / k).sqrt());
        let fit = (1.0 / w).min(1.0 / h).min(1.0);
        w *= fit;
        h *= fit;
        let (cx, cy) = (self.x + self.w / 2.0, self.y + self.h / 2.0);
        Rect {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            w,
            h,
        }
        .clamped()
    }

    /// Cadre en pixels (x, y, largeur, hauteur) d'une image de `width` × `height`.
    pub fn pixels(self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let (fw, fh) = (f64::from(width), f64::from(height));
        let x = (self.x * fw).round() as u32;
        let y = (self.y * fh).round() as u32;
        let w = ((self.w * fw).round() as u32).clamp(1, width.saturating_sub(x).max(1));
        let h = ((self.h * fh).round() as u32).clamp(1, height.saturating_sub(y).max(1));
        (
            x.min(width.saturating_sub(1)),
            y.min(height.saturating_sub(1)),
            w,
            h,
        )
    }

    /// Déplace la poignée `handle` de `(dx, dy)` (en proportions), en gardant le rapport
    /// `ratio` (image de rapport `aspect`) et le cadre dans l'image. Un angle garde l'angle
    /// opposé fixe ; un côté, le côté opposé (et, rapport fixe, le centre de l'autre axe).
    pub fn drag(self, handle: Handle, dx: f64, dy: f64, ratio: Ratio, aspect: f64) -> Rect {
        if handle == Handle::Move {
            return Rect {
                x: self.x + dx,
                y: self.y + dy,
                ..self
            }
            .clamped();
        }
        let (left, top, right, bottom) = (self.x, self.y, self.x + self.w, self.y + self.h);
        let (mut l, mut t, mut r, mut b) = (left, top, right, bottom);
        if handle.left() {
            l = (left + dx).clamp(0.0, right - MIN_SIDE);
        }
        if handle.right() {
            r = (right + dx).clamp(left + MIN_SIDE, 1.0);
        }
        if handle.top() {
            t = (top + dy).clamp(0.0, bottom - MIN_SIDE);
        }
        if handle.bottom() {
            b = (bottom + dy).clamp(top + MIN_SIDE, 1.0);
        }
        let Some(value) = ratio.value(aspect) else {
            return Rect {
                x: l,
                y: t,
                w: r - l,
                h: b - t,
            }
            .clamped();
        };
        // Rapport fixe : largeur / hauteur en proportions = k.
        let k = value / aspect;
        let (mut w, mut h) = (r - l, b - t);
        let horizontal = handle.left() || handle.right();
        let vertical = handle.top() || handle.bottom();
        if horizontal && vertical {
            // Angle : la dimension la plus tirée commande l'autre.
            if w / k > h {
                h = w / k;
            } else {
                w = h * k;
            }
        } else if horizontal {
            h = w / k;
        } else {
            w = h * k;
        }
        // Place disponible depuis le point fixe.
        let (cx, cy) = (self.x + self.w / 2.0, self.y + self.h / 2.0);
        let max_w = if handle.left() {
            right
        } else if handle.right() {
            1.0 - left
        } else {
            2.0 * cx.min(1.0 - cx)
        };
        let max_h = if handle.top() {
            bottom
        } else if handle.bottom() {
            1.0 - top
        } else {
            2.0 * cy.min(1.0 - cy)
        };
        let fit = (max_w / w).min(max_h / h).min(1.0);
        w = (w * fit).max(MIN_SIDE);
        h = (h * fit).max(MIN_SIDE);
        let x = if handle.left() {
            right - w
        } else if handle.right() {
            left
        } else {
            cx - w / 2.0
        };
        let y = if handle.top() {
            bottom - h
        } else if handle.bottom() {
            top
        } else {
            cy - h / 2.0
        };
        Rect { x, y, w, h }.clamped()
    }
}

/// Partie du cadre qu'on tire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Tout le cadre (déplacer).
    Move,
    /// Côté haut.
    Top,
    /// Côté bas.
    Bottom,
    /// Côté gauche.
    Left,
    /// Côté droit.
    Right,
    /// Angle haut gauche.
    TopLeft,
    /// Angle haut droit.
    TopRight,
    /// Angle bas gauche.
    BottomLeft,
    /// Angle bas droit.
    BottomRight,
}

impl Handle {
    fn left(self) -> bool {
        matches!(self, Handle::Left | Handle::TopLeft | Handle::BottomLeft)
    }
    fn right(self) -> bool {
        matches!(self, Handle::Right | Handle::TopRight | Handle::BottomRight)
    }
    fn top(self) -> bool {
        matches!(self, Handle::Top | Handle::TopLeft | Handle::TopRight)
    }
    fn bottom(self) -> bool {
        matches!(
            self,
            Handle::Bottom | Handle::BottomLeft | Handle::BottomRight
        )
    }
}

/// Préréglage de recadrage (idée de l'utilisateur, 19/10/2026) : « Story » = 9:16 et le
/// profil d'export Web. On le choisit dans la barre du recadrage, puis on ajuste le cadre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    /// Identifiant (`[versions.presets.<id>]`).
    pub id: String,
    /// Nom (de la version créée) ; vide : nom traduit des préréglages fournis, sinon
    /// l'identifiant.
    pub name: String,
    /// Rapport, orientation comprise (9:16 : portrait).
    pub ratio: Ratio,
    /// Profil d'export proposé ; vide : aucun.
    pub profile: String,
}

impl Preset {
    fn new(id: &str, ratio: Ratio, profile: &str) -> Preset {
        Preset {
            id: id.into(),
            name: String::new(),
            ratio,
            profile: profile.into(),
        }
    }

    /// Les clés du préréglage telles qu'écrites dans `[versions.presets.<id>]`.
    pub fn entries(&self) -> Vec<(&'static str, crate::config_edit::Value)> {
        use crate::config_edit::Value;
        vec![
            ("name", Value::Str(self.name.clone())),
            ("ratio", Value::Str(self.ratio.label())),
            ("profile", Value::Str(self.profile.clone())),
        ]
    }

    /// Les préréglages fournis : story (9:16) et publication (4:5), pour le web.
    pub fn defaults() -> Vec<Preset> {
        vec![
            Preset::new("story", Ratio::Fixed(9, 16), "web"),
            Preset::new("post", Ratio::Fixed(4, 5), "web"),
        ]
    }
}

/// Un préréglage tel qu'écrit dans le fichier : chaque clé absente vient du préréglage
/// fourni de même identifiant (comme les profils d'export).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PresetToml {
    name: Option<String>,
    ratio: Option<String>,
    profile: Option<String>,
}

/// Les préréglages : fournis, puis ceux de l'utilisateur (par identifiant).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Presets(pub Vec<Preset>);

impl Default for Presets {
    fn default() -> Self {
        Presets(Preset::defaults())
    }
}

impl<'de> Deserialize<'de> for Presets {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let written = std::collections::BTreeMap::<String, PresetToml>::deserialize(d)?;
        let mut list = Preset::defaults();
        for (id, t) in written {
            let index = match list.iter().position(|p| p.id == id) {
                Some(i) => i,
                None => {
                    list.push(Preset::new(&id, Ratio::Free, ""));
                    list.len() - 1
                }
            };
            let p = &mut list[index];
            if let Some(v) = t.name {
                p.name = v;
            }
            if let Some(v) = t.ratio {
                p.ratio = Ratio::parse(&v).ok_or_else(|| {
                    serde::de::Error::custom(crate::t!(
                        "preset-bad-ratio",
                        preset = id.as_str(),
                        value = v.as_str()
                    ))
                })?;
            }
            if let Some(v) = t.profile {
                p.profile = v;
            }
        }
        Ok(Presets(list))
    }
}

/// Réglages des versions (section `[versions]`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VersionSettings {
    /// Préréglages de recadrage (`[versions.presets.<id>]`).
    pub presets: Presets,
}

/// Identifiant libre pour un nouveau préréglage ou profil nommé `name` : minuscules,
/// lettres et chiffres (accents retirés), tirets ; suivi d'un numéro s'il est déjà pris.
pub fn new_id(name: &str, taken: impl Fn(&str) -> bool) -> String {
    let plain: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    let base: String = plain
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base = if base.is_empty() {
        "preset".to_string()
    } else {
        base
    };
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .expect("un identifiant libre")
}

/// Une version d'une photo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Version {
    /// Nom (« Story », « 4:5 »…).
    pub name: String,
    /// Cadre.
    pub crop: Rect,
    /// Rapport choisi (gardé pour retoucher le cadre plus tard).
    pub ratio: Ratio,
    /// Image recadrée.
    #[serde(default)]
    pub source: Source,
    /// Profil d'export proposé pour cette version (pré-remplit la boîte d'export).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Les versions d'une photo, dans l'ordre, et les éléments illisibles gardés tels quels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Versions {
    /// Versions lues.
    pub list: Vec<Version>,
    /// Éléments illisibles (écrits par une version plus récente de `tri` ?), réécrits
    /// tels quels à la fin de la liste.
    pub unknown: Vec<String>,
}

impl Versions {
    /// Depuis les éléments de `tri:Versions`.
    pub fn from_items(items: &[String]) -> Versions {
        let mut versions = Versions::default();
        for item in items {
            match serde_json::from_str(item) {
                Ok(v) => versions.list.push(v),
                Err(_) => versions.unknown.push(item.clone()),
            }
        }
        versions
    }

    /// Éléments à écrire dans `tri:Versions`.
    pub fn items(&self) -> Vec<String> {
        self.list
            .iter()
            .map(|v| serde_json::to_string(v).expect("version sérialisable"))
            .chain(self.unknown.iter().cloned())
            .collect()
    }

    /// Nom proposé pour une nouvelle version de rapport `ratio` : son libellé (« 4:5 »,
    /// « Libre »…), suivi d'un numéro s'il est déjà pris.
    pub fn new_name(&self, base: &str) -> String {
        let taken = |n: &str| self.list.iter().any(|v| v.name == n);
        if !taken(base) {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} ({n})"))
            .find(|n| !taken(n))
            .expect("un nom libre")
    }
}

/// Versions de `image`, lues dans son sidecar (aucune sans sidecar).
pub fn read(image: &Path) -> Result<Versions, xmp::XmpError> {
    Ok(Versions::from_items(&xmp::read_list_of(
        image,
        xmp::VERSIONS,
    )?))
}

/// Écrit les versions de `image` dans son sidecar (créé selon `default` s'il le faut) ;
/// seule la liste `tri:Versions` change. Renvoie le sidecar.
pub fn write(
    image: &Path,
    versions: &Versions,
    default: Convention,
) -> Result<PathBuf, xmp::XmpError> {
    xmp::write_list(image, xmp::VERSIONS, &versions.items(), default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn ratios_read_back_and_turn() {
        for r in [
            Ratio::Free,
            Ratio::Original { flipped: false },
            Ratio::Original { flipped: true },
            Ratio::Fixed(4, 5),
        ] {
            assert_eq!(Ratio::parse(&r.label()), Some(r));
        }
        assert_eq!(Ratio::Fixed(5, 4).rotated(), Ratio::Fixed(4, 5));
        assert!(Ratio::Fixed(4, 5).same_shape(Ratio::Fixed(5, 4)));
        assert_eq!(Ratio::parse("0:3"), None);
        // Image 3:2 : « original » vaut 1,5, tourné 2/3.
        assert_eq!(Ratio::Original { flipped: false }.value(1.5), Some(1.5));
        assert!(close(
            Ratio::Original { flipped: true }.value(1.5).unwrap(),
            2.0 / 3.0
        ));
    }

    #[test]
    fn the_largest_frame_of_a_ratio_is_centred() {
        // Image 3:2, cadre 4:5 (portrait) : toute la hauteur, largeur 4/5 × 2/3.
        let r = Rect::fit(Ratio::Fixed(4, 5), 1.5);
        assert!(close(r.h, 1.0) && close(r.w, 0.8 / 1.5));
        assert!(close(r.x, (1.0 - r.w) / 2.0) && close(r.y, 0.0));
        // En pixels, sur 6000 × 4000 : 3200 × 4000.
        assert_eq!(r.pixels(6000, 4000), (1400, 0, 3200, 4000));
        // 16:9 dans du 3:2 : toute la largeur.
        let r = Rect::fit(Ratio::Fixed(16, 9), 1.5);
        assert!(close(r.w, 1.0) && close(r.h, 1.5 / (16.0 / 9.0)));
        assert_eq!(Rect::fit(Ratio::Free, 1.5), Rect::FULL);
    }

    #[test]
    fn turning_the_frame_keeps_its_centre_and_stays_inside() {
        let r = Rect::fit(Ratio::Fixed(4, 5), 1.5);
        let turned = r.with_ratio(Ratio::Fixed(5, 4), 1.5);
        let px = turned.pixels(6000, 4000);
        assert!(
            (f64::from(px.2) / f64::from(px.3) - 1.25).abs() < 0.01,
            "{px:?}"
        );
        assert!(close(turned.x + turned.w / 2.0, 0.5) && close(turned.y + turned.h / 2.0, 0.5));
        assert!(turned.x >= 0.0 && turned.y >= 0.0 && turned.x + turned.w <= 1.0 + 1e-9);
    }

    #[test]
    fn handles_keep_the_ratio_and_the_opposite_corner() {
        let ratio = Ratio::Fixed(1, 1);
        let start = Rect::fit(ratio, 1.5);
        // Angle bas droit tiré vers le haut gauche : l'angle haut gauche ne bouge pas.
        let r = start.drag(Handle::BottomRight, -0.2, -0.1, ratio, 1.5);
        assert!(close(r.x, start.x) && close(r.y, start.y));
        let px = r.pixels(6000, 4000);
        assert!(px.2.abs_diff(px.3) <= 1, "carré : {px:?}");
        // Côté droit, rapport fixe : la hauteur suit, centrée.
        let r = start.drag(Handle::Right, -0.1, 0.0, ratio, 1.5);
        assert!(close(r.x, start.x));
        assert!(close(r.y + r.h / 2.0, start.y + start.h / 2.0));
        // Libre : seul le côté bouge.
        let r = Rect::FULL.drag(Handle::Left, 0.25, 0.3, Ratio::Free, 1.5);
        assert_eq!((r.x, r.y, r.w, r.h), (0.25, 0.0, 0.75, 1.0));
        // Déplacer : reste dans l'image.
        let r = start.drag(Handle::Move, 5.0, -5.0, ratio, 1.5);
        assert!(close(r.x + r.w, 1.0) && close(r.y, 0.0));
        // Jamais plus petit que le minimum, jamais retourné.
        let r = start.drag(Handle::BottomRight, -9.0, -9.0, Ratio::Free, 1.5);
        assert!(r.w >= MIN_SIDE && r.h >= MIN_SIDE);
    }

    #[test]
    fn presets_come_with_defaults_and_complete_from_them() {
        #[derive(Deserialize)]
        struct S {
            versions: VersionSettings,
        }
        let s: S = toml::from_str(
            "[versions.presets.story]\nprofile = \"\"\n[versions.presets.tirage]\nname = \"Tirage A4\"\nratio = \"3:2\"\nprofile = \"edit\"\n",
        )
        .unwrap();
        let p = &s.versions.presets.0;
        let ids: Vec<&str> = p.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["story", "post", "tirage"]);
        assert_eq!(
            p[0].ratio,
            Ratio::Fixed(9, 16),
            "le reste du préréglage fourni"
        );
        assert_eq!(p[0].profile, "");
        assert_eq!(p[2].ratio, Ratio::Fixed(3, 2));
        assert!(toml::from_str::<S>("[versions.presets.x]\nratio = \"large\"\n").is_err());
        let taken = |id: &str| ["story", "tirage-a4"].contains(&id);
        assert_eq!(new_id("Tirage A4", taken), "tirage-a4-2");
        assert_eq!(new_id("Été 2026 !", taken), "ete-2026");
        assert_eq!(new_id("  ", taken), "preset");
    }

    #[test]
    fn versions_survive_the_sidecar_and_unknown_ones_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0001.RAF");
        std::fs::write(&image, b"raw").unwrap();
        assert_eq!(read(&image).unwrap(), Versions::default(), "pas de sidecar");
        let mut versions = Versions::default();
        versions.list.push(Version {
            name: "Story".into(),
            crop: Rect::fit(Ratio::Fixed(9, 16), 1.5),
            ratio: Ratio::Fixed(9, 16),
            source: Source::Developed,
            profile: Some("web".into()),
        });
        versions.unknown.push(r#"{"futur":true}"#.into());
        let sidecar = write(&image, &versions, Convention::Darktable).unwrap();
        assert_eq!(sidecar, dir.path().join("DSCF0001.RAF.xmp"));
        let back = read(&image).unwrap();
        assert_eq!(back.list.len(), 1);
        assert_eq!(back.list[0].name, "Story");
        assert_eq!(back.list[0].ratio, Ratio::Fixed(9, 16));
        assert_eq!(back.unknown, versions.unknown, "gardée telle quelle");
        // Les notes écrites ensuite ne touchent pas aux versions.
        let notes = xmp::Notes {
            stars: 4,
            ..Default::default()
        };
        xmp::write(&image, &notes, Convention::Darktable).unwrap();
        assert_eq!(read(&image).unwrap().list.len(), 1);
        assert_eq!(versions.new_name("Story"), "Story (2)");
        assert_eq!(versions.new_name("4:5"), "4:5");
    }
}

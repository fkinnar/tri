//! Superpositions géométriques sur une image (idée de l'utilisateur, 19/10/2026) : grilles
//! de composition, et superpositions définies par l'utilisateur pour vérifier qu'une photo
//! convient à un usage (un texte ou un logo lisible à tel endroit, un format de
//! publication 4:5…).
//!
//! Famille « overlay géométrique pur » du `CLAUDE.md` : aucun calcul sur les pixels, des
//! **formes vectorielles** ([`Shape`]) en coordonnées relatives à l'image (0 à 1 en
//! largeur et en hauteur), que l'interface dessine à sa résolution (un TUI ferait ce
//! qu'il peut). Un trait ([`Overlay`]) et une liste : les grilles intégrées
//! ([`builtin`]), écrites en Rust, et celles de l'utilisateur, un fichier TOML chacune
//! ([`load_dir`]). Voir `doc/overlay.md`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Couleur RVB et opacité (0 transparent, 255 opaque).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    /// `"#rrggbb"` et une opacité de 0 à 1 ; `None` si la couleur est mal écrite.
    pub fn parse(hex: &str, opacity: f32) -> Option<Rgba> {
        let [r, g, b] = crate::config_edit::parse_color(hex)?;
        Some(Rgba([
            r,
            g,
            b,
            (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
        ]))
    }
}

/// Point d'ancrage d'un texte ou d'une image : un coin, un bord ou le centre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    /// En haut à gauche.
    TopLeft,
    /// En haut, centré.
    Top,
    /// En haut à droite.
    TopRight,
    /// À gauche, centré en hauteur.
    Left,
    /// Au centre.
    Center,
    /// À droite, centré en hauteur.
    Right,
    /// En bas à gauche.
    BottomLeft,
    /// En bas, centré.
    Bottom,
    /// En bas à droite.
    #[default]
    BottomRight,
}

impl Anchor {
    /// Position de l'ancre sur chaque axe : 0 (début), 0,5 (milieu), 1 (fin).
    pub fn factors(self) -> [f32; 2] {
        use Anchor::*;
        let x = match self {
            TopLeft | Left | BottomLeft => 0.0,
            Top | Center | Bottom => 0.5,
            TopRight | Right | BottomRight => 1.0,
        };
        let y = match self {
            TopLeft | Top | TopRight => 0.0,
            Left | Center | Right => 0.5,
            BottomLeft | Bottom | BottomRight => 1.0,
        };
        [x, y]
    }
}

/// Une forme, en coordonnées relatives à l'image : (0, 0) en haut à gauche, (1, 1) en bas
/// à droite.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Segment (l'épaisseur du trait est celle de l'interface).
    Line {
        /// Départ.
        from: [f32; 2],
        /// Arrivée.
        to: [f32; 2],
        /// Couleur.
        color: Rgba,
    },
    /// Rectangle, contour et / ou fond.
    Rect {
        /// Coin haut gauche.
        min: [f32; 2],
        /// Coin bas droit.
        max: [f32; 2],
        /// Contour.
        stroke: Option<Rgba>,
        /// Fond.
        fill: Option<Rgba>,
    },
    /// Texte posé à `at`, ce point étant son ancre `anchor` (`BottomRight` : `at` est son
    /// coin bas droit).
    Text {
        /// Point d'ancrage.
        at: [f32; 2],
        /// Quel point du texte est en `at`.
        anchor: Anchor,
        /// Contenu.
        text: String,
        /// Hauteur des lettres, en fraction de la hauteur de l'image.
        size: f32,
        /// Couleur.
        color: Rgba,
    },
    /// Image (logo) étirée sur le rectangle donné.
    Image {
        /// Fichier.
        path: PathBuf,
        /// Coin haut gauche.
        min: [f32; 2],
        /// Coin bas droit.
        max: [f32; 2],
        /// Opacité, de 0 à 1.
        opacity: f32,
    },
}

/// Une superposition : un nom, et ses formes pour une image donnée.
pub trait Overlay: Send + Sync {
    /// Nom affiché.
    fn name(&self) -> String;
    /// Formes pour une image de rapport largeur / hauteur `aspect`.
    fn shapes(&self, aspect: f32) -> Vec<Shape>;
}

/// Grilles de composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GridKind {
    /// Règle des tiers.
    Thirds,
    /// Nombre d'or (lignes à 0,382 et 0,618).
    Golden,
    /// Diagonales et diagonales réciproques (perpendiculaires depuis les coins opposés).
    Diagonals,
    /// Spirale d'or (Fibonacci), étirée sur l'image comme dans Lightroom ; elle s'enroule
    /// vers le bas à droite (retournable : `flip`).
    Spiral,
}

/// Spirale d'or : quarts de cercle dans les carrés successifs du rectangle d'or (φ × 1),
/// approchés par des segments, ramenés à l'image (0 à 1).
fn spiral_lines(color: Rgba) -> Vec<Shape> {
    const PHI: f32 = std::f32::consts::GOLDEN_RATIO;
    const SQUARES: usize = 10;
    const STEPS: usize = 12;
    let mut out = Vec::new();
    // Rectangle restant : coin haut gauche et taille.
    let (mut x, mut y, mut w, mut h) = (0.0_f32, 0.0_f32, PHI, 1.0_f32);
    let norm = |p: [f32; 2]| [p[0] / PHI, p[1]];
    for i in 0..SQUARES {
        let s = w.min(h);
        // Carré retiré tour à tour à gauche, en haut, à droite, en bas ; centre de l'arc
        // (coin intérieur) et ses deux extrémités.
        let (center, from, to) = match i % 4 {
            0 => {
                let r = ([x + s, y + s], [x, y + s], [x + s, y]);
                (x, w) = (x + s, w - s);
                r
            }
            1 => {
                let r = ([x, y + s], [x, y], [x + s, y + s]);
                (y, h) = (y + s, h - s);
                r
            }
            2 => {
                let r = ([x + w - s, y], [x + w, y], [x + w - s, y + s]);
                w -= s;
                r
            }
            _ => {
                let r = ([x + s, y + h - s], [x + s, y + h], [x, y + h - s]);
                h -= s;
                r
            }
        };
        let angle = |p: [f32; 2]| (p[1] - center[1]).atan2(p[0] - center[0]);
        let (a0, mut a1) = (angle(from), angle(to));
        // Le quart de tour le plus court.
        while a1 - a0 > std::f32::consts::PI {
            a1 -= std::f32::consts::TAU;
        }
        while a0 - a1 > std::f32::consts::PI {
            a1 += std::f32::consts::TAU;
        }
        let point = |t: f32| {
            let a = a0 + (a1 - a0) * t;
            norm([center[0] + s * a.cos(), center[1] + s * a.sin()])
        };
        for k in 0..STEPS {
            out.push(Shape::Line {
                from: point(k as f32 / STEPS as f32),
                to: point((k + 1) as f32 / STEPS as f32),
                color,
            });
        }
    }
    out
}

/// Retournement d'une grille (`flip` : `"x"`, `"y"`, `"xy"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Flip {
    /// Tel quel.
    #[default]
    None,
    /// Miroir gauche / droite.
    X,
    /// Miroir haut / bas.
    Y,
    /// Les deux (demi-tour).
    Xy,
}

impl Flip {
    /// Orientation suivante : l'œil de la spirale passe d'un coin au suivant, dans le
    /// sens des aiguilles d'une montre (aucune, miroir gauche / droite, demi-tour, miroir
    /// haut / bas).
    pub fn next(self) -> Flip {
        match self {
            Flip::None => Flip::X,
            Flip::X => Flip::Xy,
            Flip::Xy => Flip::Y,
            Flip::Y => Flip::None,
        }
    }

    /// Rang de l'orientation, de 1 à 4 (affichage).
    pub fn rank(self) -> usize {
        match self {
            Flip::None => 1,
            Flip::X => 2,
            Flip::Xy => 3,
            Flip::Y => 4,
        }
    }

    /// Point retourné.
    pub fn apply(self, p: [f32; 2]) -> [f32; 2] {
        match self {
            Flip::None => p,
            Flip::X => [1.0 - p[0], p[1]],
            Flip::Y => [p[0], 1.0 - p[1]],
            Flip::Xy => [1.0 - p[0], 1.0 - p[1]],
        }
    }
}

/// Lignes d'une grille, en coordonnées relatives (le rapport `aspect` sert aux
/// diagonales réciproques, à angle droit dans l'image réelle).
fn grid_lines(kind: GridKind, aspect: f32, color: Rgba) -> Vec<Shape> {
    let line = |from, to| Shape::Line { from, to, color };
    let cross = |a: f32, b: f32| {
        vec![
            line([a, 0.0], [a, 1.0]),
            line([b, 0.0], [b, 1.0]),
            line([0.0, a], [1.0, a]),
            line([0.0, b], [1.0, b]),
        ]
    };
    match kind {
        GridKind::Thirds => cross(1.0 / 3.0, 2.0 / 3.0),
        GridKind::Golden => cross(0.381_966, 0.618_034),
        // Image en hauteur : rectangle d'or vertical (x et y échangés), sans quoi la
        // spirale serait écrasée.
        GridKind::Spiral if aspect < 1.0 => spiral_lines(color)
            .into_iter()
            .map(|s| match s {
                Shape::Line { from, to, color } => Shape::Line {
                    from: [from[1], from[0]],
                    to: [to[1], to[0]],
                    color,
                },
                other => other,
            })
            .collect(),
        GridKind::Spiral => spiral_lines(color),
        GridKind::Diagonals => {
            // Pied de la perpendiculaire à la diagonale (0,0)-(1,1) depuis (1,0), et
            // depuis (0,1) : dans l'image réelle (largeur a, hauteur 1), t = a² / (a² + 1).
            let t = aspect * aspect / (aspect * aspect + 1.0);
            let u = 1.0 - t;
            vec![
                line([0.0, 0.0], [1.0, 1.0]),
                line([0.0, 1.0], [1.0, 0.0]),
                line([1.0, 0.0], [t, t]),
                line([0.0, 1.0], [u, u]),
                line([0.0, 0.0], [t, 1.0 - t]),
                line([1.0, 1.0], [u, 1.0 - u]),
            ]
        }
    }
}

/// Grille intégrée (plugin écrit en Rust).
pub struct Grid {
    /// Laquelle.
    pub kind: GridKind,
    /// Couleur des lignes.
    pub color: Rgba,
}

impl Overlay for Grid {
    fn name(&self) -> String {
        match self.kind {
            GridKind::Thirds => crate::t!("overlay-thirds"),
            GridKind::Golden => crate::t!("overlay-golden"),
            GridKind::Diagonals => crate::t!("overlay-diagonals"),
            GridKind::Spiral => crate::t!("overlay-spiral"),
        }
    }

    fn shapes(&self, aspect: f32) -> Vec<Shape> {
        grid_lines(self.kind, aspect, self.color)
    }
}

/// Les superpositions intégrées, dans l'ordre : tiers, nombre d'or, spirale d'or,
/// diagonales.
pub fn builtin(color: Rgba) -> Vec<Box<dyn Overlay>> {
    [
        GridKind::Thirds,
        GridKind::Golden,
        GridKind::Spiral,
        GridKind::Diagonals,
    ]
    .into_iter()
    .map(|kind| Box::new(Grid { kind, color }) as Box<dyn Overlay>)
    .collect()
}

/// Une forme d'un fichier de superposition (section `[[shape]]`).
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum ShapeSpec {
    /// Texte : contenu, ancrage, hauteur (fraction du petit côté), couleur, opacité, marge
    /// (fraction du petit côté).
    Text {
        text: String,
        #[serde(default)]
        anchor: Anchor,
        #[serde(default = "default_text_size")]
        size: f32,
        #[serde(default = "default_white")]
        color: String,
        #[serde(default = "default_opacity")]
        opacity: f32,
        #[serde(default = "default_margin")]
        margin: f32,
    },
    /// Image (logo PNG…) : fichier (relatif au fichier de la superposition, `~` accepté),
    /// ancrage, largeur (fraction de la largeur de l'image), marge, opacité.
    Image {
        path: String,
        #[serde(default)]
        anchor: Anchor,
        #[serde(default = "default_logo_width")]
        width: f32,
        #[serde(default = "default_margin")]
        margin: f32,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    /// Cadre d'un format (`"4:5"`, `"9:16"`, largeur:hauteur), le plus grand possible et
    /// centré ; le reste assombri (`shade` : opacité du voile, 0 : aucun).
    Frame {
        ratio: String,
        #[serde(default = "default_white")]
        color: String,
        #[serde(default = "default_opacity")]
        opacity: f32,
        #[serde(default = "default_shade")]
        shade: f32,
    },
    /// Grille de composition.
    Grid {
        grid: GridKind,
        #[serde(default = "default_white")]
        color: String,
        #[serde(default = "default_grid_opacity")]
        opacity: f32,
        #[serde(default)]
        flip: Flip,
    },
}

fn default_text_size() -> f32 {
    0.04
}
fn default_white() -> String {
    "#ffffff".into()
}
fn default_opacity() -> f32 {
    1.0
}
fn default_grid_opacity() -> f32 {
    0.6
}
fn default_margin() -> f32 {
    0.03
}
fn default_logo_width() -> f32 {
    0.15
}
fn default_shade() -> f32 {
    0.5
}

/// Fichier de superposition : un nom (sinon celui du fichier) et des formes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayFile {
    name: Option<String>,
    #[serde(default, rename = "shape")]
    shapes: Vec<ShapeSpec>,
}

/// Forme résolue au chargement : couleurs lues, logo trouvé et mesuré.
#[derive(Clone, Debug)]
enum Resolved {
    Text {
        text: String,
        anchor: Anchor,
        size: f32,
        color: Rgba,
        margin: f32,
    },
    Image {
        path: PathBuf,
        /// Largeur / hauteur du logo.
        logo_aspect: f32,
        anchor: Anchor,
        width: f32,
        margin: f32,
        opacity: f32,
    },
    Frame {
        ratio: f32,
        color: Rgba,
        shade: f32,
    },
    Grid {
        grid: GridKind,
        color: Rgba,
        flip: Flip,
    },
}

/// Superposition définie par l'utilisateur (fichier TOML).
#[derive(Clone, Debug)]
pub struct UserOverlay {
    name: String,
    shapes: Vec<Resolved>,
}

/// Rapport « 4:5 » → 0,8 (largeur / hauteur) ; `None` si mal écrit.
fn parse_ratio(text: &str) -> Option<f32> {
    let (w, h) = text.split_once(':')?;
    let (w, h): (f32, f32) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    (w > 0.0 && h > 0.0).then_some(w / h)
}

/// Chemin d'un logo : `~` développé, relatif au dossier du fichier de superposition.
fn resolve_path(base: &Path, path: &str) -> PathBuf {
    let path = match path.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| PathBuf::from(path)),
        None => PathBuf::from(path),
    };
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

impl UserOverlay {
    /// Lit une superposition : `text` est le contenu du fichier, `name` son nom par
    /// défaut, `base` le dossier des chemins relatifs. Erreur : TOML invalide, couleur,
    /// format ou logo illisible.
    pub fn parse(text: &str, name: &str, base: &Path) -> Result<UserOverlay, String> {
        let file: OverlayFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let color = |hex: &str, opacity: f32| {
            Rgba::parse(hex, opacity).ok_or_else(|| crate::t!("overlay-bad-color", color = hex))
        };
        let mut shapes = Vec::new();
        for spec in file.shapes {
            shapes.push(match spec {
                ShapeSpec::Text {
                    text,
                    anchor,
                    size,
                    color: c,
                    opacity,
                    margin,
                } => Resolved::Text {
                    text,
                    anchor,
                    size,
                    color: color(&c, opacity)?,
                    margin,
                },
                ShapeSpec::Image {
                    path,
                    anchor,
                    width,
                    margin,
                    opacity,
                } => {
                    let path = resolve_path(base, &path);
                    let (w, h) = image::image_dimensions(&path).map_err(|e| {
                        crate::t!(
                            "overlay-bad-image",
                            path = path.display().to_string(),
                            error = e.to_string()
                        )
                    })?;
                    Resolved::Image {
                        path,
                        logo_aspect: w as f32 / h.max(1) as f32,
                        anchor,
                        width,
                        margin,
                        opacity,
                    }
                }
                ShapeSpec::Frame {
                    ratio,
                    color: c,
                    opacity,
                    shade,
                } => Resolved::Frame {
                    ratio: parse_ratio(&ratio)
                        .ok_or_else(|| crate::t!("overlay-bad-ratio", ratio = ratio.as_str()))?,
                    color: color(&c, opacity)?,
                    shade,
                },
                ShapeSpec::Grid {
                    grid,
                    color: c,
                    opacity,
                    flip,
                } => Resolved::Grid {
                    grid,
                    color: color(&c, opacity)?,
                    flip,
                },
            });
        }
        Ok(UserOverlay {
            name: file.name.unwrap_or_else(|| name.to_string()),
            shapes,
        })
    }
}

/// Coin haut gauche d'une boîte de taille `size` (relative), posée selon `anchor` avec
/// les marges `margin` (relatives, par axe).
fn place(anchor: Anchor, size: [f32; 2], margin: [f32; 2]) -> [f32; 2] {
    let [fx, fy] = anchor.factors();
    let axis = |f: f32, size: f32, margin: f32| {
        // Début : après la marge ; fin : avant ; milieu : centré.
        margin + f * (1.0 - 2.0 * margin - size)
    };
    [axis(fx, size[0], margin[0]), axis(fy, size[1], margin[1])]
}

impl Overlay for UserOverlay {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn shapes(&self, aspect: f32) -> Vec<Shape> {
        let aspect = aspect.max(1e-3);
        let mut out = Vec::new();
        // Zone où se placent les formes : l'image, puis le dernier cadre posé (un texte
        // ou un logo après un cadre 4:5 tombe dans la publication, pas dans le hors-cadre).
        let mut area = ([0.0, 0.0], [1.0, 1.0]);
        for shape in &self.shapes {
            let size = [area.1[0] - area.0[0], area.1[1] - area.0[1]];
            // Rapport largeur / hauteur réel de la zone.
            let local = local_shapes(shape, aspect * size[0] / size[1].max(1e-6));
            let before = area;
            if let (Resolved::Frame { .. }, Some(Shape::Rect { min, max, .. })) =
                (shape, local.last())
            {
                area = (map(before, *min), map(before, *max));
            }
            out.extend(local.into_iter().map(|s| map_shape(before, s)));
        }
        out
    }
}

/// Point relatif à la zone `area` → point relatif à l'image.
fn map(area: ([f32; 2], [f32; 2]), p: [f32; 2]) -> [f32; 2] {
    [
        area.0[0] + p[0] * (area.1[0] - area.0[0]),
        area.0[1] + p[1] * (area.1[1] - area.0[1]),
    ]
}

/// Forme calculée dans la zone `area` → forme dans l'image.
fn map_shape(area: ([f32; 2], [f32; 2]), shape: Shape) -> Shape {
    let m = |p| map(area, p);
    match shape {
        Shape::Line { from, to, color } => Shape::Line {
            from: m(from),
            to: m(to),
            color,
        },
        Shape::Rect {
            min,
            max,
            stroke,
            fill,
        } => Shape::Rect {
            min: m(min),
            max: m(max),
            stroke,
            fill,
        },
        Shape::Text {
            at,
            anchor,
            text,
            size,
            color,
        } => Shape::Text {
            at: m(at),
            anchor,
            text,
            size: size * (area.1[1] - area.0[1]),
            color,
        },
        Shape::Image {
            path,
            min,
            max,
            opacity,
        } => Shape::Image {
            path,
            min: m(min),
            max: m(max),
            opacity,
        },
    }
}

/// Formes d'une forme résolue, relatives à une zone de rapport `aspect` (le cadre en
/// dernier, après son voile).
fn local_shapes(shape: &Resolved, aspect: f32) -> Vec<Shape> {
    let aspect = aspect.max(1e-3);
    // Fractions du petit côté → fractions de la largeur et de la hauteur.
    let short = [aspect.min(1.0) / aspect, aspect.min(1.0)];
    let mut out = Vec::new();
    match shape {
        Resolved::Text {
            text,
            anchor,
            size,
            color,
            margin,
        } => {
            let [fx, fy] = anchor.factors();
            let [mx, my] = [margin * short[0], margin * short[1]];
            out.push(Shape::Text {
                at: [mx + fx * (1.0 - 2.0 * mx), my + fy * (1.0 - 2.0 * my)],
                anchor: *anchor,
                text: text.clone(),
                size: size * short[1],
                color: *color,
            });
        }
        Resolved::Image {
            path,
            logo_aspect,
            anchor,
            width,
            margin,
            opacity,
        } => {
            // Hauteur relative : largeur réelle / rapport du logo / hauteur réelle.
            let size = [*width, width * aspect / logo_aspect.max(1e-3)];
            let min = place(*anchor, size, [margin * short[0], margin * short[1]]);
            out.push(Shape::Image {
                path: path.clone(),
                min,
                max: [min[0] + size[0], min[1] + size[1]],
                opacity: *opacity,
            });
        }
        Resolved::Frame {
            ratio,
            color,
            shade,
        } => {
            // Le plus grand cadre de ce format, centré.
            let (w, h) = if *ratio > aspect {
                (1.0, aspect / ratio)
            } else {
                (ratio / aspect, 1.0)
            };
            let min = [(1.0 - w) / 2.0, (1.0 - h) / 2.0];
            let max = [min[0] + w, min[1] + h];
            if *shade > 0.0 {
                let veil = Some(Rgba([0, 0, 0, (shade.clamp(0.0, 1.0) * 255.0) as u8]));
                // Bandes hors du cadre (deux au plus, vides sinon).
                for (a, b) in [
                    ([0.0, 0.0], [1.0, min[1]]),
                    ([0.0, max[1]], [1.0, 1.0]),
                    ([0.0, min[1]], [min[0], max[1]]),
                    ([max[0], min[1]], [1.0, max[1]]),
                ] {
                    if b[0] - a[0] > 1e-4 && b[1] - a[1] > 1e-4 {
                        out.push(Shape::Rect {
                            min: a,
                            max: b,
                            stroke: None,
                            fill: veil,
                        });
                    }
                }
            }
            out.push(Shape::Rect {
                min,
                max,
                stroke: Some(*color),
                fill: None,
            });
        }
        Resolved::Grid { grid, color, flip } => out.extend(
            grid_lines(*grid, aspect, *color)
                .into_iter()
                .map(|s| match s {
                    Shape::Line { from, to, color } => Shape::Line {
                        from: flip.apply(from),
                        to: flip.apply(to),
                        color,
                    },
                    other => other,
                }),
        ),
    }
    out
}

/// Superpositions de l'utilisateur : chaque fichier `.toml` de `dir`, triés par nom de
/// fichier. Renvoie aussi les erreurs (fichier, message), sans arrêter : un fichier mal
/// écrit n'empêche pas les autres. Dossier absent : rien.
pub fn load_dir(dir: &Path) -> (Vec<UserOverlay>, Vec<(PathBuf, String)>) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
        })
        .collect();
    files.sort();
    let mut overlays = Vec::new();
    let mut errors = Vec::new();
    for path in files {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let read = std::fs::read_to_string(&path).map_err(|e| e.to_string());
        match read.and_then(|text| UserOverlay::parse(&text, &name, dir)) {
            Ok(overlay) => overlays.push(overlay),
            Err(e) => errors.push((path, e)),
        }
    }
    (overlays, errors)
}

/// Dossier des superpositions de l'utilisateur, dans le dossier de données.
pub fn user_dir(data: &Path) -> PathBuf {
    data.join("overlays")
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgba = Rgba([255, 255, 255, 255]);

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn thirds_and_golden_lines() {
        let lines = grid_lines(GridKind::Thirds, 1.5, WHITE);
        assert_eq!(lines.len(), 4);
        assert!(matches!(lines[0], Shape::Line { from, .. } if close(from[0], 1.0 / 3.0)));
        let golden = grid_lines(GridKind::Golden, 1.5, WHITE);
        assert!(matches!(golden[1], Shape::Line { from, .. } if close(from[0], 0.618_034)));
    }

    #[test]
    fn the_golden_spiral_is_continuous_and_winds_inward() {
        let lines = spiral_lines(WHITE);
        let points: Vec<([f32; 2], [f32; 2])> = lines
            .iter()
            .map(|s| match s {
                Shape::Line { from, to, .. } => (*from, *to),
                _ => panic!(),
            })
            .collect();
        // D'un seul tenant : chaque segment part de la fin du précédent.
        for pair in points.windows(2) {
            let (a, b) = (pair[0].1, pair[1].0);
            assert!(close(a[0], b[0]) && close(a[1], b[1]), "{a:?} → {b:?}");
        }
        // Part du coin bas gauche, s'enroule vers l'œil (≈ 0,72 × 0,72 du cadre), dans
        // l'image.
        assert!(close(points[0].0[0], 0.0) && close(points[0].0[1], 1.0));
        let end = points.last().unwrap().1;
        assert!(
            (end[0] - 0.724).abs() < 0.02 && (end[1] - 0.724).abs() < 0.03,
            "{end:?}"
        );
        assert!(
            points
                .iter()
                .all(|(a, _)| (-1e-4..=1.0001).contains(&a[0]) && (-1e-4..=1.0001).contains(&a[1]))
        );
        // En hauteur : pivotée (x et y échangés).
        let portrait = grid_lines(GridKind::Spiral, 0.66, WHITE);
        assert!(
            matches!(portrait[0], Shape::Line { from, .. } if close(from[0], 1.0) && close(from[1], 0.0))
        );
        // Retournée dans un fichier : miroir.
        assert_eq!(Flip::Xy.apply([0.25, 0.1]), [0.75, 0.9]);
    }

    #[test]
    fn reciprocal_diagonals_are_perpendicular_in_the_real_image() {
        let aspect = 1.5;
        let lines = grid_lines(GridKind::Diagonals, aspect, WHITE);
        let Shape::Line { from, to, .. } = lines[2] else {
            panic!()
        };
        // Diagonale (0,0)-(1,1) dans l'image réelle : (a, 1). Segment depuis (1,0).
        let d = [aspect, 1.0];
        let s = [(to[0] - from[0]) * aspect, to[1] - from[1]];
        assert!(close(d[0] * s[0] + d[1] * s[1], 0.0), "perpendiculaire");
    }

    #[test]
    fn a_user_overlay_places_text_logo_and_frame() {
        let dir = tempfile::tempdir().unwrap();
        image::RgbaImage::new(200, 100)
            .save(dir.path().join("logo.png"))
            .unwrap();
        let text = r##"
            name = "Publication Insta"
            [[shape]]
            kind = "frame"
            ratio = "4:5"
            [[shape]]
            kind = "image"
            path = "logo.png"
            anchor = "top-left"
            width = 0.2
            margin = 0.05
            [[shape]]
            kind = "text"
            text = "© moi"
            color = "#ff0000"
            opacity = 0.5
        "##;
        let overlay = UserOverlay::parse(text, "insta", dir.path()).unwrap();
        assert_eq!(overlay.name(), "Publication Insta");
        // Paysage 3:2 : cadre 4:5 pleine hauteur, centré, deux bandes assombries.
        let shapes = overlay.shapes(1.5);
        let frame = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Rect {
                    min,
                    max,
                    stroke: Some(_),
                    ..
                } => Some((*min, *max)),
                _ => None,
            })
            .unwrap();
        assert!(close(frame.1[0] - frame.0[0], 0.8 / 1.5) && close(frame.1[1] - frame.0[1], 1.0));
        assert!(close(frame.0[0] + frame.1[0], 1.0), "centré");
        let veils = shapes
            .iter()
            .filter(|s| matches!(s, Shape::Rect { fill: Some(_), .. }))
            .count();
        assert_eq!(veils, 2);
        // Après le cadre, le logo et le texte se placent dans le cadre (largeur fw, de x0).
        let (fw, x0) = (0.8 / 1.5, (1.0 - 0.8 / 1.5) / 2.0);
        // Logo 2:1 en haut à gauche : 0,2 de la largeur du cadre ; le cadre réel fait
        // 0,8 × 1, donc hauteur 0,2 × 0,8 / 2 = 0,08 ; marge 5 % du petit côté du cadre.
        let logo = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Image { min, max, .. } => Some((*min, *max)),
                _ => None,
            })
            .unwrap();
        assert!(close(logo.1[0] - logo.0[0], 0.2 * fw));
        assert!(close(logo.1[1] - logo.0[1], 0.08));
        assert!(close(logo.0[0], x0 + 0.05 * fw), "{logo:?}");
        assert!(close(logo.0[1], 0.05 * 0.8));
        // Texte en bas à droite du cadre (défaut), couleur à demi opaque.
        let text = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Text {
                    at, anchor, color, ..
                } => Some((*at, *anchor, *color)),
                _ => None,
            })
            .unwrap();
        assert_eq!(text.1, Anchor::BottomRight);
        assert!(close(text.0[0], x0 + (1.0 - 0.03) * fw), "{:?}", text.0);
        assert!(close(text.0[1], 1.0 - 0.03 * 0.8));
        assert_eq!(text.2, Rgba([255, 0, 0, 128]));
    }

    #[test]
    fn bad_files_are_reported_and_others_kept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.toml"),
            "[[shape]]\nkind = \"grid\"\ngrid = \"thirds\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("b.toml"), "[[shape]]\nkind = \"cercle\"\n").unwrap();
        std::fs::write(
            dir.path().join("c.toml"),
            "[[shape]]\nkind = \"frame\"\nratio = \"4/5\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("d.toml"),
            "[[shape]]\nkind = \"text\"\ntext = \"x\"\ntaille = 3\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("notes.txt"), "pas une superposition").unwrap();
        let (overlays, errors) = load_dir(dir.path());
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].name(), "a", "nom du fichier par défaut");
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(errors[1].1.contains("4/5"), "{}", errors[1].1);
        assert!(errors[2].1.contains("taille"), "{}", errors[2].1);
        assert!(load_dir(&dir.path().join("absent")).0.is_empty());
    }

    #[test]
    fn the_documented_example_parses() {
        let text = include_str!("../doc/overlay.example.toml");
        let dir = tempfile::tempdir().unwrap();
        // L'exemple cite un logo : un faux, au chemin voulu, pour le test.
        image::RgbaImage::new(10, 10)
            .save(dir.path().join("logo.png"))
            .unwrap();
        let overlay = UserOverlay::parse(text, "exemple", dir.path()).unwrap();
        assert!(!overlay.shapes(1.5).is_empty());
    }
}

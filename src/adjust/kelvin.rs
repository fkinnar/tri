//! Balance des blancs en **kelvins et teinte** (retour du 10/11/2026 : les curseurs en
//! « chaleur » relative ne parlaient pas ; on tape une température comme dans un logiciel
//! de développement).
//!
//! Modèle, volontairement simple et le même pour toutes les sources :
//!
//! - un éclairage (température `K`, teinte) est un point du plan CIE 1960 (u, v) : le
//!   lieu de Planck à `K` (approximation de Krystek, 1000 à 15 000 K), décalé
//!   perpendiculairement de `teinte / TINT_SCALE` (Duv ; positif : vers le vert) ;
//! - sa couleur en sRGB linéaire ([`white`]) est celle d'un blanc éclairé par lui, vue
//!   dans un rendu équilibré pour D65 ;
//! - l'image affichée est équilibrée pour l'éclairage de la prise de vue, de couleur
//!   `shot` (lue dans le RAW : [`crate::raw::RawFile::as_shot_white`] ; D65 supposé pour
//!   une image plate). Équilibrer pour un autre éclairage `W`, c'est multiplier chaque
//!   canal par `shot / W` (adaptation de von Kries dans l'espace de sortie), ramené au
//!   vert : les [`Gains`] relatifs déjà stockés. Les kelvins ne sont donc qu'une
//!   *présentation* des gains ([`gains`], [`of_gains`]).
//!
//! Approximation assumée : chaque logiciel calcule ses kelvins avec son propre modèle du
//! boîtier ; ceux de `tri` en sont proches, pas identiques.

use super::Gains;

/// Teinte affichée pour un Duv de 1 (≈ l'échelle de Lightroom : ±150 pour ±0,05).
pub const TINT_SCALE: f32 = 3000.0;

/// Bornes du modèle (validité de l'approximation du lieu de Planck).
pub const KELVIN_MIN: f32 = 1500.0;
/// Voir [`KELVIN_MIN`].
pub const KELVIN_MAX: f32 = 15000.0;

/// Éclairage de la prise de vue supposé pour une image sans données du boîtier (image
/// plate) : D65, le blanc du sRGB.
pub const D65: [f32; 3] = [1.0, 1.0, 1.0];

/// Lieu de Planck en (u, v) CIE 1960 (Krystek, 1985).
fn locus(t: f64) -> (f64, f64) {
    let u = (0.860_117_757 + 1.541_182_54e-4 * t + 1.286_412_12e-7 * t * t)
        / (1.0 + 8.424_202_35e-4 * t + 7.081_451_63e-7 * t * t);
    let v = (0.317_398_726 + 4.228_062_45e-5 * t + 4.204_816_91e-8 * t * t)
        / (1.0 - 2.897_418_16e-5 * t + 1.614_560_53e-7 * t * t);
    (u, v)
}

/// Normale unitaire au lieu en `t`, côté vert (v croissant).
fn normal(t: f64) -> (f64, f64) {
    let (a, b) = (locus(t - 1.0), locus(t + 1.0));
    let (du, dv) = (b.0 - a.0, b.1 - a.1);
    let len = (du * du + dv * dv).sqrt().max(1e-12);
    let n = (-dv / len, du / len);
    if n.1 >= 0.0 { n } else { (-n.0, -n.1) }
}

/// (u, v) d'un éclairage.
fn uv(kelvin: f32, tint: f32) -> (f64, f64) {
    let t = f64::from(kelvin.clamp(KELVIN_MIN, KELVIN_MAX));
    let duv = f64::from(tint / TINT_SCALE);
    let (u, v) = locus(t);
    let n = normal(t);
    (u + duv * n.0, v + duv * n.1)
}

/// sRGB linéaire (D65) vers XYZ.
const SRGB_TO_XYZ: [[f64; 3]; 3] = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175_0],
    [0.019_333_9, 0.119_192_0, 0.950_304_1],
];

/// XYZ vers sRGB linéaire (D65).
const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [3.240_454_2, -1.537_138_5, -0.498_531_4],
    [-0.969_266_0, 1.876_010_8, 0.041_556_0],
    [0.055_643_4, -0.204_025_9, 1.057_225_2],
];

fn mul(m: &[[f64; 3]; 3], x: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| (0..3).map(|j| m[i][j] * x[j]).sum())
}

/// Couleur en sRGB linéaire (vert = 1) d'un blanc éclairé par (`kelvin`, `tint`).
pub fn white(kelvin: f32, tint: f32) -> [f32; 3] {
    let (u, v) = uv(kelvin, tint);
    let d = 2.0 * u - 8.0 * v + 4.0;
    let (x, y) = (3.0 * u / d, 2.0 * v / d);
    let xyz = [x / y, 1.0, (1.0 - x - y) / y];
    let rgb = mul(&XYZ_TO_SRGB, xyz);
    let g = rgb[1].max(1e-9);
    rgb.map(|c| (c / g).max(1e-6) as f32)
}

/// Température et teinte d'un éclairage de couleur `rgb` (sRGB linéaire) : le point du
/// lieu de Planck le plus proche, et l'écart à lui. Inverse de [`white`].
pub fn temperature(rgb: [f32; 3]) -> (f32, f32) {
    let xyz = mul(&SRGB_TO_XYZ, rgb.map(f64::from));
    let d = xyz[0] + 15.0 * xyz[1] + 3.0 * xyz[2];
    if d <= 0.0 {
        return (6500.0, 0.0);
    }
    let (u, v) = (4.0 * xyz[0] / d, 6.0 * xyz[1] / d);
    let dist = |t: f64| {
        let (a, b) = locus(t);
        (u - a).powi(2) + (v - b).powi(2)
    };
    // En mired (1e6 / K), à peu près uniforme le long du lieu : grossier puis affiné.
    let (lo, hi) = (1e6 / f64::from(KELVIN_MAX), 1e6 / f64::from(KELVIN_MIN));
    let steps = 200;
    let at = |i: usize| lo + (hi - lo) * i as f64 / steps as f64;
    let best = (0..=steps)
        .min_by(|&a, &b| dist(1e6 / at(a)).total_cmp(&dist(1e6 / at(b))))
        .unwrap_or(0);
    let (mut a, mut b) = (at(best.saturating_sub(1)), at((best + 1).min(steps)));
    for _ in 0..60 {
        let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
        if dist(1e6 / m1) < dist(1e6 / m2) {
            b = m2;
        } else {
            a = m1;
        }
    }
    let t = 1e6 / ((a + b) / 2.0);
    let (lu, lv) = locus(t);
    let n = normal(t);
    let duv = (u - lu) * n.0 + (v - lv) * n.1;
    (t as f32, (duv as f32) * TINT_SCALE)
}

/// Gains qui équilibrent pour l'éclairage (`kelvin`, `tint`) une image équilibrée à la
/// prise de vue pour l'éclairage de couleur `shot`.
pub fn gains(shot: [f32; 3], kelvin: f32, tint: f32) -> Gains {
    let w = white(kelvin, tint);
    let g = shot[1] / w[1];
    Gains {
        red: shot[0] / w[0] / g,
        blue: shot[2] / w[2] / g,
    }
}

/// Température et teinte des `gains` d'une image prise sous `shot` : inverse de
/// [`gains`]. Sans gains, celles de la prise de vue.
pub fn of_gains(shot: [f32; 3], gains: Gains) -> (f32, f32) {
    temperature([
        shot[0] / gains.red.max(1e-6),
        shot[1],
        shot[2] / gains.blue.max(1e-6),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d65_is_about_6500_kelvin_and_slightly_green() {
        let (k, tint) = temperature(D65);
        assert!((k - 6504.0).abs() < 60.0, "{k}");
        // D65 est un peu au-dessus du lieu de Planck (Duv ≈ +0,003).
        assert!(tint > 0.0 && tint < 20.0, "{tint}");
    }

    #[test]
    fn white_and_temperature_are_inverse() {
        for (k, tint) in [
            (2850.0, 0.0),
            (5500.0, 10.0),
            (7500.0, -20.0),
            (12000.0, 5.0),
        ] {
            let (back, t) = temperature(white(k, tint));
            assert!((back - k).abs() / k < 0.005, "{k} → {back}");
            assert!((t - tint).abs() < 0.5, "{tint} → {t}");
        }
    }

    #[test]
    fn a_warmer_setting_warms_the_image_and_tint_adds_magenta() {
        // Comme dans les logiciels de développement : plus de kelvins, image plus chaude ;
        // teinte positive, plus magenta.
        let shot = white(5500.0, 0.0);
        let warm = gains(shot, 7500.0, 0.0);
        assert!(warm.warmth() > 0.0, "{warm:?}");
        let cold = gains(shot, 3200.0, 0.0);
        assert!(cold.warmth() < 0.0, "{cold:?}");
        let magenta = gains(shot, 5500.0, 30.0);
        assert!(magenta.tint() > 0.0, "{magenta:?}");
        assert!(
            gains(shot, 5500.0, 0.0).is_none(),
            "the shot itself: nothing"
        );
    }

    #[test]
    fn gains_and_kelvin_round_trip() {
        let shot = white(4300.0, 8.0);
        let g = gains(shot, 6100.0, -12.0);
        let (k, t) = of_gains(shot, g);
        assert!((k - 6100.0).abs() < 30.0, "{k}");
        assert!((t + 12.0).abs() < 0.5, "{t}");
        let (k0, t0) = of_gains(shot, Gains::NONE);
        assert!(
            (k0 - 4300.0).abs() < 25.0 && (t0 - 8.0).abs() < 0.5,
            "{k0} {t0}"
        );
    }
}

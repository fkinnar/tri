//! Logique du plein écran, sans dépendance à l'affichage : quelle image montrer (JPEG du
//! boîtier, rendu RAW, vignette en attendant), géométrie de l'ajustement et du 100 %.

#[cfg(test)]
use eframe::egui::pos2;
use eframe::egui::{Pos2, Rect, Vec2, vec2};
use tri::cache::Level;
use tri::media::MediaKind;

/// Source préférée pour les RAW en plein écran.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// JPEG embarqué par le boîtier : disponible très vite, reflète les réglages du boîtier.
    #[default]
    Embedded,
    /// Rendu RAW par LibRaw : fidèle aux données du capteur, calculé en fond.
    Raw,
}

impl Source {
    /// L'autre source.
    pub fn toggled(self) -> Source {
        match self {
            Source::Embedded => Source::Raw,
            Source::Raw => Source::Embedded,
        }
    }
}

/// Niveau du 1:1 pour une source : le JPEG embarqué à sa taille d'origine, ou le rendu RAW
/// complet. Pour une image plate, l'image elle-même.
pub fn zoom_level(kind: MediaKind, source: Source) -> Level {
    match (kind, source) {
        (MediaKind::Raw, Source::Embedded) => Level::EmbeddedFull,
        _ => Level::Full,
    }
}

/// Niveau à afficher, parmi ceux disponibles (`have`). En 1:1 (`zoomed`), le 1:1 de la
/// source choisie s'il est prêt ; sinon la source préférée ajustée, puis l'autre, puis la
/// vignette. Source et échelle sont indépendantes : zoomer ne change jamais de source.
pub fn pick(
    kind: MediaKind,
    preferred: Source,
    zoomed: bool,
    have: impl Fn(Level) -> bool,
) -> Option<Level> {
    let zoom = zoom_level(kind, preferred);
    if zoomed && have(zoom) {
        return Some(zoom);
    }
    let order: &[Level] = match (kind, preferred) {
        (MediaKind::Flat, _) => &[Level::Standard, Level::Thumbnail],
        (MediaKind::Raw, Source::Embedded) => &[Level::Embedded, Level::Standard, Level::Thumbnail],
        (MediaKind::Raw, Source::Raw) => &[Level::Standard, Level::Embedded, Level::Thumbnail],
    };
    order.iter().copied().find(|&l| have(l))
}

/// Ce qui est affiché, en clair.
pub fn label(kind: MediaKind, level: Level) -> String {
    match (kind, level) {
        (_, Level::Thumbnail) => t!("shown-thumbnail"),
        (MediaKind::Raw, Level::Embedded) => t!("shown-embedded"),
        (MediaKind::Raw, Level::Standard) => t!("shown-raw"),
        (MediaKind::Raw, Level::Full) => t!("shown-raw-full"),
        (MediaKind::Raw, Level::EmbeddedFull) => t!("shown-embedded-full"),
        (MediaKind::Flat, Level::Full) => t!("shown-image-full"),
        (_, Level::Exposure) => t!("shown-exposure"),
        (MediaKind::Flat, _) => t!("shown-image"),
    }
}

/// Image de taille `image` ajustée dans `view` (proportions gardées, centrée, agrandie au
/// besoin).
pub fn fit_rect(view: Rect, image: Vec2) -> Rect {
    let scale = (view.width() / image.x).min(view.height() / image.y);
    Rect::from_center_size(view.center(), image * scale)
}

/// Rectangle d'une image de `size` points dont le point `center` (coordonnées relatives,
/// 0..1) est au centre de `view`.
pub fn zoom_rect(view: Rect, size: Vec2, center: Vec2) -> Rect {
    let min = view.center() - vec2(center.x * size.x, center.y * size.y);
    Rect::from_min_size(min, size)
}

/// Centre ramené dans les limites : l'image ne laisse pas de bord vide tant qu'elle est plus
/// grande que la vue ; plus petite, elle reste centrée.
pub fn clamp_center(view: Vec2, size: Vec2, center: Vec2) -> Vec2 {
    let axis = |v: f32, s: f32, c: f32| {
        if s <= v {
            0.5
        } else {
            let half = v / s / 2.0;
            c.clamp(half, 1.0 - half)
        }
    };
    vec2(
        axis(view.x, size.x, center.x),
        axis(view.y, size.y, center.y),
    )
}

/// Coordonnées relatives (0..1) du point `pos` dans l'image affichée en `rect`.
pub fn relative(rect: Rect, pos: Pos2) -> Vec2 {
    vec2(
        ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0),
        ((pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0),
    )
}

/// Centre de zoom qui garde le point de l'image situé sous `pointer` au même endroit
/// de l'écran une fois l'image affichée à `size`.
pub fn center_keeping(view: Rect, size: Vec2, point: Vec2, pointer: Pos2) -> Vec2 {
    let offset = pointer - view.center();
    vec2(point.x - offset.x / size.x, point.y - offset.y / size.y)
}

/// Taille (pixels) supposée du 1:1 d'une image dont on n'a encore qu'un aperçu de
/// proportions `aspect` (largeur / hauteur), d'après le dernier 1:1 affiché (`last`) :
/// même côté long, orientation de l'aperçu. Une série vient en général du même boîtier.
pub fn estimate_zoom_size(last: Vec2, aspect: f32) -> Vec2 {
    let long = last.x.max(last.y);
    if aspect >= 1.0 {
        vec2(long, long / aspect)
    } else {
        vec2(long * aspect, long)
    }
}

/// Origine de l'image (coin haut gauche), pour les tests.
#[cfg(test)]
fn origin(r: Rect) -> Pos2 {
    pos2(r.min.x, r.min.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have(levels: &[Level]) -> impl Fn(Level) -> bool + '_ {
        move |l| levels.contains(&l)
    }

    #[test]
    fn raw_prefers_embedded_by_default() {
        use Level::*;
        let all = [Thumbnail, Embedded, Standard, Full];
        assert_eq!(
            pick(MediaKind::Raw, Source::Embedded, false, have(&all)),
            Some(Embedded)
        );
        assert_eq!(
            pick(MediaKind::Raw, Source::Raw, false, have(&all)),
            Some(Standard)
        );
        assert_eq!(
            pick(MediaKind::Raw, Source::Raw, true, have(&all)),
            Some(Full)
        );
        assert_eq!(
            pick(
                MediaKind::Raw,
                Source::Embedded,
                true,
                have(&[Embedded, EmbeddedFull, Full])
            ),
            Some(EmbeddedFull),
            "zoomer en JPEG reste en JPEG"
        );
    }

    #[test]
    fn falls_back_to_what_is_ready() {
        use Level::*;
        assert_eq!(
            pick(
                MediaKind::Raw,
                Source::Raw,
                false,
                have(&[Thumbnail, Embedded])
            ),
            Some(Embedded),
            "rendu RAW pas encore prêt"
        );
        assert_eq!(
            pick(
                MediaKind::Raw,
                Source::Embedded,
                true,
                have(&[Thumbnail, Standard, Full])
            ),
            Some(Standard),
            "1:1 du JPEG pas prêt, pas d'aperçu boîtier : on ne bascule pas sur le RAW 100 %"
        );
        assert_eq!(
            pick(
                MediaKind::Raw,
                Source::Raw,
                true,
                have(&[Embedded, Standard])
            ),
            Some(Standard),
            "RAW 100 % pas encore prêt : rendu RAW ajusté"
        );
        assert_eq!(
            pick(MediaKind::Raw, Source::Embedded, false, have(&[Thumbnail])),
            Some(Thumbnail)
        );
        assert_eq!(
            pick(MediaKind::Raw, Source::Embedded, false, have(&[])),
            None
        );
    }

    #[test]
    fn flat_images_ignore_the_source() {
        use Level::*;
        for source in [Source::Embedded, Source::Raw] {
            assert_eq!(
                pick(
                    MediaKind::Flat,
                    source,
                    false,
                    have(&[Thumbnail, Embedded, Standard])
                ),
                Some(Standard)
            );
        }
    }

    #[test]
    fn labels_say_what_is_shown() {
        assert_eq!(label(MediaKind::Raw, Level::Embedded), "JPEG boîtier");
        assert_eq!(label(MediaKind::Raw, Level::Standard), "Rendu RAW");
        assert_eq!(label(MediaKind::Flat, Level::Standard), "Image");
        assert!(label(MediaKind::Raw, Level::Full).contains("100 %"));
        assert!(
            !label(MediaKind::Raw, Level::EmbeddedFull).contains("100 %"),
            "honnête"
        );
        assert_eq!(
            zoom_level(MediaKind::Raw, Source::Embedded),
            Level::EmbeddedFull
        );
        assert_eq!(zoom_level(MediaKind::Raw, Source::Raw), Level::Full);
        assert_eq!(zoom_level(MediaKind::Flat, Source::Embedded), Level::Full);
        assert_eq!(Source::Embedded.toggled(), Source::Raw);
    }

    #[test]
    fn fit_keeps_proportions_and_centers() {
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
        let r = fit_rect(view, vec2(400.0, 600.0)); // portrait
        assert!((r.size() - vec2(500.0 * 400.0 / 600.0, 500.0)).length() < 1e-3);
        assert_eq!(r.center(), view.center());
    }

    #[test]
    fn zoom_places_the_center_point_at_the_view_center() {
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
        let r = zoom_rect(view, vec2(4000.0, 3000.0), vec2(0.5, 0.5));
        assert_eq!(r.center(), view.center());
        let r = zoom_rect(view, vec2(4000.0, 3000.0), vec2(0.0, 0.0));
        assert_eq!(origin(r), view.center(), "coin haut gauche au centre");
    }

    #[test]
    fn clamping_avoids_empty_borders() {
        let view = vec2(1000.0, 500.0);
        let size = vec2(4000.0, 2000.0);
        // Demi-vue = 1/8 de la largeur, 1/8 de la hauteur.
        assert_eq!(clamp_center(view, size, vec2(0.0, 1.0)), vec2(0.125, 0.875));
        assert_eq!(clamp_center(view, size, vec2(0.5, 0.3)), vec2(0.5, 0.3));
        // Image plus petite que la vue sur un axe : centrée sur cet axe.
        assert_eq!(
            clamp_center(view, vec2(4000.0, 400.0), vec2(0.2, 0.9)),
            vec2(0.2, 0.5)
        );
    }

    #[test]
    fn zoom_size_estimate_follows_the_orientation_of_the_preview() {
        let last = vec2(2944.0, 4416.0); // portrait
        assert_eq!(estimate_zoom_size(last, 2944.0 / 4416.0), last);
        let landscape = estimate_zoom_size(last, 4416.0 / 2944.0);
        assert_eq!(landscape.x, 4416.0);
        assert!((landscape.y - 2944.0).abs() < 0.5);
    }

    #[test]
    fn zooming_at_the_pointer_keeps_that_point_under_it() {
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
        let fitted = fit_rect(view, vec2(4000.0, 2000.0)); // 1000 × 500
        let pointer = pos2(800.0, 100.0);
        let point = relative(fitted, pointer); // (0.8, 0.2)
        let size = vec2(4000.0, 2000.0);
        let center = center_keeping(view, size, point, pointer);
        let zoomed = zoom_rect(view, size, center);
        assert!((relative(zoomed, pointer) - point).length() < 1e-4);
    }
}

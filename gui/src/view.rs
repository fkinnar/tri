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
    /// Développé par darktable d'après le sidecar (19/10/2026) : l'image telle qu'on l'a
    /// développée, à la demande.
    Developed,
}

impl Source {
    /// Source suivante (`b`) : boîtier, RAW, puis développé si darktable est là
    /// (`developed`), et retour au boîtier.
    pub fn next(self, developed: bool) -> Source {
        match self {
            Source::Embedded => Source::Raw,
            Source::Raw if developed => Source::Developed,
            Source::Raw | Source::Developed => Source::Embedded,
        }
    }
}

/// Niveau du 1:1 pour une source : le JPEG embarqué à sa taille d'origine, ou le rendu RAW
/// complet. Pour une image plate, l'image elle-même. `developed` : le niveau développé de
/// l'image (celui de son logiciel).
pub fn zoom_level(kind: MediaKind, source: Source, developed: Level) -> Level {
    match (kind, source) {
        (MediaKind::Raw, Source::Embedded) => Level::EmbeddedFull,
        // Pas de 1:1 développé (le logiciel rend à la taille de l'aperçu) : on zoome dedans.
        (_, Source::Developed) => developed,
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
    developed: Level,
    have: impl Fn(Level) -> bool,
) -> Option<Level> {
    let zoom = zoom_level(kind, preferred, developed);
    if zoomed && have(zoom) {
        return Some(zoom);
    }
    let order: &[Level] = match (kind, preferred) {
        // Développé : en attendant (ou sans développement), le rendu RAW, puis le boîtier.
        (MediaKind::Raw, Source::Developed) => &[
            developed,
            Level::Standard,
            Level::Embedded,
            Level::Thumbnail,
        ],
        (MediaKind::Flat, Source::Developed) => &[developed, Level::Standard, Level::Thumbnail],
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
        (_, Level::Exposure | Level::Sharpness | Level::Similarity) => t!("shown-exposure"),
        (_, Level::Developed(engine)) => t!("shown-developed", engine = engine.name()),
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

/// Palier suivant (`up`) ou précédent de l'échelle `current` (1,0 = 100 %), parmi
/// `steps`, entre `min` et `max` ; `None` : rien au-delà (déjà au bout). On peut dézoomer
/// sous l'ajusté (retour du 18/10/2026 : l'image plus petite que l'écran, centrée).
pub fn next_step(current: f32, steps: &[f32], up: bool, (min, max): (f32, f32)) -> Option<f32> {
    let eps = 1e-3;
    let inside = |s: f32| s >= min * (1.0 - eps) && s <= max * (1.0 + eps);
    let pick = |better: fn(f32, f32) -> f32, keep: &dyn Fn(f32) -> bool| {
        steps
            .iter()
            .copied()
            .filter(|&s| keep(s) && inside(s))
            .fold(None, |best: Option<f32>, s| {
                Some(best.map_or(s, |b| better(b, s)))
            })
    };
    if up {
        pick(f32::min, &|s| s > current * (1.0 + eps))
            .or_else(|| (current < max * (1.0 - eps)).then_some(max))
    } else {
        pick(f32::max, &|s| s < current * (1.0 - eps))
            .or_else(|| (current > min * (1.0 + eps)).then_some(min))
    }
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

    /// Niveau développé de darktable.
    const DT: Level = Level::Developed(tri::develop::Engine::Darktable);

    fn have(levels: &[Level]) -> impl Fn(Level) -> bool + '_ {
        move |l| levels.contains(&l)
    }

    #[test]
    fn raw_prefers_embedded_by_default() {
        use Level::*;
        let all = [Thumbnail, Embedded, Standard, Full];
        assert_eq!(
            pick(MediaKind::Raw, Source::Embedded, false, DT, have(&all)),
            Some(Embedded)
        );
        assert_eq!(
            pick(MediaKind::Raw, Source::Raw, false, DT, have(&all)),
            Some(Standard)
        );
        assert_eq!(
            pick(MediaKind::Raw, Source::Raw, true, DT, have(&all)),
            Some(Full)
        );
        assert_eq!(
            pick(
                MediaKind::Raw,
                Source::Embedded,
                true,
                DT,
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
                DT,
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
                DT,
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
                DT,
                have(&[Embedded, Standard])
            ),
            Some(Standard),
            "RAW 100 % pas encore prêt : rendu RAW ajusté"
        );
        assert_eq!(
            pick(
                MediaKind::Raw,
                Source::Embedded,
                false,
                DT,
                have(&[Thumbnail])
            ),
            Some(Thumbnail)
        );
        assert_eq!(
            pick(MediaKind::Raw, Source::Embedded, false, DT, have(&[])),
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
                    DT,
                    have(&[Thumbnail, Embedded, Standard])
                ),
                Some(Standard)
            );
        }
    }

    #[test]
    fn labels_say_what_is_shown() {
        assert_eq!(label(MediaKind::Raw, Level::Embedded), "Rendu boîtier");
        assert_eq!(label(MediaKind::Raw, Level::Standard), "Rendu RAW");
        assert_eq!(label(MediaKind::Flat, Level::Standard), "Image");
        assert!(label(MediaKind::Raw, Level::Full).contains("100 %"));
        assert!(
            !label(MediaKind::Raw, Level::EmbeddedFull).contains("100 %"),
            "honnête"
        );
        assert_eq!(
            zoom_level(MediaKind::Raw, Source::Embedded, DT),
            Level::EmbeddedFull
        );
        assert_eq!(zoom_level(MediaKind::Raw, Source::Raw, DT), Level::Full);
        assert_eq!(
            zoom_level(MediaKind::Flat, Source::Embedded, DT),
            Level::Full
        );
        assert_eq!(zoom_level(MediaKind::Raw, Source::Developed, DT), DT);
        // Développé pas encore prêt : le rendu RAW, puis le boîtier.
        assert_eq!(
            pick(MediaKind::Raw, Source::Developed, false, DT, |l| l
                == Level::Embedded),
            Some(Level::Embedded)
        );
        assert_eq!(
            pick(MediaKind::Raw, Source::Developed, false, DT, |_| true),
            Some(DT)
        );
        assert_eq!(Source::Raw.next(true), Source::Developed);
        assert_eq!(Source::Raw.next(false), Source::Embedded);
        assert_eq!(Source::Developed.next(true), Source::Embedded);
        assert_eq!(Source::Embedded.next(false), Source::Raw);
    }

    #[test]
    fn fit_keeps_proportions_and_centers() {
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
        let r = fit_rect(view, vec2(400.0, 600.0)); // portrait
        assert!((r.size() - vec2(500.0 * 400.0 / 600.0, 500.0)).length() < 1e-3);
        assert_eq!(r.center(), view.center());
    }

    #[test]
    fn zoom_steps_go_up_and_down_from_the_fit() {
        let steps = [0.125, 0.25, 0.5, 1.0, 2.0, 4.0];
        let limits = (0.1, 4.0);
        // Depuis l'ajusté (0,3) : 0,5 au-dessus, 0,25 au-dessous (sous l'ajusté).
        assert_eq!(next_step(0.3, &steps, true, limits), Some(0.5));
        assert_eq!(next_step(0.3, &steps, false, limits), Some(0.25));
        assert_eq!(next_step(0.5, &steps, true, limits), Some(1.0));
        assert_eq!(next_step(4.0, &steps, true, limits), None, "plafond");
        assert_eq!(next_step(1.5, &steps, false, limits), Some(1.0));
        assert_eq!(
            next_step(0.125, &steps, false, limits),
            Some(0.1),
            "jusqu'au minimum"
        );
        assert_eq!(next_step(0.1, &steps, false, limits), None);
        // Entre deux paliers (pincement) : le palier d'au-dessus.
        assert_eq!(next_step(1.3, &steps, true, limits), Some(2.0));
        assert_eq!(
            next_step(2.5, &steps, true, (0.1, 3.0)),
            Some(3.0),
            "max réglé"
        );
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

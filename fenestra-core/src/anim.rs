//! The transition engine: per-`WidgetId` animated styles. When an element's
//! resolved target style changes, the engine starts easing from the current
//! animated values toward the new target; colors interpolate in OKLCH.

use color::{AlphaColor, HueDirection, Oklch, Srgb};
use peniko::Color;

use crate::style::{Length, Paint, Style, Transition};

/// One in-flight (or settled) animation for a widget.
#[derive(Debug)]
pub(crate) struct Anim {
    /// Style at the moment the current segment started.
    from: Style,
    /// Target style of the current segment.
    to: Style,
    /// Clock time when the segment started.
    t0: f64,
    /// Frame stamp for garbage collection.
    pub(crate) seen: u64,
}

impl Anim {
    pub(crate) fn new(target: Style, now: f64, seen: u64) -> Self {
        Self {
            from: target.clone(),
            to: target,
            t0: now,
            seen,
        }
    }

    /// Advances toward `target`, restarting the segment from the current
    /// animated value whenever the target changes. Returns the style to
    /// paint and whether the animation is still running.
    pub(crate) fn advance(
        &mut self,
        target: &Style,
        transition: Transition,
        now: f64,
        seen: u64,
    ) -> (Style, bool) {
        self.seen = seen;
        let duration = f64::from(transition.duration_ms.max(1.0)) / 1000.0;
        let elapsed = ((now - self.t0) / duration).clamp(0.0, 1.0);
        #[expect(clippy::cast_possible_truncation, reason = "progress is 0..=1")]
        let eased = transition.easing.eval(elapsed as f32);
        let current = lerp_style(&self.from, &self.to, eased, transition);

        if *target != self.to {
            // Retarget: animated properties continue from wherever they
            // visually are right now; everything else snaps to the new
            // target immediately (lerp at t=0 does exactly that).
            self.from = current;
            self.to = target.clone();
            self.t0 = now;
            return (lerp_style(&self.from, &self.to, 0.0, transition), true);
        }
        // A segment whose endpoints agree is settled regardless of elapsed.
        let running = elapsed < 1.0 && self.from != self.to;
        if !running && self.from != self.to {
            self.from = self.to.clone();
        }
        (current, running)
    }
}

/// Samples a looping [`Keyframes`](crate::style::Keyframes) timeline
/// against the frame clock: finds the two stops around the current phase,
/// resolves both against `base`, and lerps every animatable property
/// between them. Reduced motion pins the first stop.
pub(crate) fn sample_keyframes(
    kf: &crate::style::Keyframes,
    theme: &crate::theme::Theme,
    base: &Style,
    now: f64,
    reduced_motion: bool,
) -> Style {
    if kf.stops.is_empty() {
        return base.clone();
    }
    let resolve = |i: usize| (kf.stops[i].1)(theme, base.clone());
    if reduced_motion {
        return resolve(0);
    }
    let period = f64::from(kf.duration_ms.max(1.0)) / 1000.0;
    #[expect(clippy::cast_possible_truncation, reason = "phase is in 0..1")]
    let phase = (now.rem_euclid(period) / period) as f32;
    let last = kf.stops.len() - 1;
    if phase <= kf.stops[0].0 {
        return resolve(0);
    }
    if phase >= kf.stops[last].0 {
        return resolve(last);
    }
    let next = kf
        .stops
        .iter()
        .position(|(at, _)| *at >= phase)
        .unwrap_or(last);
    let prev = next.saturating_sub(1);
    let span = (kf.stops[next].0 - kf.stops[prev].0).max(1e-6);
    let local = ((phase - kf.stops[prev].0) / span).clamp(0.0, 1.0);
    let all = Transition {
        easing: kf.easing,
        ..Transition::all()
    };
    lerp_style(&resolve(prev), &resolve(next), kf.easing.eval(local), all)
}

/// Interpolates the animatable properties enabled by `transition`; all other
/// properties snap to `b`.
fn lerp_style(a: &Style, b: &Style, t: f32, transition: Transition) -> Style {
    if t >= 1.0 {
        return b.clone();
    }
    let mut out = b.clone();
    if transition.colors {
        out.fill = match (&a.fill, &b.fill) {
            (Some(Paint::Solid(ca)), Some(Paint::Solid(cb))) => {
                Some(Paint::Solid(lerp_color(*ca, *cb, t)))
            }
            _ => b.fill.clone(),
        };
        out.border = match (a.border, b.border) {
            (Some(ba), Some(bb)) => Some(crate::style::Border {
                width: lerp_f32(ba.width, bb.width, t),
                color: lerp_color(ba.color, bb.color, t),
            }),
            _ => b.border,
        };
        out.text.color = match (a.text.color, b.text.color) {
            (Some(ca), Some(cb)) => Some(lerp_color(ca, cb, t)),
            _ => b.text.color,
        };
    }
    if transition.opacity {
        out.opacity = lerp_f32(a.opacity, b.opacity, t);
    }
    if transition.shadows {
        // Shadow layers lerp alpha (and geometry) pairwise where both sides
        // have a layer; extra layers snap.
        out.shadows = b
            .shadows
            .iter()
            .enumerate()
            .map(|(i, sb)| match a.shadows.get(i) {
                Some(sa) => crate::style::Shadow {
                    dx: lerp_f32(sa.dx, sb.dx, t),
                    dy: lerp_f32(sa.dy, sb.dy, t),
                    blur: lerp_f32(sa.blur, sb.blur, t),
                    spread: lerp_f32(sa.spread, sb.spread, t),
                    color: lerp_color(sa.color, sb.color, t),
                },
                None => *sb,
            })
            .collect();
    }
    if transition.lengths {
        out.width = lerp_length(a.width, b.width, t);
        out.height = lerp_length(a.height, b.height, t);
        out.min_width = lerp_length(a.min_width, b.min_width, t);
        out.min_height = lerp_length(a.min_height, b.min_height, t);
        out.gap = lerp_f32(a.gap, b.gap, t);
        out.padding = lerp_edges(a.padding, b.padding, t);
        out.margin = lerp_edges(a.margin, b.margin, t);
        out.corner_radius = crate::style::CornerRadius {
            tl: lerp_f32(a.corner_radius.tl, b.corner_radius.tl, t),
            tr: lerp_f32(a.corner_radius.tr, b.corner_radius.tr, t),
            br: lerp_f32(a.corner_radius.br, b.corner_radius.br, t),
            bl: lerp_f32(a.corner_radius.bl, b.corner_radius.bl, t),
        };
        out.path_trim = lerp_f32(a.path_trim, b.path_trim, t);
    }
    if transition.offsets {
        out.inset = crate::style::Inset {
            top: lerp_opt(a.inset.top, b.inset.top, t),
            right: lerp_opt(a.inset.right, b.inset.right, t),
            bottom: lerp_opt(a.inset.bottom, b.inset.bottom, t),
            left: lerp_opt(a.inset.left, b.inset.left, t),
        };
    }
    out
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_opt(a: Option<f32>, b: Option<f32>, t: f32) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(lerp_f32(a, b, t)),
        _ => b,
    }
}

fn lerp_length(a: Length, b: Length, t: f32) -> Length {
    match (a, b) {
        (Length::Px(a), Length::Px(b)) => Length::Px(lerp_f32(a, b, t)),
        (Length::Pct(a), Length::Pct(b)) => Length::Pct(lerp_f32(a, b, t)),
        _ => b,
    }
}

fn lerp_edges(a: crate::style::Edges, b: crate::style::Edges, t: f32) -> crate::style::Edges {
    crate::style::Edges {
        top: lerp_f32(a.top, b.top, t),
        right: lerp_f32(a.right, b.right, t),
        bottom: lerp_f32(a.bottom, b.bottom, t),
        left: lerp_f32(a.left, b.left, t),
    }
}

/// Lerps two sRGB colors through OKLCH (shorter hue arc), clamping the
/// result back into sRGB range.
pub(crate) fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let mut ao: AlphaColor<Oklch> = a.convert();
    let mut bo: AlphaColor<Oklch> = b.convert();
    // CSS Color 4 powerless hue: an achromatic endpoint adopts the other
    // endpoint's hue, otherwise gray->blue detours through arbitrary hues.
    const ACHROMATIC: f32 = 1e-4;
    if ao.components[1] < ACHROMATIC {
        ao.components[2] = bo.components[2];
    }
    if bo.components[1] < ACHROMATIC {
        bo.components[2] = ao.components[2];
    }
    let mixed = ao.lerp(bo, t, HueDirection::Shorter).convert::<Srgb>();
    let [r, g, bch, alpha] = mixed.components;
    Color::new([
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        bch.clamp(0.0, 1.0),
        alpha.clamp(0.0, 1.0),
    ])
}

//! Paints one resolved, laid-out element into a vello scene, following the
//! spec order: shadows, fill, border, clip layer, alpha layer, children.

use kurbo::{Affine, Point, Rect, RoundedRect, RoundedRectRadii, Stroke, Vec2};
use peniko::{ColorStop, ColorStops, Fill, Gradient};
use vello::Scene;

use crate::style::{CornerRadius, Paint, Shadow, Style};

/// CSS box-shadow semantics: the gaussian standard deviation is half the
/// blur radius (CSS Backgrounds & Borders 3, §7.1.1). Locked by the shadow
/// calibration snapshot.
const BLUR_TO_STD_DEV: f32 = 0.5;

/// Builds the rounded-rect path for a rect and per-corner radii, clamping
/// each radius (including `R_FULL` = infinity) to half the short side.
pub(crate) fn rounded_rect(rect: Rect, corners: CornerRadius) -> RoundedRect {
    let clamp = |r: f32| -> f64 {
        let max = 0.5 * rect.width().min(rect.height());
        f64::from(r).clamp(0.0, max.max(0.0))
    };
    RoundedRect::from_rect(
        rect,
        RoundedRectRadii::new(
            clamp(corners.tl),
            clamp(corners.tr),
            clamp(corners.br),
            clamp(corners.bl),
        ),
    )
}

/// Average corner radius, used where vello takes a single radius (shadows).
fn uniform_radius(rect: Rect, corners: CornerRadius) -> f64 {
    let max = (0.5 * rect.width().min(rect.height())).max(0.0);
    let c = |r: f32| f64::from(r).clamp(0.0, max);
    0.25 * (c(corners.tl) + c(corners.tr) + c(corners.br) + c(corners.bl))
}

fn shadow_layer(scene: &mut Scene, rect: Rect, corners: CornerRadius, shadow: &Shadow) {
    if shadow.color.components[3] <= 0.0 {
        return;
    }
    let spread = f64::from(shadow.spread);
    let shadow_rect = rect.inflate(spread, spread).with_origin(Point::new(
        rect.x0 - spread + f64::from(shadow.dx),
        rect.y0 - spread + f64::from(shadow.dy),
    ));
    let radius = (uniform_radius(rect, corners) + spread).max(0.0);
    let std_dev = f64::from(shadow.blur * BLUR_TO_STD_DEV);
    if std_dev <= 0.0 {
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            shadow.color,
            None,
            &RoundedRect::from_rect(shadow_rect, radius),
        );
    } else {
        scene.draw_blurred_rounded_rect(
            Affine::IDENTITY,
            shadow_rect,
            shadow.color,
            radius,
            std_dev,
        );
    }
}

fn brush_for(paint: &Paint, rect: Rect) -> peniko::Brush {
    match paint {
        Paint::Solid(color) => (*color).into(),
        Paint::LinearGradient { angle_deg, stops } => {
            // CSS angle: 0 points up, clockwise positive. The gradient line
            // passes through the rect center.
            let theta = f64::from(*angle_deg).to_radians();
            let (sin, cos) = theta.sin_cos();
            let half_len = 0.5 * (rect.width() * sin.abs() + rect.height() * cos.abs());
            let center = rect.center();
            let dir = Vec2::new(sin, -cos);
            Gradient::new_linear(center - dir * half_len, center + dir * half_len)
                .with_stops(color_stops(stops))
                .into()
        }
        Paint::RadialGradient {
            center,
            radius,
            stops,
        } => {
            let c = Point::new(
                rect.x0 + f64::from(center.0) * rect.width(),
                rect.y0 + f64::from(center.1) * rect.height(),
            );
            let r = f64::from(*radius) * 0.5 * rect.width().max(rect.height());
            Gradient::new_radial(c, r as f32)
                .with_stops(color_stops(stops))
                .into()
        }
    }
}

fn color_stops(stops: &[crate::style::GradientStop]) -> ColorStops {
    ColorStops(
        stops
            .iter()
            .map(|s| ColorStop::from((s.offset, s.color)))
            .collect(),
    )
}

/// Paints the box decoration (shadows, fill, border) and pushes any clip and
/// alpha layers. Returns how many layers were pushed; the caller paints
/// children, then pops that many.
pub(crate) fn push_box(scene: &mut Scene, style: &Style, rect: Rect, canvas: Rect) -> usize {
    for shadow in &style.shadows {
        shadow_layer(scene, rect, style.corner_radius, shadow);
    }

    let path = rounded_rect(rect, style.corner_radius);
    if let Some(paint) = &style.fill {
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &brush_for(paint, rect),
            None,
            &path,
        );
    }

    if let Some(border) = style.border
        && border.width > 0.0
    {
        let half = f64::from(border.width) * 0.5;
        let inset_rect = rect.inset(-half);
        let mut corners = style.corner_radius;
        for r in [
            &mut corners.tl,
            &mut corners.tr,
            &mut corners.br,
            &mut corners.bl,
        ] {
            *r = (*r - half as f32).max(0.0);
        }
        scene.stroke(
            &Stroke::new(f64::from(border.width)),
            Affine::IDENTITY,
            border.color,
            None,
            &rounded_rect(inset_rect, corners),
        );
    }

    let mut layers = 0;
    if style.clip {
        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &path);
        layers += 1;
    }
    if style.opacity < 1.0 {
        // Alpha groups clip; bound them by the element when clipping anyway,
        // otherwise by the canvas so overflowing children stay visible.
        let bounds = if style.clip {
            path
        } else {
            rounded_rect(canvas, CornerRadius::default())
        };
        scene.push_layer(
            Fill::NonZero,
            peniko::Mix::Normal,
            style.opacity.clamp(0.0, 1.0),
            Affine::IDENTITY,
            &bounds,
        );
        layers += 1;
    }
    layers
}

/// Pops layers pushed by [`push_box`].
pub(crate) fn pop_box(scene: &mut Scene, layers: usize) {
    for _ in 0..layers {
        scene.pop_layer();
    }
}

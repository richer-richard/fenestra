//! Paints one resolved, laid-out element into a vello scene, following the
//! spec order: shadows, fill, border, clip layer, alpha layer, children.

use kurbo::{
    Affine, BezPath, ParamCurve, ParamCurveArclen, Point, Rect, RoundedRect, RoundedRectRadii,
    Stroke, Vec2,
};
use peniko::{Color, ColorStop, ColorStops, Fill, Gradient};
use vello::Scene;

use crate::element::PathData;
use crate::style::{CornerRadius, Paint, Shadow, Style};
use crate::tokens::FOCUS_RING;

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

/// Rounds a logical coordinate to the physical pixel grid.
fn snap(v: f64, scale: f64) -> f64 {
    (v * scale).round() / scale
}

/// Hairlines (sub-1.75-physical-px extents) snap to the physical grid so a
/// 1px divider or border never lands between device pixels and blurs.
fn snap_hairline_rect(rect: Rect, scale: f64) -> Rect {
    let mut r = rect;
    if rect.height() * scale < 1.75 {
        let h = (rect.height() * scale).round().max(1.0) / scale;
        r.y0 = snap(rect.y0, scale);
        r.y1 = r.y0 + h;
    }
    if rect.width() * scale < 1.75 {
        let w = (rect.width() * scale).round().max(1.0) / scale;
        r.x0 = snap(rect.x0, scale);
        r.x1 = r.x0 + w;
    }
    r
}

/// Fills a uniformly-rounded rect (used for scrollbar thumbs).
pub(crate) fn fill_rounded(scene: &mut Scene, rect: Rect, radius: f32, color: peniko::Color) {
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        color,
        None,
        &rounded_rect(rect, CornerRadius::all(radius)),
    );
}

/// Paints the box decoration (shadows, fill, border) and pushes any clip and
/// alpha layers. Returns how many layers were pushed; the caller paints
/// children, then pops that many.
pub(crate) fn push_box(
    scene: &mut Scene,
    style: &Style,
    rect: Rect,
    canvas: Rect,
    scale: f64,
) -> usize {
    let mut layers = 0;
    if style.opacity < 1.0 {
        // CSS group semantics: the element's own shadows, fill, and border
        // fade together with its children. Bounded by the canvas so
        // overflowing children and shadows stay inside the group.
        scene.push_layer(
            Fill::NonZero,
            peniko::Mix::Normal,
            style.opacity.clamp(0.0, 1.0),
            Affine::IDENTITY,
            &rounded_rect(canvas, CornerRadius::default()),
        );
        layers += 1;
    }

    for shadow in &style.shadows {
        shadow_layer(scene, rect, style.corner_radius, shadow);
    }

    let path = rounded_rect(rect, style.corner_radius);
    if let Some(paint) = &style.fill {
        let fill_rect = snap_hairline_rect(rect, scale);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &brush_for(paint, fill_rect),
            None,
            &rounded_rect(fill_rect, style.corner_radius),
        );
    }

    if let Some(border) = style.border
        && border.width > 0.0
    {
        // Snap the stroke to whole physical pixels, centered so both stroke
        // edges land on the grid.
        let width = (f64::from(border.width) * scale).round().max(1.0) / scale;
        let half = width * 0.5;
        let snapped = Rect::new(
            snap(rect.x0, scale),
            snap(rect.y0, scale),
            snap(rect.x1, scale),
            snap(rect.y1, scale),
        );
        let inset_rect = snapped.inset(-half);
        let mut corners = style.corner_radius;
        for r in [
            &mut corners.tl,
            &mut corners.tr,
            &mut corners.br,
            &mut corners.bl,
        ] {
            #[expect(clippy::cast_possible_truncation, reason = "logical px fit in f32")]
            {
                *r = (*r - half as f32).max(0.0);
            }
        }
        scene.stroke(
            &Stroke::new(width),
            Affine::IDENTITY,
            border.color,
            None,
            &rounded_rect(inset_rect, corners),
        );
    }

    if let Some(highlight) = style.highlight_top
        && highlight.components[3] > 0.0
    {
        // A 1px (physical) bar at the top inner edge, clipped to the rounded
        // shape so it follows the top corners — CSS `inset 0 1px 0`.
        let h = 1.0 / scale;
        let top = snap(rect.y0, scale);
        let bar = Rect::new(rect.x0, top, rect.x1, top + h);
        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &path);
        scene.fill(Fill::NonZero, Affine::IDENTITY, highlight, None, &bar);
        scene.pop_layer();
    }

    if style.clip {
        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &path);
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

/// Draws an RGBA image stretched to `rect`, clipped to the corner radius.
pub(crate) fn draw_image(
    scene: &mut Scene,
    image: &peniko::ImageData,
    rect: Rect,
    corners: CornerRadius,
) {
    if image.width == 0 || image.height == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let transform = Affine::translate((rect.x0, rect.y0))
        * Affine::scale_non_uniform(
            rect.width() / f64::from(image.width),
            rect.height() / f64::from(image.height),
        );
    scene.push_clip_layer(
        Fill::NonZero,
        Affine::IDENTITY,
        &rounded_rect(rect, corners),
    );
    scene.draw_image(image, transform);
    scene.pop_layer();
}

/// Paints the keyboard focus ring: a 2px accent stroke offset 2px outside
/// the element, with ring radius = element radius + 2.
pub(crate) fn focus_ring(scene: &mut Scene, rect: Rect, corners: CornerRadius, color: Color) {
    let offset = f64::from(FOCUS_RING.offset) + f64::from(FOCUS_RING.width) * 0.5;
    let ring_rect = rect.inflate(offset, offset);
    let mut ring_corners = corners;
    for r in [
        &mut ring_corners.tl,
        &mut ring_corners.tr,
        &mut ring_corners.br,
        &mut ring_corners.bl,
    ] {
        *r += FOCUS_RING.offset;
    }
    scene.stroke(
        &Stroke::new(f64::from(FOCUS_RING.width)),
        Affine::IDENTITY,
        color,
        None,
        &rounded_rect(ring_rect, ring_corners),
    );
}

/// Paints a vector path scaled from its viewbox into `rect`, optionally
/// trimmed to the first `trim` fraction of its arc length (check marks
/// draw on with this).
/// Paints a vector path scaled from its viewbox into `rect`, optionally
/// trimmed to the first `trim` fraction of its arc length and rotated
/// (radians) around the rect center (spinners).
pub(crate) fn draw_path_rotated(
    scene: &mut Scene,
    data: &PathData,
    trim: f32,
    color: Color,
    rect: Rect,
    rotation: f64,
) {
    if trim <= 0.0 {
        return;
    }
    let sx = rect.width() / data.viewbox.0.max(1e-6);
    let sy = rect.height() / data.viewbox.1.max(1e-6);
    let rotate = if rotation == 0.0 {
        Affine::IDENTITY
    } else {
        Affine::rotate_about(rotation, rect.center())
    };
    let transform =
        rotate * Affine::translate((rect.x0, rect.y0)) * Affine::scale_non_uniform(sx, sy);
    let trimmed;
    let path: &BezPath = if trim >= 1.0 {
        &data.path
    } else {
        trimmed = trim_path(&data.path, f64::from(trim));
        &trimmed
    };
    match data.stroke {
        Some(width) => {
            let stroke = Stroke::new(width)
                .with_caps(kurbo::Cap::Round)
                .with_join(kurbo::Join::Round);
            scene.stroke(&stroke, transform, color, None, path);
        }
        None => scene.fill(Fill::NonZero, transform, color, None, path),
    }
}

/// Keeps the first `t` fraction (by arc length) of a path.
fn trim_path(path: &BezPath, t: f64) -> BezPath {
    const ACCURACY: f64 = 0.1;
    let segments: Vec<kurbo::PathSeg> = path.segments().collect();
    let total: f64 = segments.iter().map(|s| s.arclen(ACCURACY)).sum();
    let mut budget = total * t.clamp(0.0, 1.0);
    let mut out = BezPath::new();
    for seg in segments {
        let len = seg.arclen(ACCURACY);
        if budget <= 0.0 {
            break;
        }
        let piece = if len <= budget {
            seg
        } else {
            // Cut by parameter; close enough to arclength for icon strokes.
            seg.subsegment(0.0..(budget / len))
        };
        let needs_move =
            out.elements().is_empty() || piece.start().distance(last_point(&out)) > 1e-6;
        if needs_move {
            out.move_to(piece.start());
        }
        match piece {
            kurbo::PathSeg::Line(l) => out.line_to(l.p1),
            kurbo::PathSeg::Quad(q) => out.quad_to(q.p1, q.p2),
            kurbo::PathSeg::Cubic(c) => out.curve_to(c.p1, c.p2, c.p3),
        }
        budget -= len;
    }
    out
}

fn last_point(path: &BezPath) -> Point {
    match path.elements().last() {
        Some(kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p)) => *p,
        Some(kurbo::PathEl::QuadTo(_, p) | kurbo::PathEl::CurveTo(_, _, p)) => *p,
        _ => Point::ORIGIN,
    }
}

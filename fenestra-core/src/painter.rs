//! Paints one resolved, laid-out element into a vello scene, following the
//! spec order: shadows, fill, border, clip layer, alpha layer, children.

use kurbo::{
    Affine, BezPath, CurveFitSample, ParamCurve, ParamCurveArclen, ParamCurveFit, Point, Rect,
    RoundedRect, RoundedRectRadii, Shape, Stroke, Vec2, fit_to_bezpath,
};
use peniko::{Color, ColorStop, ColorStops, Fill, Gradient};
use vello::Scene;

use crate::element::PathData;
use crate::style::{Border, CornerRadius, Paint, Shadow, Style};
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

/// The Lamé exponent ceiling at full smoothing (`1.0`). `5.0` is Apple-icon
/// territory: round enough to read as a true squircle without bulging toward a
/// near-square. A perceptual calibration constant, not a spec token.
const N_MAX: f64 = 5.0;

/// Fréchet tolerance, in logical px, for fitting each squircle corner to cubic
/// Béziers. ~0.08 stays well under a device pixel even at 2× scale, so the fit
/// reads as exact; kurbo returns the minimal cubic count that holds it.
const SQUIRCLE_ACCURACY: f64 = 0.08;

/// Corners at or below this radius (logical px) collapse to a sharp vertex: the
/// superellipse quadrant is degenerate there and the difference is sub-pixel.
const SQUIRCLE_MIN_RADIUS: f64 = 0.25;

/// The box silhouette: the exact kurbo rounded rect at `smoothing <= 0` (the
/// default — keeps goldens byte-identical) or a continuous-curvature squircle
/// when `smoothing > 0`. Fill, border, and clip all build from this so they
/// stay aligned.
pub(crate) enum BoxPath {
    /// Exact circular-arc rounded rect (smoothing 0; the default). Identical
    /// to the pre-smoothing path, so goldens are byte-identical.
    Arc(RoundedRect),
    /// Superellipse-blended squircle (smoothing > 0).
    Squircle(BezPath),
}

impl Shape for BoxPath {
    type PathElementsIter<'i> = Box<dyn Iterator<Item = kurbo::PathEl> + 'i>;

    fn path_elements(&self, tol: f64) -> Self::PathElementsIter<'_> {
        match self {
            BoxPath::Arc(r) => Box::new(r.path_elements(tol)),
            BoxPath::Squircle(p) => Box::new(p.path_elements(tol)),
        }
    }

    fn area(&self) -> f64 {
        match self {
            BoxPath::Arc(r) => r.area(),
            BoxPath::Squircle(p) => p.area(),
        }
    }

    fn perimeter(&self, accuracy: f64) -> f64 {
        match self {
            BoxPath::Arc(r) => r.perimeter(accuracy),
            BoxPath::Squircle(p) => p.perimeter(accuracy),
        }
    }

    fn winding(&self, pt: Point) -> i32 {
        match self {
            BoxPath::Arc(r) => r.winding(pt),
            BoxPath::Squircle(p) => p.winding(pt),
        }
    }

    fn bounding_box(&self) -> Rect {
        match self {
            BoxPath::Arc(r) => r.bounding_box(),
            BoxPath::Squircle(p) => p.bounding_box(),
        }
    }
}

/// The box silhouette for a rect, per-corner radii, and smoothing. At
/// `smoothing <= 0` (the default) this is the exact kurbo rounded rect — the
/// unchanged path, so goldens stay byte-identical; at `> 0` it is a
/// superellipse-blended squircle shared by fill, border, and clip.
pub(crate) fn corner_path(rect: Rect, corners: CornerRadius, smoothing: f32) -> BoxPath {
    if smoothing <= 0.0 {
        BoxPath::Arc(rounded_rect(rect, corners))
    } else {
        BoxPath::Squircle(build_squircle(rect, corners, smoothing))
    }
}

/// Maps `0.0..=1.0` smoothing to a Lamé (superellipse) exponent in
/// `2.0..=N_MAX`. `n == 2.0` at smoothing `0.0` is an exact circle; the curve
/// fills toward the geometric corner as `n` grows.
fn squircle_exponent(smoothing: f32) -> f64 {
    let s = f64::from(smoothing.clamp(0.0, 1.0));
    2.0 + s * (N_MAX - 2.0)
}

/// One sample on a superellipse quarter that runs from the join point
/// `center + r·u` (`theta == 0`) to `center + r·v` (`theta == pi/2`):
/// `center + r·cos(theta)^(2/n)·u + r·sin(theta)^(2/n)·v`. At `n == 2` this is
/// the exact circular quadrant; the endpoints (and thus the straight-edge join
/// points) are independent of `n`, so smoothing only reshapes corners.
fn superellipse_point(center: Point, u: Vec2, v: Vec2, r: f64, n: f64, theta: f64) -> Point {
    let cx = theta.cos().max(0.0).powf(2.0 / n);
    let sy = theta.sin().max(0.0).powf(2.0 / n);
    center + u * (r * cx) + v * (r * sy)
}

/// One rounded corner as a superellipse quadrant, sampled for cubic-Bézier
/// fitting. It runs from the join on one straight edge (`t == 0`, the point
/// `center + r·u`) to the join on the adjacent edge (`t == 1`, `center + r·v`),
/// tracing `center + r·cos(θ)^(2/n)·u + r·sin(θ)^(2/n)·v` with `θ = t·π/2`.
/// `u`/`v` are unit edge directions and `n` is the Lamé exponent from
/// [`squircle_exponent`]. Feeding this to [`fit_to_bezpath`] yields the minimal
/// run of cubics that tracks the corner to [`SQUIRCLE_ACCURACY`].
struct SuperellipseQuadrant {
    center: Point,
    u: Vec2,
    v: Vec2,
    r: f64,
    n: f64,
}

impl SuperellipseQuadrant {
    /// Pulls sampling this far (radians) inside the joins, where the parametric
    /// derivative of the superellipse is unbounded for `n > 2`. Far below a
    /// device pixel of arc at any real radius.
    const EPS: f64 = 1e-6;

    fn point(&self, theta: f64) -> Point {
        superellipse_point(self.center, self.u, self.v, self.r, self.n, theta)
    }

    /// The unit tangent in the direction of increasing `t`. Exact at the joins
    /// — `+v` leaving the start edge, `-u` entering the end edge — so the
    /// straight edges meet the curve with matched tangents (G1 continuity).
    fn tangent(&self, theta: f64) -> Vec2 {
        if theta <= Self::EPS {
            return self.v;
        }
        if theta >= std::f64::consts::FRAC_PI_2 - Self::EPS {
            return -self.u;
        }
        let (s, c) = theta.sin_cos();
        let du = -c.powf(2.0 / self.n - 1.0) * s;
        let dv = s.powf(2.0 / self.n - 1.0) * c;
        (self.u * du + self.v * dv).normalize()
    }
}

impl ParamCurveFit for SuperellipseQuadrant {
    fn sample_pt_tangent(&self, t: f64, _sign: f64) -> CurveFitSample {
        let theta = t * std::f64::consts::FRAC_PI_2;
        CurveFitSample {
            p: self.point(theta),
            tangent: self.tangent(theta),
        }
    }

    fn sample_pt_deriv(&self, t: f64) -> (Point, Vec2) {
        // Clamp away from the joins so the (otherwise unbounded) parametric
        // derivative stays finite. The fitter samples only interior points for
        // its area/arc-length integrals, so the clamp never moves the result.
        let theta = (t * std::f64::consts::FRAC_PI_2)
            .clamp(Self::EPS, std::f64::consts::FRAC_PI_2 - Self::EPS);
        let (s, c) = theta.sin_cos();
        let du = -c.powf(2.0 / self.n - 1.0) * s;
        let dv = s.powf(2.0 / self.n - 1.0) * c;
        let scale = self.r * (2.0 / self.n) * std::f64::consts::FRAC_PI_2;
        (self.point(theta), (self.u * du + self.v * dv) * scale)
    }

    fn break_cusp(&self, _range: std::ops::Range<f64>) -> Option<f64> {
        // The quadrant is smooth end to end — no cusps to split on.
        None
    }
}

/// Builds a continuous-curvature squircle `BezPath` for `rect`, clamping each
/// corner radius to half the short side exactly like [`rounded_rect`]. Each
/// corner is the superellipse quadrant fitted to cubic Béziers (via
/// [`fit_to_bezpath`]); a near-zero corner collapses to a sharp vertex. The
/// walk is clockwise — TR → right edge → BR → bottom → BL → left → TL — and the
/// closing segment forms the top edge, so fill, border, and clip share one
/// silhouette.
fn build_squircle(rect: Rect, corners: CornerRadius, smoothing: f32) -> BezPath {
    let n = squircle_exponent(smoothing);
    let max = (0.5 * rect.width().min(rect.height())).max(0.0);
    let clamp = |r: f32| f64::from(r).clamp(0.0, max);
    let (tl, tr, br, bl) = (
        clamp(corners.tl),
        clamp(corners.tr),
        clamp(corners.br),
        clamp(corners.bl),
    );
    let (x0, y0, x1, y1) = (rect.x0, rect.y0, rect.x1, rect.y1);

    let mut path = BezPath::new();
    append_corner(
        &mut path,
        Point::new(x1 - tr, y0 + tr),
        Vec2::new(0.0, -1.0),
        Vec2::new(1.0, 0.0),
        tr,
        n,
        true,
    );
    append_corner(
        &mut path,
        Point::new(x1 - br, y1 - br),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        br,
        n,
        false,
    );
    append_corner(
        &mut path,
        Point::new(x0 + bl, y1 - bl),
        Vec2::new(0.0, 1.0),
        Vec2::new(-1.0, 0.0),
        bl,
        n,
        false,
    );
    append_corner(
        &mut path,
        Point::new(x0 + tl, y0 + tl),
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, -1.0),
        tl,
        n,
        false,
    );
    path.close_path();
    path
}

/// Appends one corner to `path`, running from `center + r·u` to `center + r·v`.
/// With `r > SQUIRCLE_MIN_RADIUS` it is the superellipse quadrant fitted to
/// cubic Béziers; otherwise it collapses to the sharp rectangle vertex. `first`
/// opens the subpath with a `move_to`; later corners connect along the straight
/// edge with a `line_to`.
fn append_corner(path: &mut BezPath, center: Point, u: Vec2, v: Vec2, r: f64, n: f64, first: bool) {
    let start = center + u * r;
    if first {
        path.move_to(start);
    } else {
        path.line_to(start);
    }
    if r <= SQUIRCLE_MIN_RADIUS {
        // Degenerate corner: `start` already sits on the rectangle vertex, and
        // the next corner connects straight from here.
        return;
    }
    let quad = SuperellipseQuadrant { center, u, v, r, n };
    // `fit_to_bezpath` opens with a `move_to` at our `start`; replay only its
    // curve/line segments so the corner continues the existing subpath.
    let fitted = fit_to_bezpath(&quad, SQUIRCLE_ACCURACY);
    for el in fitted.elements().iter().skip(1) {
        path.push(*el);
    }
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
        Paint::ConicGradient { center, stops } => {
            let c = Point::new(
                rect.x0 + f64::from(center.0) * rect.width(),
                rect.y0 + f64::from(center.1) * rect.height(),
            );
            Gradient::new_sweep(c, 0.0, std::f32::consts::TAU)
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

/// Paints the box decoration (shadows, optional frosted backdrop, fill, border)
/// and pushes any clip and alpha layers. Returns how many layers were pushed;
/// the caller paints children, then pops that many.
///
/// `backdrop` is the pre-blurred image a frosted-glass pane composites under its
/// translucent fill (between the drop shadow and the tint, clipped to the same
/// rounded silhouette). It is `None` for every non-glass element and for the
/// single-pass paths, which then render byte-identically to before glass blur.
pub(crate) fn push_box(
    scene: &mut Scene,
    style: &Style,
    rect: Rect,
    canvas: Rect,
    scale: f64,
    backdrop: Option<&peniko::ImageData>,
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

    let path = corner_path(rect, style.corner_radius, style.corner_smoothing);
    // A frosted-glass pane composites its CPU-blurred backdrop here — over the
    // drop shadow, under the translucent tint — clipped to the same rounded
    // silhouette as the fill. Scaled into `rect` like `draw_image` (the image
    // is in physical px, so it down-scales by 1/scale on a HiDPI frame).
    if let Some(image) = backdrop
        && image.width > 0
        && image.height > 0
    {
        let transform = Affine::translate((rect.x0, rect.y0))
            * Affine::scale_non_uniform(
                rect.width() / f64::from(image.width),
                rect.height() / f64::from(image.height),
            );
        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &path);
        scene.draw_image(image, transform);
        scene.pop_layer();
    }
    if let Some(paint) = &style.fill {
        let fill_rect = snap_hairline_rect(rect, scale);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &brush_for(paint, fill_rect),
            None,
            &corner_path(fill_rect, style.corner_radius, style.corner_smoothing),
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
            &corner_path(inset_rect, corners, style.corner_smoothing),
        );
    }

    // Per-side borders: straight hairline strokes on each present edge (square
    // corners — use the uniform `border` for a rounded full edge). Painted after
    // the uniform border so an explicit edge sits atop it.
    let sb = style.side_borders;
    if sb.top.or(sb.right).or(sb.bottom).or(sb.left).is_some() {
        let r = Rect::new(
            snap(rect.x0, scale),
            snap(rect.y0, scale),
            snap(rect.x1, scale),
            snap(rect.y1, scale),
        );
        let mut stroke_edge = |edge: Option<Border>, p0: Point, p1: Point| {
            if let Some(edge) = edge
                && edge.width > 0.0
            {
                let w = (f64::from(edge.width) * scale).round().max(1.0) / scale;
                let mut line = BezPath::new();
                line.move_to(p0);
                line.line_to(p1);
                scene.stroke(&Stroke::new(w), Affine::IDENTITY, edge.color, None, &line);
            }
        };
        stroke_edge(sb.top, Point::new(r.x0, r.y0), Point::new(r.x1, r.y0));
        stroke_edge(sb.bottom, Point::new(r.x0, r.y1), Point::new(r.x1, r.y1));
        stroke_edge(sb.left, Point::new(r.x0, r.y0), Point::new(r.x0, r.y1));
        stroke_edge(sb.right, Point::new(r.x1, r.y0), Point::new(r.x1, r.y1));
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

/// Paints the keyboard focus halo: a soft [`FOCUS_RING`]-width stroke
/// (3px) at the ring color, sitting flush just outside the element edge so it
/// reads as a glow around the (ring-colored) border. The border swap itself
/// happens during style resolution.
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
    // Optical corrections act in viewbox space (before the viewbox→rect
    // scale): centroid centering then overshoot scaling, both about the
    // viewbox center. `IDENTITY` when neither is set, so uncorrected paths are
    // byte-identical.
    let transform = rotate
        * Affine::translate((rect.x0, rect.y0))
        * Affine::scale_non_uniform(sx, sy)
        * optical_pretransform(data);
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

/// The viewbox-space pre-transform that realizes a path's optical corrections
/// (see [`crate::optical`]): centroid centering, then overshoot scaling, both
/// about the viewbox center. Returns [`Affine::IDENTITY`] when neither is set.
fn optical_pretransform(data: &PathData) -> Affine {
    let optical = data.optical;
    if !optical.overshoot && !optical.center {
        return Affine::IDENTITY;
    }
    let center = Point::new(data.viewbox.0 / 2.0, data.viewbox.1 / 2.0);
    let mut pre = Affine::IDENTITY;
    if optical.center
        && let Some((cx, cy)) = path_anchor_centroid(&data.path)
    {
        // Shift the centroid onto the viewbox center, so the viewbox→rect map
        // then lands it at the rect center (the play-triangle nudge).
        pre = Affine::translate((center.x - cx, center.y - cy));
    }
    if optical.overshoot {
        pre = Affine::scale_about(f64::from(crate::optical::CIRCLE_OVERSHOOT), center) * pre;
    }
    pre
}

/// The centroid of a path's anchor points (the mean of its on-curve vertices),
/// via [`crate::optical::centroid`] — the visual-mass proxy used to optically
/// center an asymmetric icon. `None` for an empty path.
fn path_anchor_centroid(path: &BezPath) -> Option<(f64, f64)> {
    use kurbo::PathEl;
    let mut pts: Vec<(f32, f32)> = Vec::new();
    for el in path.elements() {
        let p = match el {
            PathEl::MoveTo(p)
            | PathEl::LineTo(p)
            | PathEl::QuadTo(_, p)
            | PathEl::CurveTo(_, _, p) => *p,
            PathEl::ClosePath => continue,
        };
        #[expect(
            clippy::cast_possible_truncation,
            reason = "icon viewbox coords are small (≤ a few hundred); f32 is exact enough for a centroid"
        )]
        pts.push((p.x as f32, p.y as f32));
    }
    if pts.is_empty() {
        return None;
    }
    let (cx, cy) = crate::optical::centroid(&pts);
    Some((f64::from(cx), f64::from(cy)))
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

#[cfg(test)]
mod optical_tests {
    use super::*;
    use crate::element::{OpticalCorrection, PathData};

    /// The play triangle from the `optical_play` golden, in a 48×48 viewbox.
    fn play_tri() -> PathData {
        let mut p = BezPath::new();
        p.move_to((14.0, 12.0));
        p.line_to((14.0, 36.0));
        p.line_to((34.0, 24.0));
        p.close_path();
        PathData {
            path: std::sync::Arc::new(p),
            viewbox: (48.0, 48.0),
            stroke: None,
            optical: OpticalCorrection::default(),
        }
    }

    #[test]
    fn no_correction_is_identity() {
        // Off by default ⇒ uncorrected paths are byte-identical.
        assert_eq!(optical_pretransform(&play_tri()), Affine::IDENTITY);
    }

    #[test]
    fn anchor_centroid_is_the_vertex_mean() {
        // centroid ≈ ((14+14+34)/3, (12+36+24)/3) = (20.667, 24).
        let (cx, cy) = path_anchor_centroid(&play_tri().path).expect("non-empty");
        assert!((cx - 62.0 / 3.0).abs() < 1e-3, "cx {cx}");
        assert!((cy - 24.0).abs() < 1e-3, "cy {cy}");
    }

    #[test]
    fn center_moves_centroid_to_viewbox_center() {
        let mut d = play_tri();
        d.optical.center = true;
        let pre = optical_pretransform(&d);
        // The centroid maps onto the viewbox center (24, 24) — the play nudge.
        let moved = pre * Point::new(62.0 / 3.0, 24.0);
        assert!(
            (moved.x - 24.0).abs() < 1e-3 && (moved.y - 24.0).abs() < 1e-3,
            "{moved:?}"
        );
    }

    #[test]
    fn overshoot_scales_about_the_viewbox_center() {
        let mut d = play_tri();
        d.optical.overshoot = true;
        let pre = optical_pretransform(&d);
        let center = Point::new(24.0, 24.0);
        // The center is the fixed point of the scale.
        let fixed = pre * center;
        assert!(
            (fixed.x - 24.0).abs() < 1e-6 && (fixed.y - 24.0).abs() < 1e-6,
            "{fixed:?}"
        );
        // A point 10 units from center moves out by exactly the overshoot ratio.
        let out = pre * Point::new(34.0, 24.0);
        let expected = 24.0 + 10.0 * f64::from(crate::optical::CIRCLE_OVERSHOOT);
        assert!((out.x - expected).abs() < 1e-4, "{out:?} vs {expected}");
    }
}

#[cfg(test)]
mod squircle_tests {
    use super::*;

    #[test]
    fn squircle_exponent_zero_is_circle() {
        // smoothing 0 maps to the Lamé exponent n=2 (a true circle); the
        // exponent rises monotonically toward N_MAX as smoothing rises.
        assert_eq!(squircle_exponent(0.0), 2.0);
        assert_eq!(squircle_exponent(1.0), N_MAX);
        let mut prev = squircle_exponent(0.0);
        for s in [0.1_f32, 0.25, 0.5, 0.75, 1.0] {
            let n = squircle_exponent(s);
            assert!(
                n > prev,
                "exponent must increase with smoothing: {prev} -> {n}"
            );
            prev = n;
        }
    }

    #[test]
    fn superellipse_reduces_to_circle_at_n2() {
        // The "reduces to the circular arc at 0" criterion: every sample of the
        // quarter at n=2 lies on the circle of radius r about the center.
        let c = Point::new(10.0, 10.0);
        let (u, v) = (Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0));
        let r = 8.0;
        for i in 0..=16 {
            let theta = (f64::from(i) / 16.0) * std::f64::consts::FRAC_PI_2;
            let p = superellipse_point(c, u, v, r, 2.0, theta);
            let circle = c + u * (r * theta.cos()) + v * (r * theta.sin());
            assert!(
                (p - circle).hypot() < 1e-9,
                "n=2 must trace the circle at theta={theta}"
            );
        }
    }

    #[test]
    fn corner_path_zero_smoothing_is_exact_arc() {
        // KEY invariant: the default path is byte-identical to today's exact
        // kurbo rounded rect, so every existing golden cannot move.
        let rect = Rect::new(0.0, 0.0, 100.0, 60.0);
        let corners = CornerRadius {
            tl: 4.0,
            tr: 8.0,
            br: 12.0,
            bl: 16.0,
        };
        match corner_path(rect, corners, 0.0) {
            BoxPath::Arc(r) => assert_eq!(r, rounded_rect(rect, corners)),
            BoxPath::Squircle(_) => panic!("zero smoothing must take the exact arc path"),
        }
    }

    #[test]
    fn squircle_corner_is_fuller_than_circle() {
        // The "fuller" criterion: the corner-bisector point (theta = pi/4) is
        // exactly r from the center at smoothing 0, and strictly past r once
        // smoothing rises.
        let c = Point::ORIGIN;
        let (u, v) = (Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0));
        let r = 10.0;
        let theta = std::f64::consts::FRAC_PI_4;
        let circle = superellipse_point(c, u, v, r, squircle_exponent(0.0), theta);
        assert!(
            ((circle - c).hypot() - r).abs() < 1e-9,
            "circle bisector sits exactly r from center"
        );
        let squircle = superellipse_point(c, u, v, r, squircle_exponent(0.6), theta);
        assert!(
            (squircle - c).hypot() > r + 1e-6,
            "squircle bisector must push past r toward the geometric corner"
        );
    }

    #[test]
    fn squircle_path_is_cubic_beziers() {
        // Corners are real cubic Béziers now, not a flattened polyline.
        let rect = Rect::new(0.0, 0.0, 120.0, 120.0);
        let path = build_squircle(rect, CornerRadius::all(32.0), 0.6);
        let curves = path
            .elements()
            .iter()
            .filter(|e| matches!(e, kurbo::PathEl::CurveTo(..)))
            .count();
        assert!(curves >= 4, "every corner fits to cubics, got {curves}");
    }

    #[test]
    fn squircle_bbox_matches_rect() {
        // The squircle is inscribed — its joins touch the straight edges — so
        // the bounding box is the rect, within the sub-pixel fit tolerance.
        let rect = Rect::new(10.0, 20.0, 130.0, 100.0);
        let bb = build_squircle(rect, CornerRadius::all(24.0), 0.6).bounding_box();
        assert!(
            (bb.x0 - rect.x0).abs() < 0.2 && (bb.y0 - rect.y0).abs() < 0.2,
            "{bb:?}"
        );
        assert!(
            (bb.x1 - rect.x1).abs() < 0.2 && (bb.y1 - rect.y1).abs() < 0.2,
            "{bb:?}"
        );
    }

    #[test]
    fn squircle_opens_on_the_top_edge_join() {
        // The walk opens at the top-right join, `r` in from the corner along
        // the top edge — exactly where the straight top edge hands off.
        let rect = Rect::new(0.0, 0.0, 100.0, 80.0);
        let r = 20.0_f32;
        let path = build_squircle(rect, CornerRadius::all(r), 0.7);
        match path.elements()[0] {
            kurbo::PathEl::MoveTo(p) => assert!(
                (p.x - (rect.x1 - f64::from(r))).abs() < 1e-9 && (p.y - rect.y0).abs() < 1e-9,
                "{p:?}"
            ),
            other => panic!("path must open with MoveTo, got {other:?}"),
        }
    }

    #[test]
    fn squircle_fit_tracks_the_superellipse() {
        // Each fitted anchor in the top-right quadrant lies on the analytic
        // superellipse (nearest of a dense sampling is within tolerance).
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let r = 30.0_f32;
        let rf = f64::from(r);
        let n = squircle_exponent(0.6);
        let c = Point::new(rect.x1 - rf, rect.y0 + rf);
        let (u, v) = (Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0));
        let dense: Vec<Point> = (0..=4000)
            .map(|i| {
                let theta = (f64::from(i) / 4000.0) * std::f64::consts::FRAC_PI_2;
                superellipse_point(c, u, v, rf, n, theta)
            })
            .collect();
        for el in build_squircle(rect, CornerRadius::all(r), 0.6).elements() {
            let p = match el {
                kurbo::PathEl::MoveTo(p)
                | kurbo::PathEl::LineTo(p)
                | kurbo::PathEl::CurveTo(_, _, p) => *p,
                _ => continue,
            };
            // Restrict to the TR quadrant's region.
            if p.x >= rect.x1 - rf - 1e-6 && p.y <= rect.y0 + rf + 1e-6 {
                let nearest = dense
                    .iter()
                    .map(|q| (p - *q).hypot())
                    .fold(f64::INFINITY, f64::min);
                assert!(nearest < 0.05, "anchor {p:?} off the curve by {nearest}");
            }
        }
    }

    #[test]
    fn squircle_zero_radius_is_a_rectangle() {
        let rect = Rect::new(0.0, 0.0, 50.0, 40.0);
        let path = build_squircle(rect, CornerRadius::all(0.0), 0.8);
        let bb = path.bounding_box();
        assert!(
            (bb.x0 - rect.x0).abs() < 1e-9 && (bb.x1 - rect.x1).abs() < 1e-9,
            "{bb:?}"
        );
        assert!(
            (bb.y0 - rect.y0).abs() < 1e-9 && (bb.y1 - rect.y1).abs() < 1e-9,
            "{bb:?}"
        );
        let curves = path
            .elements()
            .iter()
            .filter(|e| matches!(e, kurbo::PathEl::CurveTo(..)))
            .count();
        assert_eq!(curves, 0, "a zero-radius squircle is a plain rectangle");
    }
}

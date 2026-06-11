//! Hand-drawn kurbo icon paths in a 16x16 viewbox, painted in the resolved
//! text color, plus the vendored [`lucide`] subset (24x24).

pub mod lucide;

use fenestra_core::{Element, path};
use kurbo::BezPath;

const VIEWBOX: (f64, f64) = (16.0, 16.0);
const STROKE: f64 = 1.8;

/// A check mark (stroked).
pub fn check<Msg>() -> Element<Msg> {
    let mut p = BezPath::new();
    p.move_to((3.5, 8.5));
    p.line_to((6.5, 11.5));
    p.line_to((12.5, 4.5));
    path(p, VIEWBOX, Some(2.0))
}

/// A downward chevron (stroked).
pub fn chevron_down<Msg>() -> Element<Msg> {
    let mut p = BezPath::new();
    p.move_to((4.0, 6.0));
    p.line_to((8.0, 10.0));
    p.line_to((12.0, 6.0));
    path(p, VIEWBOX, Some(STROKE))
}

/// A rightward chevron (stroked).
pub fn chevron_right<Msg>() -> Element<Msg> {
    let mut p = BezPath::new();
    p.move_to((6.0, 4.0));
    p.line_to((10.0, 8.0));
    p.line_to((6.0, 12.0));
    path(p, VIEWBOX, Some(STROKE))
}

/// A close cross (stroked).
pub fn x<Msg>() -> Element<Msg> {
    let mut p = BezPath::new();
    p.move_to((4.5, 4.5));
    p.line_to((11.5, 11.5));
    p.move_to((11.5, 4.5));
    p.line_to((4.5, 11.5));
    path(p, VIEWBOX, Some(STROKE))
}

/// A magnifying glass (stroked).
pub fn search<Msg>() -> Element<Msg> {
    let mut p = BezPath::new();
    // Circle at (7, 7) r 4 via four cubic arcs (kappa = 0.5523).
    let (cx, cy, r) = (7.0, 7.0, 4.0);
    let k = 0.552_284_749_8 * r;
    p.move_to((cx + r, cy));
    p.curve_to((cx + r, cy + k), (cx + k, cy + r), (cx, cy + r));
    p.curve_to((cx - k, cy + r), (cx - r, cy + k), (cx - r, cy));
    p.curve_to((cx - r, cy - k), (cx - k, cy - r), (cx, cy - r));
    p.curve_to((cx + k, cy - r), (cx + r, cy - k), (cx + r, cy));
    p.move_to((10.0, 10.0));
    p.line_to((13.0, 13.0));
    path(p, VIEWBOX, Some(STROKE))
}

/// A filled dot inside a circle outline (radio-style indicator).
pub fn circle_dot<Msg>() -> Element<Msg> {
    let mut p = BezPath::new();
    let (cx, cy, r) = (8.0, 8.0, 2.5);
    let k = 0.552_284_749_8 * r;
    p.move_to((cx + r, cy));
    p.curve_to((cx + r, cy + k), (cx + k, cy + r), (cx, cy + r));
    p.curve_to((cx - k, cy + r), (cx - r, cy + k), (cx - r, cy));
    p.curve_to((cx - r, cy - k), (cx - k, cy - r), (cx, cy - r));
    p.curve_to((cx + k, cy - r), (cx + r, cy - k), (cx + r, cy));
    p.close_path();
    path(p, VIEWBOX, None)
}

//! The canvas substrate: camera, zoom, snapping, and zoom-compensated stroke
//! math for pan/zoom canvases (Figma/tldraw-class tools). Pure geometry — no
//! rendering, no state — so it composes with the element tree and runs
//! headless. Constants follow tldraw's defaults.

use crate::tokens::EASE_IN_OUT_CUBIC;

/// The discrete zoom levels a canvas steps through (tldraw's defaults): 5% to
/// 800%, with 100% (`1.0`) at the center.
pub const ZOOMS: [f32; 8] = [0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0];

/// Minimum zoom (the low end of [`ZOOMS`]).
pub const ZOOM_MIN: f32 = 0.05;

/// Maximum zoom (the high end of [`ZOOMS`]).
pub const ZOOM_MAX: f32 = 8.0;

/// Camera fit / zoom-to-selection animation duration in ms — tldraw's
/// "medium". Pair with [`EASE_IN_OUT_CUBIC`](crate::EASE_IN_OUT_CUBIC).
pub const CAMERA_MS: f32 = 320.0;

/// Selection-outline stroke width in *screen* pixels. Divide by the zoom (see
/// [`world_len`]) so the outline stays this thick on screen at any zoom.
pub const SELECTION_STROKE: f32 = 1.5;

/// Selection corner-handle edge length in *screen* pixels (zoom-compensate it
/// the same way).
pub const HANDLE_SIZE: f32 = 8.0;

/// The default snap grid in logical pixels (Figma's 8px).
pub const SNAP_GRID: f32 = 8.0;

/// Steps to the next discrete zoom level above `z` ([`ZOOMS`]); clamps at
/// [`ZOOM_MAX`].
#[must_use]
pub fn zoom_in(z: f32) -> f32 {
    // Strictly above z: an exact ladder value advances to the next step (it is
    // not `> itself`), while an off-ladder value snaps up to the adjacent step.
    ZOOMS.iter().copied().find(|&s| s > z).unwrap_or(ZOOM_MAX)
}

/// Steps to the next discrete zoom level below `z`; clamps at [`ZOOM_MIN`].
#[must_use]
pub fn zoom_out(z: f32) -> f32 {
    // Strictly below z (mirror of `zoom_in`): exact values step down, and
    // off-ladder values snap down to the adjacent step.
    ZOOMS
        .iter()
        .rev()
        .copied()
        .find(|&s| s < z)
        .unwrap_or(ZOOM_MIN)
}

/// A world-space length as it appears on screen at `zoom` (`world * zoom`).
#[must_use]
pub fn screen_len(world: f32, zoom: f32) -> f32 {
    world * zoom
}

/// A screen-space length expressed in world units at `zoom` (`screen / zoom`).
/// This is the zoom compensation that keeps selection outlines and handles a
/// constant size on screen: stroke world-space shapes with
/// `world_len(SELECTION_STROKE, zoom)`.
#[must_use]
pub fn world_len(screen: f32, zoom: f32) -> f32 {
    screen / zoom.max(1e-6)
}

/// Snaps `v` to the nearest multiple of `grid` (no-op for a non-positive grid).
#[must_use]
pub fn snap(v: f32, grid: f32) -> f32 {
    if grid <= 0.0 {
        v
    } else {
        (v / grid).round() * grid
    }
}

/// A 2D canvas camera: pan offset (world origin in screen space) and zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Horizontal pan in screen pixels.
    pub x: f32,
    /// Vertical pan in screen pixels.
    pub y: f32,
    /// Zoom factor (clamp to [`ZOOM_MIN`]..=[`ZOOM_MAX`]).
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self::ORIGIN
    }
}

impl Camera {
    /// The identity camera: no pan, 100% zoom.
    pub const ORIGIN: Camera = Camera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };

    /// The camera eased toward `to` at progress `t` (0..=1) along the canvas
    /// easing ([`EASE_IN_OUT_CUBIC`](crate::EASE_IN_OUT_CUBIC)) — drive `t`
    /// from a [`CAMERA_MS`] clock for a tldraw-style zoom-to-fit.
    #[must_use]
    pub fn ease_to(self, to: Camera, t: f32) -> Camera {
        let e = EASE_IN_OUT_CUBIC.eval(t.clamp(0.0, 1.0));
        Camera {
            x: self.x + (to.x - self.x) * e,
            y: self.y + (to.y - self.y) * e,
            zoom: self.zoom + (to.zoom - self.zoom) * e,
        }
    }

    /// World point → screen point under this camera.
    #[must_use]
    pub fn to_screen(self, x: f32, y: f32) -> (f32, f32) {
        (x * self.zoom + self.x, y * self.zoom + self.y)
    }

    /// Screen point → world point under this camera.
    #[must_use]
    pub fn to_world(self, x: f32, y: f32) -> (f32, f32) {
        let z = self.zoom.max(1e-6);
        ((x - self.x) / z, (y - self.y) / z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_walk_the_ladder_and_clamp() {
        assert_eq!(zoom_in(1.0), 2.0);
        assert_eq!(zoom_out(1.0), 0.5);
        assert_eq!(zoom_in(1.5), 2.0); // next step above
        assert_eq!(zoom_out(1.5), 1.0); // next step below
        assert_eq!(zoom_in(ZOOM_MAX), ZOOM_MAX); // clamps
        assert_eq!(zoom_out(ZOOM_MIN), ZOOM_MIN);
        // Off-ladder (a continuous/pinch zoom): step to the ADJACENT ladder
        // level, never past it. A value a hair below 0.1 must zoom in to 0.1,
        // not skip to 0.25.
        assert_eq!(zoom_in(0.099_95), 0.1);
        assert_eq!(zoom_out(0.100_05), 0.1);
    }

    #[test]
    fn snap_rounds_to_the_grid() {
        assert_eq!(snap(11.0, 8.0), 8.0);
        assert_eq!(snap(13.0, 8.0), 16.0);
        assert_eq!(snap(4.0, 8.0), 8.0); // .5 rounds up
        assert_eq!(snap(3.0, 0.0), 3.0); // non-positive grid is a no-op
    }

    #[test]
    fn stroke_is_zoom_compensated() {
        // A 1.5px screen stroke is half as wide in world units at 2x zoom, so
        // it paints back to 1.5px on screen.
        let world = world_len(SELECTION_STROKE, 2.0);
        assert!((world - 0.75).abs() < 1e-6);
        assert!((screen_len(world, 2.0) - SELECTION_STROKE).abs() < 1e-6);
    }

    #[test]
    fn camera_round_trips_world_screen_and_eases() {
        let cam = Camera {
            x: 40.0,
            y: -10.0,
            zoom: 2.0,
        };
        let (sx, sy) = cam.to_screen(12.0, 8.0);
        let (wx, wy) = cam.to_world(sx, sy);
        assert!((wx - 12.0).abs() < 1e-4 && (wy - 8.0).abs() < 1e-4);
        // Easing pins the endpoints.
        let a = Camera::ORIGIN;
        let b = Camera {
            x: 0.0,
            y: 0.0,
            zoom: 4.0,
        };
        assert_eq!(a.ease_to(b, 0.0).zoom, 1.0);
        assert!((a.ease_to(b, 1.0).zoom - 4.0).abs() < 1e-4);
    }
}

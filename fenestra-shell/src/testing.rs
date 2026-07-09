//! PNG golden testing: tolerance-based image comparison with an update
//! mode, used by every visual test in the workspace.
//!
//! Comparison passes when every channel delta is at most 3/255 and fewer
//! than 0.2 percent of pixels exceed that. `FENESTRA_UPDATE_SNAPSHOTS=1`
//! regenerates goldens. On failure three artifacts land next to the
//! golden: `<name>.actual.png` (what rendered), `<name>.diff.png` (the
//! offending pixels in red over the dimmed golden), and
//! `<name>.side.png` (golden | actual | diff side by side) — read the
//! diff first; it shows *where*, not just *how much*.
//!
//! Goldens are rendered on macOS/Metal; a software rasterizer (CI's
//! lavapipe) antialiases slightly differently, so the pixel budget can be
//! widened there with `FENESTRA_SNAPSHOT_BUDGET` (e.g. `0.006`) without
//! loosening the reference platform.

use std::path::Path;

use fenestra_core::{TextSize, Theme, col, div, image_rgba8, row, text};
use image::RgbaImage;

use crate::{render_element, with_headless};

/// Per-channel delta at or below this is identical enough.
const CHANNEL_TOLERANCE: u8 = 3;
/// Fraction of pixels allowed to exceed the channel tolerance (default;
/// see [`BUDGET_ENV`]).
const MAX_DIFFERING_FRACTION: f64 = 0.002;

/// Env var that regenerates goldens instead of comparing.
pub const UPDATE_ENV: &str = "FENESTRA_UPDATE_SNAPSHOTS";

/// Env var overriding the differing-pixel budget (a fraction, e.g.
/// `0.006`), for runners whose rasterizer differs from the goldens'.
pub const BUDGET_ENV: &str = "FENESTRA_SNAPSHOT_BUDGET";

fn differing_budget() -> f64 {
    std::env::var(BUDGET_ENV)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|b| b.is_finite() && (0.0..=1.0).contains(b))
        .unwrap_or(MAX_DIFFERING_FRACTION)
}

/// Compares `actual` against the golden `dir/name.png`.
///
/// # Panics
/// On size or content mismatch beyond tolerance, or when the golden is
/// missing and `FENESTRA_UPDATE_SNAPSHOTS=1` is not set.
pub fn assert_png_snapshot(dir: impl AsRef<Path>, name: &str, actual: &RgbaImage) {
    let dir = dir.as_ref();
    let golden_path = dir.join(format!("{name}.png"));
    let update = std::env::var(UPDATE_ENV).is_ok_and(|v| v == "1");

    if update {
        std::fs::create_dir_all(dir).expect("create snapshot dir");
        actual.save(&golden_path).expect("write golden");
        return;
    }

    let artifacts = [
        dir.join(format!("{name}.actual.png")),
        dir.join(format!("{name}.diff.png")),
        dir.join(format!("{name}.side.png")),
    ];

    let golden = match image::open(&golden_path) {
        Ok(img) => img.into_rgba8(),
        Err(_) => panic!(
            "missing golden {}; run with {UPDATE_ENV}=1 to create it",
            golden_path.display()
        ),
    };

    if golden.dimensions() != actual.dimensions() {
        let actual_path = dir.join(format!("{name}.actual.png"));
        actual.save(&actual_path).ok();
        panic!(
            "golden {} is {:?} but actual is {:?} (actual written to {})",
            golden_path.display(),
            golden.dimensions(),
            actual.dimensions(),
            actual_path.display()
        );
    }

    let total = u64::from(golden.width()) * u64::from(golden.height());
    let mut differing: u64 = 0;
    let mut max_delta: u8 = 0;
    let mut worst: (u32, u32) = (0, 0);
    for (x, y, a) in actual.enumerate_pixels() {
        let g = golden.get_pixel(x, y);
        let mut pixel_exceeds = false;
        for c in 0..4 {
            let delta = g.0[c].abs_diff(a.0[c]);
            if delta > max_delta {
                max_delta = delta;
                worst = (x, y);
            }
            if delta > CHANNEL_TOLERANCE {
                pixel_exceeds = true;
            }
        }
        if pixel_exceeds {
            differing += 1;
        }
    }

    #[expect(clippy::cast_precision_loss, reason = "image pixel counts are small")]
    let fraction = differing as f64 / total as f64;
    let budget = differing_budget();
    if fraction > budget {
        actual.save(&artifacts[0]).ok();
        let diff = diff_image(&golden, actual);
        diff.save(&artifacts[1]).ok();
        side_by_side(&golden, actual, &diff)
            .save(&artifacts[2])
            .ok();
        panic!(
            "snapshot {name}: {differing}/{total} pixels ({:.3}%) exceed channel tolerance \
             {CHANNEL_TOLERANCE}, over budget {:.3}% (max delta {max_delta} at {worst:?})\n\
             artifacts: {name}.actual.png, {name}.diff.png (offending pixels in red), \
             {name}.side.png — in {}\n\
             run with {UPDATE_ENV}=1 to update",
            fraction * 100.0,
            budget * 100.0,
            dir.display()
        );
    }

    // Passed: remove stale failure artifacts from earlier runs.
    for stale in &artifacts {
        let _ = std::fs::remove_file(stale);
    }
}

/// Per-cell scale bounds for [`assert_filmstrip_snapshot_scaled`]: below the
/// floor a frame is unreadably small; above 1.0 there is nothing to upscale
/// — a filmstrip never carries more detail than the frames it tiles.
const MIN_STRIP_SCALE: f32 = 0.05;
const MAX_STRIP_SCALE: f32 = 1.0;

/// Gap between cells and around the strip's edges, and the caption's height
/// budget below each thumbnail (logical px, at strip scale — the caption
/// itself is not scaled, so it stays legible even in a shrunk strip).
const STRIP_GAP: f32 = 8.0;
const STRIP_CAPTION_H: f32 = 18.0;
/// Gap between a thumbnail and its caption within one cell — shared by the
/// pre-render size check and the actual element tree, so they can't drift
/// apart.
const THUMB_CAPTION_GAP: f32 = 2.0;

/// Composes frames captured by [`crate::Harness::film`] into one horizontal
/// filmstrip and compares it against the PNG golden `dir/name.png` exactly
/// like [`assert_png_snapshot`] (same env vars, same
/// `.actual`/`.diff`/`.side` failure artifacts): each frame side by side,
/// left to right, captioned with its index and elapsed time (`index *
/// interval_ms`, matching the `interval_ms` passed to `film`).
///
/// The strip's own chrome — background, cell borders, caption text — always
/// renders under [`Theme::light`], independent of whichever theme produced
/// the frames: only the pixels *inside* each cell vary, so the strip is a
/// stable presentation around them, not a second thing under test.
///
/// This only proves the pixels a filmstrip carries render into a stable
/// strip; it says nothing about whether the frames it was given actually
/// show motion (a filmstrip of `frames` identical copies — reduced motion
/// left on, see [`crate::Harness::film`] — golden-tests just as cleanly as a
/// real transition would).
///
/// # Panics
/// If `frames` is empty, the composed strip would exceed the headless
/// renderer's maximum texture dimension (capture fewer frames or use
/// [`assert_filmstrip_snapshot_scaled`] to shrink each cell), or the golden
/// comparison fails.
pub fn assert_filmstrip_snapshot(
    dir: impl AsRef<Path>,
    name: &str,
    frames: &[RgbaImage],
    interval_ms: u64,
) {
    assert_filmstrip_snapshot_scaled(dir, name, frames, interval_ms, MAX_STRIP_SCALE);
}

/// Like [`assert_filmstrip_snapshot`], with an explicit per-cell `scale`
/// (clamped to `0.05..=1.0`) so a strip of many, or large, frames still
/// makes a small, reviewable golden.
///
/// # Panics
/// Same as [`assert_filmstrip_snapshot`].
pub fn assert_filmstrip_snapshot_scaled(
    dir: impl AsRef<Path>,
    name: &str,
    frames: &[RgbaImage],
    interval_ms: u64,
    scale: f32,
) {
    assert!(
        !frames.is_empty(),
        "assert_filmstrip_snapshot {name}: no frames captured"
    );
    let scale = if scale.is_finite() {
        scale.clamp(MIN_STRIP_SCALE, MAX_STRIP_SCALE)
    } else {
        MAX_STRIP_SCALE
    };

    #[expect(
        clippy::cast_precision_loss,
        reason = "frame pixel dimensions are far below f32's exact-integer range"
    )]
    let cells: Vec<(u32, u32, f32, f32)> = frames
        .iter()
        .map(|f| {
            let (w, h) = f.dimensions();
            (w, h, w as f32 * scale, h as f32 * scale)
        })
        .collect();

    let strip_w = STRIP_GAP
        + cells
            .iter()
            .map(|&(_, _, tw, _)| tw + STRIP_GAP)
            .sum::<f32>();
    let strip_h = STRIP_GAP * 2.0
        + cells.iter().map(|&(_, _, _, th)| th).fold(0.0f32, f32::max)
        + THUMB_CAPTION_GAP
        + STRIP_CAPTION_H;

    let max_dim = with_headless(|h| h.max_dimension()).unwrap_or(8192);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "strip_w/strip_h are sums of small positive cell sizes"
    )]
    let (strip_w, strip_h) = (strip_w.ceil() as u32, strip_h.ceil() as u32);
    assert!(
        strip_w <= max_dim && strip_h <= max_dim,
        "assert_filmstrip_snapshot {name}: strip would be {strip_w}x{strip_h}px, over the \
         renderer's {max_dim}px limit; lower `scale` (assert_filmstrip_snapshot_scaled) or \
         capture fewer frames"
    );

    let mut strip = row::<()>().p(STRIP_GAP).gap(STRIP_GAP).items_start();
    for (i, (frame, &(w, h, tw, th))) in frames.iter().zip(&cells).enumerate() {
        let at_ms = (i as u64).saturating_mul(interval_ms);
        let thumb = div::<()>()
            .child(image_rgba8(w, h, frame.as_raw().clone()).w(tw).h(th))
            .themed(|t, s| s.border(1.0, t.border_subtle));
        let cell = col::<()>()
            .gap(THUMB_CAPTION_GAP)
            .items_center()
            .child(thumb)
            .child(
                text(format!("f{i:03} +{at_ms}ms"))
                    .size(TextSize::Xs)
                    .mono()
                    .themed(|t, s| s.color(t.text_muted)),
            );
        strip = strip.child(cell);
    }

    let composed = render_element(strip, &Theme::light(), (strip_w, strip_h));
    assert_png_snapshot(dir, name, &composed);
}

/// The offending pixels in solid red over the golden dimmed to a third —
/// it shows *where* the images disagree at a glance.
fn diff_image(golden: &RgbaImage, actual: &RgbaImage) -> RgbaImage {
    let mut out = RgbaImage::new(golden.width(), golden.height());
    for (x, y, a) in actual.enumerate_pixels() {
        let g = golden.get_pixel(x, y);
        let exceeds = (0..4).any(|c| g.0[c].abs_diff(a.0[c]) > CHANNEL_TOLERANCE);
        let px = if exceeds {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([g.0[0] / 3, g.0[1] / 3, g.0[2] / 3, 255])
        };
        out.put_pixel(x, y, px);
    }
    out
}

/// Golden | actual | diff in one strip, separated by 4px dividers.
fn side_by_side(golden: &RgbaImage, actual: &RgbaImage, diff: &RgbaImage) -> RgbaImage {
    const GAP: u32 = 4;
    let (w, h) = golden.dimensions();
    let mut out = RgbaImage::from_pixel(w * 3 + GAP * 2, h, image::Rgba([128, 128, 128, 255]));
    for (i, img) in [golden, actual, diff].into_iter().enumerate() {
        #[expect(clippy::cast_possible_truncation, reason = "three panes")]
        let x0 = (w + GAP) * i as u32;
        for (x, y, px) in img.enumerate_pixels() {
            out.put_pixel(x0 + x, y, *px);
        }
    }
    out
}

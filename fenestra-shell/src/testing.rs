//! PNG golden testing: tolerance-based image comparison with an update
//! mode, used by every visual test in the workspace.
//!
//! Comparison passes when every channel delta is at most 3/255 and fewer
//! than 0.2 percent of pixels exceed that. `FENESTRA_UPDATE_SNAPSHOTS=1`
//! regenerates goldens. On failure the actual image is written next to the
//! golden as `<name>.actual.png` for inspection.
//!
//! Goldens are rendered on macOS/Metal; a software rasterizer (CI's
//! lavapipe) antialiases slightly differently, so the pixel budget can be
//! widened there with `FENESTRA_SNAPSHOT_BUDGET` (e.g. `0.006`) without
//! loosening the reference platform.

use std::path::Path;

use image::RgbaImage;

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
    for (g, a) in golden.pixels().zip(actual.pixels()) {
        let mut pixel_exceeds = false;
        for c in 0..4 {
            let delta = g.0[c].abs_diff(a.0[c]);
            max_delta = max_delta.max(delta);
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
    if fraction > differing_budget() {
        let actual_path = dir.join(format!("{name}.actual.png"));
        actual.save(&actual_path).ok();
        panic!(
            "snapshot {name}: {differing}/{total} pixels ({:.3}%) exceed channel tolerance \
             {CHANNEL_TOLERANCE} (max delta {max_delta}); actual written to {} — \
             run with {UPDATE_ENV}=1 to update",
            fraction * 100.0,
            actual_path.display()
        );
    }
}

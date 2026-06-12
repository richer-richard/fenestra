//! The golden harness's own failure UX: on mismatch it writes
//! actual/diff/side artifacts and a self-explaining message; on a later
//! pass it cleans them up.

use fenestra_shell::testing::assert_png_snapshot;
use image::{Rgba, RgbaImage};

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fenestra-snap-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn flat(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba(px))
}

#[test]
fn mismatch_writes_artifacts_and_explains_itself() {
    let dir = unique_dir("mismatch");
    let golden = flat(40, 30, [10, 10, 10, 255]);
    golden.save(dir.join("box.png")).expect("write golden");

    // A quarter of the image disagrees far beyond tolerance.
    let mut actual = golden.clone();
    for y in 0..30 {
        for x in 0..20 {
            actual.put_pixel(x, y, Rgba([200, 10, 10, 255]));
        }
    }

    let result = std::panic::catch_unwind(|| assert_png_snapshot(&dir, "box", &actual));
    let panic = result.expect_err("mismatch must fail");
    let msg = panic
        .downcast_ref::<String>()
        .cloned()
        .expect("string panic message");

    assert!(msg.contains("600/1200 pixels"), "stats in message: {msg}");
    assert!(
        msg.contains("max delta 190"),
        "worst delta in message: {msg}"
    );
    assert!(msg.contains("box.diff.png"), "points at the diff: {msg}");

    let diff = image::open(dir.join("box.diff.png"))
        .expect("diff written")
        .into_rgba8();
    assert_eq!(
        diff.get_pixel(5, 5),
        &Rgba([255, 0, 0, 255]),
        "offending pixels are red"
    );
    assert_eq!(
        diff.get_pixel(30, 5),
        &Rgba([3, 3, 3, 255]),
        "matching pixels are the dimmed golden"
    );

    let side = image::open(dir.join("box.side.png"))
        .expect("side written")
        .into_rgba8();
    assert_eq!(
        side.dimensions(),
        (40 * 3 + 8, 30),
        "three panes plus dividers"
    );
    assert!(dir.join("box.actual.png").exists());

    // A passing run cleans the stale artifacts up again.
    assert_png_snapshot(&dir, "box", &golden);
    assert!(!dir.join("box.actual.png").exists());
    assert!(!dir.join("box.diff.png").exists());
    assert!(!dir.join("box.side.png").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn within_tolerance_passes_without_artifacts() {
    let dir = unique_dir("tolerant");
    let golden = flat(20, 20, [100, 100, 100, 255]);
    golden.save(dir.join("calm.png")).expect("write golden");

    // Every channel off by exactly the tolerance: identical enough.
    let actual = flat(20, 20, [103, 103, 103, 255]);
    assert_png_snapshot(&dir, "calm", &actual);
    assert!(!dir.join("calm.diff.png").exists());

    let _ = std::fs::remove_dir_all(&dir);
}

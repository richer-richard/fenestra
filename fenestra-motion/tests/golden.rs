//! Sentinel goldens: the shipped lower-third example pinned at
//! auto-selected sentinel frames (span edges / keys / midpoints, spread
//! across the timeline) plus one contact sheet, through the workspace
//! golden harness (`FENESTRA_UPDATE_SNAPSHOTS=1` regenerates; failures
//! write .actual/.diff/.side PNGs next to the goldens).

use fenestra_motion::Composition;
use fenestra_shell::testing::assert_png_snapshot;

fn snapshot_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn lower_third() -> Composition {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/lower_third.ron"
    ))
    .expect("shipped example");
    Composition::from_ron(&src).expect("compiles")
}

#[test]
fn lower_third_sentinel_goldens() {
    let comp = lower_third();
    let sentinels = comp.sentinel_frames();
    assert!(sentinels.len() >= 5, "a real timeline has sentinels");
    // Five sentinels spread across the timeline keep the golden corpus
    // small while pinning the entrance, the hold, and the exit.
    for i in 0..5 {
        let frame = sentinels[(sentinels.len() - 1) * i / 4];
        let img = comp.render_frame(frame).expect("render");
        assert_png_snapshot(
            snapshot_dir(),
            &format!("lower_third_f{:05}", frame.0),
            &img,
        );
    }
}

#[test]
fn lower_third_contact_sheet_golden() {
    let comp = lower_third();
    let sheet = comp.contact_sheet(30, 240).expect("sheet");
    assert_png_snapshot(snapshot_dir(), "lower_third_sheet", &sheet);
}

/// Entrance / hold / exit-adjacent frames for the code demos, pinned.
#[test]
fn title_stagger_goldens() {
    let comp = fenestra_motion::demos::title_stagger();
    for frame in [8u64, 40, 145] {
        let img = comp
            .render_frame(fenestra_motion::Frames(frame))
            .expect("render");
        assert_png_snapshot(snapshot_dir(), &format!("title_stagger_f{frame:05}"), &img);
    }
}

#[test]
fn chart_race_goldens() {
    let comp = fenestra_motion::demos::chart_race();
    for frame in [0u64, 75, 149] {
        let img = comp
            .render_frame(fenestra_motion::Frames(frame))
            .expect("render");
        assert_png_snapshot(snapshot_dir(), &format!("chart_race_f{frame:05}"), &img);
    }
}

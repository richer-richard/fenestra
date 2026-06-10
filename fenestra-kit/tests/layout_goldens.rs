//! M3 acceptance: holy-grail layout goldens, a scrolled nested-scroll
//! golden, an insta layout-tree snapshot, and scroll offset persistence.

use std::path::PathBuf;

use fenestra_core::{Fonts, FrameState, Theme, build_frame};
use fenestra_kit::{holy_grail, scroll_demo};
use fenestra_shell::{render_element, render_element_with_state, testing::assert_png_snapshot};
use kurbo::Point;

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (720, 480);

#[test]
fn holy_grail_light() {
    let theme = Theme::light();
    let image = render_element(holy_grail::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "holy_grail_light", &image);
}

#[test]
fn holy_grail_dark() {
    let theme = Theme::dark();
    let image = render_element(holy_grail::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "holy_grail_dark", &image);
}

#[test]
fn holy_grail_layout_tree() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let frame = build_frame(
        &holy_grail::<()>(&theme),
        &theme,
        &mut fonts,
        &mut state,
        (720.0, 480.0),
        1.0,
    );
    insta::assert_snapshot!(frame.dump());
}

/// Scroll offsets persist per stable id across full view rebuilds, are
/// clamped to the content range, and wheel routing finds the inner
/// scrollable under the cursor.
#[test]
fn scroll_offsets_persist_and_clamp() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    let frame = build_frame(
        &scroll_demo::<()>(&theme),
        &theme,
        &mut fonts,
        &mut state,
        (480.0, 360.0),
        1.0,
    );
    // The outer scrollable owns most of the canvas; the inner list sits
    // somewhere in the middle. Route a wheel event through hit lookup.
    let outer = frame
        .scrollable_at(Point::new(440.0, 40.0))
        .expect("outer scrollable under cursor");
    state.scroll_by(outer, 250.0);

    // Rebuild (view function runs again, ids must be stable).
    let frame = build_frame(
        &scroll_demo::<()>(&theme),
        &theme,
        &mut fonts,
        &mut state,
        (480.0, 360.0),
        1.0,
    );
    assert!(
        (state.scroll_offset(outer) - 250.0).abs() < 0.01,
        "offset should persist across rebuilds, got {}",
        state.scroll_offset(outer)
    );

    // Over-scroll clamps to the content range on the next build.
    state.scroll_by(outer, 1.0e6);
    let frame2 = build_frame(
        &scroll_demo::<()>(&theme),
        &theme,
        &mut fonts,
        &mut state,
        (480.0, 360.0),
        1.0,
    );
    let max = state.scroll_offset(outer);
    assert!(
        max > 300.0 && max < 1.0e5,
        "offset should clamp to content range, got {max}"
    );
    drop((frame, frame2));
}

#[test]
fn nested_scroll_scrolled_golden() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    // Build once to discover ids via hit lookup, scroll both regions,
    // then render the settled result.
    let frame = build_frame(
        &scroll_demo::<()>(&theme),
        &theme,
        &mut fonts,
        &mut state,
        (480.0, 360.0),
        1.0,
    );
    let outer = frame
        .scrollable_at(Point::new(440.0, 40.0))
        .expect("outer scrollable");
    state.scroll_by(outer, 180.0);
    drop(frame);

    let frame = build_frame(
        &scroll_demo::<()>(&theme),
        &theme,
        &mut fonts,
        &mut state,
        (480.0, 360.0),
        1.0,
    );
    // After scrolling down, the inner list is visible; scroll it too.
    let inner = (0..360)
        .step_by(8)
        .map(|y| Point::new(240.0, f64::from(y)))
        .find_map(|p| frame.scrollable_at(p).filter(|id| *id != outer))
        .expect("inner scrollable visible after outer scroll");
    state.scroll_by(inner, 100.0);
    drop(frame);

    let image =
        render_element_with_state(scroll_demo::<()>(&theme), &theme, (480, 360), &mut state);
    assert_png_snapshot(snapshot_dir(), "nested_scroll_scrolled", &image);
}

//! Robustness regressions from the security/hardening audit: public APIs
//! must not panic on hostile inputs, and retained state must stay bounded.

use fenestra_core::{
    Element, Fonts, FrameState, InputEvent, Theme, WidgetId, build_frame, col, dispatch, raw_input,
    text,
};
use kurbo::Point;

/// Ramp steps are 1-based and documented as 1..=12; out-of-range steps
/// (a 0-based loop is the obvious caller mistake) clamp instead of panic.
#[test]
fn ramp_step_clamps_out_of_range() {
    let t = Theme::light();
    assert_eq!(t.neutrals.step(0), t.neutrals.step(1));
    assert_eq!(t.neutrals.step(13), t.neutrals.step(12));
    assert_eq!(t.neutrals.step(usize::MAX), t.neutrals.step(12));
}

/// Scroll offsets persist while their container is mounted, and entries for
/// ids no longer in the tree are dropped (no unbounded growth under
/// dynamically keyed scrollables).
#[test]
fn stale_scroll_state_is_garbage_collected() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    let view = || -> Element<()> {
        col().w(200.0).h(100.0).children([col()
            .h(80.0)
            .scroll_y()
            .id("list")
            .children([col().h(800.0).shrink0()])])
    };

    let frame = build_frame(&view(), &theme, &mut fonts, &mut state, (200.0, 100.0), 1.0);
    let list = frame
        .scrollable_at(Point::new(20.0, 20.0))
        .expect("scrollable under cursor");
    drop(frame);
    state.scroll_by(list, 64.0);
    let _ = build_frame(&view(), &theme, &mut fonts, &mut state, (200.0, 100.0), 1.0);
    assert!(
        state.scroll_offset(list) > 0.0,
        "a mounted scrollable keeps its offset across builds"
    );

    let stale = WidgetId(0xDEAD_BEEF);
    state.scroll_by(stale, 64.0);
    let _ = build_frame(&view(), &theme, &mut fonts, &mut state, (200.0, 100.0), 1.0);
    assert_eq!(
        state.scroll_offset(stale),
        0.0,
        "scroll entries for unmounted ids must be dropped"
    );
}

/// IME preedit events carry byte offsets from the platform; offsets beyond
/// the preedit text length must be clamped, not forwarded into the editor
/// (parley debug-asserts on out-of-range compose cursors).
#[test]
fn ime_preedit_with_out_of_range_cursor_does_not_panic() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    let view: Element<()> = col().p(8.0).children([raw_input("hello", "").w(160.0)]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (200.0, 60.0), 1.0);
    let _ = dispatch(&view, &frame, &mut state, &mut fonts, InputEvent::Tab);
    let _ = dispatch(
        &view,
        &frame,
        &mut state,
        &mut fonts,
        InputEvent::ImePreedit {
            text: "x".into(),
            cursor: Some((0, 6)),
        },
    );
    let _ = dispatch(
        &view,
        &frame,
        &mut state,
        &mut fonts,
        InputEvent::ImePreedit {
            text: "x".into(),
            cursor: Some((usize::MAX, usize::MAX)),
        },
    );
    // Reaching here without a panic is the assertion; render one more frame
    // to confirm the editor state is still usable.
    let _ = build_frame(&view, &theme, &mut fonts, &mut state, (200.0, 60.0), 1.0);
    let _ = text::<()>("still alive");
}

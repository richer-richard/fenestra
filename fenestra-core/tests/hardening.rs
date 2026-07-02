//! Robustness regressions from the security/hardening audit: public APIs
//! must not panic on hostile inputs, and retained state must stay bounded.

use fenestra_core::{
    Element, Fonts, FrameState, InputEvent, Theme, WidgetId, build_frame, by, col, dispatch, div,
    raw_input, text,
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

/// A paint-time transform moves where an element draws but not its layout rect,
/// so hit-testing must follow the paint: a `.translate`'d element activates at
/// the PAINTED location, not its old layout slot (the M3 invariant "what you
/// hit-test is exactly what you painted").
#[test]
fn translated_element_hit_tests_where_it_paints() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    // A 100×40 box shifted +100px in x: its layout rect stays put, but paint —
    // and therefore hit-testing — shifts right by 100.
    let view: Element<()> = col().w(400.0).h(200.0).children([div()
        .id("btn")
        .w(100.0)
        .h(40.0)
        .translate(100.0, 0.0)]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 200.0), 1.0);

    let btn = frame.get(&by::id("btn")).id;
    let layout_center = frame.rect_of(btn).expect("btn rect").center();
    let painted_center = Point::new(layout_center.x + 100.0, layout_center.y);

    assert!(
        frame.hit_chain(painted_center).contains(&btn),
        "translated element activates at its painted location {painted_center:?}"
    );
    assert!(
        !frame.hit_chain(layout_center).contains(&btn),
        "and no longer at its old layout slot {layout_center:?}"
    );
}

/// A transformed element clipped away by an ancestor at its LAYOUT position
/// but translated INTO the ancestor's visible area must hit-test where it
/// paints — the ancestor clip (`node.visible`) is stored in untransformed
/// layout space, so it must be tested against the point *before* this node's
/// own transform is undone, not after (the other order produces a false
/// miss: the child's own translated-out-of-clip layout slot never matches
/// the ancestor's untransformed clip rect).
#[test]
fn transformed_element_hit_tests_through_an_ancestor_clip() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    // A 200x200 clipping container; its child stacks below a 300px spacer
    // (landing at layout y=[300,340], entirely below the clip), then is
    // translated up by 300 to paint at y=[0,40] — inside the clip.
    let view: Element<()> = col()
        .w(200.0)
        .h(200.0)
        .overflow_hidden()
        .children([col().children([
            div().h(300.0),
            div().id("revealed").w(100.0).h(40.0).translate(0.0, -300.0),
        ])]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (200.0, 200.0), 1.0);

    let id = frame.get(&by::id("revealed")).id;
    let painted_center = Point::new(50.0, 20.0);
    assert!(
        frame.hit_chain(painted_center).contains(&id),
        "element translated into an ancestor's clip must hit-test where it paints, {painted_center:?}"
    );
}

/// Wheel routing must follow paint-time transforms the same way click
/// hit-testing does: a scrollable container translated away from its layout
/// slot must resolve `scrollable_at` at its PAINTED position, matching
/// `hit_chain`'s behavior for the same point.
#[test]
fn translated_scroll_container_resolves_wheel_routing_where_it_paints() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    // A 100x100 scrollable list shifted +150px in x: its layout rect stays
    // put, but paint — and therefore wheel routing — shifts right by 150.
    let rows: Vec<Element<()>> = (0..20)
        .map(|i| div().h(20.0).children([text(format!("row {i}"))]))
        .collect();
    let view: Element<()> = col().w(400.0).h(200.0).children([col()
        .id("list")
        .w(100.0)
        .h(100.0)
        .scroll_y()
        .translate(150.0, 0.0)
        .children(rows)]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 200.0), 1.0);

    let list = frame.get(&by::id("list")).id;
    let layout_center = frame.rect_of(list).expect("list rect").center();
    let painted_center = Point::new(layout_center.x + 150.0, layout_center.y);

    assert_eq!(
        frame.scrollable_at(painted_center),
        Some(list),
        "wheel routing resolves the translated container at its painted location {painted_center:?}"
    );
}

/// A non-finite or enormous font size hangs parley's line breaker (its advance
/// arithmetic overflows toward infinity and never fits a line). `resolve_text`
/// clamps the size to a finite, sane range; reaching the end of this test —
/// rather than spinning forever — is the assertion.
#[test]
fn pathological_font_size_does_not_hang_layout() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    // Wrapping text in a narrow column is what exercises the line breaker.
    let phrase = "the quick brown fox jumps over the lazy dog repeatedly";
    for size in [f32::INFINITY, f32::MAX, f32::NAN, 1.0e30, -8.0] {
        let view: Element<()> = col().w(120.0).children([text(phrase).size_px(size)]);
        let frame = build_frame(&view, &theme, &mut fonts, &mut state, (120.0, 200.0), 1.0);
        // It must still produce an accessible tree (degraded, not hung/panicked).
        assert!(
            frame.access_yaml().contains("quick"),
            "size {size} should still lay out the text"
        );
    }
}

/// A translated text input now ACTIVATES at its painted location (the
/// hit-testing fix above), but caret placement must follow the same
/// transform — `input_local` mapped the raw screen point against the
/// UNtransformed rect, so a click at the painted left edge of a translated
/// input dropped the caret near the far end of the text instead of near the
/// start.
#[test]
fn transformed_text_input_places_caret_where_it_paints() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    let view: Element<()> = col().w(400.0).h(100.0).children([raw_input("hello", "")
        .id("input")
        .w(150.0)
        .translate(100.0, 0.0)]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 100.0), 1.0);
    let id = frame.get(&by::id("input")).id;
    let layout_rect = frame.rect_of(id).expect("input rect");
    // A few px in from the painted left edge (layout left + the 100px
    // translate) — near the start of "hello", not the end.
    let click = Point::new(layout_rect.x0 + 100.0 + 5.0, layout_rect.center().y);

    let _ = dispatch(
        &view,
        &frame,
        &mut state,
        &mut fonts,
        InputEvent::PointerMove {
            x: click.x as f32,
            y: click.y as f32,
        },
    );
    let _ = dispatch(&view, &frame, &mut state, &mut fonts, InputEvent::PointerDown);

    // Rebuild to bake the new caret position into the access tree.
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 100.0), 1.0);
    let selection = frame.get(&by::id("input")).selection;
    assert!(
        matches!(selection, Some((a, b)) if a == b && a <= 1),
        "clicking near the painted start of \"hello\" should place the caret \
         near byte 0, got {selection:?}"
    );
}

/// A variable-height virtual list's row count feeds an O(count) allocation
/// (`HeightIndex::ensure` builds a `heights`/`prefix` `Vec<f32>` per row) with
/// no clamp: `usize::MAX` attempts a capacity-overflow-panicking allocation
/// before any window math runs. Builder-only (the describe/JSON format has
/// no virtual-list variant), but the same clamp-over-panic contract other
/// hostile counts follow applies to the native builder too.
#[test]
fn pathological_virtual_row_count_does_not_panic() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let view: Element<()> = col().w(200.0).h(120.0).children([col()
        .h(120.0)
        .scroll_y()
        .id("huge")
        .virtual_rows_variable(usize::MAX, 24.0, |i| {
            col().h(24.0).shrink0().children([text(format!("row {i}"))])
        })]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (200.0, 120.0), 1.0);
    assert!(
        frame.rect_of(frame.get(&by::id("huge")).id).is_some(),
        "a hostile row count should still build a frame, not panic"
    );
}

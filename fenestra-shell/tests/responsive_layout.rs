//! Constraints-aware layout, headless and deterministic:
//!
//! - **Tier 1** — window-size breakpoints via [`App::view_at`], driven by
//!   [`Harness::resize`] (the headless analogue of dragging a window edge).
//! - **Tier 2** — container queries via [`responsive`]: a container picks its
//!   own layout from its measured size, converging one frame after a resize
//!   (the CSS container-query model). Driven through `build_frame` directly so
//!   each frame is observable (the `Harness` builds twice on construction, which
//!   would hide the first, hint-driven frame).
//! - The defaults: an app that implements only `view` is unaffected at any size.

use fenestra_core::{
    App, Element, Fonts, Frame, FrameState, MAIN_WINDOW, Theme, build_frame, by, col, div,
    responsive, row, text,
};
use fenestra_shell::Harness;

// ---------------------------------------------------------------------------
// Tier 1: window-size breakpoints via `view_at` + `Harness::resize`.
// ---------------------------------------------------------------------------

/// Lays its two boxes out as a column below 700px wide and a row at/above it.
struct Tier1;

impl App for Tier1 {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    // The single-window fallback is never reached (view_at is always called),
    // but the trait requires it.
    fn view(&self) -> Element<()> {
        col()
    }

    fn view_at(&self, _key: &str, size: (f32, f32)) -> Element<()> {
        let (w, _h) = size;
        let kids = [div().id("a").w(40.0).h(40.0), div().id("b").w(40.0).h(40.0)];
        if w >= 700.0 {
            row().children(kids)
        } else {
            col().children(kids)
        }
    }
}

/// `b` is stacked directly below `a` at the same x (a column).
fn stacked_vertically(f: &Frame) -> bool {
    let a = f.get(&by::id("a")).rect;
    let b = f.get(&by::id("b")).rect;
    b.y0 >= a.y1 - 0.5 && (a.x0 - b.x0).abs() < 0.5
}

/// `b` sits to the right of `a` at the same y (a row).
fn side_by_side(f: &Frame) -> bool {
    let a = f.get(&by::id("a")).rect;
    let b = f.get(&by::id("b")).rect;
    b.x0 >= a.x1 - 0.5 && (a.y0 - b.y0).abs() < 0.5
}

#[test]
fn view_at_switches_layout_on_window_resize() {
    // Narrow window: the column branch.
    let mut h = Harness::new(Tier1, Theme::light(), (400, 600));
    assert!(
        stacked_vertically(h.frame()),
        "narrow (400px) should lay out as a column:\n{}",
        h.frame().access_yaml()
    );
    assert!(!side_by_side(h.frame()));

    // Resize past the 700px breakpoint: the row branch.
    h.resize(MAIN_WINDOW, 900, 600);
    assert!(
        side_by_side(h.frame()),
        "wide (900px) should lay out as a row:\n{}",
        h.frame().access_yaml()
    );
    assert!(!stacked_vertically(h.frame()));

    // And back, to prove it tracks the size rather than latching.
    h.resize(MAIN_WINDOW, 500, 600);
    assert!(stacked_vertically(h.frame()), "narrow again is a column");
}

// ---------------------------------------------------------------------------
// Tier 2: container queries via `responsive` (one-frame-deferred convergence).
// ---------------------------------------------------------------------------

/// A fixed-width container holding a `responsive` element that lays its two
/// boxes out as a row when its own measured width reaches 400px, else a column.
/// The branches differ only in direction — both are `w_full`, so the
/// container's width is parent-driven and independent of the branch (monotone).
fn container(width: f32) -> Element<()> {
    div().w(width).h(300.0).child(
        responsive(|(w, _h)| {
            let kids = [div().id("x").w(40.0).h(40.0), div().id("y").w(40.0).h(40.0)];
            if w >= 400.0 {
                row().w_full().children(kids)
            } else {
                col().w_full().children(kids)
            }
        })
        .id("resp"),
    )
}

/// `x`/`y` stacked vertically (the column branch).
fn xy_column(f: &Frame) -> bool {
    let x = f.get(&by::id("x")).rect;
    let y = f.get(&by::id("y")).rect;
    y.y0 >= x.y1 - 0.5 && (x.x0 - y.x0).abs() < 0.5
}

/// `x`/`y` side by side (the row branch).
fn xy_row(f: &Frame) -> bool {
    let x = f.get(&by::id("x")).rect;
    let y = f.get(&by::id("y")).rect;
    y.x0 >= x.x1 - 0.5 && (x.y0 - y.y0).abs() < 0.5
}

#[test]
fn responsive_container_query_converges_one_frame_after_resize() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    // One retained state across all frames, exactly like a real window: this is
    // what carries `prev_rects` (last frame's measured rects) forward.
    let mut state = FrameState::new();
    let canvas = (800.0, 600.0);

    // Frame 1: no measurement yet, so the hint (0,0) drives the "smallest"
    // (column) branch.
    let wide = container(500.0);
    let f1 = build_frame(&wide, &theme, &mut fonts, &mut state, canvas, 1.0);
    assert!(
        xy_column(&f1),
        "frame 1 uses the hint (0,0) → column branch:\n{}",
        f1.access_yaml()
    );

    // Frame 2: the container measured 500px wide last frame (>= 400) → it
    // converges to the row branch. One extra frame, no layout cycle.
    let f2 = build_frame(&wide, &theme, &mut fonts, &mut state, canvas, 1.0);
    assert!(
        xy_row(&f2),
        "frame 2 reads last frame's 500px measurement → row branch:\n{}",
        f2.access_yaml()
    );

    // Stable while the size holds.
    let f3 = build_frame(&wide, &theme, &mut fonts, &mut state, canvas, 1.0);
    assert!(xy_row(&f3), "row branch is stable at a held width");

    // Shrink the container below the threshold. The very next frame still reads
    // the stale 500px measurement (one-frame lag), so it stays a row...
    let narrow = container(300.0);
    let f4 = build_frame(&narrow, &theme, &mut fonts, &mut state, canvas, 1.0);
    assert!(
        xy_row(&f4),
        "frame after shrink still reads the stale 500px width → still a row"
    );

    // ...then re-converges to the column branch once 300px is the measurement.
    let f5 = build_frame(&narrow, &theme, &mut fonts, &mut state, canvas, 1.0);
    assert!(
        xy_column(&f5),
        "re-converges to the column branch one frame later:\n{}",
        f5.access_yaml()
    );
}

#[test]
fn responsive_hint_avoids_the_first_frame_flash() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();

    // Seeding the first-frame size with the hint lands on the correct branch
    // immediately — no one-frame column flash before converging.
    let view: Element<()> = div().w(500.0).h(300.0).child(
        fenestra_core::responsive_hinted((500.0, 300.0), |(w, _h)| {
            let kids = [div().id("x").w(40.0).h(40.0), div().id("y").w(40.0).h(40.0)];
            if w >= 400.0 {
                row().w_full().children(kids)
            } else {
                col().w_full().children(kids)
            }
        })
        .id("resp"),
    );
    let f1 = build_frame(&view, &theme, &mut fonts, &mut state, (800.0, 600.0), 1.0);
    assert!(
        xy_row(&f1),
        "a (500,300) hint lands on the row branch on the very first frame:\n{}",
        f1.access_yaml()
    );
}

/// A closure that returns another `responsive()` under the same id would
/// recurse forever in `build`; the hop cap flattens it to empty instead. The
/// assertion is that building (twice, so `prev_rects[id]` is populated on the
/// second frame — the one that would overflow) returns rather than aborting the
/// process with a stack overflow.
#[test]
fn responsive_self_wrapping_is_capped_not_a_stack_overflow() {
    // O(1) to construct — each `responsive` defers its closure; the chain is
    // only walked (and bounded) at build time.
    fn forever() -> Element<()> {
        responsive(|_avail| forever())
    }

    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    // A real sibling next to the pathological wrapper: if building overflowed the
    // stack the process would abort before the assertion, so the sibling being
    // present is the proof the frame built. (The flattened wrapper itself is an
    // empty box and drops the `loop` key, so it is not what we query.)
    let view = div()
        .w(500.0)
        .h(300.0)
        .children([div().id("sib").w(10.0).h(10.0), forever().id("loop")]);

    let _f1 = build_frame(&view, &theme, &mut fonts, &mut state, (800.0, 600.0), 1.0);
    // Second frame reads the now-populated prev_rects[id] and re-expands — the
    // frame that would recurse without the cap.
    let f2 = build_frame(&view, &theme, &mut fonts, &mut state, (800.0, 600.0), 1.0);
    assert!(
        f2.query(&by::id("sib")).is_some(),
        "the self-wrapping responsive was capped and the frame built — no stack overflow"
    );
}

// ---------------------------------------------------------------------------
// Defaults: an app that implements only `view` is unaffected at any size.
// ---------------------------------------------------------------------------

/// Implements only `view` — neither `view_for` nor `view_at`.
struct DefaultsToView;

impl App for DefaultsToView {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col().child(text("always"))
    }
}

#[test]
fn view_at_defaults_to_view_at_every_size() {
    // `view_at` falls through to `view_for`, which falls through to `view`, so
    // the same content renders regardless of the available size.
    let mut h = Harness::new(DefaultsToView, Theme::light(), (320, 240));
    assert!(
        h.query(&by::label("always")).is_some(),
        "small window shows the view's content"
    );

    h.resize(MAIN_WINDOW, 1400, 900);
    assert!(
        h.query(&by::label("always")).is_some(),
        "a 4x larger window shows exactly the same content (size ignored)"
    );
}

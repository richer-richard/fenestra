//! Horizontal (and 2D) scrolling: the persisted `offset_x` shifts children left,
//! clamps to the content range, and is independent of the vertical axis — all
//! headless (no GPU).

use fenestra_core::{Fonts, Frame, FrameState, Theme, build_frame, by, col, div, row};

fn build(state: &mut FrameState, size: (f32, f32)) -> Frame {
    // A 200x100 horizontal scroller holding a 600-wide row (two 300px cells).
    let view = row::<()>().w(200.0).h(100.0).scroll_x().id("sc").children([
        div::<()>().id("a").w(300.0).h(50.0).shrink0(),
        div::<()>().id("b").w(300.0).h(50.0).shrink0(),
    ]);
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    build_frame(&view, &theme, &mut fonts, state, size, 1.0)
}

fn x_of(f: &Frame, id: &str) -> f64 {
    f.get(&by::id(id)).rect.x0
}

#[test]
fn horizontal_scroll_shifts_children_left() {
    let mut state = FrameState::new();
    let f = build(&mut state, (400.0, 200.0));
    let a0 = x_of(&f, "a");
    let sc = f.get(&by::id("sc")).id;

    // Scroll right by 150 logical px; the first cell shifts left by 150.
    state.scroll_to_x(sc, 150.0);
    let f = build(&mut state, (400.0, 200.0));
    assert!(
        (x_of(&f, "a") - (a0 - 150.0)).abs() < 1.0,
        "a moved to {} (expected {})",
        x_of(&f, "a"),
        a0 - 150.0
    );
}

#[test]
fn horizontal_offset_clamps_to_content() {
    let mut state = FrameState::new();
    let f = build(&mut state, (400.0, 200.0));
    let sc = f.get(&by::id("sc")).id;
    let a0 = x_of(&f, "a");

    // content 600 − viewport 200 = max 400; an overscroll clamps there.
    state.scroll_to_x(sc, 10_000.0);
    let f = build(&mut state, (400.0, 200.0));
    assert!(
        (x_of(&f, "a") - (a0 - 400.0)).abs() < 1.0,
        "a clamped to {} (expected {})",
        x_of(&f, "a"),
        a0 - 400.0
    );
    assert!(
        (state.scroll_offset_x(sc) - 400.0).abs() < 1.0,
        "offset_x clamped to {}",
        state.scroll_offset_x(sc)
    );
}

#[test]
fn nested_axes_route_to_their_own_scroller() {
    // A vertical outer scroller with a horizontal inner scroller: `dy` must find
    // the outer, `dx` the inner — they are different containers.
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let view = col::<()>()
        .w(200.0)
        .h(200.0)
        .scroll_y()
        .id("outer")
        .children([
            row::<()>()
                .w(200.0)
                .h(80.0)
                .scroll_x()
                .id("inner")
                .children([div::<()>().id("c").w(600.0).h(50.0).shrink0()]),
            div::<()>().id("tall").w(180.0).h(600.0).shrink0(),
        ]);
    let f = build_frame(&view, &theme, &mut fonts, &mut state, (300.0, 300.0), 1.0);
    let inner = f.get(&by::id("inner"));
    let p = inner.rect.center();
    assert_eq!(
        f.scrollable_x_at(p),
        Some(inner.id),
        "horizontal wheel routes to the inner horizontal scroller"
    );
    assert_eq!(
        f.scrollable_y_at(p),
        Some(f.get(&by::id("outer")).id),
        "vertical wheel routes to the outer vertical scroller"
    );
}

#[test]
fn sticky_clamps_to_content_box_past_padding() {
    // A scroll container with 20px left padding: a sticky_left(0) child pins to
    // the content box (x0 + 20), not the border-box left.
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let view = || {
        row::<()>()
            .w(200.0)
            .h(100.0)
            .pl(20.0)
            .scroll_x()
            .id("sc")
            .children([
                col::<()>()
                    .id("s")
                    .w(40.0)
                    .h(50.0)
                    .shrink0()
                    .sticky_left(0.0),
                col::<()>().id("body").w(600.0).h(50.0).shrink0(),
            ])
    };
    let f = build_frame(&view(), &theme, &mut fonts, &mut state, (300.0, 200.0), 1.0);
    let sc = f.get(&by::id("sc")).id;
    let sc_x0 = f.get(&by::id("sc")).rect.x0;
    state.scroll_to_x(sc, 100.0); // scroll right so the sticky engages
    let f = build_frame(&view(), &theme, &mut fonts, &mut state, (300.0, 200.0), 1.0);
    assert!(
        (x_of(&f, "s") - (sc_x0 + 20.0)).abs() < 1.0,
        "sticky pins to content-box left {}, got {}",
        sc_x0 + 20.0,
        x_of(&f, "s")
    );
}

#[test]
fn axes_are_independent() {
    let mut state = FrameState::new();
    let f = build(&mut state, (400.0, 200.0));
    let sc = f.get(&by::id("sc")).id;
    // The horizontal scroller does not scroll vertically (content fits height).
    state.scroll_to(sc, 999.0);
    state.scroll_to_x(sc, 100.0);
    let _ = build(&mut state, (400.0, 200.0));
    assert!(
        state.scroll_offset(sc).abs() < 1.0,
        "vertical offset clamps to 0 (content fits): {}",
        state.scroll_offset(sc)
    );
    assert!((state.scroll_offset_x(sc) - 100.0).abs() < 1.0);
}

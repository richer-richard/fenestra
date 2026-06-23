//! `position: sticky`: a sticky header pins to its scroll viewport once scrolled
//! past its threshold, while non-sticky siblings scroll normally; with no scroll
//! ancestor sticky is inert. All headless.

use fenestra_core::{Fonts, Frame, FrameState, Theme, build_frame, by, col};

fn y_of(f: &Frame, id: &str) -> f64 {
    f.get(&by::id(id)).rect.y0
}

#[test]
fn sticky_top_pins_header_to_viewport() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let view = || {
        col::<()>().w(200.0).h(200.0).scroll_y().id("sc").children([
            col::<()>().id("hdr").h(30.0).shrink0().sticky_top(0.0),
            col::<()>().id("body").h(600.0).shrink0(),
        ])
    };
    let f = build_frame(&view(), &theme, &mut fonts, &mut state, (300.0, 300.0), 1.0);
    let hdr0 = y_of(&f, "hdr"); // natural top (container origin)
    let sc = f.get(&by::id("sc")).id;

    // Scroll the body down by 100; the header must stay pinned at the viewport top.
    state.scroll_to(sc, 100.0);
    let f = build_frame(&view(), &theme, &mut fonts, &mut state, (300.0, 300.0), 1.0);
    assert!(
        (y_of(&f, "hdr") - hdr0).abs() < 1.0,
        "sticky header should pin at {hdr0}, got {}",
        y_of(&f, "hdr")
    );
    // The non-sticky body scrolled up with the content (natural 30 − 100).
    assert!(
        (y_of(&f, "body") - (30.0 - 100.0)).abs() < 1.5,
        "body should scroll to {}, got {}",
        30.0 - 100.0,
        y_of(&f, "body")
    );
}

#[test]
fn sticky_stays_natural_until_scrolled_past() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    // A sticky element 100px down the content: unscrolled, it sits at its natural
    // y=100 (below the stick line), not yet pinned.
    let view = col::<()>().w(200.0).h(200.0).scroll_y().id("sc").children([
        col::<()>().id("spacer").h(100.0).shrink0(),
        col::<()>().id("s").h(30.0).shrink0().sticky_top(0.0),
        col::<()>().id("body").h(600.0).shrink0(),
    ]);
    let f = build_frame(&view, &theme, &mut fonts, &mut state, (300.0, 300.0), 1.0);
    assert!(
        (y_of(&f, "s") - 100.0).abs() < 1.0,
        "sticky below the stick line stays natural at 100, got {}",
        y_of(&f, "s")
    );
}

#[test]
fn sticky_top_wins_over_bottom_when_conflicting() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    // Element 60 tall in a 100 viewport, top:30 and bottom:30 cannot both hold
    // (top line 30 > bottom line 10) — top must win (CSS positioned layout).
    let view = || {
        col::<()>().w(200.0).h(100.0).scroll_y().id("sc").children([
            col::<()>()
                .id("s")
                .h(60.0)
                .shrink0()
                .sticky_top(30.0)
                .sticky_bottom(30.0),
            col::<()>().id("body").h(600.0).shrink0(),
        ])
    };
    let f = build_frame(&view(), &theme, &mut fonts, &mut state, (300.0, 300.0), 1.0);
    let sc = f.get(&by::id("sc")).id;
    state.scroll_to(sc, 50.0);
    let f = build_frame(&view(), &theme, &mut fonts, &mut state, (300.0, 300.0), 1.0);
    assert!(
        (y_of(&f, "s") - 30.0).abs() < 1.0,
        "top (line 30) wins over bottom (line 10), got {}",
        y_of(&f, "s")
    );
}

#[test]
fn sticky_without_scroll_ancestor_is_inert() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let view = col::<()>().w(200.0).h(400.0).children([
        col::<()>().id("s").h(30.0).sticky_top(0.0),
        col::<()>().id("b").h(100.0),
    ]);
    let f = build_frame(&view, &theme, &mut fonts, &mut state, (300.0, 500.0), 1.0);
    // No scrolling ancestor → sticky is a no-op; element keeps its flow position.
    assert!(
        y_of(&f, "s").abs() < 1.0,
        "sticky inert at {}",
        y_of(&f, "s")
    );
    assert!((y_of(&f, "b") - 30.0).abs() < 1.0);
}

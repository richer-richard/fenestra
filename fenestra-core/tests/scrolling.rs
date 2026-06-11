//! 0.4 scrolling: absolute `scroll_to`, the stick-to-bottom chat pattern,
//! and keyboard paging.

use fenestra_core::{
    Element, Fonts, FrameState, InputEvent, Key, KeyInput, Theme, build_frame, col, dispatch, div,
};
use kurbo::Point;

// 80px viewport over tall content: max offset = content - 80.
fn plain(content_h: f32) -> Element<()> {
    col().w(200.0).h(100.0).children([col()
        .h(80.0)
        .scroll_y()
        .id("list")
        .children([div().h(content_h).shrink0()])])
}

fn sticky(content_h: f32) -> Element<()> {
    col().w(200.0).h(100.0).children([col()
        .h(80.0)
        .scroll_y()
        .stick_to_bottom()
        .id("log")
        .children([div().h(content_h).shrink0()])])
}

fn build(view: &Element<()>, fonts: &mut Fonts, state: &mut FrameState) -> fenestra_core::Frame {
    build_frame(view, &Theme::light(), fonts, state, (200.0, 100.0), 1.0)
}

#[test]
fn scroll_to_is_absolute_and_clamped() {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build(&plain(800.0), &mut fonts, &mut state);
    let list = frame.scrollable_at(Point::new(20.0, 20.0)).expect("list");
    drop(frame);

    state.scroll_to(list, 1.0e9);
    let _ = build(&plain(800.0), &mut fonts, &mut state);
    assert_eq!(state.scroll_offset(list), 720.0, "clamped to the bottom");

    state.scroll_to(list, -50.0);
    let _ = build(&plain(800.0), &mut fonts, &mut state);
    assert_eq!(state.scroll_offset(list), 0.0, "clamped to the top");
}

#[test]
fn stick_to_bottom_follows_growth() {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();

    // A sticky log starts at the bottom...
    let frame = build(&sticky(800.0), &mut fonts, &mut state);
    let log = frame.scrollable_at(Point::new(20.0, 20.0)).expect("log");
    drop(frame);
    assert_eq!(state.scroll_offset(log), 720.0, "starts pinned to bottom");

    // ...and stays there as content grows.
    let _ = build(&sticky(1200.0), &mut fonts, &mut state);
    assert_eq!(state.scroll_offset(log), 1120.0, "follows growth");

    // Scrolled away from the bottom, growth leaves the offset alone.
    state.scroll_to(log, 100.0);
    let _ = build(&sticky(1200.0), &mut fonts, &mut state);
    assert_eq!(state.scroll_offset(log), 100.0);
    let _ = build(&sticky(1600.0), &mut fonts, &mut state);
    assert_eq!(
        state.scroll_offset(log),
        100.0,
        "no yanking while reading scrollback"
    );
}

#[test]
fn keyboard_paging_scrolls_the_scrollable() {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let view = plain(800.0);
    let frame = build(&view, &mut fonts, &mut state);
    let list = frame.scrollable_at(Point::new(20.0, 20.0)).expect("list");

    // No focus at all: paging still targets the first scrollable.
    let _ = dispatch(
        &view,
        &frame,
        &mut state,
        &mut fonts,
        InputEvent::Key(KeyInput::plain(Key::PageDown)),
    );
    let _ = build(&view, &mut fonts, &mut state);
    let after_page = state.scroll_offset(list);
    assert!(
        (60.0..=80.0).contains(&after_page),
        "one page is ~90% of the viewport, got {after_page}"
    );

    let frame = build(&view, &mut fonts, &mut state);
    let _ = dispatch(
        &view,
        &frame,
        &mut state,
        &mut fonts,
        InputEvent::Key(KeyInput::plain(Key::End)),
    );
    let _ = build(&view, &mut fonts, &mut state);
    assert_eq!(state.scroll_offset(list), 720.0, "End jumps to the bottom");

    let frame = build(&view, &mut fonts, &mut state);
    let _ = dispatch(
        &view,
        &frame,
        &mut state,
        &mut fonts,
        InputEvent::Key(KeyInput::plain(Key::Home)),
    );
    let _ = build(&view, &mut fonts, &mut state);
    assert_eq!(state.scroll_offset(list), 0.0, "Home jumps to the top");
}

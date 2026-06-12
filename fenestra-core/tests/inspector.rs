//! The headless inspector: `debug_tree` (layout + flags + source
//! provenance) and `access_yaml` (Playwright aria-snapshot grammar),
//! locked by snapshots so the formats stay stable for agents.

use fenestra_core::{
    Element, Fonts, FrameState, Semantics, Theme, build_frame, col, div, raw_input, row, text,
};

fn view() -> Element<()> {
    col().p(8.0).gap(4.0).children([
        text("Inbox"),
        raw_input("draft text", "Type here").id("draft"),
        row().gap(4.0).id("actions").children([
            div()
                .w(60.0)
                .h(24.0)
                .focusable(true)
                .semantics(Semantics::Button)
                .label("Send"),
            div()
                .w(60.0)
                .h(24.0)
                .focusable(true)
                .disabled(true)
                .semantics(Semantics::Button)
                .label("Discard"),
        ]),
        col()
            .h(40.0)
            .scroll_y()
            .id("log")
            .children([div().h(200.0).shrink0()]),
    ])
}

#[test]
fn debug_tree_shows_layout_flags_and_source() {
    let view = view();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(
        &view,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (320.0, 240.0),
        1.0,
    );
    let dump = frame.debug_tree();

    // Structure facts an agent greps for.
    assert!(dump.contains("input #draft"), "{dump}");
    assert!(dump.contains("value=\"draft text\""), "{dump}");
    assert!(dump.contains("#log"), "{dump}");
    assert!(dump.contains(" scroll"), "{dump}");
    assert!(dump.contains("button \"Send\""), "{dump}");
    // Source provenance points at THIS file (track_caller).
    assert!(
        dump.contains("src=fenestra-core/tests/inspector.rs:"),
        "{dump}"
    );

    // Lock the shape (sans volatile line numbers) with insta.
    let stable: String = dump
        .lines()
        .map(|l| l.split(" src=").next().unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("debug_tree", stable);
}

#[test]
fn access_yaml_matches_the_aria_grammar() {
    let view = view();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(
        &view,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (320.0, 240.0),
        1.0,
    );
    insta::assert_snapshot!("access_yaml", frame.access_yaml());
}

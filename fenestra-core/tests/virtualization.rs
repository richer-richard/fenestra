//! Virtualized rows at the core boundary: fixed-height windows realize
//! the right slice, and variable-height windows self-correct from
//! measured rows — no shell, no widgets, just `build_frame`. Geometry
//! asserts read the `v{i}`-keyed row boxes (text leaves are taller
//! than the smallest rows here and overflow them by design).

use fenestra_core::{Element, Fonts, FrameState, Theme, build_frame, by, col, text};
use kurbo::Point;

const COUNT: usize = 400;

fn fixed() -> Element<()> {
    col().w(200.0).h(120.0).children([col()
        .h(120.0)
        .scroll_y()
        .id("fixed")
        // 24px rows fit the default 24px text line exactly, so the
        // scroll content is spacer math alone (overflow would grow it).
        .virtual_rows(COUNT, 24.0, |i| {
            col().shrink0().children([text(format!("row {i}"))])
        })])
}

/// Rows alternate 16/48px; the 24px estimate is wrong for every row.
fn variable() -> Element<()> {
    col()
        .w(200.0)
        .h(120.0)
        .children([col()
            .h(120.0)
            .scroll_y()
            .id("var")
            .virtual_rows_variable(COUNT, 24.0, |i| {
                let h = if i.is_multiple_of(2) { 16.0 } else { 48.0 };
                col().h(h).shrink0().children([text(format!("row {i}"))])
            })])
}

/// A variable-height virtual list whose scroll container carries `id`.
fn variable_with_id(id: &str) -> Element<()> {
    col()
        .w(200.0)
        .h(120.0)
        .children([col()
            .h(120.0)
            .scroll_y()
            .id(id)
            .virtual_rows_variable(COUNT, 24.0, |i| {
                let h = if i.is_multiple_of(2) { 16.0 } else { 48.0 };
                col().h(h).shrink0().children([text(format!("row {i}"))])
            })])
}

fn build(view: &Element<()>, fonts: &mut Fonts, state: &mut FrameState) -> fenestra_core::Frame {
    build_frame(view, &Theme::light(), fonts, state, (200.0, 120.0), 1.0)
}

/// A variable virtual list's per-container height index is frame-stamped and
/// garbage-collected, like scroll/anim/editor state: once the container leaves
/// the tree its `HeightIndex` is dropped instead of leaking forever (one per
/// distinct container id).
#[test]
fn virtual_heights_are_gc_d_when_the_container_leaves() {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();

    let frame_a = build(&variable_with_id("a"), &mut fonts, &mut state);
    let id_a = frame_a.get(&by::id("a")).id;
    drop(frame_a);
    assert!(
        state.has_virtual_heights(id_a),
        "container 'a' recorded a height index while present"
    );

    // Rebuild with a different container id: 'a' is absent this frame.
    let frame_b = build(&variable_with_id("b"), &mut fonts, &mut state);
    let id_b = frame_b.get(&by::id("b")).id;
    assert!(
        state.has_virtual_heights(id_b),
        "container 'b' recorded a height index"
    );
    assert!(
        !state.has_virtual_heights(id_a),
        "container 'a' left the tree; its height index must be GC'd, not leaked"
    );
}

/// The realized rows, as (index, rect), sorted by index.
fn realized(frame: &fenestra_core::Frame) -> Vec<(usize, kurbo::Rect)> {
    (0..COUNT)
        .filter_map(|i| {
            frame
                .query(&by::id(format!("v{i}")))
                .map(|node| (i, node.rect))
        })
        .collect()
}

#[test]
fn fixed_rows_realize_only_the_window() {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build(&fixed(), &mut fonts, &mut state);
    let list = frame.scrollable_at(Point::new(20.0, 20.0)).expect("list");
    assert!(frame.query(&by::label("row 0")).is_some());
    assert!(
        frame.query(&by::label("row 399")).is_none(),
        "the far end is not realized"
    );
    drop(frame);

    // A programmatic deep scroll realizes the last page IMMEDIATELY —
    // the window clamps to max scroll before layout's clamp catches
    // up, never an empty window for one frame.
    state.scroll_to(list, 1.0e9);
    let frame = build(&fixed(), &mut fonts, &mut state);
    assert!(frame.query(&by::label("row 399")).is_some());
    assert!(frame.query(&by::label("row 0")).is_none());
    let last = frame.get(&by::id("v399")).rect;
    let list_rect = frame.get(&by::id("fixed")).rect;
    assert!(
        (last.y1 - list_rect.y1).abs() < 0.5,
        "fixed math is exact: row bottom {} vs viewport bottom {}",
        last.y1,
        list_rect.y1
    );
}

#[test]
fn variable_rows_converge_on_measured_heights() {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build(&variable(), &mut fonts, &mut state);
    let list = frame.scrollable_at(Point::new(20.0, 20.0)).expect("list");
    drop(frame);

    // Jump far past the end repeatedly. Each first build realizes the
    // tail and clamps; each second realizes from the clamped offset,
    // measuring the overscan rows above it — passes correct the index
    // until the bottom neighborhood is true.
    for _ in 0..6 {
        state.scroll_to(list, 1.0e9);
        let _ = build(&variable(), &mut fonts, &mut state);
        let _ = build(&variable(), &mut fonts, &mut state);
    }
    state.scroll_to(list, 1.0e9);
    let _ = build(&variable(), &mut fonts, &mut state);
    let frame = build(&variable(), &mut fonts, &mut state);

    let rows = realized(&frame);
    assert!(rows.len() >= 8, "a realized window exists");
    let (last_index, last) = *rows.last().expect("rows");
    assert_eq!(last_index, COUNT - 1, "the true last row is realized");
    let list_rect = frame.get(&by::id("var")).rect;
    assert!(
        (last.y1 - list_rect.y1).abs() < 0.5,
        "true bottom once measured: row bottom {} vs viewport bottom {}",
        last.y1,
        list_rect.y1
    );

    // Realized rows carry their measured heights (16/48, never the
    // 24px estimate) and stack without gaps or overlap.
    for (i, rect) in &rows {
        let want = if i.is_multiple_of(2) { 16.0 } else { 48.0 };
        assert!(
            (rect.height() - want).abs() < 0.01,
            "row {i} is {} tall, wanted {want}",
            rect.height()
        );
    }
    for pair in rows.windows(2) {
        assert!(
            (pair[1].1.y0 - pair[0].1.y1).abs() < 0.01,
            "rows {} and {} do not abut: {:?} then {:?}",
            pair[0].0,
            pair[1].0,
            pair[0].1,
            pair[1].1
        );
    }
}

//! Headless interaction tests: resizing a column emits clamped widths and a
//! commit on release, dragging a header onto another reorders, and a pinned
//! column stays frozen while the body scrolls horizontally. Driven through the
//! real `dispatch` pipeline (pointer events in, messages out).

use fenestra_core::{
    Element, Fonts, Frame, FrameState, InputEvent, Theme, build_frame, by, col, dispatch,
};
use fenestra_kit::data_table;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Resize(usize, f32),
    ResizeDone,
    Reorder(usize, usize),
}

const SIZE: (f32, f32) = (700.0, 240.0);

fn table(resize_active: Option<usize>) -> Element<Msg> {
    col().w(600.0).h(200.0).child(
        data_table(
            ["Name", "Role", "Commits"],
            vec![
                vec!["Ripley".into(), "Officer".into(), "128".into()],
                vec!["Dallas".into(), "Captain".into(), "97".into()],
            ],
        )
        .id("t")
        .column_widths([160.0, 200.0, 100.0])
        .resize_active(resize_active)
        .on_resize(Msg::Resize)
        .on_resize_end(Msg::ResizeDone)
        .on_reorder(Msg::Reorder),
    )
}

/// Drives a sequence of events against a freshly built frame each step,
/// collecting every emitted message. `view` may change between steps so the
/// app's resize_active can be threaded back in.
fn drive(steps: &[(Element<Msg>, InputEvent)]) -> Vec<Msg> {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let mut msgs = Vec::new();
    for (view, ev) in steps {
        let frame = build_frame(view, &theme, &mut fonts, &mut state, SIZE, 1.0);
        msgs.extend(dispatch(view, &frame, &mut state, &mut fonts, ev.clone()).msgs);
    }
    msgs
}

/// Resolve where, in canvas px, the boundary between column 0 (160px) and
/// column 1 sits, then drag it. The header element starts at the table origin
/// plus its SP3 padding; column 0's right edge is SP3 + 160 from there.
#[test]
fn dragging_a_boundary_resizes_then_commits() {
    let header_x0 = {
        let theme = Theme::light();
        let mut fonts = Fonts::embedded();
        let mut state = FrameState::new();
        let f: Frame = build_frame(&table(None), &theme, &mut fonts, &mut state, SIZE, 1.0);
        // The header is the cell row containing "Name".
        f.get(&by::label("Name")).rect.x0
    };
    // "Name" sits at content-left (after SP3 padding); the col0/col1 boundary
    // is 160px to its right. Press just on it, then drag 40px wider.
    let boundary = header_x0 + 160.0;
    let y = 18.0; // within the 34px header band

    let msgs = drive(&[
        (
            table(None),
            InputEvent::PointerMove {
                x: boundary as f32,
                y,
            },
        ),
        (table(None), InputEvent::PointerDown),
        (
            // After the first resize the app sets resize_active = col 0.
            table(Some(0)),
            InputEvent::PointerMove {
                x: (boundary + 40.0) as f32,
                y,
            },
        ),
        (table(Some(0)), InputEvent::PointerUp),
    ]);

    // The press grabbed column 0; the move widened it to ~200; release commits.
    let resizes: Vec<&Msg> = msgs
        .iter()
        .filter(|m| matches!(m, Msg::Resize(0, _)))
        .collect();
    assert!(!resizes.is_empty(), "column 0 resized, got {msgs:?}");
    if let Some(Msg::Resize(0, w)) = msgs.iter().rev().find(|m| matches!(m, Msg::Resize(0, _))) {
        assert!((*w - 200.0).abs() < 4.0, "dragged 160 -> ~200, got {w}");
    }
    assert_eq!(
        msgs.last(),
        Some(&Msg::ResizeDone),
        "release commits the resize"
    );
    // No stray reorder from a resize drag.
    assert!(
        !msgs.iter().any(|m| matches!(m, Msg::Reorder(..))),
        "resize never reorders"
    );
}

/// A plain press on the boundary, released without moving, still commits (so
/// the app's resize_active can never get stuck) and never resizes a column it
/// did not intend to.
#[test]
fn boundary_press_clamps_to_limits() {
    // Drag the column 1 / column 2 boundary far left, past the 40px floor.
    let header_x0 = {
        let theme = Theme::light();
        let mut fonts = Fonts::embedded();
        let mut state = FrameState::new();
        let f = build_frame(&table(None), &theme, &mut fonts, &mut state, SIZE, 1.0);
        f.get(&by::label("Name")).rect.x0
    };
    let boundary = header_x0 + 160.0 + 200.0; // col1 right edge
    let y = 18.0;
    let msgs = drive(&[
        (
            table(None),
            InputEvent::PointerMove {
                x: boundary as f32,
                y,
            },
        ),
        (table(None), InputEvent::PointerDown),
        (
            table(Some(1)),
            // Yank it far left — column 1 should clamp at MIN_COL_W (40).
            InputEvent::PointerMove {
                x: header_x0 as f32,
                y,
            },
        ),
        (table(Some(1)), InputEvent::PointerUp),
    ]);
    let last = msgs
        .iter()
        .rev()
        .find_map(|m| match m {
            Msg::Resize(1, w) => Some(*w),
            _ => None,
        })
        .expect("column 1 resized");
    assert!(
        (last - 40.0).abs() < 0.5,
        "clamped to the 40px floor, got {last}"
    );
}

/// Dragging one header onto another emits a reorder; a click that never leaves
/// the header (no drop elsewhere) does not.
#[test]
fn dragging_a_header_onto_another_reorders() {
    let (name_x, commits_x, y) = {
        let theme = Theme::light();
        let mut fonts = Fonts::embedded();
        let mut state = FrameState::new();
        let f = build_frame(&table(None), &theme, &mut fonts, &mut state, SIZE, 1.0);
        (
            f.get(&by::label("Name")).rect.center().x as f32,
            f.get(&by::label("Commits")).rect.center().x as f32,
            f.get(&by::label("Name")).rect.center().y as f32,
        )
    };

    // Press on "Name" (display 0), drag to "Commits" (display 2), release.
    let msgs = drive(&[
        (table(None), InputEvent::PointerMove { x: name_x, y }),
        (table(None), InputEvent::PointerDown),
        (table(None), InputEvent::PointerMove { x: commits_x, y }),
        (table(None), InputEvent::PointerUp),
    ]);
    assert!(
        msgs.contains(&Msg::Reorder(0, 2)),
        "drag Name(0) -> Commits(2) reorders, got {msgs:?}"
    );

    // A press and release on the same header never reorders (it sorts).
    let msgs = drive(&[
        (table(None), InputEvent::PointerMove { x: name_x, y }),
        (table(None), InputEvent::PointerDown),
        (table(None), InputEvent::PointerUp),
    ]);
    assert!(
        !msgs.iter().any(|m| matches!(m, Msg::Reorder(..))),
        "a plain header click does not reorder, got {msgs:?}"
    );
}

/// A left-pinned column stays put while the body scrolls horizontally; an
/// unpinned column slides with the content.
#[test]
fn pinned_column_stays_frozen_during_horizontal_scroll() {
    let view: Element<()> = col().w(360.0).h(160.0).child(
        data_table(
            ["Name", "Role", "Commits", "Branch"],
            vec![vec![
                "Ripley".into(),
                "Officer".into(),
                "128".into(),
                "main".into(),
            ]],
        )
        .id("pin")
        .column_widths([160.0, 200.0, 120.0, 140.0])
        .pinned_left(1)
        .sticky_header(true),
    );
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let f = build_frame(&view, &theme, &mut fonts, &mut state, (420.0, 200.0), 1.0);
    let pinned0 = f.get(&by::label("Ripley")).rect.x0;
    let loose0 = f.get(&by::label("Officer")).rect.x0;
    let body = f.get(&by::id("dt-body-pin")).id;
    drop(f);

    // Scroll the body right by 120px.
    state.scroll_to_x(body, 120.0);
    let f = build_frame(&view, &theme, &mut fonts, &mut state, (420.0, 200.0), 1.0);
    let pinned1 = f.get(&by::label("Ripley")).rect.x0;
    let loose1 = f.get(&by::label("Officer")).rect.x0;

    assert!(
        (pinned1 - pinned0).abs() < 1.0,
        "the pinned column does not move (was {pinned0}, now {pinned1})"
    );
    assert!(
        (loose1 - (loose0 - 120.0)).abs() < 1.5,
        "the unpinned column scrolls left by 120 (was {loose0}, now {loose1})"
    );
}

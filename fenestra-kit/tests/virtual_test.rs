//! Virtualized rows: only the visible window materializes, scrolling
//! reveals the right rows, and handlers on materialized rows fire.

use fenestra_core::{
    AccessNode, App, Element, Fonts, FrameState, Theme, build_frame, col, row, text,
};
use fenestra_kit::virtual_list;
use fenestra_shell::{SyntheticEvent, render_app};
use kurbo::Point;

fn data_row<Msg: 'static>(i: usize) -> Element<Msg> {
    row()
        .items_center()
        .px(8.0)
        .children([text(format!("Row {i}"))])
}

fn count_rows(node: &AccessNode) -> usize {
    let own = usize::from(node.label.as_deref().is_some_and(|l| l.starts_with("Row ")));
    own + node.children.iter().map(count_rows).sum::<usize>()
}

fn has_row(node: &AccessNode, label: &str) -> bool {
    node.label.as_deref() == Some(label) || node.children.iter().any(|c| has_row(c, label))
}

/// 100k rows in a 300px viewport materialize only ~a screenful of nodes.
#[test]
fn only_visible_rows_materialize() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let view: Element<()> = col()
        .w(400.0)
        .h(300.0)
        .children([virtual_list(100_000, 36.0, data_row).id("vl")]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 300.0), 1.0);
    let rows = count_rows(&frame.access_tree());
    assert!(
        (8..=40).contains(&rows),
        "expected about a viewport of rows, got {rows}"
    );
}

/// Scrolling deep into the list materializes that window.
#[test]
fn scrolling_shows_the_right_rows() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let view = || -> Element<()> {
        col()
            .w(400.0)
            .h(300.0)
            .children([virtual_list(100_000, 36.0, data_row).id("vl")])
    };
    let frame = build_frame(&view(), &theme, &mut fonts, &mut state, (400.0, 300.0), 1.0);
    let list = frame
        .scrollable_at(Point::new(50.0, 50.0))
        .expect("virtual list is scrollable");
    drop(frame);
    state.scroll_by(list, 36.0 * 50_000.0);
    let frame = build_frame(&view(), &theme, &mut fonts, &mut state, (400.0, 300.0), 1.0);
    let tree = frame.access_tree();
    assert!(
        has_row(&tree, "Row 50000"),
        "the scrolled-to window should contain row 50000"
    );
    assert!(
        !has_row(&tree, "Row 0"),
        "the start of the list should no longer be materialized"
    );
}

struct Pick {
    picked: Option<usize>,
}

#[derive(Clone)]
enum PickMsg {
    Pick(usize),
}

impl App for Pick {
    type Msg = PickMsg;

    fn update(&mut self, msg: PickMsg) {
        match msg {
            PickMsg::Pick(i) => self.picked = Some(i),
        }
    }

    fn view(&self) -> Element<PickMsg> {
        col()
            .w(400.0)
            .h(300.0)
            .children([
                virtual_list(10_000, 36.0, |i| data_row(i).on_click(PickMsg::Pick(i))).id("vl"),
            ])
    }
}

/// Handlers on materialized rows dispatch like any other element.
#[test]
fn virtual_rows_receive_clicks() {
    let theme = Theme::light();
    let mut app = Pick { picked: None };
    render_app(
        &mut app,
        &[
            // y=100 falls in row 2 (rows are 36px: 72..108).
            SyntheticEvent::MouseMove { x: 60.0, y: 100.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (400, 300),
        &theme,
    );
    assert_eq!(app.picked, Some(2));
}

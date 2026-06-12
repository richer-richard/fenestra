//! 0.10 variable-height virtualization: estimates place rows, measured
//! heights correct them, the bottom is the true bottom, rows never
//! overlap, and handlers line up with painted rows.

use fenestra_core::{App, Element, Theme, by, col, text};
use fenestra_kit::virtual_list_variable;
use fenestra_shell::Harness;

const COUNT: usize = 500;

/// Rows alternate 24/64px (the estimate of 30 is wrong for both).
fn row_height(i: usize) -> f32 {
    if i.is_multiple_of(3) { 64.0 } else { 24.0 }
}

struct Feed {
    clicked: Option<usize>,
}

#[derive(Clone)]
struct Open(usize);

impl App for Feed {
    type Msg = Open;

    fn update(&mut self, Open(i): Open) {
        self.clicked = Some(i);
    }

    fn view(&self) -> Element<Open> {
        col()
            .w(320.0)
            .h(400.0)
            .children([virtual_list_variable(COUNT, 30.0, |i| {
                col()
                    .h(row_height(i))
                    .shrink0()
                    .on_click(Open(i))
                    .children([text(format!("row {i}"))])
            })
            .id("feed")])
    }
}

fn scroll_to_bottom(h: &mut Harness<Feed>) {
    // Wheel far past the end repeatedly; each rebuild re-clamps against
    // the (correcting) content height.
    for _ in 0..6 {
        h.wheel(&by::id("feed"), -1.0e7);
    }
}

#[test]
fn bottom_is_the_true_last_row() {
    let mut h = Harness::new(Feed { clicked: None }, Theme::light(), (340, 420));
    scroll_to_bottom(&mut h);
    let last = h.get(&by::label(format!("row {}", COUNT - 1)));
    let list = h.get(&by::id("feed"));
    // The real last row sits flush with the viewport bottom (within a
    // pixel — offsets are exact once its neighborhood measured).
    assert!(
        (last.rect.y1 - list.rect.y1).abs() < 1.5,
        "last row bottom {} vs viewport bottom {}",
        last.rect.y1,
        list.rect.y1
    );
}

#[test]
fn rows_never_overlap_and_match_their_heights() {
    let mut h = Harness::new(Feed { clicked: None }, Theme::light(), (340, 420));
    // Walk a few screenfuls, checking every visible neighbor pair.
    for step in 0..12 {
        let rows: Vec<_> = h.get_all(&by::label_contains("row "));
        let mut sorted = rows.clone();
        sorted.sort_by(|a, b| a.rect.y0.total_cmp(&b.rect.y0));
        for pair in sorted.windows(2) {
            assert!(
                pair[1].rect.y0 >= pair[0].rect.y1 - 0.01,
                "rows overlap at step {step}: {:?} then {:?}",
                pair[0].label,
                pair[1].label
            );
        }
        h.wheel(&by::id("feed"), -380.0);
    }
}

#[test]
fn handlers_line_up_with_painted_rows() {
    let mut h = Harness::new(Feed { clicked: None }, Theme::light(), (340, 420));
    scroll_to_bottom(&mut h);
    let target = COUNT - 2;
    h.click(&by::label(format!("row {target}")));
    assert_eq!(h.app().clicked, Some(target), "the right row received it");
}

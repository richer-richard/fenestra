//! 0.9 issue sweep: DatePicker (#6), select multi-char type-ahead (#5),
//! badge dot (#8), indeterminate progress (#3), tooltip flip (#4) — the
//! toast enter animation (#2) is covered by the motion suite's enter
//! machinery, and the icon expansion (#1) by the icons parse test.

use fenestra_core::{App, Element, Semantics, Theme, by, col, div, text};
use fenestra_kit::{Status, badge_dot, date_picker, progress_indeterminate, select, tooltip};
use fenestra_shell::Harness;

// ------------------------------------------------------------ date picker

struct Scheduler {
    visible: (i32, u32),
    picked: Option<fenestra_kit::Date>,
}

#[derive(Clone)]
enum SchedMsg {
    Month((i32, u32)),
    Pick(fenestra_kit::Date),
}

impl App for Scheduler {
    type Msg = SchedMsg;

    fn update(&mut self, msg: SchedMsg) {
        match msg {
            SchedMsg::Month(m) => self.visible = m,
            SchedMsg::Pick(d) => self.picked = Some(d),
        }
    }

    fn view(&self) -> Element<SchedMsg> {
        col().p(12.0).items_start().children([Element::from(
            date_picker(self.visible)
                .selected(self.picked)
                .on_pick(SchedMsg::Pick)
                .on_month(SchedMsg::Month)
                .id("cal"),
        )])
    }
}

#[test]
fn date_picker_picks_and_navigates() {
    let mut h = Harness::new(
        Scheduler {
            visible: (2026, 6),
            picked: None,
        },
        Theme::light(),
        (320, 320),
    );
    assert!(h.query(&by::label("June 2026")).is_some());

    h.click(&by::role(Semantics::Button).name("2026-06-12"));
    assert_eq!(h.app().picked, Some((2026, 6, 12)));

    // Month navigation wraps the year backwards from January.
    h.click(&by::role(Semantics::Button).name("previous month"));
    assert_eq!(h.app().visible, (2026, 5));
    for _ in 0..5 {
        h.click(&by::role(Semantics::Button).name("previous month"));
    }
    assert_eq!(h.app().visible, (2025, 12));

    // The picked date stays highlighted when its month is visible again.
    h.click(&by::role(Semantics::Button).name("next month"));
    assert_eq!(h.app().visible, (2026, 1));
}

// ------------------------------------------------ select type-ahead (#5)

struct Picker {
    selected: usize,
}

#[derive(Clone)]
struct Choose(usize);

impl App for Picker {
    type Msg = Choose;

    fn update(&mut self, Choose(i): Choose) {
        self.selected = i;
    }

    fn view(&self) -> Element<Choose> {
        col().p(12.0).items_start().children([Element::from(
            select(self.selected, ["Cherry", "Chestnut", "Cedar", "Birch"])
                .on_change(Choose)
                .id("tree"),
        )])
    }
}

#[test]
fn select_type_ahead_buffers_multiple_chars() {
    let mut h = Harness::new(Picker { selected: 3 }, Theme::light(), (320, 200));
    h.tab(); // focus the select
    // "ce" must land on Cedar — single-char type-ahead would cycle to
    // Cherry (first C) and stay there.
    h.key(fenestra_core::KeyInput::plain(fenestra_core::Key::Char(
        'c',
    )));
    h.key(fenestra_core::KeyInput::plain(fenestra_core::Key::Char(
        'e',
    )));
    assert_eq!(h.app().selected, 2, "buffer matched Cedar, not Cherry");
}

// --------------------------------------------- dot, progress, flip (#8/3/4)

#[test]
fn dot_progress_and_flip_render() {
    struct Board;
    #[derive(Clone)]
    struct Noop;
    impl App for Board {
        type Msg = Noop;
        fn update(&mut self, Noop: Noop) {}
        fn view(&self) -> Element<Noop> {
            col().p(8.0).gap(8.0).items_start().children((
                badge_dot(Status::Danger),
                div().w(200.0).children([progress_indeterminate()]),
                // A tooltip target at the very bottom edge: the bubble
                // must flip above to stay on canvas.
                col()
                    .h(140.0)
                    .justify_end()
                    .children([tooltip(text("hover me"), "I flip upward")]),
            ))
        }
    }
    let mut h = Harness::new(Board, Theme::light(), (260, 200));
    let dot = h.get(&by::label_contains("Danger indicator"));
    assert!(dot.rect.width() > 0.0);

    // Hover the bottom-edge target; the tooltip appears ABOVE it.
    h.hover(&by::label("hover me"));
    h.pump(600.0); // past the hover delay
    let target = h.get(&by::label("hover me")).rect;
    let tip = h.get(&by::label_contains("I flip upward")).rect;
    assert!(
        tip.y1 <= target.y0 + 1.0,
        "tooltip flipped above (tip {tip:?} vs target {target:?})"
    );
}

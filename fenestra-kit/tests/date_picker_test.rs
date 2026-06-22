//! Complete date-picker tests: keyboard grid navigation, min/max
//! constraints, range selection, today marker, and year/month quick-jump.
//! Behavioral tests use `Harness`; visual tests use headless PNG rendering.

use std::path::PathBuf;

use fenestra_core::{App, Element, Key, KeyInput, Theme, col};
use fenestra_kit::{Date, date_picker, date_range_picker};
use fenestra_shell::{Harness, render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

// ═══════════════════════════════════════════════════════════════════════════
// Behavioral test apps
// ═══════════════════════════════════════════════════════════════════════════

// ─── single-date picker with keyboard ────────────────────────────────────────

struct CalApp {
    visible: (i32, u32),
    selected: Option<Date>,
    focused: Option<Date>,
}

#[derive(Clone)]
enum CalMsg {
    Month((i32, u32)),
    Pick(Date),
    Focus(Date),
}

impl App for CalApp {
    type Msg = CalMsg;

    fn update(&mut self, msg: CalMsg) {
        match msg {
            CalMsg::Month(m) => self.visible = m,
            CalMsg::Pick(d) => self.selected = Some(d),
            CalMsg::Focus(d) => {
                self.focused = Some(d);
                // Follow focus to the right month.
                self.visible = (d.0, d.1);
            }
        }
    }

    fn view(&self) -> Element<CalMsg> {
        col().p(12.0).items_start().children([Element::from(
            date_picker(self.visible)
                .selected(self.selected)
                .focused_day(self.focused)
                .on_pick(CalMsg::Pick)
                .on_month(CalMsg::Month)
                .on_focus(CalMsg::Focus)
                .id("cal"),
        )])
    }
}

fn cal_at(visible: (i32, u32)) -> CalApp {
    CalApp {
        visible,
        selected: None,
        focused: None,
    }
}

// ─── constrained picker (min/max) ────────────────────────────────────────────

struct ConstrainedApp {
    visible: (i32, u32),
    selected: Option<Date>,
    focused: Option<Date>,
}

#[derive(Clone)]
enum ConstrainedMsg {
    Month((i32, u32)),
    Pick(Date),
    Focus(Date),
}

impl App for ConstrainedApp {
    type Msg = ConstrainedMsg;

    fn update(&mut self, msg: ConstrainedMsg) {
        match msg {
            ConstrainedMsg::Month(m) => self.visible = m,
            ConstrainedMsg::Pick(d) => self.selected = Some(d),
            ConstrainedMsg::Focus(d) => {
                self.focused = Some(d);
                self.visible = (d.0, d.1);
            }
        }
    }

    fn view(&self) -> Element<ConstrainedMsg> {
        col().p(12.0).items_start().children([Element::from(
            date_picker(self.visible)
                .selected(self.selected)
                .focused_day(self.focused)
                .min((2026, 6, 10))
                .max((2026, 6, 20))
                .on_pick(ConstrainedMsg::Pick)
                .on_month(ConstrainedMsg::Month)
                .on_focus(ConstrainedMsg::Focus)
                .id("cal"),
        )])
    }
}

// ─── range picker ─────────────────────────────────────────────────────────────

struct RangeApp {
    visible: (i32, u32),
    start: Option<Date>,
    end: Option<Date>,
    focused: Option<Date>,
}

#[derive(Clone)]
enum RangeMsg {
    Month((i32, u32)),
    Pick((Option<Date>, Option<Date>)),
    Focus(Date),
}

impl App for RangeApp {
    type Msg = RangeMsg;

    fn update(&mut self, msg: RangeMsg) {
        match msg {
            RangeMsg::Month(m) => self.visible = m,
            RangeMsg::Pick((s, e)) => {
                self.start = s;
                self.end = e;
            }
            RangeMsg::Focus(d) => {
                self.focused = Some(d);
                self.visible = (d.0, d.1);
            }
        }
    }

    fn view(&self) -> Element<RangeMsg> {
        col().p(12.0).items_start().children([Element::from(
            date_range_picker(self.visible)
                .range(self.start, self.end)
                .focused_day(self.focused)
                .on_pick_range(RangeMsg::Pick)
                .on_month(RangeMsg::Month)
                .on_focus(RangeMsg::Focus)
                .id("cal"),
        )])
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Keyboard navigation tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn arrow_right_steps_forward_one_day() {
    let mut h = Harness::new(cal_at((2026, 6)), Theme::light(), (340, 360));
    // 4 header buttons (‹‹ ‹ › ››) + grid container = 5 tab stops.
    for _ in 0..5 {
        h.tab();
    }

    // First arrow key from no focused day starts from the 1st.
    h.key(KeyInput::plain(Key::ArrowRight));
    assert_eq!(
        h.app().focused,
        Some((2026, 6, 2)),
        "→ from day 1 lands on day 2"
    );

    h.key(KeyInput::plain(Key::ArrowRight));
    assert_eq!(h.app().focused, Some((2026, 6, 3)));
}

#[test]
fn arrow_down_steps_one_week() {
    let mut h = Harness::new(cal_at((2026, 6)), Theme::light(), (340, 360));
    // Tab into the grid (5 tabs for 4 header buttons + grid).
    for _ in 0..5 {
        h.tab();
    }

    // Start from day 1, ↓ should go to day 8.
    h.key(KeyInput::plain(Key::ArrowDown));
    assert_eq!(h.app().focused, Some((2026, 6, 8)));
}

#[test]
fn arrow_up_steps_one_week_back() {
    let mut h = Harness::new(
        CalApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 15)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::ArrowUp));
    assert_eq!(h.app().focused, Some((2026, 6, 8)));
}

#[test]
fn arrow_left_steps_back_one_day() {
    let mut h = Harness::new(
        CalApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 10)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::ArrowLeft));
    assert_eq!(h.app().focused, Some((2026, 6, 9)));
}

#[test]
fn arrow_crosses_month_boundary() {
    // Start on June 1, press ← to go to May 31.
    let mut h = Harness::new(
        CalApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 1)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::ArrowLeft));
    assert_eq!(h.app().focused, Some((2026, 5, 31)));
    // App's update follows focus to May.
    assert_eq!(h.app().visible, (2026, 5));
}

#[test]
fn home_end_navigate_week() {
    // June 19, 2026 is a Friday (weekday 4).
    let mut h = Harness::new(
        CalApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 19)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::Home));
    assert_eq!(
        h.app().focused,
        Some((2026, 6, 15)),
        "Home → Monday of week"
    );

    h.key(KeyInput::plain(Key::End));
    assert_eq!(h.app().focused, Some((2026, 6, 21)), "End → Sunday of week");
}

#[test]
fn page_up_down_jump_month() {
    let mut h = Harness::new(
        CalApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 15)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::PageUp));
    assert_eq!(h.app().focused, Some((2026, 5, 15)));

    h.key(KeyInput::plain(Key::PageDown));
    assert_eq!(h.app().focused, Some((2026, 6, 15)));
}

#[test]
fn shift_page_up_down_jump_year() {
    let mut h = Harness::new(
        CalApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 15)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    let shift_pgup = KeyInput {
        key: Key::PageUp,
        shift: true,
        ctrl: false,
        alt: false,
        meta: false,
    };
    h.key(shift_pgup);
    assert_eq!(h.app().focused, Some((2025, 6, 15)));

    let shift_pgdn = KeyInput {
        key: Key::PageDown,
        shift: true,
        ctrl: false,
        alt: false,
        meta: false,
    };
    h.key(shift_pgdn);
    assert_eq!(h.app().focused, Some((2026, 6, 15)));
}

#[test]
fn enter_selects_focused_day() {
    let mut h = Harness::new(
        CalApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 12)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::Enter));
    assert_eq!(h.app().selected, Some((2026, 6, 12)));
}

#[test]
fn space_selects_focused_day() {
    let mut h = Harness::new(
        CalApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 8)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::Space));
    assert_eq!(h.app().selected, Some((2026, 6, 8)));
}

// ═══════════════════════════════════════════════════════════════════════════
// Min / max constraint tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn keyboard_clamps_at_min_boundary() {
    // min = June 10; start focused on June 10, pressing ← should not go below min.
    let mut h = Harness::new(
        ConstrainedApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 10)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::ArrowLeft));
    // Clamped to min: stays at June 10.
    assert_eq!(h.app().focused, Some((2026, 6, 10)), "clamped at min");
}

#[test]
fn keyboard_clamps_at_max_boundary() {
    // max = June 20; focus on June 20, → should not exceed max.
    let mut h = Harness::new(
        ConstrainedApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 20)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::ArrowRight));
    assert_eq!(h.app().focused, Some((2026, 6, 20)), "clamped at max");
}

#[test]
fn enter_on_disabled_day_does_not_pick() {
    // Focus on a day before min; Enter should not emit a pick.
    let mut h = Harness::new(
        ConstrainedApp {
            visible: (2026, 6),
            selected: None,
            focused: Some((2026, 6, 5)), // below min
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    h.key(KeyInput::plain(Key::Enter));
    assert_eq!(h.app().selected, None, "disabled day cannot be picked");
}

#[test]
fn clicking_disabled_day_does_not_pick() {
    // Days outside [min, max] are rendered but clicking them must not emit a pick.
    let mut h = Harness::new(
        ConstrainedApp {
            visible: (2026, 6),
            selected: None,
            focused: None,
        },
        Theme::light(),
        (340, 360),
    );
    use fenestra_core::{Semantics, by};
    // June 1 is before min (June 10) — should be present in the a11y tree.
    assert!(
        h.query(&by::role(Semantics::Button).name("2026-06-01"))
            .is_some(),
        "disabled day is still in the a11y tree"
    );
    // Clicking a day inside range should work.
    h.click(&by::role(Semantics::Button).name("2026-06-15"));
    assert_eq!(h.app().selected, Some((2026, 6, 15)));
    // Disabled days have no click handler; trying to click them should be a no-op.
    // (We verify this by checking the selection does not change.)
    let before = h.app().selected;
    // June 30 is beyond max (June 20) — also in the tree (last days of month exist).
    if h.query(&by::role(Semantics::Button).name("2026-06-25"))
        .is_some()
    {
        h.click(&by::role(Semantics::Button).name("2026-06-25"));
    }
    // June 25 is disabled (max = June 20) — selection must be unchanged.
    assert_eq!(
        h.app().selected,
        before,
        "clicking disabled day must be a no-op"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Range selection tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn range_first_click_sets_start() {
    let mut h = Harness::new(
        RangeApp {
            visible: (2026, 6),
            start: None,
            end: None,
            focused: None,
        },
        Theme::light(),
        (340, 360),
    );
    use fenestra_core::{Semantics, by};
    h.click(&by::role(Semantics::Button).name("2026-06-10"));
    assert_eq!(h.app().start, Some((2026, 6, 10)));
    assert_eq!(h.app().end, None);
}

#[test]
fn range_second_click_completes_range() {
    let mut h = Harness::new(
        RangeApp {
            visible: (2026, 6),
            start: Some((2026, 6, 10)),
            end: None,
            focused: None,
        },
        Theme::light(),
        (340, 360),
    );
    use fenestra_core::{Semantics, by};
    h.click(&by::role(Semantics::Button).name("2026-06-20"));
    assert_eq!(h.app().start, Some((2026, 6, 10)));
    assert_eq!(h.app().end, Some((2026, 6, 20)));
}

#[test]
fn range_second_click_before_start_swaps_order() {
    let mut h = Harness::new(
        RangeApp {
            visible: (2026, 6),
            start: Some((2026, 6, 20)),
            end: None,
            focused: None,
        },
        Theme::light(),
        (340, 360),
    );
    use fenestra_core::{Semantics, by};
    h.click(&by::role(Semantics::Button).name("2026-06-05"));
    assert_eq!(h.app().start, Some((2026, 6, 5)));
    assert_eq!(h.app().end, Some((2026, 6, 20)));
}

#[test]
fn range_third_click_restarts() {
    let mut h = Harness::new(
        RangeApp {
            visible: (2026, 6),
            start: Some((2026, 6, 10)),
            end: Some((2026, 6, 20)),
            focused: None,
        },
        Theme::light(),
        (340, 360),
    );
    use fenestra_core::{Semantics, by};
    h.click(&by::role(Semantics::Button).name("2026-06-15"));
    assert_eq!(h.app().start, Some((2026, 6, 15)));
    assert_eq!(h.app().end, None);
}

#[test]
fn range_keyboard_enter_advances_range() {
    let mut h = Harness::new(
        RangeApp {
            visible: (2026, 6),
            start: None,
            end: None,
            focused: Some((2026, 6, 10)),
        },
        Theme::light(),
        (340, 360),
    );
    for _ in 0..5 {
        h.tab();
    }
    // First Enter: sets start.
    h.key(KeyInput::plain(Key::Enter));
    assert_eq!(h.app().start, Some((2026, 6, 10)));
    assert_eq!(h.app().end, None);

    // Navigate and press Enter again to complete range.
    h.key(KeyInput::plain(Key::ArrowDown)); // → June 17
    h.key(KeyInput::plain(Key::Enter));
    assert_eq!(h.app().start, Some((2026, 6, 10)));
    assert_eq!(h.app().end, Some((2026, 6, 17)));
}

// ═══════════════════════════════════════════════════════════════════════════
// Year / month header navigation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn header_prev_next_year_buttons() {
    use fenestra_core::{Semantics, by};
    let mut h = Harness::new(cal_at((2026, 6)), Theme::light(), (340, 360));
    h.click(&by::role(Semantics::Button).name("next year"));
    assert_eq!(h.app().visible, (2027, 6));
    h.click(&by::role(Semantics::Button).name("previous year"));
    assert_eq!(h.app().visible, (2026, 6));
}

#[test]
fn header_prev_next_month_buttons() {
    use fenestra_core::{Semantics, by};
    let mut h = Harness::new(cal_at((2026, 6)), Theme::light(), (340, 360));
    h.click(&by::role(Semantics::Button).name("next month"));
    assert_eq!(h.app().visible, (2026, 7));
    h.click(&by::role(Semantics::Button).name("previous month"));
    assert_eq!(h.app().visible, (2026, 6));
}

// ═══════════════════════════════════════════════════════════════════════════
// Golden PNG tests
// ═══════════════════════════════════════════════════════════════════════════

fn single_picker_element(_theme: &Theme) -> fenestra_core::Element<()> {
    // Single mode with: today marker, a selected date, disabled days.
    col().p(12.0).items_start().children([Element::from(
        date_picker((2026, 6))
            .today((2026, 6, 22))
            .selected(Some((2026, 6, 12)))
            .min((2026, 6, 5))
            .max((2026, 6, 25))
            .focused_day(Some((2026, 6, 15))),
    )])
}

fn range_picker_element() -> fenestra_core::Element<()> {
    // Range mode with start + end selected (interior highlighted).
    col().p(12.0).items_start().children([Element::from(
        date_range_picker((2026, 6))
            .today((2026, 6, 22))
            .range(Some((2026, 6, 10)), Some((2026, 6, 20))),
    )])
}

#[test]
fn date_picker_single_light() {
    let theme = Theme::light();
    let image = render_element(single_picker_element(&theme), &theme, (340, 360));
    assert_png_snapshot(snapshot_dir(), "date_picker_single_light", &image);
}

#[test]
fn date_picker_single_dark() {
    let theme = Theme::dark();
    let image = render_element(single_picker_element(&theme), &theme, (340, 360));
    assert_png_snapshot(snapshot_dir(), "date_picker_single_dark", &image);
}

#[test]
fn date_picker_range_light() {
    let theme = Theme::light();
    let image = render_element(range_picker_element(), &theme, (340, 360));
    assert_png_snapshot(snapshot_dir(), "date_picker_range_light", &image);
}

#[test]
fn date_picker_range_dark() {
    let theme = Theme::dark();
    let image = render_element(range_picker_element(), &theme, (340, 360));
    assert_png_snapshot(snapshot_dir(), "date_picker_range_dark", &image);
}

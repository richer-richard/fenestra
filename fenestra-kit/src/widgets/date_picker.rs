//! Date picker: a month calendar, Elm-pure — the app owns the visible
//! month, selection, keyboard focus, and today; the widget only emits.
//! Civil-date math is inline (no chrono): proleptic Gregorian, ISO
//! weekday columns.
//!
//! Supports:
//! - Single-date selection ([`date_picker`])
//! - Start..=end range selection ([`date_range_picker`])
//! - min/max constraints (disabled days, skip on keyboard nav)
//! - Today marker (pass via `.today()` — widget is clock-free)
//! - WAI-ARIA datepicker keyboard grid (← → ↑ ↓ Home End PgUp PgDn
//!   Shift+PgUp/PgDn, Enter/Space to select)
//! - Year quick-jump (‹‹ / ›› header buttons)

use fenestra_core::{
    Cursor, Element, Key, SP1, SP2, Semantics, TextSize, Theme, Transition, Weight, col, row, text,
};

/// A calendar date as plain numbers: year, month 1..=12, day 1..=31.
pub type Date = (i32, u32, u32);

/// Shared month-navigation mapping.
type MonthFn<Msg> = std::rc::Rc<dyn Fn((i32, u32)) -> Msg>;
/// Shared range-pick mapping (emits the new ordered (start, end) pair).
type RangeFn<Msg> = std::rc::Rc<dyn Fn((Option<Date>, Option<Date>)) -> Msg>;

// ─── civil-date math ─────────────────────────────────────────────────────────

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 30, // hostile month index: render something sane
    }
}

/// Day of week, Monday = 0 (Sakamoto's method).
fn weekday(y: i32, m: u32, d: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let m = m.clamp(1, 12);
    let y = if m < 3 { y - 1 } else { y };
    #[expect(clippy::cast_possible_wrap, reason = "day/month are tiny")]
    let dow = (y + y / 4 - y / 100 + y / 400 + T[(m - 1) as usize] + d as i32).rem_euclid(7);
    // Sakamoto yields Sunday = 0; shift to Monday = 0.
    (dow as u32 + 6) % 7
}

/// Days since 1970-01-01 using Howard Hinnant's civil-calendar algorithm.
fn ymd_to_days(y: i32, m: u32, d: u32) -> i32 {
    let m = m.clamp(1, 12);
    let y = y - i32::from(m <= 2);
    let era: i32 = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32; // [0, 399]
    let m_adj = if m > 2 { m - 3 } else { m + 9 }; // [0, 11]
    let doy = (153 * m_adj + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i32 - 719_468
}

/// Day number back to (year, month, day).
fn days_to_ymd(z: i32) -> (i32, u32, u32) {
    let z = z as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let m_le2 = i64::from(m <= 2);
    ((y + m_le2) as i32, m, d)
}

fn add_days(date: Date, delta: i32) -> Date {
    days_to_ymd(ymd_to_days(date.0, date.1, date.2) + delta)
}

fn clamp_to_bounds(date: Date, min: Option<Date>, max: Option<Date>) -> Date {
    let d = match min {
        Some(mn) if date < mn => mn,
        _ => date,
    };
    match max {
        Some(mx) if d > mx => mx,
        _ => d,
    }
}

fn is_disabled(date: Date, min: Option<Date>, max: Option<Date>) -> bool {
    min.is_some_and(|mn| date < mn) || max.is_some_and(|mx| date > mx)
}

/// Jump one month forward or back, clamping the day to the new month length.
fn shift_month(y: i32, m: u32, d: u32, delta: i32) -> Date {
    let total = y * 12 + m as i32 - 1 + delta;
    let ny = total.div_euclid(12);
    let nm = total.rem_euclid(12) as u32 + 1;
    let nd = d.min(days_in_month(ny, nm));
    (ny, nm, nd)
}

/// Jump one year forward or back.
fn shift_year(y: i32, m: u32, d: u32, delta: i32) -> Date {
    let ny = y + delta;
    let nd = d.min(days_in_month(ny, m));
    (ny, m, nd)
}

// ─── public types ─────────────────────────────────────────────────────────────

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Internal selection mode and current state.
#[derive(Clone, Debug, PartialEq)]
enum PickMode {
    Single(Option<Date>),
    Range(Option<Date>, Option<Date>),
}

/// A date picker under construction; converts into an [`Element`].
pub struct DatePicker<Msg> {
    visible: (i32, u32),
    mode: PickMode,
    focused_day: Option<Date>,
    today: Option<Date>,
    min: Option<Date>,
    max: Option<Date>,
    on_pick: Option<std::rc::Rc<dyn Fn(Date) -> Msg>>,
    on_pick_range: Option<RangeFn<Msg>>,
    on_month: Option<MonthFn<Msg>>,
    on_focus: Option<std::rc::Rc<dyn Fn(Date) -> Msg>>,
    key: Option<String>,
}

fn make_picker<Msg>(visible: (i32, u32), mode: PickMode) -> DatePicker<Msg> {
    DatePicker {
        visible: (visible.0, visible.1.clamp(1, 12)),
        mode,
        focused_day: None,
        today: None,
        min: None,
        max: None,
        on_pick: None,
        on_pick_range: None,
        on_month: None,
        on_focus: None,
        key: None,
    }
}

/// A month calendar showing `visible` (year, month 1..=12). Clicking a
/// day emits `on_pick`; the header buttons emit `on_month`. Pass
/// `.today(date)` to mark today, `.min`/`.max` to constrain, and
/// `.focused_day` + `.on_focus` to drive keyboard grid navigation.
/// The grid is a single tab stop.
pub fn date_picker<Msg>(visible: (i32, u32)) -> DatePicker<Msg> {
    make_picker(visible, PickMode::Single(None))
}

/// A range-selection calendar. Works like [`date_picker`] but clicking
/// emits an updated `(start, end)` pair via `on_pick_range`. Seed the
/// current selection with `.range(start, end)`.
pub fn date_range_picker<Msg>(visible: (i32, u32)) -> DatePicker<Msg> {
    make_picker(visible, PickMode::Range(None, None))
}

impl<Msg> DatePicker<Msg> {
    /// Highlights the selected date in single-date mode.
    #[must_use]
    pub fn selected(mut self, date: Option<Date>) -> Self {
        if matches!(self.mode, PickMode::Single(_)) {
            self.mode = PickMode::Single(date);
        }
        self
    }

    /// Sets the current range selection (start, end) in range mode.
    #[must_use]
    pub fn range(mut self, start: Option<Date>, end: Option<Date>) -> Self {
        if matches!(self.mode, PickMode::Range(_, _)) {
            self.mode = PickMode::Range(start, end);
        }
        self
    }

    /// The date visually marked as "today" (ring/outline). Pass from your
    /// app state — the widget does not call a system clock.
    #[must_use]
    pub fn today(mut self, date: Date) -> Self {
        self.today = Some(date);
        self
    }

    /// Days before `date` are rendered disabled and are not selectable.
    #[must_use]
    pub fn min(mut self, date: Date) -> Self {
        self.min = Some(date);
        self
    }

    /// Days after `date` are rendered disabled and are not selectable.
    #[must_use]
    pub fn max(mut self, date: Date) -> Self {
        self.max = Some(date);
        self
    }

    /// The currently keyboard-focused day (the grid cursor). Keyboard
    /// navigation emits new positions via `.on_focus`; store the result
    /// in app state and feed it back here each frame.
    #[must_use]
    pub fn focused_day(mut self, date: Option<Date>) -> Self {
        self.focused_day = date;
        self
    }

    /// Maps a clicked day to a message in single-date mode.
    pub fn on_pick(mut self, f: impl Fn(Date) -> Msg + 'static) -> Self {
        self.on_pick = Some(std::rc::Rc::new(f));
        self
    }

    /// Maps a completed range update to a message in range mode.
    /// The emitted pair is always ordered (start ≤ end).
    pub fn on_pick_range(
        mut self,
        f: impl Fn((Option<Date>, Option<Date>)) -> Msg + 'static,
    ) -> Self {
        self.on_pick_range = Some(std::rc::Rc::new(f));
        self
    }

    /// Maps the prev/next month and year header buttons to a message.
    pub fn on_month(mut self, f: impl Fn((i32, u32)) -> Msg + 'static) -> Self {
        self.on_month = Some(std::rc::Rc::new(f));
        self
    }

    /// Called when keyboard navigation moves the grid cursor to a new date.
    /// The app should update its `focused_day` state — and if the date's
    /// month differs from `visible`, update `visible` too so the grid scrolls.
    pub fn on_focus(mut self, f: impl Fn(Date) -> Msg + 'static) -> Self {
        self.on_focus = Some(std::rc::Rc::new(f));
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

// ─── selection helpers ────────────────────────────────────────────────────────

/// Given the current (start, end) range and a newly chosen date, return
/// the updated (start, end) pair following the "two-click" protocol:
///   • Both None → (Some(date), None)  [start picking]
///   • Start Some, end None → (lo, Some(hi))  [finish the range, ordered]
///   • Both Some → (Some(date), None)  [restart]
fn update_range(
    start: Option<Date>,
    end: Option<Date>,
    date: Date,
) -> (Option<Date>, Option<Date>) {
    match (start, end) {
        (None, _) => (Some(date), None),
        (Some(_), Some(_)) => (Some(date), None),
        (Some(s), None) => {
            if date <= s {
                (Some(date), Some(s))
            } else {
                (Some(s), Some(date))
            }
        }
    }
}

/// Whether `date` falls strictly inside the [lo, hi] range (exclusive endpoints).
fn in_range_interior(date: Date, lo: Date, hi: Date) -> bool {
    date > lo && date < hi
}

// ─── keyboard navigation ──────────────────────────────────────────────────────

/// Derive a good starting point for keyboard navigation when `focused_day`
/// is `None` but the user presses an arrow key.
fn default_focus(selected: Option<Date>, today: Option<Date>, year: i32, month: u32) -> Date {
    // Prefer the selected date if visible, then today, then day 1.
    if let Some(s) = selected
        && s.0 == year
        && s.1 == month
    {
        return s;
    }
    if let Some(t) = today
        && t.0 == year
        && t.1 == month
    {
        return t;
    }
    (year, month, 1)
}

// ─── rendering ────────────────────────────────────────────────────────────────

impl<Msg: Clone + 'static> From<DatePicker<Msg>> for Element<Msg> {
    fn from(p: DatePicker<Msg>) -> Self {
        let (year, month) = p.visible;
        let month = month.clamp(1, 12);
        let title = format!("{} {year}", MONTHS[(month - 1) as usize]);

        // Snapshot values used both in rendering and in the on_key closure.
        let min = p.min;
        let max = p.max;
        let today = p.today;
        let focused_day = p.focused_day;

        let single_selected = if let PickMode::Single(s) = p.mode {
            s
        } else {
            None
        };
        let (range_start, range_end) = if let PickMode::Range(s, e) = p.mode {
            (s, e)
        } else {
            (None, None)
        };
        let is_range = p.on_pick_range.is_some();

        // ── header ────────────────────────────────────────────────────────────

        let mut header = row().items_center().gap(SP1).children([text(title)
            .size(TextSize::Sm)
            .weight(Weight::Semibold)
            .grow()]);

        if let Some(f) = &p.on_month {
            // prev-year, prev-month, next-month, next-year
            let prev_year = (year - 1, month);
            let prev_month = if month == 1 {
                (year - 1, 12)
            } else {
                (year, month - 1)
            };
            let next_month = if month == 12 {
                (year + 1, 1)
            } else {
                (year, month + 1)
            };
            let next_year = (year + 1, month);

            for (label, target, name) in [
                ("‹‹", prev_year, "previous year"),
                ("‹", prev_month, "previous month"),
                ("›", next_month, "next month"),
                ("››", next_year, "next year"),
            ] {
                header = header.child(
                    row()
                        .items_center()
                        .justify_center()
                        .w(26.0)
                        .h(26.0)
                        .themed(|t: &Theme, s| s.rounded((t.radius.md - 4.0).max(0.0)))
                        .cursor(Cursor::Pointer)
                        .focusable(true)
                        .on_click(f(target))
                        .semantics(Semantics::Button)
                        .label(name)
                        .state_layer(|t| t.text)
                        .children([text(label).size(TextSize::Sm)]),
                );
            }
        }

        // ── day-of-week row ───────────────────────────────────────────────────

        let dow_row = row()
            .gap(2.0)
            .children(["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"].map(|d| {
                row()
                    .w(30.0)
                    .h(22.0)
                    .items_center()
                    .justify_center()
                    .shrink0()
                    .children([text(d)
                        .size(TextSize::Xs)
                        .themed(|t: &Theme, s| s.color(t.text_muted))])
            }));

        // ── week rows ─────────────────────────────────────────────────────────

        let first_col = weekday(year, month, 1);
        let total = days_in_month(year, month);
        let mut weeks: Vec<Element<Msg>> = Vec::new();
        let mut day: u32 = 1;

        while day <= total {
            let mut cells: Vec<Element<Msg>> = Vec::new();
            for col_idx in 0..7u32 {
                let blank = (weeks.is_empty() && col_idx < first_col) || day > total;
                if blank {
                    cells.push(row().w(30.0).h(30.0).shrink0());
                    continue;
                }

                let date = (year, month, day);
                let disabled = is_disabled(date, min, max);
                let is_today = today == Some(date);
                let is_focused = focused_day == Some(date);
                let is_selected = single_selected == Some(date);
                let is_start = range_start == Some(date);
                let is_end = range_end == Some(date);
                let is_endpoint = is_start || is_end;
                let in_interior = match (range_start, range_end) {
                    (Some(lo), Some(hi)) => in_range_interior(date, lo, hi),
                    _ => false,
                };

                // Build the day number label (plain text child). Tabular so the
                // day digits stay column-aligned across the calendar grid.
                let day_text = text(day.to_string()).size(TextSize::Sm).tabular();

                let mut cell = row()
                    .w(30.0)
                    .h(30.0)
                    .items_center()
                    .justify_center()
                    .themed(|t: &Theme, s| s.rounded((t.radius.md - 2.0).max(0.0)))
                    .shrink0()
                    .semantics(Semantics::Button)
                    .label(format!("{year}-{month:02}-{day:02}"))
                    .transition(Transition::colors());

                if disabled {
                    // Disabled: dim, no interaction.
                    cell = cell
                        .disabled(true)
                        .themed(|t: &Theme, s| s.color(t.text_disabled));
                } else {
                    cell = cell.cursor(Cursor::Pointer).state_layer(|t| t.text);

                    // Background fills (priority: endpoint > interior > focused).
                    if is_selected || is_endpoint {
                        cell = cell
                            .themed(|t: &Theme, s| s.bg(t.accent))
                            .themed(|t: &Theme, s| s.color(t.on_accent));
                    } else if in_interior {
                        cell = cell.themed(|t: &Theme, s| s.bg(t.accent_bg));
                    }

                    // Keyboard focus ring (skip when cell is the selected/endpoint fill).
                    if is_focused && !is_selected && !is_endpoint {
                        cell = cell.themed(|t: &Theme, s| s.border(2.0, t.accent_border));
                    }

                    // Click handler — on_click auto-sets focusable=true, so opt back out.
                    if is_range {
                        if let Some(f) = &p.on_pick_range {
                            let new_pair = update_range(range_start, range_end, date);
                            cell = cell.on_click(f(new_pair)).focusable(false);
                        } else {
                            cell = cell.focusable(false);
                        }
                    } else if let Some(f) = &p.on_pick {
                        cell = cell.on_click(f(date)).focusable(false);
                    } else {
                        cell = cell.focusable(false);
                    }
                }

                // Today marker: ring over the cell (regardless of other state).
                if is_today && !is_selected && !is_endpoint {
                    cell = cell.themed(|t: &Theme, s| s.border(1.5, t.accent));
                }

                cells.push(cell.children([day_text]));
                day += 1;
            }
            weeks.push(row().gap(2.0).children(cells));
        }

        // ── keyboard-navigable grid container ─────────────────────────────────

        let has_keyboard = p.on_focus.is_some() || p.on_pick.is_some() || p.on_pick_range.is_some();

        let grid = col().gap(2.0).children(weeks);

        let grid = if has_keyboard {
            // Capture everything the on_key closure needs.
            let on_focus = p.on_focus.clone();
            let on_pick_c = p.on_pick.clone();
            let on_pick_range_c = p.on_pick_range.clone();

            grid.focusable(true).on_key(move |k| {
                use Key::{
                    ArrowDown, ArrowLeft, ArrowRight, ArrowUp, End, Enter, Home, PageDown, PageUp,
                    Space,
                };

                // Derive the effective focus starting point.
                let cur = focused_day
                    .unwrap_or_else(|| default_focus(single_selected, today, year, month));

                let navigate = |new_day: Date| -> Option<Msg> {
                    let clamped = clamp_to_bounds(new_day, min, max);
                    on_focus.as_ref().map(|f| f(clamped))
                };

                let select_cur = || -> Option<Msg> {
                    if is_disabled(cur, min, max) {
                        return None;
                    }
                    if is_range {
                        on_pick_range_c
                            .as_ref()
                            .map(|f| f(update_range(range_start, range_end, cur)))
                    } else {
                        on_pick_c.as_ref().map(|f| f(cur))
                    }
                };

                match k.key {
                    ArrowLeft => navigate(add_days(cur, -1)),
                    ArrowRight => navigate(add_days(cur, 1)),
                    ArrowUp => navigate(add_days(cur, -7)),
                    ArrowDown => navigate(add_days(cur, 7)),
                    Home => {
                        // Monday of the focused day's week.
                        let wd = weekday(cur.0, cur.1, cur.2);
                        navigate(add_days(cur, -(wd as i32)))
                    }
                    End => {
                        // Sunday of the focused day's week.
                        let wd = weekday(cur.0, cur.1, cur.2);
                        navigate(add_days(cur, 6 - wd as i32))
                    }
                    PageUp if k.shift => navigate(shift_year(cur.0, cur.1, cur.2, -1)),
                    PageDown if k.shift => navigate(shift_year(cur.0, cur.1, cur.2, 1)),
                    PageUp => navigate(shift_month(cur.0, cur.1, cur.2, -1)),
                    PageDown => navigate(shift_month(cur.0, cur.1, cur.2, 1)),
                    Enter | Space => select_cur(),
                    _ => None,
                }
            })
        } else {
            grid
        };

        // ── root container ────────────────────────────────────────────────────

        let mut root = col()
            .p(SP2)
            .gap(SP1)
            .themed(|t: &Theme, s| s.rounded(t.radius.md))
            .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
            .child(header)
            .child(dow_row)
            .child(grid);

        if let Some(key) = &p.key {
            root = root.id(key);
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_math_is_right() {
        assert_eq!(weekday(2026, 6, 12), 4); // Friday
        assert_eq!(weekday(2024, 2, 29), 3); // leap Thursday
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 2), 28);
        assert_eq!(days_in_month(2026, 6), 30);
    }

    #[test]
    fn day_arithmetic_roundtrips() {
        let cases = [
            (2026, 1, 1),
            (2026, 6, 15),
            (2026, 12, 31),
            (2024, 2, 29), // leap day
            (2000, 1, 1),
            (1970, 1, 1),
        ];
        for (y, m, d) in cases {
            let serial = ymd_to_days(y, m, d);
            assert_eq!(
                days_to_ymd(serial),
                (y, m, d),
                "roundtrip failed for {y}-{m:02}-{d:02}"
            );
        }
    }

    #[test]
    fn add_days_crosses_months_and_years() {
        assert_eq!(add_days((2026, 1, 31), 1), (2026, 2, 1));
        assert_eq!(add_days((2026, 12, 31), 1), (2027, 1, 1));
        assert_eq!(add_days((2024, 2, 28), 1), (2024, 2, 29)); // leap
        assert_eq!(add_days((2026, 3, 1), -1), (2026, 2, 28)); // non-leap
        assert_eq!(add_days((2026, 6, 15), -7), (2026, 6, 8));
        assert_eq!(add_days((2026, 6, 15), 7), (2026, 6, 22));
    }

    #[test]
    fn shift_month_clamps_day() {
        // Jan 31 → Feb: clamped to 28 (2025 non-leap)
        assert_eq!(shift_month(2025, 1, 31, 1), (2025, 2, 28));
        // Jan 31 → Feb: clamped to 29 (2024 leap)
        assert_eq!(shift_month(2024, 1, 31, 1), (2024, 2, 29));
        // Dec→Jan wraps the year
        assert_eq!(shift_month(2025, 12, 15, 1), (2026, 1, 15));
        // Jan→Dec wraps the year back
        assert_eq!(shift_month(2026, 1, 15, -1), (2025, 12, 15));
    }

    #[test]
    fn update_range_protocol() {
        // First click: start only
        assert_eq!(
            update_range(None, None, (2026, 6, 10)),
            (Some((2026, 6, 10)), None)
        );
        // Second click after start (end > start): ordered pair
        assert_eq!(
            update_range(Some((2026, 6, 10)), None, (2026, 6, 20)),
            (Some((2026, 6, 10)), Some((2026, 6, 20)))
        );
        // Second click before start: swapped
        assert_eq!(
            update_range(Some((2026, 6, 10)), None, (2026, 6, 5)),
            (Some((2026, 6, 5)), Some((2026, 6, 10)))
        );
        // Both set: restart
        assert_eq!(
            update_range(Some((2026, 6, 10)), Some((2026, 6, 20)), (2026, 6, 15)),
            (Some((2026, 6, 15)), None)
        );
    }

    #[test]
    fn is_disabled_respects_bounds() {
        let min = Some((2026, 6, 5));
        let max = Some((2026, 6, 25));
        assert!(is_disabled((2026, 6, 4), min, max));
        assert!(!is_disabled((2026, 6, 5), min, max));
        assert!(!is_disabled((2026, 6, 15), min, max));
        assert!(!is_disabled((2026, 6, 25), min, max));
        assert!(is_disabled((2026, 6, 26), min, max));
    }

    #[test]
    fn weekday_home_end_navigation() {
        // 2026-06-15 is a Monday (weekday 0) → Home = same day, End = Sunday 21
        assert_eq!(weekday(2026, 6, 15), 0);
        let cur = (2026, 6, 15);
        let wd = weekday(cur.0, cur.1, cur.2);
        assert_eq!(add_days(cur, -(wd as i32)), (2026, 6, 15)); // Monday→Monday
        assert_eq!(add_days(cur, 6 - wd as i32), (2026, 6, 21)); // Mon→Sun

        // 2026-06-19 is a Friday (weekday 4) → Home = Mon 15, End = Sun 21
        assert_eq!(weekday(2026, 6, 19), 4);
        let cur2 = (2026, 6, 19);
        let wd2 = weekday(cur2.0, cur2.1, cur2.2);
        assert_eq!(add_days(cur2, -(wd2 as i32)), (2026, 6, 15));
        assert_eq!(add_days(cur2, 6 - wd2 as i32), (2026, 6, 21));
    }
}

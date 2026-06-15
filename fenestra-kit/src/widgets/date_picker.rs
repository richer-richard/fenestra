//! Date picker: a month calendar, Elm-pure — the app owns the visible
//! month and the selected date; the widget only emits. Civil-date math
//! is inline (no chrono): proleptic Gregorian, ISO weekday columns.

use fenestra_core::{
    Cursor, Element, SP1, SP2, Semantics, TextSize, Theme, Transition, Weight, col, row, text,
};

/// A calendar date as plain numbers: year, month 1..=12, day 1..=31.
pub type Date = (i32, u32, u32);

/// Shared month-navigation mapping.
type MonthFn<Msg> = std::rc::Rc<dyn Fn((i32, u32)) -> Msg>;

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

/// A date picker under construction; converts into an [`Element`].
pub struct DatePicker<Msg> {
    visible: (i32, u32),
    selected: Option<Date>,
    on_pick: Option<std::rc::Rc<dyn Fn(Date) -> Msg>>,
    on_month: Option<MonthFn<Msg>>,
    key: Option<String>,
}

/// A month calendar showing `visible` (year, month 1..=12). Clicking a
/// day emits `on_pick`; the ‹ › header buttons emit `on_month` with
/// the adjacent month — store both in your app state.
pub fn date_picker<Msg>(visible: (i32, u32)) -> DatePicker<Msg> {
    DatePicker {
        visible: (visible.0, visible.1.clamp(1, 12)),
        selected: None,
        on_pick: None,
        on_month: None,
        key: None,
    }
}

impl<Msg> DatePicker<Msg> {
    /// Highlights the selected date (if it falls in the visible month).
    #[must_use]
    pub fn selected(mut self, date: Option<Date>) -> Self {
        self.selected = date;
        self
    }

    /// Maps a clicked day to a message.
    pub fn on_pick(mut self, f: impl Fn(Date) -> Msg + 'static) -> Self {
        self.on_pick = Some(std::rc::Rc::new(f));
        self
    }

    /// Maps the previous/next month buttons to a message.
    pub fn on_month(mut self, f: impl Fn((i32, u32)) -> Msg + 'static) -> Self {
        self.on_month = Some(std::rc::Rc::new(f));
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: Clone + 'static> From<DatePicker<Msg>> for Element<Msg> {
    fn from(p: DatePicker<Msg>) -> Self {
        let (year, month) = p.visible;
        let title = format!("{} {year}", MONTHS[(month - 1) as usize]);

        let mut header = row().items_center().gap(SP1).children([text(title)
            .size(TextSize::Sm)
            .weight(Weight::Semibold)
            .grow()]);
        if let Some(f) = &p.on_month {
            let prev = if month == 1 {
                (year - 1, 12)
            } else {
                (year, month - 1)
            };
            let next = if month == 12 {
                (year + 1, 1)
            } else {
                (year, month + 1)
            };
            for (label, target, name) in [("‹", prev, "previous month"), ("›", next, "next month")]
            {
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
                        .children([text(label)]),
                );
            }
        }

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

        let first_col = weekday(year, month, 1);
        let total = days_in_month(year, month);
        let mut weeks: Vec<Element<Msg>> = Vec::new();
        let mut day: u32 = 1;
        while day <= total {
            let mut cells: Vec<Element<Msg>> = Vec::new();
            for col_idx in 0..7 {
                let blank = (weeks.is_empty() && col_idx < first_col) || day > total;
                if blank {
                    cells.push(row().w(30.0).h(30.0).shrink0());
                    continue;
                }
                let date = (year, month, day);
                let is_selected = p.selected == Some(date);
                let mut cell = row()
                    .w(30.0)
                    .h(30.0)
                    .items_center()
                    .justify_center()
                    .themed(|t: &Theme, s| s.rounded((t.radius.md - 2.0).max(0.0)))
                    .shrink0()
                    .cursor(Cursor::Pointer)
                    .focusable(true)
                    .semantics(Semantics::Button)
                    .label(format!("{year}-{month:02}-{day:02}"))
                    .transition(Transition::colors())
                    .state_layer(|t| t.text)
                    .children([text(day.to_string()).size(TextSize::Sm)]);
                if is_selected {
                    cell = cell.themed(|t: &Theme, s| s.bg(t.accent));
                }
                if let Some(f) = &p.on_pick {
                    cell = cell.on_click(f(date));
                }
                cells.push(cell);
                day += 1;
            }
            weeks.push(row().gap(2.0).children(cells));
        }

        let mut root = col()
            .p(SP2)
            .gap(SP1)
            .themed(|t: &Theme, s| s.rounded(t.radius.md))
            .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
            .child(header)
            .child(dow_row)
            .children(weeks);
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
}

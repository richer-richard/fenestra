//! The client-side function library dynamic values can call. Implemented
//! deterministically with no locale data — formatting is documented as
//! approximate (a real l10n pass would swap this layer out). Unknown
//! functions resolve to a placeholder and record a note.

use serde_json::Value;

/// Formats `n` with thousands separators (`1234567.5` → `1,234,567.5`).
pub(crate) fn format_number(n: f64) -> String {
    let negative = n < 0.0;
    let s = format!("{}", n.abs());
    let (int, frac) = s
        .split_once('.')
        .map_or((s.as_str(), None), |(i, f)| (i, Some(f)));
    let mut out = String::new();
    let digits: Vec<char> = int.chars().collect();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*c);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    if negative { format!("-{out}") } else { out }
}

/// Approximate currency formatting: symbol for the majors, code prefix
/// otherwise, always two decimals.
pub(crate) fn format_currency(n: f64, currency: &str) -> String {
    let sym = match currency {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        _ => "",
    };
    let cents = format!("{:.2}", n.abs());
    let (int, frac) = cents.split_once('.').unwrap_or((cents.as_str(), "00"));
    let grouped = format_number(int.parse::<f64>().unwrap_or(0.0));
    let sign = if n < 0.0 { "-" } else { "" };
    if sym.is_empty() {
        format!("{sign}{currency} {grouped}.{frac}")
    } else {
        format!("{sign}{sym}{grouped}.{frac}")
    }
}

/// A parsed ISO-8601 timestamp (date, optional time). Timezone suffixes
/// are accepted and ignored — rendering is wall-clock of the given value.
struct Stamp {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

fn parse_iso(value: &str) -> Option<Stamp> {
    let (date, time) = match value.split_once(['T', ' ']) {
        Some((d, t)) => (d, Some(t)),
        None => (value, None),
    };
    let mut parts = date.splitn(3, '-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let (mut hour, mut minute, mut second) = (0, 0, 0);
    if let Some(t) = time {
        let t = t
            .trim_end_matches('Z')
            .split(['+'])
            .next()
            .unwrap_or(t)
            .split('-')
            .next()
            .unwrap_or(t);
        let mut tp = t.splitn(3, ':');
        hour = tp.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        minute = tp.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        second = tp
            .next()
            .and_then(|v| v.split('.').next())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
    }
    Some(Stamp {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
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
const WEEKDAYS: [&str; 7] = [
    "Saturday",
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
];

/// Zeller's congruence: day of week for a Gregorian date (0 = Saturday in
/// [`WEEKDAYS`]' order).
fn weekday(s: &Stamp) -> usize {
    let (mut y, mut m) = (s.year, i64::from(s.month));
    if m < 3 {
        m += 12;
        y -= 1;
    }
    let (k, j) = (y % 100, y / 100);
    let d = i64::from(s.day);
    let h = (d + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 + 5 * j).rem_euclid(7);
    usize::try_from(h).unwrap_or(0)
}

/// Formats an ISO timestamp with a CLDR-ish pattern subset: `yyyy`, `MMMM`,
/// `MMM`, `MM`, `M`, `EEEE`, `E`, `dd`, `d`, `HH`, `hh`, `h`, `mm`, `ss`,
/// `a`. Unknown letters pass through.
pub(crate) fn format_date(value: &str, pattern: &str) -> Option<String> {
    let s = parse_iso(value)?;
    let wd = weekday(&s);
    let month_name = MONTHS[(s.month - 1) as usize];
    let h12 = match s.hour % 12 {
        0 => 12,
        h => h,
    };
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let run = chars[i..].iter().take_while(|&&x| x == c).count();
        match (c, run) {
            ('y', _) => out.push_str(&s.year.to_string()),
            ('M', r) if r >= 4 => out.push_str(month_name),
            ('M', 3) => out.push_str(&month_name[..3]),
            ('M', 2) => out.push_str(&format!("{:02}", s.month)),
            ('M', _) => out.push_str(&s.month.to_string()),
            ('E', r) if r >= 4 => out.push_str(WEEKDAYS[wd]),
            ('E', _) => out.push_str(&WEEKDAYS[wd][..3]),
            ('d', r) if r >= 2 => out.push_str(&format!("{:02}", s.day)),
            ('d', _) => out.push_str(&s.day.to_string()),
            ('H', _) => out.push_str(&format!("{:02}", s.hour)),
            ('h', r) if r >= 2 => out.push_str(&format!("{h12:02}")),
            ('h', _) => out.push_str(&h12.to_string()),
            ('m', _) => out.push_str(&format!("{:02}", s.minute)),
            ('s', _) => out.push_str(&format!("{:02}", s.second)),
            ('a', _) => out.push_str(if s.hour < 12 { "AM" } else { "PM" }),
            ('\'', _) => {
                // Quoted literal: copy until the closing quote.
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '\'' {
                    out.push(chars[j]);
                    j += 1;
                }
                i = j + 1;
                continue;
            }
            _ => {
                for _ in 0..run {
                    out.push(c);
                }
            }
        }
        i += run;
    }
    Some(out)
}

/// `pluralize`: picks `one`/`other` by the numeric value.
pub(crate) fn pluralize(value: f64, one: &str, other: &str) -> String {
    if (value - 1.0).abs() < f64::EPSILON {
        one.to_owned()
    } else {
        other.to_owned()
    }
}

/// Renders a JSON value as display text (strings unquoted).
pub(crate) fn display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_group() {
        assert_eq!(format_number(1_234_567.5), "1,234,567.5");
        assert_eq!(format_number(-1000.0), "-1,000");
        assert_eq!(format_number(42.0), "42");
    }

    #[test]
    fn currency_formats() {
        assert_eq!(format_currency(1234.5, "USD"), "$1,234.50");
        assert_eq!(format_currency(-3.2, "CHF"), "-CHF 3.20");
    }

    #[test]
    fn dates_format() {
        assert_eq!(
            format_date("2026-02-02T15:17:00Z", "E, MMM d").as_deref(),
            Some("Mon, Feb 2")
        );
        assert_eq!(
            format_date("2026-02-02T15:17:00Z", "h:mm a").as_deref(),
            Some("3:17 PM")
        );
        assert_eq!(
            format_date("2026-12-25", "EEEE, MMMM d, yyyy").as_deref(),
            Some("Friday, December 25, 2026")
        );
    }

    #[test]
    fn plural_picks() {
        assert_eq!(pluralize(1.0, "review", "reviews"), "review");
        assert_eq!(pluralize(3.0, "review", "reviews"), "reviews");
    }
}

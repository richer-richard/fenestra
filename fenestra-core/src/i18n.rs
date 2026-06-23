//! Lightweight internationalization: a [`Locale`] (language tag + writing
//! direction + number separators), a message [`Catalog`] (key → string with
//! `{name}` interpolation), and locale-aware number formatting. No ICU or heavy
//! data — enough to localize a fenestra app's strings and numbers and to pick a
//! [`WritingDir`](crate::WritingDir) for the theme.
//!
//! ```
//! use fenestra_core::{Catalog, Locale};
//!
//! let ar = Locale::new("ar");
//! assert!(ar.is_rtl());
//! assert_eq!(Locale::new("en-US").format_int(1_234_567), "1,234,567");
//!
//! let mut cat = Catalog::new();
//! cat.insert("greeting", "Hello, {name}!");
//! assert_eq!(cat.t("greeting", &[("name", "Ada")]), "Hello, Ada!");
//! assert_eq!(cat.t("missing", &[]), "missing"); // falls back to the key
//! ```

use std::collections::HashMap;

use crate::theme::WritingDir;

/// A locale: a BCP-47-ish language tag plus the writing direction and the
/// decimal / grouping separators used to format numbers. Construct with
/// [`Locale::new`] (which infers everything from the tag) or tune the separators
/// with the builders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    tag: String,
    rtl: bool,
    decimal: char,
    grouping: char,
}

impl Locale {
    /// Builds a locale from a language tag (`"en"`, `"en-US"`, `"ar"`,
    /// `"de-DE"`). The primary subtag decides the writing direction and a
    /// sensible default for the number separators.
    #[must_use]
    pub fn new(tag: &str) -> Self {
        let primary = tag
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let rtl = matches!(
            primary.as_str(),
            "ar" | "he" | "fa" | "ur" | "ps" | "sd" | "yi" | "dv" | "ckb"
        );
        // Comma-decimal locales (a pragmatic subset): most of continental Europe.
        let comma_decimal = matches!(
            primary.as_str(),
            "de" | "fr"
                | "es"
                | "it"
                | "pt"
                | "nl"
                | "pl"
                | "ru"
                | "tr"
                | "sv"
                | "da"
                | "fi"
                | "cs"
                | "el"
                | "hu"
                | "ro"
                | "uk"
        );
        let (decimal, grouping) = if comma_decimal {
            (',', '.')
        } else {
            ('.', ',')
        };
        Self {
            tag: tag.to_string(),
            rtl,
            decimal,
            grouping,
        }
    }

    /// The full language tag this locale was built from.
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Whether this locale is written right-to-left.
    #[must_use]
    pub fn is_rtl(&self) -> bool {
        self.rtl
    }

    /// The [`WritingDir`] for this locale — pair with
    /// [`Theme::with_direction`](crate::Theme::with_direction).
    #[must_use]
    pub fn direction(&self) -> WritingDir {
        if self.rtl {
            WritingDir::Rtl
        } else {
            WritingDir::Ltr
        }
    }

    /// Overrides the decimal and grouping separators (e.g. `(',', ' ')`).
    #[must_use]
    pub fn with_separators(mut self, decimal: char, grouping: char) -> Self {
        self.decimal = decimal;
        self.grouping = grouping;
        self
    }

    /// Formats an integer with this locale's grouping separator
    /// (`1234567` → `"1,234,567"` in `en`, `"1.234.567"` in `de`).
    #[must_use]
    pub fn format_int(&self, n: i64) -> String {
        let digits = n.unsigned_abs().to_string();
        let grouped = group_digits(&digits, self.grouping);
        if n < 0 {
            format!("-{grouped}")
        } else {
            grouped
        }
    }

    /// Formats a number to `decimals` places with this locale's grouping and
    /// decimal separators (`1234.5, 2` → `"1,234.50"` in `en`, `"1.234,50"` in
    /// `de`). Non-finite values render as `"—"`.
    #[must_use]
    pub fn format_f64(&self, x: f64, decimals: usize) -> String {
        if !x.is_finite() {
            return "—".to_string();
        }
        let sign = if x.is_sign_negative() { "-" } else { "" };
        let s = format!("{:.*}", decimals, x.abs());
        let (int_part, frac_part) = s.split_once('.').unwrap_or((s.as_str(), ""));
        let grouped = group_digits(int_part, self.grouping);
        if frac_part.is_empty() {
            format!("{sign}{grouped}")
        } else {
            format!("{sign}{grouped}{}{frac_part}", self.decimal)
        }
    }
}

/// Inserts `sep` every three digits from the right of a run of digit chars.
fn group_digits(digits: &str, sep: char) -> String {
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

/// A message catalog: keys mapped to translated strings with `{name}`
/// placeholder interpolation. A missing key falls back to the key itself, so a
/// view never renders blank.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    messages: HashMap<String, String>,
}

impl Catalog {
    /// An empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a catalog from `(key, message)` pairs.
    pub fn from_pairs<K: Into<String>, V: Into<String>>(
        pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        Self {
            messages: pairs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// Adds or replaces one message.
    pub fn insert(&mut self, key: impl Into<String>, message: impl Into<String>) {
        self.messages.insert(key.into(), message.into());
    }

    /// The raw message for `key`, or `key` itself when absent.
    #[must_use]
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.messages.get(key).map_or(key, String::as_str)
    }

    /// The message for `key` with each `{name}` placeholder replaced by its
    /// `args` value. Unmatched placeholders are left as written; a missing key
    /// falls back to the key (then still interpolated).
    #[must_use]
    pub fn t(&self, key: &str, args: &[(&str, &str)]) -> String {
        let template = self.get(key);
        if args.is_empty() || !template.contains('{') {
            return template.to_string();
        }
        let mut out = template.to_string();
        for (name, value) in args {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_from_tag() {
        assert!(Locale::new("ar").is_rtl());
        assert!(Locale::new("he-IL").is_rtl());
        assert!(!Locale::new("en-US").is_rtl());
        assert_eq!(Locale::new("fa").direction(), WritingDir::Rtl);
        assert_eq!(Locale::new("ja").direction(), WritingDir::Ltr);
    }

    #[test]
    fn integer_grouping_per_locale() {
        assert_eq!(Locale::new("en").format_int(1_234_567), "1,234,567");
        assert_eq!(Locale::new("de").format_int(1_234_567), "1.234.567");
        assert_eq!(Locale::new("en").format_int(-12), "-12");
        assert_eq!(Locale::new("en").format_int(0), "0");
        assert_eq!(Locale::new("en").format_int(999), "999");
    }

    #[test]
    fn decimal_formatting_per_locale() {
        assert_eq!(Locale::new("en").format_f64(1234.5, 2), "1,234.50");
        assert_eq!(Locale::new("de").format_f64(1234.5, 2), "1.234,50");
        assert_eq!(Locale::new("en").format_f64(-0.5, 1), "-0.5");
        assert_eq!(Locale::new("en").format_f64(42.0, 0), "42");
        assert_eq!(Locale::new("en").format_f64(f64::NAN, 2), "—");
    }

    #[test]
    fn separators_override() {
        let fr = Locale::new("fr").with_separators(',', ' ');
        assert_eq!(fr.format_f64(1234.5, 2), "1 234,50");
    }

    #[test]
    fn catalog_interpolates_and_falls_back() {
        let cat = Catalog::from_pairs([("hi", "Hello, {name}!"), ("bye", "Goodbye")]);
        assert_eq!(cat.t("hi", &[("name", "Ada")]), "Hello, Ada!");
        assert_eq!(cat.t("bye", &[]), "Goodbye");
        // Missing key falls back to the key itself.
        assert_eq!(cat.t("unknown", &[]), "unknown");
        // Unmatched placeholder is left intact.
        assert_eq!(cat.t("hi", &[]), "Hello, {name}!");
    }
}

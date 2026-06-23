//! Form constraint validation: a small, pure engine mirroring the web's
//! Constraint Validation API. Declare [`Constraint`]s, run [`validate`] against a
//! value, and get a [`Validity`] (valid + the first failing message) to drive a
//! control's `.invalid(..)` ring and a [`field`](crate::field)'s error text.
//!
//! Validation stays out of the widgets (Elm-pure): the app holds the value, calls
//! `validate` in its `view`, and wires the result. Constraints other than
//! [`Constraint::Required`] ignore an empty value — exactly like HTML, where an
//! empty optional field is valid until you require it.
//!
//! ```
//! use fenestra_kit::validation::{Constraint, validate};
//!
//! let v = validate("ada@example.com", &[Constraint::Required, Constraint::Email]);
//! assert!(v.valid);
//!
//! let v = validate("nope", &[Constraint::Email]);
//! assert!(!v.valid);
//! ```
//!
//! Regex `pattern` validation is intentionally not here — the widget crate stays
//! dependency-light (no `regex`); validate a pattern in the app, or at the
//! `fenestra-describe` boundary where `regex` already lives.

/// One field constraint. Every variant but [`Self::Required`] passes on an empty
/// value (an optional field is valid until filled).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constraint {
    /// The value must be non-empty (after trimming whitespace).
    Required,
    /// At least `n` characters (Unicode scalar values).
    MinLen(usize),
    /// At most `n` characters (Unicode scalar values).
    MaxLen(usize),
    /// A number `>= n` (non-numeric values pass — pair with [`Self::Number`]).
    Min(f64),
    /// A number `<= n` (non-numeric values pass — pair with [`Self::Number`]).
    Max(f64),
    /// A syntactically valid email address (one `@`, a dotted domain, no spaces).
    Email,
    /// A whole number (parses as a signed integer).
    Integer,
    /// A finite decimal number (parses as `f64`).
    Number,
}

impl Constraint {
    /// The failure message when `value` violates this constraint, else `None`.
    #[must_use]
    fn check(self, value: &str) -> Option<String> {
        let trimmed = value.trim();
        // Optional constraints don't fire on an empty value (HTML semantics).
        if trimmed.is_empty() && self != Self::Required {
            return None;
        }
        match self {
            Self::Required => (trimmed.is_empty()).then(|| "This field is required.".to_string()),
            Self::MinLen(n) => (value.chars().count() < n)
                .then(|| format!("Must be at least {n} character{}.", plural(n))),
            Self::MaxLen(n) => (value.chars().count() > n)
                .then(|| format!("Must be at most {n} character{}.", plural(n))),
            Self::Min(m) => match trimmed.parse::<f64>() {
                Ok(v) if v < m => Some(format!("Must be at least {m}.")),
                _ => None,
            },
            Self::Max(m) => match trimmed.parse::<f64>() {
                Ok(v) if v > m => Some(format!("Must be at most {m}.")),
                _ => None,
            },
            Self::Email => (!is_email(trimmed)).then(|| "Enter a valid email address.".to_string()),
            Self::Integer => trimmed
                .parse::<i64>()
                .is_err()
                .then(|| "Enter a whole number.".to_string()),
            Self::Number => {
                let bad = trimmed
                    .parse::<f64>()
                    .map(|v| !v.is_finite())
                    .unwrap_or(true);
                bad.then(|| "Enter a number.".to_string())
            }
        }
    }
}

/// The result of [`validate`]: whether the value satisfies every constraint, and
/// the first failing constraint's message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validity {
    /// True when every constraint passed.
    pub valid: bool,
    /// The first failing constraint's message (`None` when [`Self::valid`]).
    pub message: Option<String>,
}

impl Validity {
    /// A passing validity.
    #[must_use]
    pub fn valid() -> Self {
        Self {
            valid: true,
            message: None,
        }
    }
}

/// Validates `value` against `constraints` in order, returning the first failure
/// (so list the most fundamental constraint — usually [`Constraint::Required`] —
/// first).
#[must_use]
pub fn validate(value: &str, constraints: &[Constraint]) -> Validity {
    for c in constraints {
        if let Some(message) = c.check(value) {
            return Validity {
                valid: false,
                message: Some(message),
            };
        }
    }
    Validity::valid()
}

/// `""` for `1`, `"s"` otherwise — for pluralizing the count messages.
fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// A pragmatic email check: a non-empty local part, a single `@`, and a domain
/// with a dot and no surrounding/embedded whitespace. Not RFC 5322 — deliberately
/// the same "good enough" shape browsers use for `type=email`.
fn is_email(s: &str) -> bool {
    if s.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_fires_only_on_empty() {
        assert!(!validate("", &[Constraint::Required]).valid);
        assert!(!validate("   ", &[Constraint::Required]).valid);
        assert!(validate("x", &[Constraint::Required]).valid);
    }

    #[test]
    fn optional_constraints_pass_on_empty() {
        // An empty value is valid for everything except Required.
        for c in [
            Constraint::MinLen(3),
            Constraint::Email,
            Constraint::Integer,
            Constraint::Min(1.0),
        ] {
            assert!(validate("", &[c]).valid, "{c:?} should pass on empty");
        }
    }

    #[test]
    fn length_bounds() {
        assert!(!validate("ab", &[Constraint::MinLen(3)]).valid);
        assert!(validate("abc", &[Constraint::MinLen(3)]).valid);
        assert!(!validate("abcd", &[Constraint::MaxLen(3)]).valid);
        // Unicode scalar count, not bytes.
        assert!(validate("é", &[Constraint::MaxLen(1)]).valid);
    }

    #[test]
    fn numeric_range() {
        assert!(!validate("3", &[Constraint::Min(5.0)]).valid);
        assert!(validate("7", &[Constraint::Min(5.0)]).valid);
        assert!(!validate("9", &[Constraint::Max(5.0)]).valid);
        // Non-numeric passes Min/Max (a type check is a separate constraint).
        assert!(validate("abc", &[Constraint::Min(5.0)]).valid);
    }

    #[test]
    fn integer_and_number() {
        assert!(validate("42", &[Constraint::Integer]).valid);
        assert!(!validate("4.2", &[Constraint::Integer]).valid);
        assert!(validate("4.2", &[Constraint::Number]).valid);
        assert!(!validate("4.2.0", &[Constraint::Number]).valid);
    }

    #[test]
    fn email_shape() {
        assert!(validate("ada@example.com", &[Constraint::Email]).valid);
        assert!(!validate("ada@example", &[Constraint::Email]).valid);
        assert!(!validate("ada example.com", &[Constraint::Email]).valid);
        assert!(!validate("@example.com", &[Constraint::Email]).valid);
        assert!(!validate("ada@.com", &[Constraint::Email]).valid);
    }

    #[test]
    fn first_failure_wins_in_order() {
        // Required is listed first, so an empty value reports the required message,
        // not the email one.
        let v = validate("", &[Constraint::Required, Constraint::Email]);
        assert_eq!(v.message.as_deref(), Some("This field is required."));
    }
}

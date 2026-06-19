//! Runtime state for declarative bindings: a `Description` may declare an initial
//! `state` map, and widgets `bind` to a key. The framework owns the transition
//! (toggle a bool, set an input's text, set a slider's value) — no logic crosses
//! the boundary. Handlers carry an [`Action`] the engine applies to the state.

use std::collections::BTreeMap;

use serde_json::Value;

/// A description's runtime state: keys to JSON values (bool / number / string).
pub type StateMap = BTreeMap<String, Value>;

/// What a widget handler emits. Unbound widgets emit an inert [`Action::Intent`];
/// bound widgets emit a framework-owned state change the engine applies.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// An inert author intent (an unbound handler's string).
    Intent(String),
    /// Set a bound key to a boolean (checkbox / switch).
    SetBool(String, bool),
    /// Set a bound key to text (text input / area).
    SetText(String, String),
    /// Set a bound key to a number (slider).
    SetNumber(String, f32),
}

/// Reads a bound boolean from state, falling back to `default`.
#[must_use]
pub fn bound_bool(state: &StateMap, key: &str, default: bool) -> bool {
    state.get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// Reads a bound string from state, falling back to `default`.
#[must_use]
pub fn bound_text(state: &StateMap, key: &str, default: &str) -> String {
    state
        .get(key)
        .and_then(Value::as_str)
        .map_or_else(|| default.to_string(), ToString::to_string)
}

/// Reads a bound number from state, falling back to `default`.
#[must_use]
pub fn bound_number(state: &StateMap, key: &str, default: f32) -> f32 {
    #[expect(clippy::cast_possible_truncation, reason = "state numbers are small")]
    state
        .get(key)
        .and_then(Value::as_f64)
        .map_or(default, |v| v as f32)
}

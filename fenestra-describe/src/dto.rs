//! Serializable output types: the typed access tree, query results, the
//! accessibility report, and the aria diff an agent reads. Plain data — the
//! structural engine (`inspect`) and the cli engine fill these in. Leading with
//! these typed values (not pixels) is the point: an agent acts on structure.

use serde::{Deserialize, Serialize};

/// A node's layout rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// One node of the typed access tree — the agent's primary view of a UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessNodeDto {
    /// Stable reference for `query`/`interact`: the node's key when set, else a
    /// structural path like `/0/2/1`.
    #[serde(rename = "ref")]
    pub ref_: String,
    /// ARIA role word (`button`, `textbox`, …), or `generic`.
    pub role: String,
    /// Accessible name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Current value (text fields).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Checked/on state (checkbox, switch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// Selected state (radio, tab).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    /// Whether the node is keyboard-focusable.
    pub focusable: bool,
    /// Layout rectangle.
    pub bounds: Bounds,
    /// Children in paint order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<AccessNodeDto>,
}

/// The result of a `query`: exact matches, plus the nearest candidates on a miss.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    /// Nodes matching the selector, in tree order.
    pub matches: Vec<AccessNodeDto>,
    /// When `matches` is empty, up to a few nearest candidates to guide a retry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nearest: Vec<AccessNodeDto>,
}

/// One contrast shortfall between a theme text/background role pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContrastDto {
    /// The role pair that fell short, e.g. `"text_muted on surface_raised"`.
    pub pair: String,
    /// Measured APCA Lc magnitude.
    pub measured_lc: f64,
    /// The Lc floor it failed to reach.
    pub required_lc: f64,
}

/// Per-text-node legibility, measured on the real resolved colors and sizes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegibilityDto {
    /// The text whose legibility this describes.
    pub text: String,
    /// Foreground (text) color as `#rrggbb`.
    pub fg: String,
    /// Effective background color as `#rrggbb`.
    pub bg: String,
    /// Rendered size in logical pixels.
    pub size_px: f32,
    /// Numeric OpenType weight.
    pub weight: f32,
    /// Measured APCA Lc magnitude.
    pub lc: f64,
    /// The APCA Lc floor required at this size/weight.
    pub required_lc: f64,
    /// WCAG 2 contrast ratio.
    pub wcag2: f64,
    /// Whether the text clears its APCA floor.
    pub passes_apca: bool,
    /// Whether the text clears WCAG 2 (4.5:1, or 3:1 for large text).
    pub passes_wcag2: bool,
    /// Layout rectangle of the text.
    pub bounds: Bounds,
}

/// The accessibility report: theme contrast, labeling, and per-node legibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct A11yReport {
    /// True when there are no contrast violations and every text node is legible.
    pub legible: bool,
    /// Theme role pairs that fall short of their APCA floor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contrast_violations: Vec<ContrastDto>,
    /// Interactive nodes with no accessible name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unlabeled: Vec<AccessNodeDto>,
    /// Per-text-node legibility measurements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_legibility: Vec<LegibilityDto>,
}

/// The result of an aria-snapshot match: pass/fail plus a readable line diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AriaDiff {
    /// Whether the actual tree matched the expected snapshot.
    pub ok: bool,
    /// A unified-style diff (empty when `ok`).
    pub diff: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_node_dto_round_trips() {
        let node = AccessNodeDto {
            ref_: "/0".into(),
            role: "button".into(),
            name: Some("Add".into()),
            value: None,
            checked: None,
            selected: None,
            focusable: true,
            bounds: Bounds {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
            },
            children: vec![],
        };
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"ref\":\"/0\""), "{json}");
        // Optional empty fields are omitted.
        assert!(!json.contains("value"), "{json}");
        let back: AccessNodeDto = serde_json::from_str(&json).unwrap();
        assert_eq!(node, back);
    }
}

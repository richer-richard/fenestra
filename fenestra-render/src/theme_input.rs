//! Resolve a description's optional `theme` to a concrete [`Theme`]: a
//! `{"preset": "light" | "dark"}` selector, or a `ThemeSpec` recipe.

use fenestra_core::{Theme, ThemeSpec};

/// Resolves a description's optional theme value. `None` (no theme set) is the
/// light theme; `{"preset":"dark"}` selects a preset; any other object is parsed
/// as a `ThemeSpec` recipe and resolved.
///
/// # Errors
/// An unknown preset name, or a theme object that is not a valid `ThemeSpec`.
pub fn resolve_theme(value: Option<&serde_json::Value>) -> Result<Theme, String> {
    let Some(value) = value else {
        return Ok(Theme::light());
    };
    if let Some(preset) = value.get("preset").and_then(serde_json::Value::as_str) {
        return match preset {
            "light" => Ok(Theme::light()),
            "dark" => Ok(Theme::dark()),
            other => Err(format!(
                "unknown theme preset {other:?}; expected \"light\" or \"dark\""
            )),
        };
    }
    let spec: ThemeSpec =
        serde_json::from_value(value.clone()).map_err(|e| format!("invalid theme: {e}"))?;
    Ok(spec.theme())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_light() {
        assert_eq!(resolve_theme(None).unwrap().mode, Theme::light().mode);
    }

    #[test]
    fn preset_dark() {
        let v = serde_json::json!({ "preset": "dark" });
        assert_eq!(resolve_theme(Some(&v)).unwrap().mode, Theme::dark().mode);
    }

    #[test]
    fn unknown_preset_errors() {
        let v = serde_json::json!({ "preset": "neon" });
        assert!(resolve_theme(Some(&v)).is_err());
    }

    #[test]
    fn themespec_recipe_resolves() {
        let v = serde_json::json!({
            "mode": "dark",
            "duotone": { "neutral_hue": 152.0, "chroma": 6.0, "accent_hue": 72.0 }
        });
        assert_eq!(resolve_theme(Some(&v)).unwrap().mode, Theme::dark().mode);
    }
}

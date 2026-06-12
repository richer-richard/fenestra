//! 0.7 theme files: the recipe round-trips through JSON, resolves to
//! the same theme the builder calls produce, and typos fail loudly.

use fenestra_core::{Mode, Theme, ThemeSpec};

#[test]
fn specs_resolve_like_the_builders() {
    let stock = ThemeSpec::from_json(r#"{"mode": "dark"}"#).expect("parse");
    assert_eq!(stock.theme().bg, Theme::dark().bg);

    let accent = ThemeSpec::from_json(r#"{"mode": "light", "accent_hue": 265.0}"#).expect("parse");
    assert_eq!(
        accent.theme().accent,
        Theme::from_accent(265.0, Mode::Light).accent
    );

    let duo = ThemeSpec::from_json(
        r#"{"mode": "dark", "duotone": {"neutral_hue": 152.0, "chroma": 6.0, "accent_hue": 72.0}}"#,
    )
    .expect("parse");
    assert_eq!(
        duo.theme().bg,
        Theme::duotone(152.0, 6.0, 72.0, Mode::Dark).bg
    );
}

#[test]
fn round_trips_and_stays_tiny() {
    let spec = ThemeSpec::from_json(r#"{"mode": "dark", "accent_hue": 32.0}"#).expect("parse");
    let json = spec.to_json();
    let back = ThemeSpec::from_json(&json).expect("reparse");
    assert_eq!(spec, back);
    assert!(json.len() < 200, "recipes stay tiny: {json}");
}

#[test]
fn typos_fail_loudly() {
    assert!(ThemeSpec::from_json(r#"{"mode": "dark", "acent_hue": 32.0}"#).is_err());
    assert!(ThemeSpec::from_json(r#"{"mode": "darkk"}"#).is_err());
}

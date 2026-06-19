//! Description → render golden + determinism: the JSON boundary produces the
//! same pixels the builders do, reproducibly. Goldens are referenced against
//! macOS / Metal output (3/255 + 0.2% tolerance).

use std::path::PathBuf;

use fenestra_core::Theme;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::aria_snapshot;
use fenestra_describe::parse::to_element;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const FORM: &str = r#"{
  "schema": "fenestra/1",
  "root": { "col": {
    "style": { "p": 24, "gap": 16, "bg": "surface", "rounded": 12 },
    "children": [
      { "text": { "content": "Sign in", "style": { "size_px": 22, "weight": 600 } } },
      { "text_input": { "value": "ada@example.com", "placeholder": "Email", "on_input": "email" } },
      { "checkbox": { "checked": true, "label": "Remember me", "on_change": "remember" } },
      { "row": { "style": { "gap": 8, "justify": "end" }, "children": [
        { "button": { "label": "Cancel", "on_click": "cancel" } },
        { "button": { "label": "Sign in", "on_click": "submit" } }
      ] } }
    ]
  } }
}"#;

fn form() -> Description {
    serde_json::from_str(FORM).expect("valid description")
}

#[test]
fn described_form_golden() {
    let theme = Theme::light();
    let el = to_element(&form(), &theme).expect("parses");
    let image = render_element(el, &theme, (480, 320));
    assert_png_snapshot(snapshot_dir(), "described_form", &image);
}

#[test]
fn render_is_deterministic() {
    let theme = Theme::light();
    let a = render_element(to_element(&form(), &theme).unwrap(), &theme, (480, 320));
    let b = render_element(to_element(&form(), &theme).unwrap(), &theme, (480, 320));
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "the same description must render identical pixels"
    );
}

#[test]
fn described_form_aria_is_stable() {
    let aria = aria_snapshot(&form(), &Theme::light(), (480, 320)).unwrap();
    for needle in [
        "Sign in",
        "textbox",
        "checkbox",
        r#"button "Cancel""#,
        r#"button "Sign in""#,
    ] {
        assert!(aria.contains(needle), "aria missing {needle:?}:\n{aria}");
    }
}

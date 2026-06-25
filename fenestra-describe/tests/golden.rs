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

/// fenestra's signature Liquid Glass surface — a frosted vibrancy `material`,
/// specular rim, body sheen, backdrop-adaptive tint, and `rounded_full` pill
/// chips — authored entirely in JSON over a vivid striped backdrop. The moat in
/// one artifact: an agent authors the headline visual *and* verifies it headlessly,
/// no Rust touched. Exercises both authoring batches (the optics + the custom
/// material bg).
const GLASS: &str = r#"{
  "schema": "fenestra/1",
  "root": { "stack": { "style": { "w": 560, "h": 360 }, "children": [
    { "row": { "style": { "w": 560, "h": 360 }, "children": [
      { "div": { "style": { "w": 112, "h": 360, "bg": {"oklch":[0.62,0.18,25]} } } },
      { "div": { "style": { "w": 112, "h": 360, "bg": {"oklch":[0.68,0.16,140]} } } },
      { "div": { "style": { "w": 112, "h": 360, "bg": {"oklch":[0.60,0.19,265]} } } },
      { "div": { "style": { "w": 112, "h": 360, "bg": {"oklch":[0.70,0.17,85]} } } },
      { "div": { "style": { "w": 112, "h": 360, "bg": {"oklch":[0.64,0.18,330]} } } }
    ] } },
    { "col": { "style": {
        "absolute": true, "top": 96, "left": 120, "w": 320, "p": 24, "gap": 12,
        "material": {"tint": {"oklch":[0.72,0.04,265]}, "fill_alpha": 0.5, "blur": 24, "saturation": 1.6},
        "specular_edge": "glass", "sheen": "glass", "adaptive_tint": "glass",
        "rounded": 24, "border": {"width": 1, "color": {"oklch":[1,0,0]}}
      }, "children": [
        { "text": { "content": "Glass, authored in JSON", "style": { "size_px": 20, "weight": 600, "color": {"oklch":[0.98,0,0]} } } },
        { "text": { "content": "blur · rim · sheen · adaptive tint", "style": { "size_px": 13, "color": {"oklch":[0.90,0.02,265]} } } },
        { "row": { "style": { "gap": 8 }, "children": [
          { "div": { "style": { "w": 60, "h": 26, "rounded_full": true, "bg": {"oklch":[0.62,0.20,25]} } } },
          { "div": { "style": { "w": 60, "h": 26, "rounded_full": true, "bg": {"oklch":[0.70,0.17,140]} } } },
          { "div": { "style": { "w": 60, "h": 26, "rounded_full": true, "bg": {"oklch":[0.65,0.18,265]} } } }
        ] } }
      ]
    } }
  ] } }
}"#;

#[test]
fn glass_authored_in_json_golden() {
    let theme = Theme::dark();
    let desc: Description = serde_json::from_str(GLASS).expect("valid glass description");
    let el = to_element(&desc, &theme).expect("glass authors cleanly");
    let image = render_element(el, &theme, (560, 360));
    assert_png_snapshot(snapshot_dir(), "glass_authored", &image);
}

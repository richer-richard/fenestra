//! Parse, render, and clamp tests for the R3 vocabulary expansion: `image`
//! plus the thirteen kit widgets that completed the fenestra/1 grammar
//! (`field`, `split_pane`, `combobox`, `multi_select`, `tag_input`,
//! `date_picker`, `tree`, `toast`, `data_table`, `virtual_list`, `popover`,
//! `dropdown_menu`, `command_palette`), plus the `color_picker` follow-up
//! added once the kit shipped that widget. Each widget gets a parse test, a
//! structural assertion (an access-tree role or label), and — where the node
//! carries a hostile-input clamp — a negative test proving the clamp fires
//! instead of panicking.

use fenestra_core::{Element, Fonts, FrameState, Theme, build_frame};
use fenestra_describe::format::Description;
use fenestra_describe::parse::{MAX_LIST_ITEMS, to_element, to_element_lenient, validate};
use fenestra_describe::state::Action;

/// Builds a frame from an element and returns its aria snapshot (no GPU).
fn light_yaml(el: &Element<Action>) -> String {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(
        el,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (640.0, 480.0),
        1.0,
    );
    frame.access_yaml()
}

fn parse(json: &str) -> Description {
    serde_json::from_str(json).expect("valid description")
}

fn build(json: &str) -> Element<Action> {
    to_element(&parse(json), &Theme::light()).expect("parses and builds")
}

// ── base64 / PNG test-fixture builders ────────────────────────────────────────
//
// These mirror (independently, not by calling into) the RFC 4648 encode and
// PNG chunk-framing operations the library's own `decode_base64` and the
// `image` crate perform, so the fixtures below can construct hostile inputs
// (an oversized declared canvas) without shipping a decoder in the test.

/// A minimal 1x1 transparent PNG, base64-encoded — the same fixture the
/// vocabulary's own `image` example uses.
const TINY_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// CRC-32 (ISO 3309), the checksum every PNG chunk trailer uses.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + data.len() + 4);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "test fixtures only ever build small chunks"
    )]
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

/// A syntactically valid PNG (signature + IHDR + IEND, no pixel data) whose
/// header declares `width`x`height` — enough for a decoder to read the
/// dimensions and reject an oversized declaration before ever touching pixel
/// data, without this test actually allocating or shipping a huge image.
fn png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, no interlace
    bytes.extend(png_chunk(b"IHDR", &ihdr));
    bytes.extend(png_chunk(b"IEND", &[]));
    bytes
}

// ── image ──────────────────────────────────────────────────────────────────────

#[test]
fn image_renders_with_label_and_role() {
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{TINY_PNG_B64}","label":"A transparent pixel"}}}}}}"#
    );
    let el = build(&json);
    let yaml = light_yaml(&el);
    assert!(yaml.contains("image"), "yaml: {yaml}");
    assert!(yaml.contains("A transparent pixel"), "yaml: {yaml}");
}

#[test]
fn image_style_resizes_and_rounds() {
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{TINY_PNG_B64}","label":"Avatar","style":{{"w":48,"h":48,"rounded_full":true}}}}}}}}"#
    );
    assert!(validate(&json).is_ok());
    let _ = build(&json);
}

#[test]
fn image_empty_label_records_error() {
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{TINY_PNG_B64}","label":""}}}}}}"#
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("label")), "{errs:?}");
}

#[test]
fn image_invalid_base64_degrades_with_error() {
    let json =
        r#"{"schema":"fenestra/1","root":{"image":{"png":"not valid base64!!","label":"X"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (el, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("png")), "{errs:?}");
    // Degrades to an invisible spacer rather than panicking.
    let _ = light_yaml(&el);
}

#[test]
fn image_not_a_png_degrades_with_error() {
    // Valid base64, but the decoded bytes are not a PNG at all.
    let payload = base64_encode(b"hello world, not a png");
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{payload}","label":"X"}}}}}}"#
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (el, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("png")), "{errs:?}");
    let _ = light_yaml(&el);
}

#[test]
fn image_oversized_base64_payload_records_error() {
    // Longer than the 8 MiB base64-character clamp; the clamp fires before
    // any decode is attempted, so this doesn't need to be a real image.
    let payload = "A".repeat(8 * 1024 * 1024 + 4);
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{payload}","label":"X"}}}}}}"#
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (el, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(
        errs.iter()
            .any(|e| e.path.contains("png") && e.message.contains("8388608")),
        "{errs:?}"
    );
    let _ = light_yaml(&el);
}

#[test]
fn image_oversized_dimensions_rejected_pre_decode() {
    // A tiny file (no pixel data) whose IHDR declares a canvas far past the
    // 8192px/axis clamp — proves the width/height `Limits` are actually wired
    // (a decompression-bomb shape), not merely documented.
    let huge = png_with_dimensions(20_000, 20_000);
    let payload = base64_encode(&huge);
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{payload}","label":"X"}}}}}}"#
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (el, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("png")), "{errs:?}");
    let _ = light_yaml(&el);
}

// ── split_pane ─────────────────────────────────────────────────────────────────

#[test]
fn split_pane_renders_both_children() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"split_pane":{"first":{"text":{"content":"Left"}},"second":{"text":{"content":"Right"}}}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(
        yaml.contains("Left") && yaml.contains("Right"),
        "yaml: {yaml}"
    );
}

#[test]
fn split_pane_bind_reads_state() {
    let json = r#"{"schema":"fenestra/1","state":{"split":0.3},"root":{"split_pane":{"first":{"text":{"content":"A"}},"second":{"text":{"content":"B"}},"bind":"split"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

// ── field ────────────────────────────────────────────────────────────────────

#[test]
fn field_wraps_control_with_label() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"field":{"label":"Email","control":{"text_input":{"value":"a@b.com"}},"required":true}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Email"), "yaml: {yaml}");
    assert!(yaml.contains("a@b.com"), "yaml: {yaml}");
}

#[test]
fn field_error_message_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"field":{"label":"Email","control":{"text_input":{"value":""}},"help":"We'll never share it.","error":"Required"}}}"#,
    );
    let yaml = light_yaml(&el);
    // Error wins over help.
    assert!(yaml.contains("Required"), "yaml: {yaml}");
}

// ── combobox ─────────────────────────────────────────────────────────────────

#[test]
fn combobox_renders_combobox_role() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"combobox":{"options":["Rust","Ruby","Python"],"value":"ru","open":true}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("combobox"), "yaml: {yaml}");
}

#[test]
fn combobox_bind_reads_state_value() {
    let json = r#"{"schema":"fenestra/1","state":{"lang":"py"},"root":{"combobox":{"options":["python","ruby"],"bind":"lang"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

#[test]
fn combobox_options_clamp_records_error() {
    let options: Vec<String> = (0..MAX_LIST_ITEMS + 1).map(|i| i.to_string()).collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"combobox": {"options": options}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("options")), "{errs:?}");
}

// ── multi_select ───────────────────────────────────────────────────────────────

#[test]
fn multi_select_renders_checkbox_chips() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"multi_select":{"options":["Rust","Go","Zig"],"selected":[0,2]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("checkbox"), "yaml: {yaml}");
    assert!(yaml.contains("Zig"), "yaml: {yaml}");
}

#[test]
fn multi_select_options_clamp_records_error() {
    let options: Vec<String> = (0..MAX_LIST_ITEMS + 1).map(|i| i.to_string()).collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"multi_select": {"options": options}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("options")), "{errs:?}");
}

// ── tag_input ──────────────────────────────────────────────────────────────────

#[test]
fn tag_input_renders_tags() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"tag_input":{"tags":["design","rust"],"placeholder":"Add a tag…"}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(
        yaml.contains("design") && yaml.contains("rust"),
        "yaml: {yaml}"
    );
}

#[test]
fn tag_input_tags_clamp_records_error() {
    let tags: Vec<String> = (0..MAX_LIST_ITEMS + 1).map(|i| i.to_string()).collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"tag_input": {"tags": tags}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("tags")), "{errs:?}");
}

// ── date_picker ──────────────────────────────────────────────────────────────

#[test]
fn date_picker_single_mode_builds() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"date_picker":{"year":2026,"month":6,"selected":[2026,6,15]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("June"), "yaml: {yaml}");
}

#[test]
fn date_picker_range_mode_builds() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"date_picker":{"year":2026,"month":6,"range":true,"range_start":[2026,6,5],"range_end":[2026,6,20]}}}"#,
    );
    let _ = light_yaml(&el);
}

#[test]
fn date_picker_invalid_month_records_error() {
    let json = r#"{"schema":"fenestra/1","root":{"date_picker":{"year":2026,"month":13}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("month")), "{errs:?}");
}

#[test]
fn date_picker_invalid_selected_day_records_error() {
    let json = r#"{"schema":"fenestra/1","root":{"date_picker":{"year":2026,"month":6,"selected":[2026,6,99]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("selected")), "{errs:?}");
}

#[test]
fn date_picker_day_out_of_range_for_its_month_records_error() {
    // A day in 1..=31 but past the actual month length (April 31, Feb 30) must
    // still error — otherwise it silently selects no cell with no explanation.
    for (spec, month) in [("[2026,4,31]", "April"), ("[2026,2,30]", "February")] {
        let json = format!(
            r#"{{"schema":"fenestra/1","root":{{"date_picker":{{"year":2026,"month":6,"selected":{spec}}}}}}}"#
        );
        let desc: Description = serde_json::from_str(&json).unwrap();
        let (_, errs) = to_element_lenient(&desc, &Theme::light());
        assert!(
            errs.iter().any(|e| e.path.contains("selected")),
            "{month} spec {spec} should record a day error; got {errs:?}"
        );
    }
    // A legitimate leap-day (2028 is a leap year) must NOT error.
    let json = r#"{"schema":"fenestra/1","root":{"date_picker":{"year":2028,"month":2,"selected":[2028,2,29]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(
        !errs.iter().any(|e| e.path.contains("selected")),
        "Feb 29 2028 is valid; got {errs:?}"
    );
}

// ── tree ─────────────────────────────────────────────────────────────────────

#[test]
fn tree_renders_expanded_children() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"tree":{"items":[{"id":"root","label":"Root","children":[{"id":"child","label":"Child"}]}],"expanded":["root"]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(
        yaml.contains("Root") && yaml.contains("Child"),
        "yaml: {yaml}"
    );
}

#[test]
fn tree_total_node_clamp_records_error() {
    let items: Vec<_> = (0..=MAX_LIST_ITEMS)
        .map(|i| serde_json::json!({"id": format!("n{i}"), "label": format!("Node {i}")}))
        .collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"tree": {"items": items}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("items")), "{errs:?}");
}

// ── toast ────────────────────────────────────────────────────────────────────

#[test]
fn toast_renders_message_and_alert_role() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"toast":{"items":[{"message":"Report saved","status":"success"}]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("alert"), "yaml: {yaml}");
    assert!(yaml.contains("Report saved"), "yaml: {yaml}");
}

#[test]
fn toast_empty_items_renders_nothing() {
    let el = build(r#"{"schema":"fenestra/1","root":{"toast":{"items":[]}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn toast_items_clamp_records_error() {
    let items: Vec<_> = (0..=MAX_LIST_ITEMS)
        .map(|i| serde_json::json!({"message": format!("msg {i}")}))
        .collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"toast": {"items": items}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("items")), "{errs:?}");
}

// ── data_table ─────────────────────────────────────────────────────────────────

#[test]
fn data_table_renders_columns_and_rows() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"data_table":{"columns":["Name","Role"],"rows":[["Ripley","Officer"]]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(
        yaml.contains("Name") && yaml.contains("Ripley"),
        "yaml: {yaml}"
    );
}

#[test]
fn data_table_selection_and_filter_build() {
    let json = r#"{"schema":"fenestra/1","root":{"data_table":{
        "columns":["Name"],
        "rows":[["Ripley"],["Hicks"]],
        "selection":[true,false],
        "on_select_all":"select_all",
        "filter":["rip"],
        "on_filter":"filter_changed",
        "column_widths":[160.0],
        "pinned_left":1
    }}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

#[test]
fn data_table_columns_clamp_records_error() {
    let columns: Vec<String> = (0..200).map(|i| i.to_string()).collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"data_table": {"columns": columns, "rows": []}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("columns")), "{errs:?}");
}

#[test]
fn data_table_rows_clamp_records_error() {
    let rows: Vec<Vec<String>> = (0..=MAX_LIST_ITEMS).map(|i| vec![i.to_string()]).collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"data_table": {"columns": ["n"], "rows": rows}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("rows")), "{errs:?}");
}

// ── virtual_list ───────────────────────────────────────────────────────────────

#[test]
fn virtual_list_renders_literal_row_items() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"virtual_list":{"items":[{"text":{"content":"Item 0"}},{"text":{"content":"Item 1"}}],"row_height":32}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(
        yaml.contains("Item 0") && yaml.contains("Item 1"),
        "yaml: {yaml}"
    );
}

#[test]
fn virtual_list_invalid_row_height_records_error() {
    let json = r#"{"schema":"fenestra/1","root":{"virtual_list":{"items":[],"row_height":-5}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (el, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(
        errs.iter().any(|e| e.path.contains("row_height")),
        "{errs:?}"
    );
    // Degrades to a positive default rather than panicking downstream.
    let _ = light_yaml(&el);
}

#[test]
fn virtual_list_items_clamp_records_error() {
    let items: Vec<_> = (0..=MAX_LIST_ITEMS)
        .map(|i| serde_json::json!({"text": {"content": format!("Item {i}")}}))
        .collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"virtual_list": {"items": items, "row_height": 24.0}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("items")), "{errs:?}");
}

// ── popover / dropdown_menu / command_palette ─────────────────────────────────

#[test]
fn popover_renders_trigger_and_builds_content() {
    let json = r#"{"schema":"fenestra/1","root":{"popover":{"trigger":{"button":{"label":"Open"}},"content":{"text":{"content":"Panel content"}}}}}"#;
    let el = build(json);
    let yaml = light_yaml(&el);
    // `Overlay::menu()` is `OverlayMode::Toggle` (closed by default), so the
    // panel's own content is absent from a closed headless snapshot — same
    // as `dropdown_menu`. The trigger is always visible; `content` parsing
    // through `node_to_element` (never a raw string) is checked directly.
    assert!(yaml.contains("Open"), "yaml: {yaml}");
    assert!(
        to_element(&parse(json), &Theme::light()).is_ok(),
        "content must parse cleanly even though it isn't shown while closed"
    );
}

#[test]
fn dropdown_menu_renders_trigger_and_items() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"dropdown_menu":{"trigger":{"button":{"label":"Actions"}},"items":[{"label":"Rename"},{"label":"Delete"}]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Actions"), "yaml: {yaml}");
}

#[test]
fn dropdown_menu_items_clamp_records_error() {
    let items: Vec<_> = (0..=MAX_LIST_ITEMS)
        .map(|i| serde_json::json!({"label": format!("Item {i}")}))
        .collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"dropdown_menu": {"trigger": {"button": {"label": "Actions"}}, "items": items}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("items")), "{errs:?}");
}

#[test]
fn command_palette_renders_dialog_role() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"command_palette":{"commands":[{"label":"New file"},{"label":"Open"}]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("dialog"), "yaml: {yaml}");
    assert!(yaml.contains("New file"), "yaml: {yaml}");
}

#[test]
fn command_palette_bind_reads_state_query() {
    let json = r#"{"schema":"fenestra/1","state":{"q":"open"},"root":{"command_palette":{"commands":[{"label":"Open"}],"bind":"q"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

#[test]
fn command_palette_commands_clamp_records_error() {
    let commands: Vec<_> = (0..=MAX_LIST_ITEMS)
        .map(|i| serde_json::json!({"label": format!("Command {i}")}))
        .collect();
    let json = serde_json::json!({
        "schema": "fenestra/1",
        "root": {"command_palette": {"commands": commands}}
    })
    .to_string();
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("commands")), "{errs:?}");
}

// ── Reject unknown fields ──────────────────────────────────────────────────────

#[test]
fn image_rejects_unknown_field() {
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{TINY_PNG_B64}","label":"X","alt":"Y"}}}}}}"#
    );
    assert!(serde_json::from_str::<Description>(&json).is_err());
}

#[test]
fn data_table_rejects_unknown_field() {
    let json = r#"{"schema":"fenestra/1","root":{"data_table":{"columns":[],"rows":[],"resize_active":0}}}"#;
    assert!(serde_json::from_str::<Description>(json).is_err());
}

// ── color_picker (follow-up: fenestra_kit::color_picker landed after R3) ──────

#[test]
fn color_picker_renders_swatch_and_label() {
    let json = r##"{"schema":"fenestra/1","root":{"color_picker":{"value":"#3b82f6","label":"Accent color"}}}"##;
    let el = build(json);
    let yaml = light_yaml(&el);
    assert!(yaml.contains("image"), "yaml: {yaml}");
    assert!(yaml.contains("Current color #3b82f6"), "yaml: {yaml}");
    assert!(yaml.contains("Accent color"), "yaml: {yaml}");
    // The hue/alpha thumbs are keyboard-accessible sliders.
    assert!(
        yaml.contains("Hue") && yaml.contains("Alpha"),
        "yaml: {yaml}"
    );
}

#[test]
fn color_picker_defaults_to_neutral_gray_and_default_label() {
    let el = build(r#"{"schema":"fenestra/1","root":{"color_picker":{}}}"#);
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Current color #808080"), "yaml: {yaml}");
    assert!(yaml.contains("\"Color\""), "yaml: {yaml}");
}

#[test]
fn color_picker_accepts_oklch_text() {
    let json = r#"{"schema":"fenestra/1","root":{"color_picker":{"value":"oklch(0.7 0.15 250)"}}}"#;
    assert!(validate(json).is_ok());
    let _ = build(json);
}

#[test]
fn color_picker_bind_reads_state_value() {
    let json = r##"{"schema":"fenestra/1","state":{"accent":"#ff0000"},"root":{"color_picker":{"bind":"accent"}}}"##;
    let desc: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&desc, &Theme::light()).expect("parses and builds");
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Current color #ff0000"), "yaml: {yaml}");
}

#[test]
fn color_picker_invalid_value_falls_back_and_records_error() {
    let json = r#"{"schema":"fenestra/1","root":{"color_picker":{"value":"not a color"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (el, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("value")), "{errs:?}");
    // Degrades to the documented neutral-gray fallback rather than panicking.
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Current color #808080"), "yaml: {yaml}");
}

#[test]
fn color_picker_disabled_builds() {
    let json =
        r##"{"schema":"fenestra/1","root":{"color_picker":{"value":"#00ff00","disabled":true}}}"##;
    let el = build(json);
    let _ = light_yaml(&el);
}

#[test]
fn color_picker_oversized_pad_size_is_clamped_by_the_widget_not_an_error() {
    // The kit widget itself clamps `pad_size` to 80.0..=480.0; describe does
    // not duplicate that clamp (see the `ColorPickerNode` doc comment), so
    // this must build without recording an error.
    let json = r#"{"schema":"fenestra/1","root":{"color_picker":{"pad_size":10000.0}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn color_picker_non_finite_pad_size_records_error() {
    // `1e39` is a valid, finite `f64` (`f64::MAX` is ~1.8e308) but overflows
    // `f32::MAX` (~3.4e38), so it saturates to `f32::INFINITY` when narrowed
    // — a legitimate non-finite value can reach the parser this way even
    // though JSON syntax itself disallows bare `NaN`/`Infinity` tokens.
    let json = r#"{"schema":"fenestra/1","root":{"color_picker":{"pad_size":1e39}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("pad_size")), "{errs:?}");
}

#[test]
fn color_picker_rejects_unknown_field() {
    let json =
        r##"{"schema":"fenestra/1","root":{"color_picker":{"value":"#000000","opacity":0.5}}}"##;
    assert!(serde_json::from_str::<Description>(json).is_err());
}

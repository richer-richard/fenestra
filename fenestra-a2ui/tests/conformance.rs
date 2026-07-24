//! Conformance against the official A2UI v0.9 gallery examples (vendored
//! under `fixtures/`, see NOTICE): every stream parses, folds, and renders
//! headlessly; representative surfaces are pinned as goldens; bindings,
//! templates, actions, and two-way writes behave per the protocol.

use std::path::PathBuf;

use fenestra_a2ui::{A2uiMsg, A2uiSignal, Client, parse_stream};
use fenestra_core::{Theme, by};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(path).expect("fixture exists")
}

fn client_for(name: &str) -> Client {
    let msgs = parse_stream(&fixture(name)).expect("fixture parses");
    let mut client = Client::new();
    client.apply_all(&msgs).expect("stream applies");
    client
}

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

/// Every vendored official example parses, applies, and renders without a
/// structural failure.
#[test]
fn every_official_example_renders() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).expect("fixtures dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let msgs = parse_stream(&std::fs::read_to_string(&path).expect("read"))
            .unwrap_or_else(|e| panic!("{name}: parse failed: {e}"));
        let mut client = Client::new();
        client
            .apply_all(&msgs)
            .unwrap_or_else(|e| panic!("{name}: apply failed: {e}"));
        let surface = client.single_surface().expect("one surface per example");
        let rendered = surface.render(&Theme::light());
        let img = render_element(rendered.element, &Theme::light(), (480, 640));
        assert!(img.width() > 0, "{name}: rendered");
        count += 1;
    }
    assert!(count >= 10, "the fixture corpus is present ({count})");
}

/// The weather example exercises templated children and formatString —
/// the resolved values must reach the accessibility tree.
#[test]
fn weather_bindings_and_templates_resolve() {
    let client = client_for("04_weather-current.json");
    let surface = client.single_surface().expect("surface");
    let rendered = surface.render(&Theme::light());
    let frame = fenestra_shell::render_element(rendered.element, &Theme::light(), (480, 640));
    drop(frame);
    // Structural check through a fresh render (render is pure).
    let rendered = surface.render(&Theme::light());
    let mut fonts = fenestra_core::Fonts::embedded();
    let mut state = fenestra_core::FrameState::new();
    let frame = fenestra_core::build_frame(
        &rendered.element,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (480.0, 640.0),
        1.0,
    );
    let tree = frame.debug_tree();
    // The data model in the fixture carries the location and temps that
    // only reach the tree through bindings + ${…} interpolation.
    assert!(
        tree.contains("San Francisco") || tree.contains("72"),
        "resolved data must appear, got tree:\n{tree}"
    );
}

/// Goldens for representative surfaces (login form: inputs; product card:
/// functions + layout).
#[test]
fn login_form_golden() {
    let client = client_for("00_simple-login-form.json");
    let rendered = client
        .single_surface()
        .expect("surface")
        .render(&Theme::light());
    let img = render_element(rendered.element, &Theme::light(), (420, 320));
    assert_png_snapshot(snapshot_dir(), "a2ui_login_form", &img);
}

#[test]
fn product_card_golden() {
    let client = client_for("05_product-card.json");
    let rendered = client
        .single_surface()
        .expect("surface")
        .render(&Theme::light());
    let img = render_element(rendered.element, &Theme::light(), (420, 560));
    assert_png_snapshot(snapshot_dir(), "a2ui_product_card", &img);
}

/// Two-way binding: an input's SetString writes into the data model, and
/// the next render reflects it.
#[test]
fn two_way_binding_writes_the_data_model() {
    let mut client = client_for("00_simple-login-form.json");
    let id = client.surfaces().next().expect("surface").id().to_owned();
    let surface = client.surface_mut(&id).expect("surface");
    let signal = surface.handle(A2uiMsg::SetString {
        path: "/username".into(),
        value: "ada".into(),
    });
    assert!(signal.is_none(), "binding writes are internal");
    assert_eq!(surface.data().pointer("/username").unwrap(), "ada");
}

/// A button action resolves its context against the data model and, with
/// sendDataModel, attaches the model to the signal.
#[test]
fn actions_surface_as_signals_with_the_data_model() {
    let mut client = client_for("00_simple-login-form.json");
    let id = client.surfaces().next().expect("surface").id().to_owned();
    let surface = client.surface_mut(&id).expect("surface");
    surface.handle(A2uiMsg::SetString {
        path: "/username".into(),
        value: "ada".into(),
    });
    let signal = surface.handle(A2uiMsg::Event {
        name: "login".into(),
        context: serde_json::Value::Null,
        source_id: "submit_button".into(),
    });
    match signal {
        Some(A2uiSignal::Event {
            name,
            data_model,
            source_id,
            ..
        }) => {
            assert_eq!(name, "login");
            assert_eq!(
                source_id, "submit_button",
                "the firing component rides along"
            );
            let model = data_model.expect("sendDataModel is true in the fixture");
            assert_eq!(model.pointer("/username").unwrap(), "ada");
        }
        other => panic!("expected an event signal, got {other:?}"),
    }
    // The client→server action message carries the ids the spec requires.
    let surface = client.surface(&id).expect("surface");
    let msg = surface.action_message(
        "login",
        "submit_button",
        &serde_json::Value::Null,
        "2026-07-24T00:00:00Z",
    );
    assert_eq!(msg["surfaceId"], id.as_str());
    assert_eq!(msg["name"], "login");
}

/// updateDataModel changes what renders (the protocol's live-update loop).
#[test]
fn data_model_updates_rerender() {
    let mut client = client_for("00_simple-login-form.json");
    let id = client.surfaces().next().expect("surface").id().to_owned();
    let update = format!(
        r#"[{{"version":"v0.9","updateDataModel":{{"surfaceId":"{id}","path":"/username","value":"grace"}}}}]"#
    );
    client
        .apply_all(&parse_stream(&update).expect("parses"))
        .expect("applies");
    let rendered = client
        .surface(&id)
        .expect("surface")
        .render(&Theme::light());
    let mut fonts = fenestra_core::Fonts::embedded();
    let mut state = fenestra_core::FrameState::new();
    let frame = fenestra_core::build_frame(
        &rendered.element,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (420.0, 320.0),
        1.0,
    );
    assert!(
        frame.query(&by::value("grace")).is_some(),
        "the updated value must render"
    );
}

/// A reference cycle degrades to a pointed note, never a stack overflow.
#[test]
fn reference_cycles_degrade_with_a_note() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"Column","children":["a"]},
            {"id":"a","component":"Column","children":["root"]}
        ]}}
    ]"#;
    let mut client = Client::new();
    client
        .apply_all(&parse_stream(stream).expect("parses"))
        .expect("applies");
    let rendered = client
        .surface("s")
        .expect("surface")
        .render(&Theme::light());
    assert!(
        rendered.notes.iter().any(|n| n.contains("cycle")),
        "cycle must be reported, got: {:?}",
        rendered.notes
    );
}

/// Unknown components degrade to labeled placeholders with a note.
#[test]
fn unknown_components_degrade_with_a_note() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"custom"}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"FancyGauge","value":42}
        ]}}
    ]"#;
    let mut client = Client::new();
    client
        .apply_all(&parse_stream(stream).expect("parses"))
        .expect("applies");
    let rendered = client
        .surface("s")
        .expect("surface")
        .render(&Theme::light());
    assert!(
        rendered.notes.iter().any(|n| n.contains("FancyGauge")),
        "got: {:?}",
        rendered.notes
    );
}

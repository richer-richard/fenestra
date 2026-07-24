//! Regression tests from the 2026-07-24 adversarial review of the A2UI
//! renderer: binding resolution gaps, protocol tolerance, and silent
//! write failures. Each test names the finding it pins.

use fenestra_a2ui::{A2uiMsg, A2uiSignal, Client, parse_stream};
use fenestra_core::{Element, Theme};

fn apply(stream: &str) -> Client {
    let msgs = parse_stream(stream).expect("stream parses");
    let mut client = Client::new();
    client.apply_all(&msgs).expect("stream applies");
    client
}

fn frame_tree(el: &Element<A2uiMsg>, size: (f32, f32)) -> String {
    let mut fonts = fenestra_core::Fonts::embedded();
    let mut state = fenestra_core::FrameState::new();
    let frame = fenestra_core::build_frame(el, &Theme::light(), &mut fonts, &mut state, size, 1.0);
    frame.debug_tree()
}

/// Finding 1: `Icon.name` is a dynamic value in the catalog — a bound name
/// must resolve against the data model, not stringify the binding object.
/// The official task-card example binds `/priorityIcon` = `"priority_high"`
/// (a Material name, honestly noted as outside the vendored Lucide set) —
/// the note must name the *resolved* value, never `{"path": …}`.
#[test]
fn bound_icon_names_resolve() {
    let stream = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/07_task-card.json"),
    )
    .expect("fixture exists");
    let client = apply(&stream);
    let rendered = client
        .single_surface()
        .expect("surface")
        .render(&Theme::light());
    assert!(
        !rendered.notes.iter().any(|n| n.contains("{\"path\"")),
        "the binding object leaked into rendering, notes: {:?}",
        rendered.notes
    );
    let tree = frame_tree(&rendered.element, (480.0, 640.0));
    assert!(
        rendered.notes.iter().any(|n| n.contains("priority_high"))
            || tree.contains("priority_high"),
        "the bound icon name must resolve to priority_high; notes: {:?}",
        rendered.notes
    );
}

/// Finding 2: a template with an *absolute* path inside a collection scope
/// must resolve item scopes from that absolute path, not from a corrupted
/// `{scope}//abs` join.
#[test]
fn nested_template_absolute_paths_resolve() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateDataModel":{"surfaceId":"s","path":"/","value":{
            "rows": [{"title": "first"}, {"title": "second"}],
            "orders": [{"name": "alpha"}, {"name": "beta"}]
        }}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"Column","children":{"componentId":"row-tpl","path":"/rows"}},
            {"id":"row-tpl","component":"Column","children":{"componentId":"order-tpl","path":"/orders"}},
            {"id":"order-tpl","component":"Text","text":{"path":"name"}}
        ]}}
    ]"#;
    let client = apply(stream);
    let rendered = client
        .single_surface()
        .expect("surface")
        .render(&Theme::light());
    let tree = frame_tree(&rendered.element, (480.0, 640.0));
    assert!(
        tree.contains("alpha") && tree.contains("beta"),
        "absolute template paths under a scope must resolve; notes: {:?}\ntree:\n{tree}",
        rendered.notes
    );
}

/// Finding 3: an action's message must carry the source component id, so
/// hosts can populate the client→server action message's required
/// `sourceComponentId`.
#[test]
fn event_actions_carry_the_source_component() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"Column","children":["go"]},
            {"id":"go","component":"Button","child":"go-label",
             "action":{"event":{"name":"launch"}}},
            {"id":"go-label","component":"Text","text":"Go"}
        ]}}
    ]"#;
    let mut client = apply(stream);
    let rendered = client
        .surface("s")
        .expect("surface")
        .render(&Theme::light());
    let msg = find_click(&rendered.element).expect("the button carries a click message");
    let A2uiMsg::Event {
        ref source_id,
        ref name,
        ..
    } = msg
    else {
        panic!("expected an event message, got {msg:?}");
    };
    assert_eq!(name, "launch");
    assert_eq!(source_id, "go", "the firing component's id must ride along");
    let signal = client
        .surface_mut("s")
        .expect("surface")
        .handle(msg.clone())
        .expect("events surface as signals");
    let A2uiSignal::Event { source_id, .. } = signal else {
        panic!("expected an event signal");
    };
    assert_eq!(source_id, "go");
}

fn find_click(el: &Element<A2uiMsg>) -> Option<A2uiMsg> {
    if let Some(msg) = el.click_msg() {
        return Some(msg.clone());
    }
    el.children_ref().iter().find_map(find_click)
}

/// Finding 4: toggling a literal-valued CheckBox stores a local edit; the
/// next render must *read it back* (the toggle message flips).
#[test]
fn literal_checkbox_toggles_take_effect() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"CheckBox","label":"Agree","value":false}
        ]}}
    ]"#;
    let mut client = apply(stream);
    let rendered = client
        .surface("s")
        .expect("surface")
        .render(&Theme::light());
    let toggle = find_click(&rendered.element).expect("the checkbox toggles on click");
    let A2uiMsg::LocalEdit { ref value, .. } = toggle else {
        panic!("a literal checkbox stores a local edit, got {toggle:?}");
    };
    assert_eq!(
        value,
        &serde_json::Value::Bool(true),
        "unchecked toggles on"
    );
    assert!(
        client
            .surface_mut("s")
            .expect("surface")
            .handle(toggle)
            .is_none()
    );
    let rendered = client
        .surface("s")
        .expect("surface")
        .render(&Theme::light());
    let toggle = find_click(&rendered.element).expect("still toggleable");
    let A2uiMsg::LocalEdit { value, .. } = toggle else {
        panic!("expected a local edit");
    };
    assert_eq!(
        value,
        serde_json::Value::Bool(false),
        "after toggling on, the checkbox must render checked (next toggle turns it off)"
    );
}

/// Finding 5: an unknown message type (a newer protocol revision) is
/// skipped with a note on the surface it names — the stream around it
/// still applies and renders.
#[test]
fn unknown_message_types_are_skipped_not_fatal() {
    let stream = r##"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateTheme":{"surfaceId":"s","primaryColor":"#ff0000"}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"Text","text":"still here"}
        ]}}
    ]"##;
    let client = apply(stream);
    let surface = client.surface("s").expect("the stream still applies");
    let rendered = surface.render(&Theme::light());
    let tree = frame_tree(&rendered.element, (480.0, 640.0));
    assert!(
        tree.contains("still here"),
        "known messages around the unknown one apply"
    );
    assert!(
        surface.notes().iter().any(|n| n.contains("updateTheme")),
        "the skipped message type is noted, got: {:?}",
        surface.notes()
    );
}

/// Finding 6: a *known* component whose body is malformed (Slider without
/// `max`) degrades to a placeholder with a note; its siblings and the rest
/// of the stream are untouched.
#[test]
fn malformed_known_component_degrades_not_fails() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"Column","children":["ok","broken"]},
            {"id":"ok","component":"Text","text":"fine"},
            {"id":"broken","component":"Slider","value":3}
        ]}}
    ]"#;
    let client = apply(stream);
    let rendered = client
        .single_surface()
        .expect("the stream applies despite the malformed component")
        .render(&Theme::light());
    let tree = frame_tree(&rendered.element, (480.0, 640.0));
    assert!(tree.contains("fine"), "siblings render");
    assert!(
        rendered.notes.iter().any(|n| n.contains("Slider")),
        "the malformed component is noted, got: {:?}",
        rendered.notes
    );
}

/// Finding 8: data-model array writes support RFC 6901 `-` (append), and a
/// write that cannot apply records a note instead of vanishing.
#[test]
fn array_appends_apply_and_bad_writes_are_noted() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateDataModel":{"surfaceId":"s","path":"/items","value":[1]}},
        {"version":"v0.9","updateDataModel":{"surfaceId":"s","path":"/items/-","value":2}},
        {"version":"v0.9","updateDataModel":{"surfaceId":"s","path":"/items/9","value":3}}
    ]"#;
    let client = apply(stream);
    let surface = client.surface("s").expect("surface");
    assert_eq!(
        surface.data().pointer("/items").unwrap(),
        &serde_json::json!([1, 2]),
        "`-` appends"
    );
    assert!(
        surface.notes().iter().any(|n| n.contains("/items/9")),
        "the dropped out-of-range write is noted, got: {:?}",
        surface.notes()
    );
}

/// Finding 4 (mature form): every literal-valued input control stays
/// interactive through local edits — a ChoicePicker with a literal value
/// renders the locally edited selection after the user changes it.
#[test]
fn literal_choice_picker_reads_local_edits() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"ChoicePicker","variant":"mutuallyExclusive",
             "options":[{"label":"Pro","value":"pro"},{"label":"Basic","value":"basic"}],
             "value":"pro"}
        ]}}
    ]"#;
    let mut client = apply(stream);
    let signal = client
        .surface_mut("s")
        .expect("surface")
        .handle(A2uiMsg::LocalEdit {
            id: "root".into(),
            value: serde_json::json!(["basic"]),
        });
    assert!(signal.is_none(), "local edits are internal");
    let rendered = client
        .surface("s")
        .expect("surface")
        .render(&Theme::light());
    let tree = frame_tree(&rendered.element, (480.0, 640.0));
    assert!(
        tree.contains("Basic"),
        "the locally edited selection must render; tree:\n{tree}"
    );
}

/// Finding 10: a mutually-exclusive ChoicePicker bound to a *string* value
/// selects the matching option, exactly like the literal-string form.
#[test]
fn bound_string_choice_picker_selects() {
    let stream = r#"[
        {"version":"v0.9","createSurface":{"surfaceId":"s","catalogId":"basic"}},
        {"version":"v0.9","updateDataModel":{"surfaceId":"s","path":"/plan","value":"basic"}},
        {"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
            {"id":"root","component":"ChoicePicker","variant":"mutuallyExclusive",
             "options":[{"label":"Pro","value":"pro"},{"label":"Basic","value":"basic"}],
             "value":{"path":"/plan"}}
        ]}}
    ]"#;
    let client = apply(stream);
    let rendered = client
        .single_surface()
        .expect("surface")
        .render(&Theme::light());
    let tree = frame_tree(&rendered.element, (480.0, 640.0));
    assert!(
        tree.contains("Basic"),
        "the bound string selection must show; tree:\n{tree}"
    );
}

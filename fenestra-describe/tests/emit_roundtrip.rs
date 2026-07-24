//! The emitter's round-trip contract: for the JSON-expressible subset
//! (layout, text, literal styles, click intents), parse → emit → parse
//! renders byte-identical pixels and the emitter reports zero warnings.
//! Anything lossy must be *reported*, never silent.

use fenestra_core::{Theme, col, div, row, text};
use fenestra_describe::emit::{emit_description, emit_element};
use fenestra_describe::format::{Description, Node, SCHEMA_V1};
use fenestra_describe::parse::to_element;
use fenestra_shell::render_element;

fn parse_doc(json: &str) -> Description {
    serde_json::from_str(json).expect("test doc deserializes")
}

/// parse → emit → parse must render the same bytes, with no warnings, for
/// documents inside the expressible subset.
#[test]
fn literal_styled_documents_round_trip_pixel_identically() {
    let theme = Theme::light();
    for doc in [
        // Layout: nesting, grow, percent sizes, gap, padding, scroll.
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"p":16,"gap":8,"w":320,"h":240},
            "children":[
              {"row":{"style":{"gap":4,"align":"center","justify":"between"},"children":[
                {"text":{"content":"Header","style":{"size_px":20,"weight":600}}},
                {"div":{"style":{"w":24,"h":24,"rounded_full":true,"bg":{"oklch":[0.6,0.15,250]}}}}
              ]}},
              {"row":{"style":{"gap":8,"grow":true},"children":[
                {"div":{"style":{"w":"30%","bg":{"oklch":[0.9,0.02,100]},"rounded":6}}},
                {"div":{"style":{"grow":2,"bg":{"oklch":[0.8,0.05,200]},"rounded":6}}}
              ]}},
              {"col":{"style":{"h":60,"scroll":"y","gap":2},"children":[
                {"text":{"content":"line one"}},
                {"text":{"content":"line two"}}
              ]}}
            ]}}}"#,
        // Text styling, alignment, opacity, click intent, absolute child.
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"p":12,"w":260,"h":140},"children":[
              {"text":{"content":"Click me","on_click":"go","style":{"color":{"oklch":[0.5,0.2,30]},"text_align":"center"}}},
              {"div":{"style":{"absolute":true,"left":10,"top":40,"w":40,"h":20,
                       "bg":{"oklch":[0.7,0.1,140]},"opacity":0.5,"rotate":10}}},
              {"div":{"style":{"border":{"width":2,"color":{"oklch":[0.4,0.1,300]}},
                       "corners":[2,4,6,8],"h":30,"mt":50}}}
            ]}}}"#,
    ] {
        let desc = parse_doc(doc);
        let el = to_element(&desc, &theme).expect("original parses");
        let (emitted, warnings) = emit_description(&el);
        assert!(
            warnings.is_empty(),
            "expressible docs must emit warning-free, got: {warnings:?}"
        );
        let el2 = to_element(&emitted, &theme).unwrap_or_else(|e| {
            panic!(
                "emitted doc must re-parse cleanly, got {e:?}\n{}",
                serde_json::to_string_pretty(&emitted).unwrap()
            )
        });
        let el1 = to_element(&desc, &theme).expect("original parses");
        let a = render_element(el1, &theme, (360, 280));
        let b = render_element(el2, &theme, (360, 280));
        assert_eq!(
            a.as_raw(),
            b.as_raw(),
            "round-trip must render byte-identically for:\n{doc}"
        );
        drop(el);
    }
}

/// The headline use case: a builder-authored UI imported as data. No
/// handlers, literal styles → warning-free and pixel-faithful.
#[test]
fn builder_ui_imports_as_json() {
    let theme = Theme::light();
    let view = || -> fenestra_core::Element<()> {
        col().p(20.0).gap(10.0).w(300.0).h(200.0).children((
            text("Imported").size_px(22.0),
            row().gap(6.0).children((
                div()
                    .w(60.0)
                    .h(30.0)
                    .rounded(4.0)
                    .bg(fenestra_core::oklch(0.85, 0.05, 220.0)),
                div()
                    .grow()
                    .h(30.0)
                    .rounded(4.0)
                    .bg(fenestra_core::oklch(0.7, 0.1, 20.0)),
            )),
        ))
    };
    let (node, warnings) = emit_element(&view());
    assert!(warnings.is_empty(), "got: {warnings:?}");
    let desc = Description {
        schema: SCHEMA_V1.to_owned(),
        root: node,
        theme: None,
        state: fenestra_describe::state::StateMap::default(),
    };
    let el = to_element(&desc, &theme).expect("emitted builder UI parses");
    let a = render_element(view(), &theme, (340, 240));
    let b = render_element(el, &theme, (340, 240));
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "imported UI must render identically"
    );
}

/// Lossy content must be *reported*: a themed kit widget (button) lowers to
/// primitives with dynamic-style warnings, never silently.
#[test]
fn widget_lowering_reports_lossy_styles() {
    let theme = Theme::light();
    let doc = r#"{"schema":"fenestra/1","root":{"button":{"label":"Save","on_click":"save"}}}"#;
    let el = to_element(&parse_doc(doc), &theme).expect("button parses");
    let (_, warnings) = emit_description(&el);
    assert!(
        warnings
            .iter()
            .any(|w| w.message.contains("style closures")),
        "themed widget internals must be reported, got: {warnings:?}"
    );
}

/// Click intents round-trip through `Action`.
#[test]
fn intents_round_trip() {
    let theme = Theme::light();
    let doc = r#"{"schema":"fenestra/1","root":{"text":{"content":"Go","on_click":"navigate"}}}"#;
    let el = to_element(&parse_doc(doc), &theme).expect("parses");
    let (emitted, warnings) = emit_description(&el);
    assert!(warnings.is_empty(), "got: {warnings:?}");
    match &emitted.root {
        Node::Text(t) => assert_eq!(t.on_click.as_deref(), Some("navigate")),
        other => panic!("expected a text node, got {other:?}"),
    }
}

/// Out-of-range percent sizes clamp at parse time, so emitting one
/// warning-free would break the zero-warnings ⇒ identical-re-render
/// contract (2026-07-24 review): the emitter must report it.
#[test]
fn out_of_range_percent_sizes_warn_on_emit() {
    let el: fenestra_core::Element<fenestra_describe::state::Action> = div()
        .w(fenestra_core::Length::Pct(150.0))
        .child(text("wide"));
    let (_, warnings) = emit_description(&el);
    assert!(
        warnings.iter().any(|w| w.message.contains("0..=100")),
        "a >100% width cannot re-render identically and must warn, got: {warnings:?}"
    );
}

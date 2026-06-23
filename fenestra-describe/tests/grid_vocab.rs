//! Grid templates are authorable in the JSON vocabulary: fixed / `fr` columns,
//! and the responsive `repeat(auto-fit, minmax(...))`, lay out through the
//! describe boundary exactly as the builders do — and bad track strings degrade
//! with a path-pointed error.

use fenestra_core::{Frame, Theme, by};
use fenestra_describe::format::Description;
use fenestra_describe::inspect::build;
use fenestra_describe::parse::to_element_lenient;

fn desc(json: &str) -> Description {
    serde_json::from_str(json).expect("valid description")
}

fn x_of(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.x0
}
fn y_of(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.y0
}
fn w_of(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.width()
}

/// `grid-template-areas` authored in JSON: children placed only by `grid_area`
/// resolve to the right cells through the describe boundary (the holy-grail).
#[test]
fn json_template_areas_holy_grail() {
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{
            "style":{
                "grid_cols":["120px","1fr"],
                "grid_rows":["40px","1fr","30px"],
                "grid_template_areas":["header header","nav main","footer footer"],
                "w":600,"h":300
            },
            "children":[
                {"col":{"id":"header","style":{"grid_area":"header"}}},
                {"col":{"id":"nav","style":{"grid_area":"nav"}}},
                {"col":{"id":"main","style":{"grid_area":"main"}}},
                {"col":{"id":"footer","style":{"grid_area":"footer"}}}
            ]}}}"#,
    );
    let frame = build(&d, &Theme::light(), (600, 300)).expect("builds");
    assert!(
        (x_of(&frame, "main") - 120.0).abs() < 2.0,
        "main x = {}",
        x_of(&frame, "main")
    );
    assert!(
        (w_of(&frame, "header") - 600.0).abs() < 2.0,
        "header w = {}",
        w_of(&frame, "header")
    );
    assert!(
        (y_of(&frame, "footer") - 270.0).abs() < 2.0,
        "footer y = {}",
        y_of(&frame, "footer")
    );
}

/// Named grid lines author a span: `grid_col_lines: ["b","e"]` over six 100px
/// columns named `a..g` starts at line `b` (x = 100) and spans to line `e`.
#[test]
fn json_named_line_span() {
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{
            "style":{"grid_cols":["100px","100px","100px","100px","100px","100px"],
                     "grid_col_names":["a","b","c","d","e","f","g"],"w":600},
            "children":[{"col":{"id":"item","style":{"grid_col_lines":["b","e"]}}}]}}}"#,
    );
    let frame = build(&d, &Theme::light(), (600, 100)).expect("builds");
    assert!(
        (x_of(&frame, "item") - 100.0).abs() < 2.0,
        "item x = {}",
        x_of(&frame, "item")
    );
    assert!(
        (w_of(&frame, "item") - 300.0).abs() < 2.0,
        "item w = {}",
        w_of(&frame, "item")
    );
}

/// `"grid_cols": ["100px", "1fr"]` puts the second item at x = 100.
#[test]
fn json_fixed_and_fr_columns() {
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{
            "style":{"grid_cols":["100px","1fr"],"w":500},
            "children":[
                {"button":{"label":"a","id":"a"}},
                {"button":{"label":"b","id":"b"}}
            ]}}}"#,
    );
    let frame = build(&d, &Theme::light(), (500, 200)).expect("builds");
    assert!(
        (x_of(&frame, "a") - 0.0).abs() < 2.0,
        "a x = {}",
        x_of(&frame, "a")
    );
    assert!(
        (x_of(&frame, "b") - 100.0).abs() < 2.0,
        "b x = {}",
        x_of(&frame, "b")
    );
}

/// `repeat(auto-fit, minmax(180px, 1fr))` in a 600px container authors three
/// 200px columns — the responsive grid, from JSON.
#[test]
fn json_repeat_auto_fit_minmax() {
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{
            "style":{"grid_cols":[{"repeat":{"count":"auto-fit","tracks":[{"minmax":["180px","1fr"]}]}}],"w":600},
            "children":[
                {"button":{"label":"0","id":"c0"}},
                {"button":{"label":"1","id":"c1"}},
                {"button":{"label":"2","id":"c2"}}
            ]}}}"#,
    );
    let frame = build(&d, &Theme::light(), (600, 200)).expect("builds");
    assert!(
        (x_of(&frame, "c0") - 0.0).abs() < 2.0,
        "c0 x = {}",
        x_of(&frame, "c0")
    );
    assert!(
        (x_of(&frame, "c1") - 200.0).abs() < 2.0,
        "c1 x = {}",
        x_of(&frame, "c1")
    );
    assert!(
        (x_of(&frame, "c2") - 400.0).abs() < 2.0,
        "c2 x = {}",
        x_of(&frame, "c2")
    );
}

/// `repeat` with an integer count also works (three equal columns).
#[test]
fn json_repeat_count() {
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{
            "style":{"grid_cols":[{"repeat":{"count":3,"tracks":["1fr"]}}],"w":600},
            "children":[
                {"button":{"label":"0","id":"r0"}},
                {"button":{"label":"1","id":"r1"}},
                {"button":{"label":"2","id":"r2"}}
            ]}}}"#,
    );
    let frame = build(&d, &Theme::light(), (600, 200)).expect("builds");
    assert!((x_of(&frame, "r0") - 0.0).abs() < 2.0);
    assert!((x_of(&frame, "r1") - 200.0).abs() < 2.0);
    assert!((x_of(&frame, "r2") - 400.0).abs() < 2.0);
}

/// An unknown track string degrades to `1fr` and records one path-pointed error.
#[test]
fn unknown_track_string_records_error_lenient() {
    let d = desc(
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"grid_cols":["bogus","1fr"]},"children":[]}}}"#,
    );
    let (_, errs) = to_element_lenient(&d, &Theme::light());
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(
        errs[0].path.contains("grid_cols/0"),
        "path = {}",
        errs[0].path
    );
    // Strict parsing rejects the same description.
    assert!(
        fenestra_describe::parse::to_element(&d, &Theme::light()).is_err(),
        "strict parse rejects an unknown track"
    );
}

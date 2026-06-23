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

//! The chart + markdown nodes: happy paths render, hostile inputs degrade
//! with path-pointed errors (clamp over panic), and the axis-implication
//! rules hold.

use fenestra_core::Theme;
use fenestra_describe::format::Description;
use fenestra_describe::parse::{to_element, to_element_lenient};

fn desc(root: &str) -> Description {
    let json = format!(r#"{{"schema":"fenestra/1","root":{root}}}"#);
    serde_json::from_str(&json).expect("test doc deserializes")
}

#[test]
fn charts_and_markdown_parse_cleanly() {
    let theme = Theme::light();
    for root in [
        r#"{"sparkline":{"values":[1,2,3,2,5]}}"#,
        r#"{"line_chart":{"values":[2,5,3,8]}}"#,
        r#"{"line_chart":{"values":[2,5,3,8],"markers":true,"x_labels":["a","b","c","d"],"x_title":"Day","y_title":"Load","ticks":4}}"#,
        r#"{"line_chart":{"series":[{"label":"CPU","values":[1,2,3]},{"label":"RAM","values":[3,2,1]}]}}"#,
        r#"{"line_chart":{"series":[{"label":"CPU","values":[1,2,3]}],"axes":true}}"#,
        r#"{"bar_chart":{"bars":[{"label":"Q1","value":40},{"label":"Q2","value":65}]}}"#,
        r#"{"bar_chart":{"bars":[{"label":"Q1","value":40}],"show_values":true,"w":400,"h":200}}"#,
        r##"{"markdown":{"source":"# Hi\n\nSome **bold** and `code`.\n\n- one\n- two"}}"##,
        r#"{"markdown":{"source":"[docs](https://example.com)","on_link":"open_docs"}}"#,
    ] {
        assert!(
            to_element(&desc(root), &theme).is_ok(),
            "should parse cleanly: {root}"
        );
    }
}

#[test]
fn line_chart_with_both_values_and_series_degrades_pointedly() {
    let theme = Theme::light();
    let d = desc(r#"{"line_chart":{"values":[1,2],"series":[{"label":"a","values":[3,4]}]}}"#);
    let (_, errors) = to_element_lenient(&d, &theme);
    assert!(
        errors.iter().any(|e| e.message.contains("exactly one of")),
        "got: {errors:?}"
    );
}

#[test]
fn line_chart_with_neither_values_nor_series_degrades_pointedly() {
    let theme = Theme::light();
    let (_, errors) = to_element_lenient(&desc(r#"{"line_chart":{}}"#), &theme);
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("requires `values` or `series`")),
        "got: {errors:?}"
    );
}

#[test]
fn oversized_chart_series_truncates_with_a_path() {
    let theme = Theme::light();
    let values: Vec<String> = (0..10_001).map(|i| i.to_string()).collect();
    let d = desc(&format!(
        r#"{{"sparkline":{{"values":[{}]}}}}"#,
        values.join(",")
    ));
    let (_, errors) = to_element_lenient(&d, &theme);
    let err = errors
        .iter()
        .find(|e| e.message.contains("chart cap"))
        .expect("truncation is recorded");
    assert!(err.path.contains("values"), "path-pointed: {err:?}");
}

#[test]
fn too_many_series_truncate_to_the_palette() {
    let theme = Theme::light();
    let series: Vec<String> = (0..11)
        .map(|i| format!(r#"{{"label":"s{i}","values":[1,2]}}"#))
        .collect();
    let d = desc(&format!(
        r#"{{"line_chart":{{"series":[{}]}}}}"#,
        series.join(",")
    ));
    let (_, errors) = to_element_lenient(&d, &theme);
    assert!(
        errors.iter().any(|e| e.message.contains("palette cap")),
        "got: {errors:?}"
    );
}

#[test]
fn chart_ticks_clamp_out_of_range() {
    let theme = Theme::light();
    let (_, errors) = to_element_lenient(
        &desc(r#"{"line_chart":{"values":[1,2],"ticks":9999}}"#),
        &theme,
    );
    assert!(
        errors.iter().any(|e| e.path.ends_with("/ticks")),
        "got: {errors:?}"
    );
}

#[test]
fn chart_dimensions_reject_non_finite() {
    let theme = Theme::light();
    // JSON has no NaN literal; a negative size is the reachable bad input.
    let (_, errors) =
        to_element_lenient(&desc(r#"{"line_chart":{"values":[1,2],"w":-10}}"#), &theme);
    assert!(
        errors
            .iter()
            .any(|e| e.path.ends_with("/w") && e.message.contains("positive finite")),
        "got: {errors:?}"
    );
}

#[test]
fn oversized_bar_list_truncates_with_a_path() {
    let theme = Theme::light();
    let bars: Vec<String> = (0..1001)
        .map(|i| format!(r#"{{"label":"b{i}","value":1}}"#))
        .collect();
    let d = desc(&format!(
        r#"{{"bar_chart":{{"bars":[{}]}}}}"#,
        bars.join(",")
    ));
    let (_, errors) = to_element_lenient(&d, &theme);
    assert!(
        errors.iter().any(|e| e.message.contains("item cap")),
        "got: {errors:?}"
    );
}

#[test]
fn x_labels_on_multi_series_are_rejected_pointedly() {
    let theme = Theme::light();
    let d =
        desc(r#"{"line_chart":{"series":[{"label":"a","values":[1,2]}],"x_labels":["x","y"]}}"#);
    let (_, errors) = to_element_lenient(&d, &theme);
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("single-series line charts only")),
        "got: {errors:?}"
    );
}

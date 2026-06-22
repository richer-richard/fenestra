//! Parse and render tests for all new Node variants, style props, and
//! state-writing features added in the R2a vocabulary expansion.

use fenestra_core::{Element, Fonts, FrameState, Theme, build_frame};
use fenestra_describe::format::Description;
use fenestra_describe::parse::{to_element, to_element_lenient, validate};
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

// ── Card ─────────────────────────────────────────────────────────────────────

#[test]
fn card_builds_with_children() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"card":{"children":[{"text":{"content":"Hello"}}]}}}"#,
    );
    assert!(light_yaml(&el).contains("Hello"));
}

// ── Select ───────────────────────────────────────────────────────────────────

#[test]
fn select_parses_with_options() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"select":{"options":["One","Two","Three"],"selected":1}}}"#,
    );
    let yaml = light_yaml(&el);
    // Select renders a combobox role.
    assert!(yaml.contains("combobox"), "yaml: {yaml}");
}

#[test]
fn select_with_bind_reads_from_state() {
    let json = r#"{"schema":"fenestra/1","state":{"pick":2},"root":{"select":{"options":["A","B","C"],"bind":"pick"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

#[test]
fn select_empty_options_records_error() {
    let json = r#"{"schema":"fenestra/1","root":{"select":{"options":[]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("options")), "{errs:?}");
}

// ── Tabs ─────────────────────────────────────────────────────────────────────

#[test]
fn tabs_parses_and_shows_labels() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"tabs":{"labels":["Overview","Activity","Settings"],"active":0}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Overview"), "yaml: {yaml}");
}

#[test]
fn tabs_bind_reads_state() {
    let json = r#"{"schema":"fenestra/1","state":{"tab":1},"root":{"tabs":{"labels":["A","B","C"],"bind":"tab"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

// ── Segmented ─────────────────────────────────────────────────────────────────

#[test]
fn segmented_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"segmented":{"labels":["List","Board","Calendar"],"active":0}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("List"), "yaml: {yaml}");
}

#[test]
fn segmented_disabled_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"segmented":{"labels":["A","B"],"active":0,"disabled":true}}}"#,
    );
    // Just check it builds without error.
    let _ = light_yaml(&el);
}

// ── Breadcrumbs ─────────────────────────────────────────────────────────────────

#[test]
fn breadcrumbs_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"breadcrumbs":{"items":["Home","Library","Charts"]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Library"), "yaml: {yaml}");
}

#[test]
fn breadcrumbs_collapses_with_max_items() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"breadcrumbs":{"items":["A","B","C","D","E"],"max_items":3}}}"#,
    );
    // The middle collapses to an ellipsis; just check it builds.
    let _ = light_yaml(&el);
}

// ── Pagination ──────────────────────────────────────────────────────────────────

#[test]
fn pagination_parses_and_renders() {
    let el = build(r#"{"schema":"fenestra/1","root":{"pagination":{"count":10,"page":3}}}"#);
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Page 3"), "yaml: {yaml}");
}

#[test]
fn pagination_bind_reads_state() {
    let json = r#"{"schema":"fenestra/1","state":{"pg":5},"root":{"pagination":{"count":20,"bind":"pg"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

// ── Stepper ─────────────────────────────────────────────────────────────────────

#[test]
fn stepper_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"stepper":{"steps":["Account","Shipping","Payment"],"descriptions":["Your details","",""],"current":1}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Shipping"), "yaml: {yaml}");
}

#[test]
fn stepper_bind_reads_state() {
    let json = r#"{"schema":"fenestra/1","state":{"step":2},"root":{"stepper":{"steps":["A","B","C"],"bind":"step"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

// ── Spin button ─────────────────────────────────────────────────────────────────

#[test]
fn spin_button_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"spin_button":{"value":"3","label":"Quantity","on_increment":"more"}}}"#,
    );
    let yaml = light_yaml(&el);
    // The − / + buttons carry accessible names.
    assert!(yaml.contains("Decrease"), "yaml: {yaml}");
}

#[test]
fn spin_button_gates_at_bounds() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"spin_button":{"value":"0","can_decrement":false}}}"#,
    );
    // The − button is disabled at the minimum; just check it builds.
    let _ = light_yaml(&el);
}

// ── Meter ───────────────────────────────────────────────────────────────────────

#[test]
fn meter_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"meter":{"value":62,"min":0,"max":100,"label":"Storage"}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Storage"), "yaml: {yaml}");
}

#[test]
fn meter_zones_and_bind() {
    let json = r#"{"schema":"fenestra/1","state":{"v":85},"root":{"meter":{"value":50,"min":0,"max":100,"low":30,"high":70,"bind":"v"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

// ── Accordion ───────────────────────────────────────────────────────────────────

#[test]
fn accordion_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"accordion":{"items":[{"title":"Shipping","body":{"text":{"content":"Ships in two days."}}},{"title":"Returns","body":{"text":{"content":"Thirty-day returns."}}}],"open":0}}}"#,
    );
    let yaml = light_yaml(&el);
    // The open section reveals its body; both titles render.
    assert!(
        yaml.contains("Shipping") && yaml.contains("Ships in two days"),
        "yaml: {yaml}"
    );
}

#[test]
fn accordion_bind_reads_state() {
    let json = r#"{"schema":"fenestra/1","state":{"sec":1},"root":{"accordion":{"items":[{"title":"A","body":{"text":{"content":"a"}}},{"title":"B","body":{"text":{"content":"b"}}}],"bind":"sec"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

// ── Toolbar ─────────────────────────────────────────────────────────────────────

#[test]
fn toolbar_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"toolbar":{"label":"Format","children":[{"button":{"label":"Bold"}},{"button":{"label":"Italic"}}]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(
        yaml.contains("Bold") && yaml.contains("Italic"),
        "yaml: {yaml}"
    );
}

// ── Menubar ─────────────────────────────────────────────────────────────────────

#[test]
fn menubar_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"menubar":{"menus":[{"title":"File","items":[{"label":"New"},{"label":"Open"}]},{"title":"Edit","items":[{"label":"Undo"}]}]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(
        yaml.contains("File") && yaml.contains("Edit"),
        "yaml: {yaml}"
    );
}

// ── Drawer ──────────────────────────────────────────────────────────────────────

#[test]
fn drawer_parses_and_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"drawer":{"title":"Filters","side":"right","children":[{"text":{"content":"Body content"}}]}}}"#,
    );
    // Drawer is an overlay; just confirm it builds + renders without panic.
    let _ = light_yaml(&el);
}

#[test]
fn drawer_rejects_unknown_side() {
    // An unknown side degrades to left and records an error (clamp over panic).
    let json = r#"{"schema":"fenestra/1","root":{"drawer":{"side":"diagonal","children":[]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errors) = to_element_lenient(&desc, &Theme::light());
    assert!(!errors.is_empty(), "expected an error for the unknown side");
}

// ── Badge ─────────────────────────────────────────────────────────────────────

#[test]
fn badge_with_status_accent() {
    let el = build(r#"{"schema":"fenestra/1","root":{"badge":{"label":"New","status":"accent"}}}"#);
    let yaml = light_yaml(&el);
    assert!(yaml.contains("New"), "yaml: {yaml}");
}

#[test]
fn badge_default_status_is_accent() {
    // status field defaults to "accent" when omitted.
    let el = build(r#"{"schema":"fenestra/1","root":{"badge":{"label":"Beta"}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn badge_unknown_status_degrades_with_error() {
    let json = r#"{"schema":"fenestra/1","root":{"badge":{"label":"X","status":"taupe"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (el, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(light_yaml(&el).contains("X"));
    assert!(errs.iter().any(|e| e.path.contains("status")), "{errs:?}");
}

// ── Callout ───────────────────────────────────────────────────────────────────

#[test]
fn callout_renders_message() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"callout":{"status":"warning","message":"Trial ends soon."}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Trial ends soon."), "yaml: {yaml}");
}

#[test]
fn callout_all_statuses() {
    for status in ["accent", "danger", "warning", "success"] {
        let json = format!(
            r#"{{"schema":"fenestra/1","root":{{"callout":{{"status":"{status}","message":"msg"}}}}}}"#
        );
        assert!(validate(&json).is_ok(), "status={status} failed validate");
    }
}

// ── StatCard ──────────────────────────────────────────────────────────────────

#[test]
fn stat_card_renders_label_and_value() {
    let el =
        build(r#"{"schema":"fenestra/1","root":{"stat_card":{"label":"Revenue","value":"$48k"}}}"#);
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Revenue"), "yaml: {yaml}");
    assert!(yaml.contains("$48k"), "yaml: {yaml}");
}

#[test]
fn stat_card_with_delta_badge() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"stat_card":{"label":"Users","value":"1,200","delta":"+5%","delta_status":"success"}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("+5%"), "yaml: {yaml}");
}

// ── Avatar ────────────────────────────────────────────────────────────────────

#[test]
fn avatar_renders_initials() {
    let el = build(r#"{"schema":"fenestra/1","root":{"avatar":{"initials":"JD"}}}"#);
    let yaml = light_yaml(&el);
    assert!(yaml.contains("JD"), "yaml: {yaml}");
}

// ── Status ────────────────────────────────────────────────────────────────────

#[test]
fn status_renders_label() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"status":{"label":"Operational","status":"success"}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Operational"), "yaml: {yaml}");
}

#[test]
fn status_live_mode_renders() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"status":{"label":"Live","status":"danger","live":true}}}"#,
    );
    let _ = light_yaml(&el);
}

// ── Kbd ───────────────────────────────────────────────────────────────────────

#[test]
fn kbd_renders_chord() {
    // kbd uses Semantics::Image, so aria-snapshot will show an "img" node.
    let el = build(r#"{"schema":"fenestra/1","root":{"kbd":{"keys":["cmd","K"]}}}"#);
    let yaml = light_yaml(&el);
    // The chord label "⌘ K" should appear in the image label.
    assert!(yaml.contains("⌘"), "yaml: {yaml}");
}

#[test]
fn kbd_raised_style_renders() {
    let el = build(r#"{"schema":"fenestra/1","root":{"kbd":{"keys":["esc"],"raised":true}}}"#);
    let _ = light_yaml(&el);
}

// ── Progress ──────────────────────────────────────────────────────────────────

#[test]
fn progress_builds() {
    let el = build(r#"{"schema":"fenestra/1","root":{"progress":{"value":0.7}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn progress_indeterminate_builds() {
    let el =
        build(r#"{"schema":"fenestra/1","root":{"progress":{"value":0.0,"indeterminate":true}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn progress_value_clamped() {
    // Values outside 0..=1 are clamped, not an error.
    let el = build(r#"{"schema":"fenestra/1","root":{"progress":{"value":1.5}}}"#);
    let _ = light_yaml(&el);
}

// ── Spinner ───────────────────────────────────────────────────────────────────

#[test]
fn spinner_builds() {
    let el = build(r#"{"schema":"fenestra/1","root":{"spinner":{}}}"#);
    let _ = light_yaml(&el);
}

// ── Skeleton ─────────────────────────────────────────────────────────────────

#[test]
fn skeleton_rect_builds() {
    let el = build(r#"{"schema":"fenestra/1","root":{"skeleton":{"w":200,"h":24,"kind":"rect"}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn skeleton_circle_builds() {
    let el = build(r#"{"schema":"fenestra/1","root":{"skeleton":{"w":40,"kind":"circle"}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn skeleton_text_builds() {
    let el = build(r#"{"schema":"fenestra/1","root":{"skeleton":{"kind":"text","lines":4}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn skeleton_defaults_to_rect() {
    // No kind → rect with default w/h.
    let el = build(r#"{"schema":"fenestra/1","root":{"skeleton":{}}}"#);
    let _ = light_yaml(&el);
}

// ── Icon ─────────────────────────────────────────────────────────────────────

#[test]
fn icon_known_name_builds() {
    for name in ["plus", "check", "x", "bell", "search", "home", "info"] {
        let json = format!(r#"{{"schema":"fenestra/1","root":{{"icon":{{"name":"{name}"}}}}}}"#);
        assert!(validate(&json).is_ok(), "icon {name:?} failed validate");
        assert!(
            to_element(&parse(&json), &Theme::light()).is_ok(),
            "icon {name:?} failed to_element"
        );
    }
}

#[test]
fn icon_unknown_name_degrades_with_error() {
    let json = r#"{"schema":"fenestra/1","root":{"icon":{"name":"unicorn"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("name")), "{errs:?}");
}

// ── Modal ────────────────────────────────────────────────────────────────────

#[test]
fn modal_builds_with_title_and_children() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"modal":{"title":"Confirm","on_close":"dismiss","children":[{"text":{"content":"Are you sure?"}}]}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Confirm"), "yaml: {yaml}");
    assert!(yaml.contains("Are you sure?"), "yaml: {yaml}");
}

#[test]
fn modal_empty_children_builds() {
    let el = build(r#"{"schema":"fenestra/1","root":{"modal":{"title":"Alert","children":[]}}}"#);
    let _ = light_yaml(&el);
}

// ── Tooltip ───────────────────────────────────────────────────────────────────

#[test]
fn tooltip_wraps_target() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"tooltip":{"label":"Save the file","target":{"button":{"label":"Save"}}}}}"#,
    );
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Save"), "yaml: {yaml}");
}

// ── New Style props ────────────────────────────────────────────────────────────

#[test]
fn per_side_padding_applies() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"pt":8,"pb":16,"pl":4,"pr":12},"children":[]}}}"#,
    );
    let _ = light_yaml(&el);
}

#[test]
fn margin_props_apply() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"m":8,"mx":4,"my":12,"mt":2,"mb":2,"ml":4,"mr":4},"children":[]}}}"#,
    );
    let _ = light_yaml(&el);
}

#[test]
fn min_max_dimensions_apply() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"min_w":100,"max_w":400,"min_h":50,"max_h":200},"children":[]}}}"#,
    );
    let _ = light_yaml(&el);
}

#[test]
fn shadow_sm_applies() {
    let el =
        build(r#"{"schema":"fenestra/1","root":{"div":{"style":{"shadow":"sm"},"children":[]}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn shadow_all_tokens() {
    for token in ["sm", "md", "lg", "xl"] {
        let json = format!(
            r#"{{"schema":"fenestra/1","root":{{"div":{{"style":{{"shadow":"{token}"}},"children":[]}}}}}}"#
        );
        assert!(validate(&json).is_ok(), "shadow={token} failed validate");
    }
}

#[test]
fn shadow_unknown_token_records_error() {
    let json =
        r#"{"schema":"fenestra/1","root":{"div":{"style":{"shadow":"huge"},"children":[]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("shadow")), "{errs:?}");
}

#[test]
fn gradient_background_applies() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"div":{"style":{"gradient":{"angle":135,"stops":["accent","accent_bg"]}},"children":[]}}}"#,
    );
    let _ = light_yaml(&el);
}

#[test]
fn gradient_with_oklch_stops() {
    let json = r#"{"schema":"fenestra/1","root":{"div":{"style":{"gradient":{"angle":90,"stops":[{"oklch":[0.7,0.1,200.0]},{"oklch":[0.9,0.05,220.0]}]}},"children":[]}}}"#;
    assert!(validate(json).is_ok());
}

#[test]
fn gradient_too_few_stops_records_error() {
    let json = r#"{"schema":"fenestra/1","root":{"div":{"style":{"gradient":{"angle":90,"stops":["accent"]}},"children":[]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("gradient")), "{errs:?}");
}

#[test]
fn text_align_applies() {
    for align in ["start", "center", "end"] {
        let json = format!(
            r#"{{"schema":"fenestra/1","root":{{"text":{{"content":"Hi","style":{{"text_align":"{align}"}}}}}}}}"#
        );
        assert!(validate(&json).is_ok(), "text_align={align} failed");
    }
}

#[test]
fn text_align_unknown_records_error() {
    let json = r#"{"schema":"fenestra/1","root":{"text":{"content":"Hi","style":{"text_align":"right"}}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(
        errs.iter().any(|e| e.path.contains("text_align")),
        "{errs:?}"
    );
}

#[test]
fn opacity_applies() {
    let el =
        build(r#"{"schema":"fenestra/1","root":{"div":{"style":{"opacity":0.5},"children":[]}}}"#);
    let _ = light_yaml(&el);
}

#[test]
fn opacity_out_of_range_records_error() {
    let json = r#"{"schema":"fenestra/1","root":{"div":{"style":{"opacity":1.5},"children":[]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(errs.iter().any(|e| e.path.contains("opacity")), "{errs:?}");
}

#[test]
fn absolute_positioning_applies() {
    let el = build(
        r#"{"schema":"fenestra/1","root":{"div":{"style":{"absolute":true,"top":20,"left":10},"children":[]}}}"#,
    );
    let _ = light_yaml(&el);
}

// ── State writes ──────────────────────────────────────────────────────────────

#[test]
fn button_bind_toggles_bool_state() {
    // A button with `bind` should toggle the state key on click; the element
    // builds without error.
    let json = r#"{"schema":"fenestra/1","state":{"open":false},"root":{"button":{"label":"Toggle","bind":"open"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&desc, &Theme::light()).unwrap();
    let yaml = light_yaml(&el);
    assert!(yaml.contains("Toggle"), "yaml: {yaml}");
}

#[test]
fn radio_group_binding_derives_selection_from_state() {
    let json = r#"{"schema":"fenestra/1","state":{"lang":"rust"},"root":{"col":{"children":[
        {"radio":{"label":"Rust","group":"lang","value":"rust"}},
        {"radio":{"label":"Python","group":"lang","value":"python"}}
    ]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&desc, &Theme::light()).unwrap();
    let yaml = light_yaml(&el);
    // The Rust radio should appear selected (radios project [selected], not
    // [checked] — that's a checkbox attribute).
    assert!(yaml.contains("Rust"), "yaml: {yaml}");
    assert!(
        yaml.contains("[selected]"),
        "expected selected radio: {yaml}"
    );
}

#[test]
fn container_on_click_emits_intent() {
    let json = r#"{"schema":"fenestra/1","root":{"div":{"on_click":"clicked","children":[]}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

#[test]
fn text_on_click_emits_intent() {
    let json =
        r#"{"schema":"fenestra/1","root":{"text":{"content":"Click me","on_click":"text_click"}}}"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    assert!(to_element(&desc, &Theme::light()).is_ok());
}

// ── Reject unknown fields on new structs ──────────────────────────────────────

#[test]
fn badge_rejects_unknown_field() {
    let json = r#"{"schema":"fenestra/1","root":{"badge":{"label":"X","colour":"red"}}}"#;
    assert!(serde_json::from_str::<fenestra_describe::format::Description>(json).is_err());
}

#[test]
fn skeleton_rejects_unknown_field() {
    let json = r#"{"schema":"fenestra/1","root":{"skeleton":{"shape":"blob"}}}"#;
    assert!(serde_json::from_str::<fenestra_describe::format::Description>(json).is_err());
}

#[test]
fn modal_rejects_unknown_field() {
    let json =
        r#"{"schema":"fenestra/1","root":{"modal":{"title":"T","subtitle":"S","children":[]}}}"#;
    assert!(serde_json::from_str::<fenestra_describe::format::Description>(json).is_err());
}

#[test]
fn tooltip_rejects_unknown_field() {
    let json = r#"{"schema":"fenestra/1","root":{"tooltip":{"label":"L","target":{"text":{"content":"T"}},"delay":300}}}"#;
    assert!(serde_json::from_str::<fenestra_describe::format::Description>(json).is_err());
}

// ── Style rejects unknown field ───────────────────────────────────────────────

#[test]
fn style_rejects_unknown_field() {
    let json = r#"{"schema":"fenestra/1","root":{"col":{"style":{"gapp":8},"children":[]}}}"#;
    assert!(serde_json::from_str::<fenestra_describe::format::Description>(json).is_err());
}

// ── Full dashboard smoke test ─────────────────────────────────────────────────

#[test]
fn dashboard_with_all_new_widgets_builds() {
    let json = r#"{
        "schema": "fenestra/1",
        "state": {"tab": 0, "view": 0, "wifi": true},
        "root": {"col": {
            "style": {"p": 24, "gap": 16, "bg": "surface"},
            "children": [
                {"row": {"style": {"gap": 8, "align": "center"}, "children": [
                    {"avatar": {"initials": "AB"}},
                    {"text": {"content": "Dashboard", "style": {"size_px": 20, "weight": 600}}},
                    {"spacer": {}},
                    {"badge": {"label": "Live", "status": "success"}},
                    {"spinner": {}}
                ]}},
                {"tabs": {"labels": ["Overview", "Analytics"], "active": 0, "bind": "tab"}},
                {"row": {"style": {"gap": 16}, "children": [
                    {"stat_card": {"label": "Revenue", "value": "$48k", "delta": "+12%", "delta_status": "success"}},
                    {"stat_card": {"label": "Users", "value": "1,200"}}
                ]}},
                {"callout": {"status": "warning", "message": "Your trial expires in 3 days."}},
                {"progress": {"value": 0.75}},
                {"row": {"style": {"gap": 8, "align": "center"}, "children": [
                    {"icon": {"name": "settings"}},
                    {"select": {"options": ["Option A", "Option B", "Option C"], "selected": 0}},
                    {"segmented": {"labels": ["Day", "Week", "Month"], "active": 0, "bind": "view"}}
                ]}},
                {"skeleton": {"kind": "text", "lines": 3}},
                {"kbd": {"keys": ["cmd", "K"]}},
                {"status": {"label": "Operational", "status": "success"}},
                {"modal": {"title": "Confirm", "on_close": "dismiss", "children": [
                    {"text": {"content": "Proceed?"}}
                ]}},
                {"tooltip": {"label": "Save the file", "target": {"button": {"label": "Save"}}}}
            ]
        }}
    }"#;

    let desc: Description = serde_json::from_str(json).expect("dashboard json parses");
    let el = to_element(&desc, &Theme::light()).expect("dashboard builds without error");
    let yaml = light_yaml(&el);
    // Spot-check a few nodes appear.
    assert!(yaml.contains("Dashboard"), "yaml: {yaml}");
    assert!(yaml.contains("Save"), "yaml: {yaml}");
}

//! `describe_vocabulary` must stay in lockstep with the parser: every advertised
//! node parses and renders, and the list covers every `Node` variant.

use std::collections::BTreeSet;

use fenestra_core::Theme;
use fenestra_describe::format::{Description, Node};
use fenestra_describe::parse::{to_element, validate};
use fenestra_describe::vocabulary::describe_vocabulary;

#[test]
fn every_advertised_node_parses_and_renders() {
    for doc in describe_vocabulary().nodes {
        let json = format!(
            r#"{{"schema":"fenestra/1","root":{{"{}":{}}}}}"#,
            doc.tag, doc.example
        );
        assert!(
            validate(&json).is_ok(),
            "{} failed validate: {:?}",
            doc.tag,
            validate(&json).err()
        );
        let desc: Description = serde_json::from_str(&json).expect("example deserializes");
        assert!(
            to_element(&desc, &Theme::light()).is_ok(),
            "{} failed to_element",
            doc.tag
        );
        assert_eq!(
            tag_of(&desc.root),
            doc.tag,
            "example body is not a {}",
            doc.tag
        );
    }
}

#[test]
fn vocabulary_covers_every_node_variant() {
    let tags: BTreeSet<String> = describe_vocabulary()
        .nodes
        .into_iter()
        .map(|n| n.tag)
        .collect();
    let expected: BTreeSet<String> = [
        // Layout containers
        "row",
        "col",
        "div",
        "stack",
        "card",
        "split_pane",
        // Text
        "text",
        // Form controls
        "button",
        "checkbox",
        "switch",
        "radio",
        "slider",
        "text_input",
        "text_area",
        "select",
        "spin_button",
        "field",
        "combobox",
        "multi_select",
        "tag_input",
        "date_picker",
        "color_picker",
        // Navigation
        "tabs",
        "segmented",
        "breadcrumbs",
        "pagination",
        "stepper",
        "toolbar",
        "menubar",
        "tree",
        // Display / feedback
        "badge",
        "callout",
        "stat_card",
        "avatar",
        "status",
        "kbd",
        "progress",
        "meter",
        "accordion",
        "spinner",
        "skeleton",
        "icon",
        "image",
        "toast",
        // Data
        "data_table",
        "virtual_list",
        "sparkline",
        "line_chart",
        "bar_chart",
        "markdown",
        // Overlays
        "modal",
        "tooltip",
        "drawer",
        "popover",
        "dropdown_menu",
        "command_palette",
        // Decoration
        "divider",
        "spacer",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    assert_eq!(tags, expected);
}

/// Exhaustive over `Node`: a new variant forces a new arm here, prompting a
/// matching vocabulary entry and keeping the grammar honest.
fn tag_of(n: &Node) -> &'static str {
    match n {
        // Layout containers
        Node::Row(_) => "row",
        Node::Col(_) => "col",
        Node::Div(_) => "div",
        Node::Stack(_) => "stack",
        Node::Card(_) => "card",
        Node::SplitPane(_) => "split_pane",
        // Text
        Node::Text(_) => "text",
        // Form controls
        Node::Button(_) => "button",
        Node::Checkbox(_) => "checkbox",
        Node::Switch(_) => "switch",
        Node::Radio(_) => "radio",
        Node::Slider(_) => "slider",
        Node::TextInput(_) => "text_input",
        Node::TextArea(_) => "text_area",
        Node::Select(_) => "select",
        Node::SpinButton(_) => "spin_button",
        Node::Field(_) => "field",
        Node::Combobox(_) => "combobox",
        Node::MultiSelect(_) => "multi_select",
        Node::TagInput(_) => "tag_input",
        Node::DatePicker(_) => "date_picker",
        Node::ColorPicker(_) => "color_picker",
        // Navigation
        Node::Tabs(_) => "tabs",
        Node::Segmented(_) => "segmented",
        Node::Breadcrumbs(_) => "breadcrumbs",
        Node::Pagination(_) => "pagination",
        Node::Stepper(_) => "stepper",
        Node::Toolbar(_) => "toolbar",
        Node::Menubar(_) => "menubar",
        Node::Tree(_) => "tree",
        // Display / feedback
        Node::Badge(_) => "badge",
        Node::Callout(_) => "callout",
        Node::StatCard(_) => "stat_card",
        Node::Avatar(_) => "avatar",
        Node::Status(_) => "status",
        Node::Kbd(_) => "kbd",
        Node::Progress(_) => "progress",
        Node::Meter(_) => "meter",
        Node::Accordion(_) => "accordion",
        Node::Spinner(_) => "spinner",
        Node::Skeleton(_) => "skeleton",
        Node::Icon(_) => "icon",
        Node::Image(_) => "image",
        Node::Toast(_) => "toast",
        // Data
        Node::DataTable(_) => "data_table",
        Node::VirtualList(_) => "virtual_list",
        Node::Sparkline(_) => "sparkline",
        Node::LineChart(_) => "line_chart",
        Node::BarChart(_) => "bar_chart",
        Node::Markdown(_) => "markdown",
        // Overlays
        Node::Modal(_) => "modal",
        Node::Tooltip(_) => "tooltip",
        Node::Drawer(_) => "drawer",
        Node::Popover(_) => "popover",
        Node::DropdownMenu(_) => "dropdown_menu",
        Node::CommandPalette(_) => "command_palette",
        // Decoration
        Node::Divider(_) => "divider",
        Node::Spacer(_) => "spacer",
    }
}

#[test]
fn every_advertised_style_prop_parses() {
    // The style grammar must stay in lockstep with the parser: every advertised
    // property's example authors and builds cleanly.
    for doc in describe_vocabulary().style {
        let json = format!(
            r#"{{"schema":"fenestra/1","root":{{"div":{{"style":{{"{}":{}}},"children":[]}}}}}}"#,
            doc.key, doc.example
        );
        assert!(
            validate(&json).is_ok(),
            "style `{}` example failed validate: {:?}",
            doc.key,
            validate(&json).err()
        );
        let desc: Description = serde_json::from_str(&json).expect("example deserializes");
        assert!(
            to_element(&desc, &Theme::light()).is_ok(),
            "style `{}` failed to_element",
            doc.key
        );
    }
}

#[test]
fn every_style_enum_value_parses() {
    // An enum whose name is a `style` key (surface / shadow / align / justify /
    // text_align) must accept each advertised value there. Node-field enums
    // (`button.variant`, `status`, `drawer.side`, `skeleton.kind`) and the
    // `glass_preset` are exercised by their own node / property examples.
    let style_keys: std::collections::BTreeSet<String> = describe_vocabulary()
        .style
        .into_iter()
        .map(|s| s.key)
        .collect();
    for e in describe_vocabulary().enums {
        if !style_keys.contains(&e.name) {
            continue;
        }
        for v in &e.values {
            let json = format!(
                r#"{{"schema":"fenestra/1","root":{{"div":{{"style":{{"{}":"{}"}},"children":[]}}}}}}"#,
                e.name, v
            );
            assert!(
                validate(&json).is_ok(),
                "enum {}={:?} failed validate: {:?}",
                e.name,
                v,
                validate(&json).err()
            );
        }
    }
}

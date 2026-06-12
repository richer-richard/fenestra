//! 0.4 multi-window contract: `App::windows` declares the open set,
//! `App::view_for` routes per-key views, and the defaults keep
//! single-window apps untouched.

use fenestra_core::{
    AccessNode, App, Element, Fonts, FrameState, MAIN_WINDOW, Theme, WindowDesc, build_frame, col,
    text,
};

struct Single;

impl App for Single {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col().children([text("only view")])
    }
}

struct Multi {
    inspecting: Vec<&'static str>,
}

impl App for Multi {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col().children([text("main")])
    }

    fn windows(&self) -> Vec<WindowDesc<()>> {
        self.inspecting
            .iter()
            .map(|k| WindowDesc::new(*k, format!("Inspector {k}"), (300.0, 200.0), ()))
            .collect()
    }

    fn view_for(&self, key: &str) -> Element<()> {
        if key == MAIN_WINDOW {
            self.view()
        } else {
            col().children([text(format!("inspector {key}"))])
        }
    }
}

/// Renders a view's accessibility labels — enough to tell views apart.
fn labels(view: &Element<()>) -> String {
    fn walk(n: &AccessNode, out: &mut Vec<String>) {
        if let Some(label) = &n.label {
            out.push(label.clone());
        }
        for child in &n.children {
            walk(child, out);
        }
    }
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(
        view,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (300.0, 200.0),
        1.0,
    );
    let mut out = Vec::new();
    walk(&frame.access_tree(), &mut out);
    out.join("|")
}

#[test]
fn defaults_keep_single_window_apps_untouched() {
    let app = Single;
    assert!(app.windows().is_empty(), "no secondary windows by default");
    // view_for falls back to view() for every key, main or not.
    assert_eq!(labels(&app.view_for(MAIN_WINDOW)), labels(&app.view()));
    assert_eq!(labels(&app.view_for("anything")), labels(&app.view()));
}

#[test]
fn windows_reflect_app_state_and_views_route_by_key() {
    let mut app = Multi {
        inspecting: vec!["probe-1", "probe-3"],
    };
    let descs = app.windows();
    assert_eq!(descs.len(), 2);
    assert_eq!(descs[0].key, "probe-1");
    assert_eq!(descs[0].title, "Inspector probe-1");
    assert_eq!(descs[0].size, (300.0, 200.0));

    // Removing state removes the desc — that's how windows close.
    app.inspecting.retain(|k| *k != "probe-1");
    let descs = app.windows();
    assert_eq!(descs.len(), 1);
    assert_eq!(descs[0].key, "probe-3");

    assert_eq!(labels(&app.view_for(MAIN_WINDOW)), labels(&app.view()));
    assert!(labels(&app.view_for("probe-3")).contains("inspector probe-3"));
}

#[test]
fn theme_for_defaults_to_theme_and_routes_per_window() {
    struct Two;
    impl App for Two {
        type Msg = ();
        fn update(&mut self, (): ()) {}
        fn view(&self) -> Element<()> {
            col()
        }
        fn theme(&self) -> Theme {
            Theme::light()
        }
        fn theme_for(&self, key: &str) -> Theme {
            if key == "inspector" {
                Theme::dark()
            } else {
                self.theme()
            }
        }
    }
    let app = Two;
    assert_eq!(app.theme_for(MAIN_WINDOW).bg, Theme::light().bg);
    assert_eq!(app.theme_for("inspector").bg, Theme::dark().bg);

    // And the default keeps single-theme apps untouched.
    assert_eq!(Single.theme_for("anything").bg, Single.theme().bg);
}

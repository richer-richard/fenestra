//! Kit v2 widgets, driven semantically: tree toggling/selection,
//! command-palette filter + Enter, data-table sort/select, split-pane
//! resize.

use fenestra_core::{App, Element, Key, KeyInput, Semantics, Theme, by, col, text};
use fenestra_kit::{TreeNode, command_palette, data_table, split_pane, tree_view};
use fenestra_shell::Harness;

// ---------------------------------------------------------------- tree

#[derive(Default)]
struct Files {
    expanded: Vec<String>,
    selected: Option<String>,
}

#[derive(Clone)]
enum FsMsg {
    Toggle(String),
    Select(String),
}

impl App for Files {
    type Msg = FsMsg;

    fn update(&mut self, msg: FsMsg) {
        match msg {
            FsMsg::Toggle(id) => {
                if self.expanded.contains(&id) {
                    self.expanded.retain(|e| e != &id);
                } else {
                    self.expanded.push(id);
                }
            }
            FsMsg::Select(id) => self.selected = Some(id),
        }
    }

    fn view(&self) -> Element<FsMsg> {
        col().p(8.0).children([Element::from(
            tree_view([
                TreeNode::new("src", "src").children([
                    TreeNode::new("main", "main.rs"),
                    TreeNode::new("lib", "lib.rs"),
                ]),
                TreeNode::new("readme", "README.md"),
            ])
            .expanded(self.expanded.iter().cloned())
            .selected(self.selected.clone())
            .on_toggle(|id| FsMsg::Toggle(id.to_owned()))
            .on_select(|id| FsMsg::Select(id.to_owned())),
        )])
    }
}

#[test]
fn tree_toggles_branches_and_selects_leaves() {
    let mut h = Harness::new(Files::default(), Theme::light(), (300, 300));
    assert!(
        h.query(&by::role(Semantics::Button).name("main.rs"))
            .is_none(),
        "collapsed"
    );

    h.click(&by::role(Semantics::Button).name("src"));
    assert!(
        h.query(&by::role(Semantics::Button).name("main.rs"))
            .is_some(),
        "expanded"
    );

    h.click(&by::role(Semantics::Button).name("main.rs"));
    assert_eq!(h.app().selected.as_deref(), Some("main"));

    // Left arrow on the focused branch collapses it.
    h.focus(&by::role(Semantics::Button).name("src"));
    h.key(KeyInput::plain(Key::ArrowLeft));
    assert!(
        h.query(&by::role(Semantics::Button).name("main.rs"))
            .is_none(),
        "collapsed again"
    );
}

// ------------------------------------------------------------- palette

#[derive(Default)]
struct Editor {
    open: bool,
    query: String,
    ran: Option<&'static str>,
}

#[derive(Clone)]
enum EdMsg {
    Open,
    Close,
    Query(String),
    Run(&'static str),
}

impl App for Editor {
    type Msg = EdMsg;

    fn update(&mut self, msg: EdMsg) {
        match msg {
            EdMsg::Open => self.open = true,
            EdMsg::Close => {
                self.open = false;
                self.query.clear();
            }
            EdMsg::Query(q) => self.query = q,
            EdMsg::Run(what) => {
                self.ran = Some(what);
                self.open = false;
            }
        }
    }

    fn view(&self) -> Element<EdMsg> {
        col().p(8.0).children((
            fenestra_kit::button("Commands").on_click(EdMsg::Open),
            command_palette(
                &self.query,
                self.open,
                [
                    ("Format document", EdMsg::Run("format")),
                    ("Find references", EdMsg::Run("references")),
                    ("Rename symbol", EdMsg::Run("rename")),
                ],
            )
            .on_input(EdMsg::Query)
            .on_close(EdMsg::Close)
            .id("palette"),
        ))
    }
}

#[test]
fn palette_filters_and_enter_runs_the_first_match() {
    let mut h = Harness::new(Editor::default(), Theme::light(), (600, 400));
    h.click(&by::role(Semantics::Button).name("Commands"));
    // The input autofocused; typing filters.
    h.type_text("rena");
    assert!(
        h.query(&by::label("Format document")).is_none(),
        "filtered out"
    );
    h.key(KeyInput::plain(Key::Enter));
    assert_eq!(h.app().ran, Some("rename"));
    assert!(!h.app().open, "running a command closed it");
}

#[test]
fn palette_escape_closes() {
    let mut h = Harness::new(Editor::default(), Theme::light(), (600, 400));
    h.click(&by::role(Semantics::Button).name("Commands"));
    h.key(KeyInput::plain(Key::Escape));
    assert!(!h.app().open);
    assert!(h.app().ran.is_none());
}

// ----------------------------------------------------------- data table

struct Crew {
    sort: (usize, bool),
    selected: Option<usize>,
}

#[derive(Clone)]
enum CrewMsg {
    Sort(usize),
    Pick(usize),
}

impl App for Crew {
    type Msg = CrewMsg;

    fn update(&mut self, msg: CrewMsg) {
        match msg {
            CrewMsg::Sort(col) => {
                let asc = if self.sort.0 == col {
                    !self.sort.1
                } else {
                    true
                };
                self.sort = (col, asc);
            }
            CrewMsg::Pick(row) => self.selected = Some(row),
        }
    }

    fn view(&self) -> Element<CrewMsg> {
        col().p(8.0).children([Element::from(
            data_table(
                ["name", "role"],
                vec![
                    vec!["Ripley".into(), "warrant officer".into()],
                    vec!["Dallas".into(), "captain".into()],
                ],
            )
            .sort(self.sort.0, self.sort.1)
            .selected(self.selected)
            .on_sort(CrewMsg::Sort)
            .on_select(CrewMsg::Pick),
        )])
    }
}

#[test]
fn data_table_sorts_and_selects() {
    let mut h = Harness::new(
        Crew {
            sort: (0, true),
            selected: None,
        },
        Theme::light(),
        (420, 300),
    );
    // The sorted header shows the indicator.
    assert!(h.query(&by::label_contains("name ▲")).is_some());

    // Clicking the same header flips direction; another column resets.
    h.click(&by::role(Semantics::Button).name("sort by name"));
    assert_eq!(h.app().sort, (0, false));
    h.click(&by::role(Semantics::Button).name("sort by role"));
    assert_eq!(h.app().sort, (1, true));

    h.click(&by::label("row 1"));
    assert_eq!(h.app().selected, Some(1));
}

// ----------------------------------------------------------- split pane

struct Workbench {
    fraction: f32,
}

#[derive(Clone)]
struct Resize(f32);

impl App for Workbench {
    type Msg = Resize;

    fn update(&mut self, Resize(f): Resize) {
        self.fraction = f;
    }

    fn view(&self) -> Element<Resize> {
        col().w_full().h_full().children([Element::from(
            split_pane(
                self.fraction,
                col().children([text("left")]),
                col().children([text("right")]),
            )
            .on_resize(Resize)
            .id("split"),
        )])
    }
}

#[test]
fn split_pane_drag_emits_the_new_fraction() {
    let mut h = Harness::new(Workbench { fraction: 0.5 }, Theme::light(), (400, 200));
    // Press on the divider (at 50% of 400 = x≈200) and drag right.
    h.input(fenestra_core::InputEvent::PointerMove { x: 201.0, y: 100.0 });
    h.input(fenestra_core::InputEvent::PointerDown);
    h.input(fenestra_core::InputEvent::PointerMove { x: 300.0, y: 100.0 });
    h.input(fenestra_core::InputEvent::PointerUp);
    assert!(
        (h.app().fraction - 0.75).abs() < 0.02,
        "fraction followed the drag: {}",
        h.app().fraction
    );
}

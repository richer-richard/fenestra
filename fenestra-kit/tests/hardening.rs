//! Robustness regressions from the security/hardening audit: widget
//! callbacks must never emit values the host cannot use safely.

use fenestra_core::{
    AccessNode, App, Element, Fonts, FrameState, Key, KeyInput, SP4, Semantics, Theme, build_frame,
    col,
};
use fenestra_kit::{pagination, select, text_input};
use fenestra_shell::{SyntheticEvent, render_app};

struct PickEmpty {
    picked: Option<usize>,
}

#[derive(Clone)]
enum PickMsg {
    Pick(usize),
}

impl App for PickEmpty {
    type Msg = PickMsg;

    fn update(&mut self, msg: PickMsg) {
        match msg {
            PickMsg::Pick(i) => self.picked = Some(i),
        }
    }

    fn view(&self) -> Element<PickMsg> {
        col()
            .p(SP4)
            .items_start()
            .children([select(0, Vec::<String>::new())
                .on_change(PickMsg::Pick)
                .id("empty-select")])
    }
}

struct OrgName {
    value: String,
}

#[derive(Clone)]
enum NameMsg {
    Set(String),
}

impl App for OrgName {
    type Msg = NameMsg;

    fn update(&mut self, msg: NameMsg) {
        match msg {
            NameMsg::Set(s) => self.value = s,
        }
    }

    fn view(&self) -> Element<NameMsg> {
        col()
            .p(SP4)
            .items_start()
            .children([text_input(&self.value).on_input(NameMsg::Set).id("name")])
    }
}

/// Control characters arriving as `Key::Char` (the keyboard path) must be
/// filtered exactly like the text-commit and paste paths already are: a
/// single-line input must never contain `\r`, `\n`, `\t`, or DEL.
#[test]
fn control_characters_never_enter_text_input() {
    let theme = Theme::light();
    let mut app = OrgName {
        value: String::new(),
    };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Char('\r'))),
            SyntheticEvent::Key(KeyInput::plain(Key::Char('\n'))),
            SyntheticEvent::Key(KeyInput::plain(Key::Char('\t'))),
            SyntheticEvent::Key(KeyInput::plain(Key::Char('\u{7f}'))),
            SyntheticEvent::Text("ok".into()),
        ],
        (300, 100),
        &theme,
    );
    assert_eq!(
        app.value, "ok",
        "control characters must be filtered from keyboard input"
    );
}

/// A select with zero options must never emit an index: the documented
/// contract is that `on_change` receives a valid index into `options`, and
/// hosts index into their data with it.
#[test]
fn empty_select_never_emits_an_index() {
    let theme = Theme::light();
    let mut app = PickEmpty { picked: None };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Home)),
            SyntheticEvent::Key(KeyInput::plain(Key::End)),
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowDown)),
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowUp)),
            SyntheticEvent::Key(KeyInput::plain(Key::Char('a'))),
        ],
        (300, 100),
        &theme,
    );
    assert_eq!(
        app.picked, None,
        "an empty select emitted an option index that does not exist"
    );
}

/// Counts the focusable page/arrow cells (the `Button`-roled nodes) a frame
/// exposes — the pagination strip's materialized cells.
fn count_buttons(node: &AccessNode) -> usize {
    let here = usize::from(matches!(node.semantics, Some(Semantics::Button)));
    here + node.children.iter().map(count_buttons).sum::<usize>()
}

/// Collects every accessible label in the tree, so a test can assert a
/// particular page cell was (or was not) materialized by name.
fn collect_labels(node: &AccessNode, out: &mut Vec<String>) {
    if let Some(label) = &node.label {
        out.push(label.clone());
    }
    for child in &node.children {
        collect_labels(child, out);
    }
}

/// A pagination strip fed an adversarial page count and sibling window must not
/// try to materialize one cell per page. Before the clamp, `page + siblings`
/// overflowed `usize` (a debug panic; in release it wrapped to a ~1.8e19-wide
/// `lo..=hi` range that OOMs building a cell per page). The window is now
/// clamped to a small constant and the arithmetic saturates, so the strip stays
/// tiny regardless of input.
#[test]
fn pagination_clamps_adversarial_count_and_siblings() {
    let view: Element<()> = pagination(usize::MAX, usize::MAX)
        .siblings(usize::MAX)
        .on_select(|_| ())
        .into();
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (800.0, 80.0), 1.0);
    let buttons = count_buttons(&frame.access_tree());
    assert!(
        buttons <= 128,
        "pagination must clamp its window to a small constant; got {buttons} materialized cells"
    );
}

/// A legitimate large pager must address its full range. The siblings window is
/// what bounds the rendered strip (see the adversarial test above), so `count`
/// itself carries no allocation cost and must never be silently truncated: the
/// last-page cell, the current-page highlight, and the reachable range all read
/// straight from `count`/`page`. A 50 000-page table at page 25 000 must expose
/// a "page 50000" cell and mark 25000 (not a clamped 10000) as current.
#[test]
fn pagination_addresses_full_range_of_a_large_pager() {
    let view: Element<()> = pagination(25_000, 50_000)
        .siblings(1)
        .on_select(|_| ())
        .into();
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (800.0, 80.0), 1.0);
    let mut labels = Vec::new();
    collect_labels(&frame.access_tree(), &mut labels);
    assert!(
        labels.iter().any(|l| l.contains("50000")),
        "the last page of a 50000-page pager must be reachable; labels were {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == "Page 25000, current"),
        "the true current page (25000) must be highlighted, not a clamped value; labels were {labels:?}"
    );
}

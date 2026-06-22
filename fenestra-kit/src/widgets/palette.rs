//! Command palette: a modal filtering launcher (Cmd-K pattern). The app
//! owns the query and the open flag (Elm-pure).

use fenestra_core::{
    Cursor, Element, Key, Overlay, SP1, SP2, Semantics, Surface, TextSize, Theme, Transition, col,
    row, text,
};

use crate::text_input;

/// A command palette under construction; converts into an [`Element`].
pub struct CommandPalette<Msg> {
    query: String,
    open: bool,
    commands: Vec<(String, Msg)>,
    highlighted: Option<usize>,
    on_input: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
    on_navigate: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
    on_close: Option<Msg>,
    key: Option<String>,
}

/// A modal launcher: typing filters `commands` (case-insensitive
/// contains), clicking one emits its message, Enter runs the first
/// match, Escape / outside click emit `on_close`. Mount it while your
/// open flag is set (exactly like a modal).
pub fn command_palette<Msg>(
    query: impl Into<String>,
    open: bool,
    commands: impl IntoIterator<Item = (impl Into<String>, Msg)>,
) -> CommandPalette<Msg> {
    CommandPalette {
        query: query.into(),
        open,
        commands: commands
            .into_iter()
            .map(|(label, msg)| (label.into(), msg))
            .collect(),
        highlighted: None,
        on_input: None,
        on_navigate: None,
        on_close: None,
        key: None,
    }
}

impl<Msg> CommandPalette<Msg> {
    /// Maps every edit of the query to a message.
    pub fn on_input(mut self, f: impl Fn(String) -> Msg + 'static) -> Self {
        self.on_input = Some(std::rc::Rc::new(f));
        self
    }

    /// The app-owned keyboard cursor: the index (into the currently visible,
    /// filtered commands) drawn as the active row and run by Enter. Pair with
    /// [`CommandPalette::on_navigate`] and reset it to `Some(0)` when the query
    /// changes. `None` draws no cursor (Enter still runs the top match).
    pub fn highlighted(mut self, index: Option<usize>) -> Self {
        self.highlighted = index;
        self
    }

    /// Maps an arrow-key step to a message carrying the new highlight index
    /// (clamped to the visible range, like [`crate::select`]). Store it so the
    /// next render moves the cursor.
    pub fn on_navigate(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_navigate = Some(std::rc::Rc::new(f));
        self
    }

    /// Emitted when the palette wants to close (Escape, outside click).
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Stable identity key (recommended).
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: Clone + 'static> From<CommandPalette<Msg>> for Element<Msg> {
    fn from(p: CommandPalette<Msg>) -> Self {
        if !p.open {
            return col();
        }
        let needle = p.query.to_lowercase();
        let filtered: Vec<(String, Msg)> = p
            .commands
            .iter()
            .filter(|(label, _)| needle.is_empty() || label.to_lowercase().contains(&needle))
            .cloned()
            .collect();

        // The keyboard cursor, clamped into the visible (filtered) range:
        // `active` is the command Enter runs — the top match when the app owns
        // no highlight, preserving the original "Enter runs the first match"
        // behavior — while `cursor` draws the veil only once a highlight is
        // owned (so the existing modal renders unchanged).
        let active = filtered
            .len()
            .checked_sub(1)
            .map_or(0, |last| p.highlighted.unwrap_or(0).min(last));
        let cursor = (!filtered.is_empty() && p.highlighted.is_some()).then_some(active);

        let mut input = text_input(&p.query)
            .placeholder("Type a command…")
            .width(420.0);
        if let Some(f) = &p.on_input {
            let f = std::rc::Rc::clone(f);
            input = input.on_input(move |s| f(s.to_owned()));
        }
        let mut input = Element::from(input).autofocus();
        // The autofocused input drives the list: Up/Down step the cursor
        // (clamped, matching `select`), Enter runs the active command. Home/End
        // keep moving the text caret (the focused editor consumes them before
        // `on_key`), and Escape closes the modal via `on_close`.
        if !filtered.is_empty() {
            let cmds = filtered.clone();
            let nav = p.on_navigate.clone();
            input = input.on_key(move |k| match k.key {
                Key::ArrowDown => (active + 1 < cmds.len())
                    .then_some(active + 1)
                    .and_then(|i| nav.as_ref().map(|f| f(i))),
                Key::ArrowUp => active
                    .checked_sub(1)
                    .and_then(|i| nav.as_ref().map(|f| f(i))),
                Key::Enter => Some(cmds[active].1.clone()),
                _ => None,
            });
        }

        // The floating panel derives its material from the surface bundle
        // (`Surface::Menu`: Elevated(2) fill, subtle border, Lg shadow,
        // theme-radius corners) instead of a hand-rolled recipe — one source of
        // truth shared with menus/popovers, and it tracks the radius knob.
        let mut panel = col()
            .p(SP2)
            .gap(SP1)
            .w(440.0)
            .surface(Surface::Menu)
            .semantics(Semantics::Dialog)
            .label("Command palette")
            .child(input);
        if filtered.is_empty() {
            panel = panel.child(
                text("No matching commands")
                    .size(TextSize::Sm)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
            );
        } else {
            panel = panel.child(command_list(filtered, cursor));
        }
        let mut panel = panel.overlay(Overlay::modal());
        if let Some(close) = p.on_close {
            panel = panel.on_close(close);
        }
        if let Some(key) = &p.key {
            panel = panel.id(key);
        }
        // The overlay child needs an anchor in normal flow.
        col().child(panel)
    }
}

/// The styled list of command rows nested inside the palette panel (the same
/// `Surface::Menu` recipe as a dropdown), with the keyboard cursor tinted by
/// the accent veil. A `None` cursor draws a plain list, byte-identical to a
/// bare [`crate::menu`].
fn command_list<Msg: Clone + 'static>(
    items: Vec<(String, Msg)>,
    cursor: Option<usize>,
) -> Element<Msg> {
    col()
        .p(SP1)
        .gap(2.0)
        .min_w(160.0)
        .surface(Surface::Menu)
        .children(
            items
                .into_iter()
                .enumerate()
                .map(move |(i, (label, msg))| option_row(&label, cursor == Some(i), msg)),
        )
}

/// One command row: a pointer target carrying its message, tinted with the
/// accent veil when it is the keyboard cursor. The autofocused input owns
/// focus, so the row is not a tab stop and opts out of the focus ring after
/// `on_click`.
fn option_row<Msg: Clone + 'static>(label: &str, active: bool, on_click: Msg) -> Element<Msg> {
    let mut text_el = text(label.to_owned()).size(TextSize::Sm);
    if active {
        text_el = text_el.themed(|t: &Theme, s| s.color(t.accent_text));
    }
    let mut item = row()
        .items_center()
        .px(SP2)
        .h(30.0)
        .themed(|t: &Theme, s| s.rounded((t.radius.lg - SP1).max(0.0)))
        .shrink0()
        .cursor(Cursor::Pointer)
        .semantics(Semantics::Button)
        .label(label.to_owned())
        .transition(Transition::colors())
        .state_layer(|t| t.text)
        .children([text_el])
        .on_click(on_click)
        .focusable(false);
    if active {
        item = item.themed(|t: &Theme, s| s.bg(t.accent_bg));
    }
    item
}

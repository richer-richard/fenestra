//! Command palette: a modal filtering launcher (Cmd-K pattern). The app
//! owns the query and the open flag (Elm-pure).

use fenestra_core::{
    Element, Key, Overlay, SP1, SP2, Semantics, ShadowToken, TextSize, Theme, col, text,
};

use crate::{menu, text_input};

/// A command palette under construction; converts into an [`Element`].
pub struct CommandPalette<Msg> {
    query: String,
    open: bool,
    commands: Vec<(String, Msg)>,
    on_input: Option<std::rc::Rc<dyn Fn(String) -> Msg>>,
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
        on_input: None,
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
        let first = filtered.first().map(|(_, msg)| msg.clone());

        let mut input = text_input(&p.query)
            .placeholder("Type a command…")
            .width(420.0);
        if let Some(f) = &p.on_input {
            let f = std::rc::Rc::clone(f);
            input = input.on_input(move |s| f(s.to_owned()));
        }
        let mut input = Element::from(input).autofocus();
        if let Some(first) = first {
            input = input.on_key(move |k| matches!(k.key, Key::Enter).then(|| first.clone()));
        }

        let mut panel = col()
            .p(SP2)
            .gap(SP1)
            .w(440.0)
            .themed(|t: &Theme, s| s.rounded(t.radius.md))
            .shadow(ShadowToken::Lg)
            .themed(|t: &Theme, s| s.bg(t.elevated_surface(2)).border(1.0, t.border_subtle))
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
            panel = panel.child(menu(filtered));
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

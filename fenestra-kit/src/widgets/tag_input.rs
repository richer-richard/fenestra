//! Tag / token field: a bordered, rounded container — styled like
//! [`text_input`](crate::text_input) (raised surface, 1px border, theme radius)
//! — holding a wrapping row of removable pill "chips" for the current tags
//! followed by an inline field for typing new ones.
//!
//! Elm-pure: the app owns the `tags` vector. Removing a chip emits
//! `on_remove(i)`; the inline field reports its typed text through `on_add`.
//!
//! Two framework limits shape the wiring (see the method docs):
//! - There is no submit hook that surfaces the editor buffer, and this widget
//!   keeps no app-owned draft, so [`TagInput::on_add`] is fed by the inline
//!   field's per-edit `on_input` rather than a true commit-on-Enter.
//! - Keyboard focus styling is per-element with no "focus-within", so the
//!   container border can't switch to the accent ring while the inline field is
//!   focused; the field shows focus through its own caret instead.
//!
//! ```
//! use fenestra_kit::tag_input;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Remove(usize),
//!     Draft(String),
//! }
//!
//! let el: fenestra_core::Element<Msg> = tag_input(["design", "rust", "gpu"])
//!     .placeholder("Add a tag…")
//!     .on_remove(Msg::Remove)
//!     .on_add(Msg::Draft)
//!     .into();
//! ```

use std::rc::Rc;

use fenestra_core::{
    Cursor, Element, SP1, SP2, Semantics, TextSize, Theme, Transition, raw_input, row, text,
};

use crate::icons;

/// A tag input under construction; converts into an [`Element`].
pub struct TagInput<Msg> {
    tags: Vec<String>,
    placeholder: String,
    width: f32,
    on_remove: Option<Rc<dyn Fn(usize) -> Msg>>,
    on_add: Option<Rc<dyn Fn(String) -> Msg>>,
    key: Option<String>,
}

/// A multi-select **tag / token field**: the app-owned `tags` render as
/// removable chips inside a bordered, rounded box, trailed by an inline field
/// for adding more. Wire [`TagInput::on_remove`] and [`TagInput::on_add`] to
/// edit the vector in your `update`.
pub fn tag_input<Msg>(tags: impl IntoIterator<Item = impl Into<String>>) -> TagInput<Msg> {
    TagInput {
        tags: tags.into_iter().map(Into::into).collect(),
        placeholder: String::new(),
        width: 280.0,
        on_remove: None,
        on_add: None,
        key: None,
    }
}

impl<Msg> TagInput<Msg> {
    /// Placeholder shown in the inline field while it is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Maps a chip's remove button (its `×`) to a message carrying the tag's
    /// index in the current `tags` vector.
    pub fn on_remove(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_remove = Some(Rc::new(f));
        self
    }

    /// Maps the inline field's typed text to a message.
    ///
    /// The framework exposes no submit hook that surfaces the editor buffer and
    /// this widget holds no app-owned draft, so the callback fires on **every
    /// edit** of the inline field with its full current text — not once on
    /// Enter. Keep the latest string as your in-progress draft and commit it to
    /// `tags` when you decide (a delimiter, a separate button); committing on
    /// Enter needs a core submit hook or an app-owned draft this fixed API does
    /// not carry.
    pub fn on_add(mut self, f: impl Fn(String) -> Msg + 'static) -> Self {
        self.on_add = Some(Rc::new(f));
        self
    }

    /// Stable identity key for the inline field (recommended: editor state —
    /// caret, selection, IME — is kept per id).
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

/// One removable chip: a rounded-full `element`-tinted pill holding the tag
/// label and a small `×` remove button that emits `on_remove(i)`.
fn chip<Msg: 'static>(
    i: usize,
    tag: String,
    on_remove: Option<Rc<dyn Fn(usize) -> Msg>>,
) -> Element<Msg> {
    let remove_label = format!("Remove {tag}");
    // Stable identity (the tag itself) so a chip keeps its identity when a
    // sibling is removed — the survivors FLIP into the gap, the removed chip
    // shrinks away.
    let key = tag.clone();
    let label = text(tag)
        .size(TextSize::Sm)
        .themed(|t: &Theme, s| s.color(t.text));

    let mut remove = row()
        .items_center()
        .justify_center()
        .w(16.0)
        .h(16.0)
        .rounded_full()
        .shrink0()
        .cursor(Cursor::Pointer)
        .transition(Transition::colors())
        .state_layer(|t| t.text)
        .press_scale()
        .semantics(Semantics::Button)
        .label(remove_label)
        .children([icons::x()
            .w(10.0)
            .h(10.0)
            .themed(|t: &Theme, s| s.color(t.text_muted))]);
    // `on_click` auto-focuses the button, so the `×` is keyboard-reachable.
    if let Some(f) = on_remove {
        remove = remove.on_click(f(i));
    }

    row()
        .items_center()
        .gap(SP1)
        .pl(SP2)
        .pr(SP1)
        .py(2.0)
        .rounded_full()
        .shrink0()
        .id(&key)
        .animate_layout()
        .exit_to(0.0, 0.8, 0.0, 0.0)
        .themed(|t: &Theme, s| s.bg(t.element))
        .children([label])
        .children([remove])
}

impl<Msg: 'static> From<TagInput<Msg>> for Element<Msg> {
    fn from(t: TagInput<Msg>) -> Self {
        let on_remove = t.on_remove;
        let chips = t
            .tags
            .into_iter()
            .enumerate()
            .map(move |(i, tag)| chip(i, tag, on_remove.clone()));

        // The inline field is a bare `raw_input` (no border/bg of its own — the
        // container carries those) that grows to fill the rest of its line.
        let mut field = raw_input(String::new(), t.placeholder.clone())
            .size(TextSize::Sm)
            .h(24.0)
            .min_w(72.0)
            .grow()
            .themed(|theme: &Theme, s| s.color(theme.text));
        if !t.placeholder.is_empty() {
            field = field.label(t.placeholder);
        }
        if let Some(add) = t.on_add {
            field = field.on_input(move |s| add(s.to_owned()));
        }
        if let Some(key) = &t.key {
            field = field.id(key);
        }

        // The container mirrors `text_input`: raised surface, 1px border, theme
        // radius, a Fast color transition, and a stronger border on hover. It
        // wraps so chips flow onto new lines as they overflow the width.
        row()
            .wrap()
            .items_center()
            .gap(SP1)
            .px(SP2)
            .py(SP1)
            .w(t.width)
            .transition(Transition::colors())
            .themed(|theme: &Theme, s| {
                s.rounded(theme.radius.md)
                    .bg(theme.surface_raised)
                    .border(1.0, theme.border)
            })
            .hover_themed(|theme: &Theme, s| s.border(1.0, theme.border_strong))
            .children(chips)
            .children([field])
    }
}

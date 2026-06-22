//! Radio button: a 16px circle whose selected state is an accent ring.
//!
//! ```
//! use fenestra_kit::radio;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Pick(u8),
//! }
//!
//! let el: fenestra_core::Element<Msg> =
//!     radio(true).label("Annual billing").on_select(Msg::Pick(0)).into();
//! ```

use fenestra_core::{
    Cursor, Element, Key, SP2, Semantics, TextSize, Theme, Transition, col, row, text,
};

/// A radio button under construction; converts into an [`Element`].
pub struct Radio<Msg> {
    selected: bool,
    label: Option<String>,
    disabled: bool,
    on_select: Option<Msg>,
    key: Option<String>,
}

/// A radio button reflecting `selected`. Group behavior comes from the
/// app: each radio emits its own select message.
pub fn radio<Msg>(selected: bool) -> Radio<Msg> {
    Radio {
        selected,
        label: None,
        disabled: false,
        on_select: None,
        key: None,
    }
}

impl<Msg> Radio<Msg> {
    /// Adds a clickable text label to the right.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Disables interaction.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Emits this message when selected.
    pub fn on_select(mut self, msg: Msg) -> Self {
        self.on_select = Some(msg);
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg> From<Radio<Msg>> for Element<Msg> {
    fn from(r: Radio<Msg>) -> Self {
        let selected = r.selected;
        let circle = radio_circle::<Msg>(selected);

        let mut el = row()
            .items_center()
            .gap(SP2)
            .shrink0()
            .children([circle])
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(r.disabled)
            .semantics(Semantics::Radio { selected });
        if let Some(label) = r.label {
            el = el
                .label(label.clone())
                .children([text(label).size(TextSize::Sm)]);
        }
        if r.disabled {
            el = el.opacity(0.5);
        }
        if let Some(key) = &r.key {
            el = el.id(key);
        }
        if let Some(msg) = r.on_select {
            el = el.on_click(msg);
        }
        el
    }
}

/// The 16px ring/dot shared by [`radio`] and [`radio_group`].
fn radio_circle<Msg>(selected: bool) -> Element<Msg> {
    fenestra_core::div()
        .w(16.0)
        .h(16.0)
        .rounded_full()
        .shrink0()
        .transition(Transition::colors())
        .themed(move |t: &Theme, s| {
            if selected {
                // A 5px accent ring leaves a 6px surface hole: the dot.
                s.bg(t.surface_raised).border(5.0, t.accent)
            } else {
                s.bg(t.surface_raised).border(1.0, t.border_strong)
            }
        })
}

/// A single-select **radio group**: a column of radios that is one tab stop and
/// is driven like a WAI-ARIA radio group — arrows move *and* select, and the
/// ends wrap. `selected` is the chosen index; choosing a row (click or arrow)
/// emits `on_select(index)`. Elm-pure: the host owns the index and echoes it
/// back.
///
/// ```
/// use fenestra_kit::radio_group;
///
/// #[derive(Clone)]
/// enum Msg {
///     Plan(usize),
/// }
///
/// let el: fenestra_core::Element<Msg> =
///     radio_group(0, ["Monthly", "Annual"], Msg::Plan);
/// ```
pub fn radio_group<Msg: 'static>(
    selected: usize,
    labels: impl IntoIterator<Item = impl Into<String>>,
    on_select: impl Fn(usize) -> Msg + 'static,
) -> Element<Msg> {
    let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
    let n = labels.len();
    let selected = if n == 0 { 0 } else { selected.min(n - 1) };
    let on_select = std::rc::Rc::new(on_select);
    let row_select = on_select.clone();
    let rows = labels.into_iter().enumerate().map(move |(i, label)| {
        let is_sel = i == selected;
        row()
            .items_center()
            .gap(SP2)
            .shrink0()
            .cursor(Cursor::Pointer)
            .on_click(row_select(i))
            // One tab stop for the group (see the group's key handler);
            // `on_click` auto-focuses, so opt the rows back out.
            .focusable(false)
            .semantics(Semantics::Radio { selected: is_sel })
            .label(label.clone())
            .children([radio_circle::<Msg>(is_sel), text(label).size(TextSize::Sm)])
    });
    let group = col().gap(SP2).shrink0().children(rows);
    if n > 1 {
        // One tab stop; arrows move + select with wrap, Home/End jump.
        group.focusable(true).on_key(move |k| match k.key {
            Key::ArrowDown | Key::ArrowRight => Some(on_select((selected + 1) % n)),
            Key::ArrowUp | Key::ArrowLeft => Some(on_select((selected + n - 1) % n)),
            Key::Home => Some(on_select(0)),
            Key::End => Some(on_select(n - 1)),
            _ => None,
        })
    } else {
        group
    }
}

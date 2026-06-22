//! Disclosure widgets: [`accordion`] — stacked expandable sections, each a
//! focusable header (with a chevron that rotates as it opens) over a
//! collapsible body.
//!
//! ```
//! use fenestra_core::text;
//! use fenestra_kit::{accordion, accordion_item};
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Toggle(usize),
//! }
//!
//! let el: fenestra_core::Element<Msg> = accordion([
//!     accordion_item("Shipping", text("Ships in two days."))
//!         .open(true)
//!         .on_toggle(Msg::Toggle(0)),
//!     accordion_item("Returns", text("Thirty-day returns.")).on_toggle(Msg::Toggle(1)),
//! ])
//! .into();
//! ```

use fenestra_core::{
    Cursor, Element, SP3, SP4, Semantics, TextSize, Theme, Transition, Weight, col, row, text,
};

use crate::icons;

/// One section of an [`accordion`]: a titled header over collapsible body content.
pub struct AccordionItem<Msg> {
    title: String,
    body: Element<Msg>,
    open: bool,
    on_toggle: Option<Msg>,
    key: Option<String>,
}

/// An accordion section with the given header title and body content. Collapsed
/// by default; drive [`AccordionItem::open`] from app state and wire
/// [`AccordionItem::on_toggle`] to flip it.
pub fn accordion_item<Msg>(
    title: impl Into<String>,
    body: impl Into<Element<Msg>>,
) -> AccordionItem<Msg> {
    AccordionItem {
        title: title.into(),
        body: body.into(),
        open: false,
        on_toggle: None,
        key: None,
    }
}

impl<Msg> AccordionItem<Msg> {
    /// Whether the section is expanded.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Emits this message when the header is activated (click or Enter/Space).
    #[must_use]
    pub fn on_toggle(mut self, msg: Msg) -> Self {
        self.on_toggle = Some(msg);
        self
    }

    /// Stable identity key.
    #[must_use]
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

/// A stack of [`accordion_item`]s under construction; converts into an [`Element`].
pub struct Accordion<Msg> {
    items: Vec<AccordionItem<Msg>>,
}

/// A vertically stacked set of expandable sections inside one bordered, rounded
/// card. Each header is an independently focusable button whose chevron rotates
/// as the section opens; open sections reveal their body below. Multiple
/// sections may be open at once — expansion is driven entirely by each item's
/// [`AccordionItem::open`] flag, so single-open behaviour is an app-state choice.
pub fn accordion<Msg>(items: impl IntoIterator<Item = AccordionItem<Msg>>) -> Accordion<Msg> {
    Accordion {
        items: items.into_iter().collect(),
    }
}

impl<Msg> From<Accordion<Msg>> for Element<Msg> {
    fn from(a: Accordion<Msg>) -> Self {
        let mut sections: Vec<Element<Msg>> = Vec::new();
        for (i, item) in a.items.into_iter().enumerate() {
            let open = item.open;
            let title_text = item.title.clone();

            // The disclosure caret: a right chevron that rotates a quarter turn
            // to point down when open. The transition animates the spin live.
            let chevron = icons::chevron_right()
                .w(16.0)
                .h(16.0)
                .shrink0()
                .rotate(if open { 90.0 } else { 0.0 })
                .transition(Transition::all())
                .themed(|t: &Theme, s| s.color(t.text_muted));
            let title = text(item.title)
                .size(TextSize::Sm)
                .weight(Weight::Medium)
                .themed(|t: &Theme, s| s.color(t.text));

            let mut header = row()
                .w_full()
                .items_center()
                .gap(SP3)
                .h(48.0)
                .px(SP4)
                .children([chevron, title])
                .transition(Transition::colors())
                .state_layer(|t| t.text)
                .focusable(true)
                .cursor(Cursor::Pointer)
                .semantics(Semantics::Button)
                .label(title_text);
            if let Some(key) = &item.key {
                header = header.id(key);
            }
            if let Some(msg) = item.on_toggle {
                header = header.on_click(msg);
            }

            let mut section_kids: Vec<Element<Msg>> = vec![header];
            if open {
                section_kids.push(col().w_full().px(SP4).pb(SP4).children([item.body]));
            }
            let mut section = col().w_full().children(section_kids);
            if i > 0 {
                section = section.themed(|t: &Theme, s| s.border_top(1.0, t.border_subtle));
            }
            sections.push(section);
        }

        col()
            .w_full()
            .overflow_hidden()
            .themed(|t: &Theme, s| s.rounded(t.radius.md).border(1.0, t.border))
            .children(sections)
    }
}

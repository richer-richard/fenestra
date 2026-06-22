//! Keyboard shortcut key-caps ([`kbd`] flat chips, [`kbd_raised`] 3D keys).
//!
//! ```
//! use fenestra_kit::{kbd, kbd_raised};
//!
//! let inline: fenestra_core::Element<()> = kbd(["cmd", "K"]); // for palette rows
//! let docs: fenestra_core::Element<()> = kbd_raised(["esc"]); // a chunky keycap
//! ```

use fenestra_core::{Element, SP1, Semantics, TextSize, Theme, Weight, row, text};

/// Maps a logical key name to a compact display glyph. Modifier names map to
/// their conventional symbols (⌘ ⇧ ⌥ ⌃) and common editing keys to their
/// glyphs; a single letter/symbol is upper-cased, and anything else is shown
/// verbatim (so `"F2"`, `"Home"`, `"/"` all read correctly).
fn key_glyph(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "cmd" | "command" | "meta" | "super" | "win" => "⌘".to_owned(),
        "shift" => "⇧".to_owned(),
        "alt" | "opt" | "option" => "⌥".to_owned(),
        "ctrl" | "control" => "⌃".to_owned(),
        "enter" | "return" => "↵".to_owned(),
        "esc" | "escape" => "Esc".to_owned(),
        "backspace" => "⌫".to_owned(),
        "delete" | "del" => "Del".to_owned(),
        "tab" => "Tab".to_owned(),
        "up" => "↑".to_owned(),
        "down" => "↓".to_owned(),
        "left" => "←".to_owned(),
        "right" => "→".to_owned(),
        "space" => "␣".to_owned(),
        other if other.chars().count() == 1 => other.to_uppercase(),
        _ => key.to_owned(),
    }
}

/// A single key-cap box, flat or raised.
fn cap<Msg>(glyph: String, raised: bool) -> Element<Msg> {
    let inner = row()
        .items_center()
        .justify_center()
        .h(20.0)
        .min_w(20.0)
        .px(5.0)
        .shrink0();
    let styled = if raised {
        // A chunky key: raised surface, near-square corners, and a thick darker
        // bottom border that reads as the key's front lip.
        inner.themed(|t: &Theme, s| {
            s.rounded(3.0)
                .bg(t.surface_raised)
                .border_top(1.0, t.border_subtle)
                .border_left(1.0, t.border_subtle)
                .border_right(1.0, t.border_subtle)
                .border_bottom(2.5, t.border_strong)
        })
    } else {
        // A flat chip: subtle fill + a hairline, no shadow — for palette rows.
        inner.themed(|t: &Theme, s| {
            s.rounded(t.radius.sm)
                .bg(t.element)
                .border(1.0, t.border_subtle)
        })
    };
    styled.children([text(glyph)
        .size(TextSize::Xs)
        .weight(Weight::Medium)
        .themed(move |t: &Theme, s| s.color(if raised { t.text } else { t.text_muted }))])
}

/// Builds the chord row shared by [`kbd`] and [`kbd_raised`].
fn chord<Msg>(keys: impl IntoIterator<Item = impl Into<String>>, raised: bool) -> Element<Msg> {
    let glyphs: Vec<String> = keys.into_iter().map(|k| key_glyph(&k.into())).collect();
    let name = glyphs.join(" ");
    row()
        .items_center()
        .gap(SP1)
        .shrink0()
        .semantics(Semantics::Image)
        .label(name)
        .children(glyphs.into_iter().map(move |g| cap(g, raised)))
}

/// Flat-chip keyboard key-caps for a chord, e.g. `kbd(["cmd", "K"])` renders
/// `⌘ K`. The caps are flat chips (subtle fill + a hairline border, no shadow)
/// sized to sit inside menus, tooltips, and command-palette rows; modifier
/// names map to their platform glyphs. The whole chord exposes one accessible
/// label so a screen reader announces the shortcut once.
pub fn kbd<Msg>(keys: impl IntoIterator<Item = impl Into<String>>) -> Element<Msg> {
    chord(keys, false)
}

/// Raised 3D keyboard key-caps — the chunky keycap look (a raised surface with
/// a thick bottom lip), for documentation and onboarding rather than dense
/// menu rows. Same glyph mapping and accessible label as [`kbd`].
pub fn kbd_raised<Msg>(keys: impl IntoIterator<Item = impl Into<String>>) -> Element<Msg> {
    chord(keys, true)
}

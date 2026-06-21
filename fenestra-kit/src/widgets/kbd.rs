//! Keyboard shortcut key-caps ([`kbd`]).
//!
//! ```
//! use fenestra_kit::kbd;
//!
//! let chord: fenestra_core::Element<()> = kbd(["cmd", "K"]);
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

/// A single flat key-cap box: subtle neutral fill, hairline border, near-square.
fn cap<Msg>(glyph: String) -> Element<Msg> {
    row()
        .items_center()
        .justify_center()
        .h(20.0)
        .min_w(20.0)
        .px(5.0)
        .shrink0()
        .themed(|t: &Theme, s| {
            s.rounded(t.radius.sm)
                .bg(t.element)
                .border(1.0, t.border_subtle)
        })
        .children([text(glyph)
            .size(TextSize::Xs)
            .weight(Weight::Medium)
            .themed(|t: &Theme, s| s.color(t.text_muted))])
}

/// Keyboard shortcut key-caps for a chord, e.g. `kbd(["cmd", "K"])` renders
/// `⌘ K`. The caps are flat chips (subtle fill + a hairline border, no shadow)
/// sized to sit inside menus, tooltips, and command-palette rows; modifier
/// names map to their platform glyphs. The whole chord exposes one accessible
/// label so a screen reader announces the shortcut once.
pub fn kbd<Msg>(keys: impl IntoIterator<Item = impl Into<String>>) -> Element<Msg> {
    let glyphs: Vec<String> = keys.into_iter().map(|k| key_glyph(&k.into())).collect();
    let name = glyphs.join(" ");
    row()
        .items_center()
        .gap(SP1)
        .shrink0()
        .semantics(Semantics::Image)
        .label(name)
        .children(glyphs.into_iter().map(cap))
}

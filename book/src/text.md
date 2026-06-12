# Text and inputs

Text shapes through parley with three embedded Inter faces (400/500/600)
for determinism; the windowed runner adds system fonts, which also
provides per-script fallback (CJK works out of the box).

`text("...")` wraps to its container, truncates with `.truncate()`,
aligns with `.text_align(..)`, and exposes the full editorial controls:
`.size_px`, `.tracking`, `.leading`, `.family`.

## Single-line input

`text_input(&self.value).placeholder("Search…").on_input(Msg::Set).id("q")`
— parley editing with selection, caret blink, clipboard (Cmd/Ctrl A/C/X/V),
word jumps, Home/End, IME preedit, and horizontal follow-scroll. Control
characters are filtered on every path. The app owns the value.

## Multiline

`text_area(&self.notes).on_input(Msg::Notes).id("notes")` wraps to its
width, accepts Enter as a newline, moves by line with the arrows, and
grows with its content from `min_height`. Cap growth with an outer scroll
container.

## IME

Composition works in both inputs: preedit shows inline with an
underline, commit inserts atomically. The windowed runner anchors the
OS candidate window to the caret as you type, in every window.

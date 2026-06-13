# Text and inputs

Text shapes through parley with three embedded Inter faces (400/500/600)
for determinism; the windowed runner adds system fonts, which also
provides per-script fallback (CJK works out of the box).

`text("...")` wraps to its container, truncates with `.truncate()`,
aligns with `.text_align(..)`, and exposes the full editorial controls:
`.size_px`, `.tracking`, `.leading`, `.family`.

## Reading measure

Long prose reads best in a column near ~66 characters per line, not the full
window. `.measure(chars)` caps an element's width in CSS `ch` units (1ch is the
advance of `'0'` in its *own* resolved text style), so the column holds a
comfortable line at any window width — resolved to pixels during layout, where
the font metrics live. The default `MEASURE_CH` (52ch, tuned so the body face
renders ~66 characters) drives the kit's `reading_column()`, the markdown
widget's prose, and the `ai_chat` showcase. Because fenestra has no style
inheritance, set the container's `.size(..)` and `.family(..)` to match the
prose it wraps, so the measure tracks the real glyphs.

## Single-line input

`text_input(&self.value).placeholder("Search…").on_input(Msg::Set).id("q")`
— parley editing with selection, caret blink, clipboard (Cmd/Ctrl A/C/X/V),
word jumps, Home/End, IME preedit, and horizontal follow-scroll. Control
characters are filtered on every path. The app owns the value.

Selection works the way fingers expect: drag selects, double-click
selects the word, triple-click the line, shift-click extends from the
caret, shift-arrows extend by character/word/line.

## Undo and redo

Cmd/Ctrl+Z undoes, Shift+Cmd/Ctrl+Z (or Ctrl+Y) redoes — per field,
with the classic rules: typing coalesces into one undo unit; moving
the caret, clicking, pasting, or cutting starts a new one; a fresh
edit clears the redo stack; undo restores the selection too. History
is bounded (100 steps). Because the app owns the value, undo emits
`on_input` like any other edit — your `update` sees it.

## Multiline

`text_area(&self.notes).on_input(Msg::Notes).id("notes")` wraps to its
width, accepts Enter as a newline, moves by line with the arrows, and
grows with its content from `min_height`. Cap growth with an outer scroll
container.

## Selecting static text

`.selectable()` on text and rich text gives users browser-grade
selection: drag selects, double-click takes the word, triple-click the
line, Cmd/Ctrl+C copies. One selection lives at a time; any press
elsewhere clears it. The highlight uses the input selection color, and
tests read the selected byte range from `AccessNode::selection`.

## Rich text

`rich_text([span("Ship it "), span("boldly").weight(Weight::Semibold)
.color(theme.accent), span(" today").italic()])` — one wrapped
paragraph, per-span weight/color/size/family/italic, spans flowing
together across line breaks. Display-only (inputs stay plain), and the
spans concatenate into one accessible label.

## Emoji

Color emoji (COLR/sbix) render through system-font fallback
(`Fonts::with_system`, the windowed default) — pixel-proven on macOS.
Embedded fonts have no emoji (determinism trades for coverage, same as
CJK). Known caveat: VS16 emoji-presentation sequences (like ❤️ =
U+2764+FE0F) currently select the monochrome text glyph.

## Bidi and RTL

parley shapes mixed-direction text (Arabic/Hebrew embedded in Latin
and vice versa) out of the box; glyph coverage for RTL scripts comes
from system fonts (`Fonts::with_system`), exactly like CJK. UI
mirroring (flipping layout direction app-wide) is not implemented yet.

## IME

Composition works in both inputs: preedit shows inline with an
underline, commit inserts atomically. The windowed runner anchors the
OS candidate window to the caret as you type, in every window.

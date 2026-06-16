//! Single-line text editing on parley's `PlainEditor`: per-widget editor
//! state, keyboard/pointer handling, and painting (selection, caret,
//! placeholder, horizontal follow-scroll).

use kurbo::{Affine, Rect};
use parley::PositionedLayoutItem;
use peniko::{Color, Fill};
use vello::Scene;

use crate::clipboard::Clipboard;
use crate::events::{Key, KeyInput};
use crate::text::{Fonts, LayoutBrush, ResolvedText};

/// Caret blink period: 530ms visible, 530ms hidden.
const BLINK_HALF_PERIOD: f64 = 0.53;
/// Caret stroke width in logical px.
const CARET_WIDTH: f64 = 1.5;

/// Retained editing state for one input widget.
pub(crate) struct EditorState {
    pub editor: parley::PlainEditor<LayoutBrush>,
    /// Horizontal scroll keeping the caret visible in a narrow box.
    pub scroll_x: f64,
    /// Clock time of the last edit or caret move (restarts the blink).
    pub last_activity: f64,
    /// Frame stamp for garbage collection.
    pub seen: u64,
    /// Multiline mode: wraps, accepts newlines, moves by line.
    pub multiline: bool,
    /// Undo/redo history (QUndoStack semantics: coalesced runs,
    /// boundaries on caret moves, redo cleared by new edits).
    pub undo: UndoStack,
}

/// What kind of edit a coalescing run holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditRun {
    Insert,
    Delete,
}

/// One restorable editor moment: text plus the selected byte range.
#[derive(Debug, Clone)]
struct Snapshot {
    text: String,
    selection: (usize, usize),
}

/// Bounded undo/redo stacks with typing-run coalescing.
#[derive(Debug, Default)]
pub(crate) struct UndoStack {
    undos: Vec<Snapshot>,
    redos: Vec<Snapshot>,
    /// The open coalescing run; edits of the same kind merge into it.
    run: Option<EditRun>,
}

/// History depth: plenty for a field, bounded so editors never grow
/// without limit.
const UNDO_LIMIT: usize = 100;

fn snapshot(editor: &parley::PlainEditor<LayoutBrush>) -> Snapshot {
    let range = editor.raw_selection().text_range();
    Snapshot {
        text: editor.raw_text().to_owned(),
        selection: (range.start, range.end),
    }
}

impl UndoStack {
    /// Opens (or continues) a coalescing run of `kind`: the first edit
    /// of a run snapshots the pre-edit state and clears redos.
    fn begin(&mut self, editor: &parley::PlainEditor<LayoutBrush>, kind: EditRun) {
        if self.run != Some(kind) {
            self.undos.push(snapshot(editor));
            if self.undos.len() > UNDO_LIMIT {
                self.undos.remove(0);
            }
            self.redos.clear();
            self.run = Some(kind);
        }
    }

    /// Ends the open run: the next edit starts a fresh undo unit.
    /// Called on caret/selection moves and external value changes.
    pub(crate) fn break_run(&mut self) {
        self.run = None;
    }
}

impl EditorState {
    pub(crate) fn new(style: &ResolvedText, now: f64, multiline: bool) -> Self {
        let mut editor = parley::PlainEditor::new(style.px);
        apply_style(&mut editor, style);
        editor.set_width(None); // multiline editors get their width at paint
        Self {
            editor,
            scroll_x: 0.0,
            last_activity: now,
            seen: 0,
            multiline,
            undo: UndoStack::default(),
        }
    }

    /// Syncs the editor with the app-provided value and text style. The app
    /// is the source of truth: external changes reset the buffer.
    pub(crate) fn sync(&mut self, value: &str, style: &ResolvedText) {
        if self.editor.raw_text() != value {
            let fresh = self.editor.raw_text().is_empty()
                && self.undo.undos.is_empty()
                && self.undo.redos.is_empty();
            if fresh {
                // First fill of a brand-new editor: nothing to undo.
                self.editor.set_text(value);
                apply_style(&mut self.editor, style);
                return;
            }
            // External (programmatic) change: its own undo unit, so
            // undo can step back across it.
            self.undo.undos.push(snapshot(&self.editor));
            if self.undo.undos.len() > UNDO_LIMIT {
                self.undo.undos.remove(0);
            }
            self.undo.redos.clear();
            self.undo.break_run();
            self.editor.set_text(value);
        }
        apply_style(&mut self.editor, style);
    }
}

fn apply_style(editor: &mut parley::PlainEditor<LayoutBrush>, style: &ResolvedText) {
    use parley::{FontFamily, FontWeight, GenericFamily, LineHeight, StyleProperty};
    let styles = editor.edit_styles();
    styles.insert(StyleProperty::FontSize(style.px));
    styles.insert(StyleProperty::FontWeight(FontWeight::new(style.weight)));
    styles.insert(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
        style.line_height,
    )));
    styles.insert(StyleProperty::LetterSpacing(style.letter_spacing));
    // Features and optical sizing are insert-or-*remove*: re-applying a style
    // that turned a feature (or the opsz axis) off must clear the prior property,
    // not leave it stuck on a persistent editor (the 0.16 known limitation).
    // `edit_styles` is a discriminant-keyed set, so removal by variant is exact.
    match style.features.feature_string() {
        Some(s) => {
            styles.insert(StyleProperty::FontFeatures(parley::FontFeatures::Source(
                std::borrow::Cow::Owned(s),
            )));
        }
        None => {
            styles.remove(std::mem::discriminant(&StyleProperty::FontFeatures(
                parley::FontFeatures::empty(),
            )));
        }
    }
    // Optical sizing (no-op on the editor's static Inter; correct if an app
    // registers a variable face).
    match style.optical.opsz_at(style.px) {
        Some(opsz) => {
            styles.insert(StyleProperty::FontVariations(
                parley::FontVariations::Source(std::borrow::Cow::Owned(format!("\"opsz\" {opsz}"))),
            ));
        }
        None => {
            styles.remove(std::mem::discriminant(&StyleProperty::FontVariations(
                parley::FontVariations::empty(),
            )));
        }
    }
    styles.insert(StyleProperty::FontFamily(match style.family {
        crate::tokens::FamilyRole::Sans => FontFamily::named("Inter"),
        crate::tokens::FamilyRole::Mono => FontFamily::Single(GenericFamily::Monospace.into()),
        // Editors keep the body face; display/serif faces are for static
        // text (registered names are not plumbed into editors yet).
        crate::tokens::FamilyRole::Display | crate::tokens::FamilyRole::Serif => {
            FontFamily::named("Inter")
        }
    }));
}

/// The outcome of one editor event.
pub(crate) struct EditOutcome {
    /// The buffer changed (emit `on_input`).
    pub changed: bool,
    /// The event was consumed by the editor (do not forward to `on_key`).
    pub consumed: bool,
}

const HANDLED: EditOutcome = EditOutcome {
    changed: true,
    consumed: true,
};
const MOVED: EditOutcome = EditOutcome {
    changed: false,
    consumed: true,
};
const IGNORED: EditOutcome = EditOutcome {
    changed: false,
    consumed: false,
};

/// Applies a key press to the editor. Word jumps follow the common
/// conventions: Alt+arrow (mac) or Ctrl+arrow jump words; Cmd/Ctrl+arrow or
/// Home/End jump to the line edges; Cmd/Ctrl+A/C/X/V are the selection and
/// clipboard shortcuts.
pub(crate) fn handle_key(
    state: &mut EditorState,
    fonts: &mut Fonts,
    clipboard: &mut dyn Clipboard,
    key: &KeyInput,
) -> EditOutcome {
    let outcome = handle_key_inner(state, fonts, clipboard, key);
    if !outcome.changed {
        // Caret/selection moves are coalescing boundaries: the next
        // edit starts a fresh undo unit.
        state.undo.break_run();
    }
    outcome
}

fn handle_key_inner(
    state: &mut EditorState,
    fonts: &mut Fonts,
    clipboard: &mut dyn Clipboard,
    key: &KeyInput,
) -> EditOutcome {
    let multiline = state.multiline;
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let mut drv = state.editor.driver(font_cx, layout_cx);
    let shortcut = key.meta || key.ctrl;
    let word = key.alt || (key.ctrl && !key.meta);

    match key.key {
        Key::Char(c) if shortcut => match c.to_ascii_lowercase() {
            'a' => {
                drv.select_all();
                MOVED
            }
            'c' => {
                if let Some(text) = drv.editor.selected_text() {
                    clipboard.set(text.to_owned());
                }
                MOVED
            }
            'x' => {
                if let Some(text) = drv.editor.selected_text() {
                    clipboard.set(text.to_owned());
                    // Cut is its own undo unit, never coalesced.
                    state.undo.break_run();
                    state.undo.begin(&state.editor, EditRun::Delete);
                    let (font_cx, layout_cx) = fonts.editor_contexts();
                    let mut drv = state.editor.driver(font_cx, layout_cx);
                    drv.delete_selection();
                    state.undo.break_run();
                    return HANDLED;
                }
                MOVED
            }
            'v' => {
                if let Some(text) = clipboard.get() {
                    // Paste is its own undo unit, never coalesced.
                    state.undo.break_run();
                    state.undo.begin(&state.editor, EditRun::Insert);
                    let (font_cx, layout_cx) = fonts.editor_contexts();
                    let mut drv = state.editor.driver(font_cx, layout_cx);
                    drv.insert_or_replace_selection(&sanitize(&text, multiline));
                    state.undo.break_run();
                    return HANDLED;
                }
                MOVED
            }
            'z' => {
                let applied = if key.shift {
                    redo(state, fonts)
                } else {
                    undo(state, fonts)
                };
                if applied { HANDLED } else { MOVED }
            }
            'y' if key.ctrl => {
                if redo(state, fonts) {
                    HANDLED
                } else {
                    MOVED
                }
            }
            _ => IGNORED,
        },
        // Control characters (Enter arriving as '\r', Tab, DEL, ...) never
        // become text, matching the text-commit and paste path filters.
        Key::Char(c) if c.is_control() => IGNORED,
        Key::Char(c) if !key.ctrl && !key.meta => {
            state.undo.begin(&state.editor, EditRun::Insert);
            let (font_cx, layout_cx) = fonts.editor_contexts();
            let mut drv = state.editor.driver(font_cx, layout_cx);
            drv.insert_or_replace_selection(&c.to_string());
            HANDLED
        }
        Key::ArrowLeft => {
            match (key.shift, word, key.meta) {
                (true, _, true) => drv.select_to_line_start(),
                (false, _, true) => drv.move_to_line_start(),
                (true, true, _) => drv.select_word_left(),
                (false, true, _) => drv.move_word_left(),
                (true, false, _) => drv.select_left(),
                (false, false, _) => drv.move_left(),
            }
            MOVED
        }
        Key::ArrowRight => {
            match (key.shift, word, key.meta) {
                (true, _, true) => drv.select_to_line_end(),
                (false, _, true) => drv.move_to_line_end(),
                (true, true, _) => drv.select_word_right(),
                (false, true, _) => drv.move_word_right(),
                (true, false, _) => drv.select_right(),
                (false, false, _) => drv.move_right(),
            }
            MOVED
        }
        Key::Home => {
            if key.shift {
                drv.select_to_line_start();
            } else {
                drv.move_to_line_start();
            }
            MOVED
        }
        Key::End => {
            if key.shift {
                drv.select_to_line_end();
            } else {
                drv.move_to_line_end();
            }
            MOVED
        }
        Key::Enter if multiline => {
            state.undo.begin(&state.editor, EditRun::Insert);
            let (font_cx, layout_cx) = fonts.editor_contexts();
            let mut drv = state.editor.driver(font_cx, layout_cx);
            drv.insert_or_replace_selection("\n");
            HANDLED
        }
        Key::ArrowUp if multiline => {
            if key.shift {
                drv.select_up();
            } else {
                drv.move_up();
            }
            MOVED
        }
        Key::ArrowDown if multiline => {
            if key.shift {
                drv.select_down();
            } else {
                drv.move_down();
            }
            MOVED
        }
        Key::Backspace => {
            state.undo.begin(&state.editor, EditRun::Delete);
            let (font_cx, layout_cx) = fonts.editor_contexts();
            let mut drv = state.editor.driver(font_cx, layout_cx);
            if word {
                drv.backdelete_word();
            } else {
                drv.backdelete();
            }
            HANDLED
        }
        Key::Delete => {
            state.undo.begin(&state.editor, EditRun::Delete);
            let (font_cx, layout_cx) = fonts.editor_contexts();
            let mut drv = state.editor.driver(font_cx, layout_cx);
            if word {
                drv.delete_word();
            } else {
                drv.delete();
            }
            HANDLED
        }
        _ => IGNORED,
    }
}

/// Filters control characters out of committed or pasted text. Multiline
/// editors keep newlines (normalized to `\n`); single-line editors strip
/// them with everything else.
fn sanitize(text: &str, multiline: bool) -> String {
    if multiline {
        text.replace("\r\n", "\n")
            .chars()
            .map(|c| if c == '\r' { '\n' } else { c })
            .filter(|c| *c == '\n' || !c.is_control())
            .collect()
    } else {
        text.chars().filter(|c| !c.is_control()).collect()
    }
}

/// Inserts committed text (typing or IME commit).
/// Applies one undo step. Returns whether anything changed.
pub(crate) fn undo(state: &mut EditorState, fonts: &mut Fonts) -> bool {
    let Some(snap) = state.undo.undos.pop() else {
        return false;
    };
    state.undo.redos.push(snapshot(&state.editor));
    apply_snapshot(state, fonts, &snap);
    true
}

/// Applies one redo step. Returns whether anything changed.
pub(crate) fn redo(state: &mut EditorState, fonts: &mut Fonts) -> bool {
    let Some(snap) = state.undo.redos.pop() else {
        return false;
    };
    state.undo.undos.push(snapshot(&state.editor));
    apply_snapshot(state, fonts, &snap);
    true
}

fn apply_snapshot(state: &mut EditorState, fonts: &mut Fonts, snap: &Snapshot) {
    state.editor.set_text(&snap.text);
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let mut drv = state.editor.driver(font_cx, layout_cx);
    drv.select_byte_range(snap.selection.0, snap.selection.1);
    state.undo.break_run();
}

pub(crate) fn handle_text(state: &mut EditorState, fonts: &mut Fonts, text: &str) -> EditOutcome {
    state.undo.begin(&state.editor, EditRun::Insert);
    let sanitized = sanitize(text, state.multiline);
    if sanitized.is_empty() {
        return IGNORED;
    }
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let mut drv = state.editor.driver(font_cx, layout_cx);
    drv.insert_or_replace_selection(&sanitized);
    HANDLED
}

/// Updates the IME preedit text.
pub(crate) fn handle_preedit(
    state: &mut EditorState,
    fonts: &mut Fonts,
    text: &str,
    cursor: Option<(usize, usize)>,
) {
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let mut drv = state.editor.driver(font_cx, layout_cx);
    if text.is_empty() {
        drv.clear_compose();
    } else {
        // Platform IMEs are not trusted to keep offsets in range, and
        // parley debug-asserts on out-of-range compose cursors.
        let cursor = cursor.map(|(a, b)| (a.min(text.len()), b.min(text.len())));
        drv.set_compose(text, cursor);
    }
}

/// Moves the caret (or extends the selection) to a pointer position given in
/// coordinates local to the text origin.
pub(crate) fn pointer_down(
    state: &mut EditorState,
    fonts: &mut Fonts,
    x: f64,
    y: f64,
    shift: bool,
) {
    state.undo.break_run();
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let mut drv = state.editor.driver(font_cx, layout_cx);
    #[expect(clippy::cast_possible_truncation, reason = "text coords fit in f32")]
    if shift {
        drv.shift_click_extension(x as f32, y as f32);
    } else {
        drv.move_to_point(x as f32, y as f32);
    }
}

/// Selects the word at a pointer position (double-click).
pub(crate) fn select_word_at(state: &mut EditorState, fonts: &mut Fonts, x: f64, y: f64) {
    state.undo.break_run();
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let mut drv = state.editor.driver(font_cx, layout_cx);
    #[expect(clippy::cast_possible_truncation, reason = "text coords fit in f32")]
    drv.select_word_at_point(x as f32, y as f32);
}

/// Selects the whole line at a pointer position (triple-click).
pub(crate) fn select_line_at(state: &mut EditorState, fonts: &mut Fonts, x: f64, y: f64) {
    state.undo.break_run();
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let mut drv = state.editor.driver(font_cx, layout_cx);
    #[expect(clippy::cast_possible_truncation, reason = "text coords fit in f32")]
    drv.select_line_at_point(x as f32, y as f32);
}

/// Extends the selection during a drag.
pub(crate) fn pointer_drag(state: &mut EditorState, fonts: &mut Fonts, x: f64, y: f64) {
    state.undo.break_run();
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let mut drv = state.editor.driver(font_cx, layout_cx);
    #[expect(clippy::cast_possible_truncation, reason = "text coords fit in f32")]
    drv.extend_selection_to_point(x as f32, y as f32);
}

/// Everything the painter needs for one input, resolved at build time.
pub(crate) struct InputPaint {
    pub placeholder: String,
    pub style: ResolvedText,
    pub placeholder_color: Color,
    pub caret_color: Color,
    pub selection_color: Color,
    pub focused: bool,
    /// Left/right padding inside the box, logical px.
    pub pad_x: f64,
    /// Top padding inside the box (multiline is top-aligned), logical px.
    pub pad_y: f64,
    /// Multiline mode: wrapped, top-aligned, no horizontal follow-scroll.
    pub multiline: bool,
}

/// Paints an input's content (selection, text or placeholder, caret) inside
/// `rect`, clipped, vertically centered, with horizontal follow-scroll.
/// Returns the focused caret rect (for IME positioning), if any.
pub(crate) fn paint(
    scene: &mut Scene,
    fonts: &mut Fonts,
    state: &mut EditorState,
    data: &InputPaint,
    rect: Rect,
    now: f64,
    reduced_motion: bool,
) -> Option<Rect> {
    let content = Rect::new(rect.x0 + data.pad_x, rect.y0, rect.x1 - data.pad_x, rect.y1);

    // Multiline editors wrap to the content width (set before layout).
    if data.multiline {
        #[expect(clippy::cast_possible_truncation, reason = "widths fit in f32")]
        state
            .editor
            .set_width(Some(content.width().max(0.0) as f32));
    }

    // Refresh the layout; single lines center vertically, multiline is
    // top-aligned under the padding.
    let (font_cx, layout_cx) = fonts.editor_contexts();
    let layout_height = f64::from(state.editor.layout(font_cx, layout_cx).height());
    let text_y = if data.multiline {
        rect.y0 + data.pad_y
    } else {
        rect.y0 + (rect.height() - layout_height) * 0.5
    };

    // Follow-scroll: keep the caret inside the content box (single line
    // only; multiline grows vertically instead).
    if data.multiline {
        state.scroll_x = 0.0;
    } else if let Some(caret) = state.editor.cursor_geometry(1.0) {
        let caret_x = caret.x0; // layout-local
        let visible_w = content.width().max(0.0);
        if caret_x - state.scroll_x > visible_w {
            state.scroll_x = caret_x - visible_w;
        }
        if caret_x - state.scroll_x < 0.0 {
            state.scroll_x = caret_x;
        }
        let max_scroll = (f64::from(
            state
                .editor
                .try_layout()
                .map_or(0.0, parley::Layout::full_width),
        ) - visible_w)
            .max(0.0);
        state.scroll_x = state.scroll_x.clamp(0.0, max_scroll + CARET_WIDTH);
    } else {
        state.scroll_x = 0.0;
    }
    let text_origin = (content.x0 - state.scroll_x, text_y);

    scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &rect);

    // Selection behind the glyphs.
    if data.focused {
        for (bb, _) in state.editor.selection_geometry() {
            let sel = Rect::new(
                text_origin.0 + bb.x0,
                text_origin.1 + bb.y0,
                text_origin.0 + bb.x1,
                text_origin.1 + bb.y1,
            );
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                data.selection_color,
                None,
                &sel,
            );
        }
    }

    // Text, or the placeholder when empty.
    if state.editor.raw_text().is_empty() {
        if !data.placeholder.is_empty() {
            let mut placeholder_style = data.style;
            placeholder_style.color = data.placeholder_color;
            let ph_bottom = if data.multiline {
                rect.y1 - data.pad_y
            } else {
                text_y + layout_height
            };
            let ph_rect = Rect::new(content.x0, text_y, content.x1, ph_bottom);
            fonts.paint(scene, &data.placeholder, &placeholder_style, ph_rect, None);
        }
    } else {
        let transform = Affine::translate(text_origin);
        let color = data.style.color;
        let (font_cx, layout_cx) = fonts.editor_contexts();
        let layout = state.editor.layout(font_cx, layout_cx);
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                let run = glyph_run.run();
                scene
                    .draw_glyphs(run.font())
                    .brush(color)
                    .hint(true)
                    .transform(transform)
                    .font_size(run.font_size())
                    .normalized_coords(run.normalized_coords())
                    .draw(
                        Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let gx = x + glyph.x;
                            let gy = y + glyph.y;
                            x += glyph.advance;
                            vello::Glyph {
                                id: glyph.id,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
            }
        }
    }

    // The caret, blinking on a 530ms half-period. The rect is computed
    // whenever focused (even while blink-hidden) so the IME popup can
    // anchor to it.
    let mut caret_rect = None;
    if data.focused {
        let phase = ((now - state.last_activity).max(0.0) / BLINK_HALF_PERIOD) as u64;
        let visible = reduced_motion || phase.is_multiple_of(2);
        if let Some(bb) = state.editor.cursor_geometry(1.0) {
            let caret = Rect::new(
                (text_origin.0 + bb.x0 - CARET_WIDTH * 0.5).max(rect.x0),
                text_origin.1 + bb.y0 + 1.0,
                (text_origin.0 + bb.x0 + CARET_WIDTH * 0.5).max(rect.x0 + CARET_WIDTH),
                text_origin.1 + bb.y1 - 1.0,
            );
            caret_rect = Some(caret);
            if visible {
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    data.caret_color,
                    None,
                    &caret,
                );
            }
        }
    }

    scene.pop_layer();
    caret_rect
}

#[cfg(test)]
mod tests {
    use super::{LayoutBrush, apply_style};
    use crate::style::{NumericSpacing, OpticalSizing, TextStyle};
    use crate::text::resolve_text;
    use crate::theme::Theme;
    use std::mem::discriminant;

    /// Toggling an OpenType feature (or the opsz axis) off on a *persistent*
    /// editor must clear the prior property, not leave it stuck (the 0.16
    /// known limitation). White-box: inspect the editor's resolved style set.
    #[test]
    fn editor_clears_toggled_off_feature_and_opsz() {
        let theme = Theme::light();
        let mut editor = parley::PlainEditor::<LayoutBrush>::new(16.0);

        let feat_key = discriminant(&parley::StyleProperty::<LayoutBrush>::FontFeatures(
            parley::FontFeatures::empty(),
        ));
        let var_key = discriminant(&parley::StyleProperty::<LayoutBrush>::FontVariations(
            parley::FontVariations::empty(),
        ));

        // Apply a style WITH tabular figures and auto optical sizing.
        let on = TextStyle {
            features: crate::style::FontFeatures {
                spacing: NumericSpacing::Tabular,
                ..Default::default()
            },
            optical: OpticalSizing::Auto,
            ..Default::default()
        };
        apply_style(&mut editor, &resolve_text(&on, &theme));
        assert!(
            editor.edit_styles().inner().contains_key(&feat_key),
            "feature property is set when enabled"
        );
        assert!(
            editor.edit_styles().inner().contains_key(&var_key),
            "opsz variation is set under Auto"
        );

        // Re-apply the default style (feature off, opsz Default): both must clear.
        apply_style(&mut editor, &resolve_text(&TextStyle::default(), &theme));
        assert!(
            !editor.edit_styles().inner().contains_key(&feat_key),
            "feature property is removed when toggled off"
        );
        assert!(
            !editor.edit_styles().inner().contains_key(&var_key),
            "opsz variation is removed when toggled off"
        );
    }
}

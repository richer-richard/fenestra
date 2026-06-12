# The builder vocabulary

Everything autocompletes; nothing is a macro.

**Constructors** — `div()`, `row()`, `col()`, `stack()` (z-stack),
`text(s)`, `spacer()`, `divider()`, `path(bez, viewbox, stroke)`,
`image_rgba8(w, h, pixels)`, `raw_input(value, placeholder)`,
`raw_text_area(value, placeholder)`.

**Layout** — padding `.p .px .py .pt .pr .pb .pl`, margins `.m .mx ...`,
`.gap`, sizes `.w .h .min_w .max_w .min_h .max_h` (`f32` = logical px,
`Length::Pct`), `.w_full() .h_full() .grow() .shrink0() .wrap()`,
alignment `.items_start/center/end/baseline()`,
`.justify_start/center/end/between()`, position `.absolute()` +
`.top/.right/.bottom/.left`, grid `.grid_cols/.grid_rows(tracks)` +
`.grid_col/.grid_row(start, span)`, `.overflow_hidden()`, `.scroll_y()`,
`.stick_to_bottom()`.

**Paint** — `.bg(color_or_gradient)`, `.border(w, color)`, `.rounded(r)`,
`.rounded_full()`, `.shadow(ShadowToken::Sm)`, `.opacity(v)`.

**Text** — `.size(TextSize::Sm)`, `.size_px(148.0)`, `.weight(Weight::Semibold)`,
`.color(c)`, `.mono()`, `.family(FamilyRole::Display)`, `.tracking(em)`,
`.leading(multiple)`, `.truncate()`, `.text_align(..)`.

**Interaction** — `.on_click(msg)`, `.on_right_click(msg)`,
`.on_double_click(msg)`, `.on_hover(msg)`, `.on_key(f)`, `.on_drag(f)`,
`.on_input(f)`, `.on_close(msg)`, `.on_file_drop(f)`, `.drag_source(s)`,
`.on_drop(f)`, `.focusable(true)`, `.autofocus()`,
`.disabled(b)`, `.cursor(Cursor::Pointer)`; state variants
`.hover/.active/.focus(|s| ...)` and `_themed` versions;
`.transition(Transition::colors())`; `.keyframes(..)`; `.spin(ms)`.

**Kit widgets** — `button checkbox switch radio slider text_input
text_area select tooltip modal toast_stack tabs card stat_card badge
avatar progress spinner table callout virtual_list menu dropdown_menu
context_menu popover combobox icons::* icons::lucide::*`.

**Tokens** — spacing `SP0..SP16`, radii `R_SM..R_FULL`, `TextSize`,
`Weight`, `ShadowToken`, `MotionDuration`, themes via
`Theme::{light, dark, from_accent, duotone}`.

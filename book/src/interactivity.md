# Interactivity and transitions

Pointer: `.on_click(msg)` fires on press+release over the same element;
the pressed element captures the pointer until release. `.on_drag(f)`
maps captured pointer positions (as 0..1 fractions of the element rect)
to messages — sliders are built on it. `.on_right_click(msg)` fires on
right press (pair it with `context_menu`); `.on_double_click(msg)` fires
on two clicks within 0.4 s — both single clicks still fire first, so use
it for select-then-open patterns.

Keyboard: Tab/Shift-Tab cycle `.focusable(true)` elements in tree order;
Enter/Space activate the focused clickable; `.on_key(f)` sees key presses
while focused. Focus rings paint only for keyboard-driven focus.
`.autofocus()` focuses an element when it first appears — dialogs and
search fields, without any imperative call.

## Drag and drop

Files from the OS: `.on_file_drop(|path| Msg::Import(path))` receives
each dropped file at the pointer position (the deepest handler under the
pointer wins; when the platform reports no position, the first handler
in tree order receives it).

Within the app: mark sources `.drag_source("payload")` and targets
`.on_drop(|payload| msg)`. Pressing a source starts the drag; releasing
over a target delivers the payload; releasing anywhere else cancels.

State variants layer styling without new elements:

```rust,ignore
div()
    .themed(|t, s| s.bg(t.surface_raised))
    .hover_themed(|t, s| s.bg(t.neutrals.step(3)))
    .active_themed(|t, s| s.bg(t.neutrals.step(4)))
    .focus_themed(|t, s| s.border(1.0, t.accent))
```

## Transitions

`.transition(Transition::colors())` animates property changes between
frames (colors and shadows by default; opt into lengths/offsets/opacity).
Retargeting mid-flight continues from the current value.
`Transition::spring()` (or `.with_spring(stiffness, damping)`) swaps
the duration+curve pair for physical motion: underdamped springs
overshoot on lengths and offsets and settle on physics, while colors,
opacity, and shadows clamp at the target. `.enter(transition)` plays
an element in from transparent the first time its id appears — list
rows, toasts; exit animations are not supported yet. Theme switching
crossfades automatically wherever `.transition(Transition::colors())`
is set: the themed target changes, the retarget machinery does the
rest. `Keyframes`
timelines handle looping ambient motion (pulses, shimmer), sampled from
the frame clock; `.spin(ms)` rotates paths (spinners). Reduced motion
snaps everything, keeping headless renders deterministic.

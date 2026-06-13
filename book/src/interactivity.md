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
while focused. Focus rings paint only for keyboard-driven focus — the
shadcn model: the control's border swaps to the accent and a soft 3px halo
sits flush outside it. Mark a field `.invalid(true)` to recolor the ring to
the danger hue. `.autofocus()` focuses an element when it first appears —
dialogs and search fields, without any imperative call.

## Drag and drop

Files from the OS: `.on_file_drop(|path| Msg::Import(path))` receives
each dropped file at the pointer position (the deepest handler under the
pointer wins; when the platform reports no position, the first handler
in tree order receives it).

Within the app: mark sources `.drag_source("payload")` and targets
`.on_drop(|payload| msg)`. Pressing a source starts the drag; releasing
over a target delivers the payload; releasing anywhere else cancels.

## State layers

The kit's controls share one interaction recipe — Material's state layer.
`.state_layer(|t| t.text)` declares the *content* color (the ink drawn on the
control), and the framework veils it over the control's container on hover
(8%), keyboard focus and press (12%), and drag (16%), so you never hand-pick a
hover color:

```rust,ignore
div()
    .themed(|t, s| s.bg(t.surface_raised))
    .state_layer(|t| t.text)   // hover / focus / press / drag, one recipe
    .press_scale()             // a 0.97 tactile dip while pressed
```

The per-state closures are still there for full control
(`.hover_themed` / `.active_themed` / `.focus_themed`). Solid brand fills use
them to *step the ramp* (`accent` → `accent_hover` → `accent_active`) rather
than take a veil, because a light content veil would wash a saturated accent
out. Disabled controls fade their container and dim content to `text_disabled`.

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

The easing families follow Material 3 — `EASE_STANDARD` for two-way state
changes, `EASE_DECELERATE` for entrances, `EASE_ACCELERATE` for exits — and
durations sit on the `MotionDuration` scale (`Micro` 100 ms / `Fast` 120 /
`Base` 200 / `Slow` 300), with `exit_ms` running an exit ~25% quicker than its
entrance. Keyboard-driven changes snap: a keyboard-focused control shows its
ring and state layer instantly rather than lagging behind a fast keyboard
user.

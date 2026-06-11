# Interactivity and transitions

Pointer: `.on_click(msg)` fires on press+release over the same element;
the pressed element captures the pointer until release. `.on_drag(f)`
maps captured pointer positions (as 0..1 fractions of the element rect)
to messages — sliders are built on it.

Keyboard: Tab/Shift-Tab cycle `.focusable(true)` elements in tree order;
Enter/Space activate the focused clickable; `.on_key(f)` sees key presses
while focused. Focus rings paint only for keyboard-driven focus.

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
Retargeting mid-flight continues from the current value. `Keyframes`
timelines handle looping ambient motion (pulses, shimmer), sampled from
the frame clock; `.spin(ms)` rotates paths (spinners). Reduced motion
snaps everything, keeping headless renders deterministic.

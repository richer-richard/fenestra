# Headless rendering and testing

The thesis feature. Everything renders without a window:

```rust,ignore
use fenestra::shell::{render_app, render_element, SyntheticEvent};

// Pixels for any element tree:
let image = render_element(view, &Theme::dark(), (800, 600));
image.save("preview.png")?;

// Drive a real app with scripted input, then look:
let image = render_app(&mut app, &[
    SyntheticEvent::Tab,
    SyntheticEvent::Text("hello".into()),
], (800, 600), &Theme::light());
assert_eq!(app.value, "hello");
```

Determinism: embedded fonts, scale 1.0, reduced motion, in-memory
clipboard, sizes clamped to the device range. Custom design faces render
through `render_element_with(el, theme, size, &mut fonts)`.

## Golden tests

`assert_png_snapshot(dir, name, &image)` compares against a committed
PNG with a small tolerance (3/255 per channel, 0.2% pixel budget;
macOS/Metal is the reference platform, software rasterizers in CI widen
the budget via `FENESTRA_SNAPSHOT_BUDGET`). Failures write
`<name>.actual.png` next to the golden. `FENESTRA_UPDATE_SNAPSHOTS=1`
regenerates — then look at the images before committing.

fenestra's own kit is verified this way: every widget, every state, both
themes, on every push, on three operating systems.

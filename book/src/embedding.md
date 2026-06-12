# Embedding in your wgpu app

The batteries-included runner (`fenestra::run`) is the easy path. The
narrow waist underneath it is `Embedded`: run a fenestra `App` inside
an event loop, device, and surface that *you* own — a game, an engine
editor, an existing wgpu tool.

```rust,ignore
use fenestra::shell::{Embedded, wgpu, winit};

// Once, on your device (the renderer compiles vello's shaders here):
let mut ui = Embedded::new(MyHud::default(), Theme::dark(), &device, surface_format);
ui.set_clear(Color::TRANSPARENT);   // your scene shows through

// Every winit event:
let response = ui.handle_window_event(&window, &event);
if response.repaint { window.request_redraw(); }
if response.consumed { return; }    // fenestra took it — skip your handling

// Every frame, after your own passes:
ui.render(&device, &queue, &surface_view, (width, height), scale_factor);
```

What the pieces mean:

- **`render`** builds the frame, paints it with vello into an internal
  premultiplied-alpha texture on your device, and composites onto the
  target view with alpha blending. With a transparent clear, the UI
  floats over whatever you drew first. For custom compositing (sampling
  the UI in your own pipeline), take `texture_view()` instead and skip
  the built-in blit.
- **`handle_window_event`** uses the same winit translation as the
  runner — printable/shortcut keyboard split, IME commit/preedit,
  modifier tracking, wheel conventions. `EventResponse.consumed` is the
  arbitration contract: true when the pointer is over fenestra content
  or a widget holds keyboard focus.
- **`input(InputEvent)`** is the window-system-agnostic layer beneath
  it — what non-winit hosts and tests drive.
- **`pump()`** drains proxied messages (from `App::init` / threads);
  `animating()` says whether to keep scheduling frames; `frame()`
  exposes the last frame for semantic queries and inspector dumps —
  embedded UIs are just as verifiable as windowed ones.

Version-matching matters: integration code must use the same `wgpu` and
`winit` fenestra was built with, so the shell re-exports both
(`fenestra::shell::{wgpu, winit, vello}`).

Out of scope in embedded mode (use the runner): secondary windows
(`App::windows`) and IME candidate-window positioning.
`examples/embedded.rs` is a complete host app.

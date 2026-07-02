# fenestra-motion

Declarative motion graphics over [fenestra](https://github.com/richer-richard/fenestra):
a composition is a **pure function of integer frame number**, sampled into
fenestra element trees, rasterized through the existing headless pipeline,
and written out as straight-alpha PNG sequences or piped to ffmpeg. The
target user is an agent that authors a timeline, renders frames, inspects
them, and asserts on them — no human eye in the loop required.

```rust
use fenestra_core::text;
use fenestra_motion::{Clip, Composition, Frames, Prop, Track, key, EASE_CRISP};

let comp = Composition::new(1920, 1080, 60)
    .duration(Frames(300))
    .clip(
        Clip::new("title", 0..120)
            .element(|| text("Q3 Revenue").size_px(96.0))
            .animate(Prop::Opacity, Track::new([
                key(0, 0.0f32).ease(EASE_CRISP),
                key(30, 1.0),
            ]))
            .animate(Prop::TranslateY, Track::new([
                key(0, 24.0f32).ease(EASE_CRISP),
                key(30, 0.0),
            ])),
    );

comp.render_frame(Frames(45))?;              // any frame, standalone
comp.sample(Frames(90)).resolve("title");    // props + bbox, no pixels
```

## Concepts

- **Composition** — canvas (`width × height` logical px), `fps`, duration,
  background (transparent by default), theme, clips. `sample(frame)`
  depends only on `(composition, frame)`.
- **Clip** — a unique string id, an active span `start..end` (half-open,
  comp frames; invisible outside it), z-order (insertion order, `.z(i32)`
  overrides), an anchor, content, and typed animation tracks.
- **Content is a factory.** fenestra element trees are single-use and
  rebuilt every frame — the framework's own rendering model — so
  `.element(|| …)` takes a closure, and `Clip::dynamic(id, span, |frame| …)`
  is the escape hatch that builds any tree from the clip-relative frame
  (data-driven charts, typewriters, counters).
- **Track** — typed keyframes `key(at, value).ease(..)`, frames
  clip-relative (frame 0 = the clip's first active frame). Hold before the
  first key, hold after the last, exact hits return the key value, a
  single-key track is a constant.
- **Props v1** — `Opacity`, `TranslateX/Y`, `Scale`, `ScaleXY`, `Rotate`
  (degrees), `FillColor`, `StrokeColor`, `TextColor`. Transform order is
  fixed: **scale, then rotate, then translate, all about the anchor**
  (bbox-relative, center by default). Color tracks style the clip's *root*
  element (fenestra styles don't cascade); deeper nodes belong to the
  dynamic closure.
- **Easing** — `linear()`, `hold()`, CSS `ease_in/out/in_out()`, any
  `CubicBezier` (including fenestra's Material tokens), the named curves
  `EASE_CRISP` / `EASE_EDITORIAL` / `EASE_POP`, and closed-form damped
  springs (`spring(stiffness, damping)`, optional initial velocity). A
  spring occupies its segment and settles to the segment's end value
  (decay envelope < 0.1%); a shorter segment truncates the tail. Overshoot
  curves (control y > 1, springs) extrapolate geometry past its endpoints
  mid-segment and clamp colors — the same rule fenestra's interactive
  engine applies.
- **Color** interpolates in **Oklab** by default; `.srgb()` opts a track
  into raw component lerp. No raw hex anywhere — theme roles or `oklch`.

## The determinism contract

Layered, and stated exactly:

1. **Sampling is exactly deterministic.** `sample(frame)`, `resolve(id)`,
   bboxes, lints: pure math plus deterministic layout (embedded Inter,
   scale 1.0, reduced motion). Byte-equal, every time, on every machine.
2. **Pixels carry the GPU's noise floor, nothing more.** vello's compute
   rasterizer wobbles ±1 LSB on a tiny fraction of antialiased pixels even
   for the same scene rendered twice in one process (measured on the
   macOS/Metal reference). CI pins renders to that bound: same frame twice,
   and parallel sequence vs standalone frames, agree within ±1 per channel
   on < 0.1% of pixels. Parallel rendering adds *nothing* on top — frames
   fan out over rayon, the GPU serializes behind one device mutex, and an
   order-restoring writer emits byte-stable files.
3. **Across machines**, renders are compared with the workspace golden
   tolerance (3/255 per channel, 0.2% of pixels; macOS/Metal is the
   reference, software rasterizers widen the budget).

Purity rules that make this true: no wall clock, no RNG, no dependence on
prior frames, integer frames as ground truth (seconds are derived per
frame as `frame / fps`, never accumulated).

**FORBIDDEN inside clip content:** fenestra's wall-clock animation surface —
`.transition(..)`, `.keyframes(..)`, `.spin(..)`, `.enter(..)`/`.exit(..)`,
`.animate_layout()`. Headless rendering pins it (reduced motion), so it
will not render, and it breaks frame purity. Animate through tracks or the
`Clip::dynamic` frame argument, always.

## The data form

RON is primary (JSON parses the same shape), versioned (`version: 1`),
strict (`deny_unknown_fields`, path-pointed errors):

```ron
(
    version: 1,
    width: 1280, height: 720, fps: 60,
    duration: 240,
    background: "transparent",   // or a theme role / (oklch: (l, c, h))
    theme: dark,
    clips: [(
        id: "title",
        start: 10, end: 240,
        anchor: bottom_left,
        element: text(content: "Ada Lovelace", style: (size_px: 40.0, color: "text")),
        tracks: [(prop: opacity, keys: [
            (at: 0, value: scalar(0.0), ease: crisp),
            (at: 16, value: scalar(1.0)),
        ])],
    )],
)
```

Clip content embeds the **fenestra-describe (`fenestra/1`) node
vocabulary** — the same grammar the `fenestra` CLI and MCP server speak —
so a motion document authors real fenestra UI: text, containers, badges,
buttons, icons, style blocks, colors as theme roles or `oklch`.
`Composition::from_ron / from_json / to_ron / to_json` round-trip;
`Clip::dynamic` is code-only (closures don't serialize).
See [`examples/lower_third.ron`](examples/lower_third.ron).

## The CLI

```
motion render <comp.ron> --frame 45 --out f.png [--scale 0.25]
motion render <comp.ron> --frames 0..240 --out frames/
motion render <comp.ron> --mp4 out.mp4          # ffmpeg pipe, opaque path
motion probe  <comp.ron> --frame 45 [--clip title]
motion lint   <comp.ron>                        # exit 1 on findings
motion sheet  <comp.ron> --every 30 --out sheet.png
```

JSON to stdout, artifacts to `--out`, notes to stderr; exit `0` ok, `1` a
verification failed, `3` a parse/IO error. `--scale 0.25` renders 1/16 of
the pixels for a cheap mid-authoring look.

## Verification cookbook

Structural, pre-raster — the point of the crate:

```rust
use fenestra_motion::verify::{discontinuities, monotone, settled, Direction};
use fenestra_motion::{Frames, Prop};

// Props and bboxes at any instant, no pixels:
let scene = comp.sample(Frames(45));
let title = scene.resolve("title").unwrap();
assert!(title.visible);
assert!((title.props.opacity - 1.0).abs() < 1e-6);
let bbox = title.bbox.unwrap();               // post-transform AABB
assert!(bbox.y1 <= 720.0 - 48.0);             // respects the safe area
assert_eq!(scene.paint_order(), ["plate", "bar", "title", "subtitle"]);

// Temporal lints over ranges:
assert!(discontinuities(&comp, None).is_empty());   // no undeclared jumps
assert!(monotone(&comp, "title", Prop::Opacity, 10..26, Direction::Increasing).is_empty());
assert!(settled(&comp, Frames(40)).is_empty());     // nothing moves after 40

// Golden coverage at the interesting instants:
for frame in comp.sentinel_frames() { /* render + assert_png_snapshot */ }

// One image an agent reviews in a single look:
comp.contact_sheet(30, 240)?.save("sheet.png")?;
```

Intentional jumps are declared: `.cut(Frames(n))` blesses a discontinuity
at `n` (a hard cut between scenes) so the lint stays honest everywhere else.

### Patterns

- **Timing vs mapping.** Compute one normalized progress in a dynamic clip
  and derive several properties from it, instead of duplicating keyframes:
  a `Track<f32>` from `(0, 0.0)` to `(n, 1.0)` *is* a progress function —
  sample it, then map.
- **Stagger.** One clip per item, spans offset by a constant step
  (`i * 6..end`); hold-before-first-key means a clip that hasn't started
  simply rests at its first value. See `demos::title_stagger`, which also
  measures word slots by probing a layout pass instead of hand-tuning
  pixels.
- **Typewriter.** String slicing per frame in a dynamic clip — never
  per-character opacity: `text(&full[..chars_at(frame)])`. A blinking
  caret is `frame % blink < blink / 2`.
- **Data races.** Keep the *data* in `Track<f32>`s and rebuild the chart
  per frame from rank-sorted samples (`demos::chart_race`) —
  fenestra-charts is a pure function of `(data, theme)`.

## Designing frames (video-first, not web-first)

Videos are watched, not read. One message per frame; a generous safe area
(≥ 80px sides / 100px top-bottom at 1080-wide, scaled); headline ≥ 84px,
supporting ≥ 44px, labels ≥ 32px at 1080-wide. Reserve layout slots with
fenestra's `col`/`row`/`stack` and animate elements *from their slot* via
transforms — don't scatter absolutes, and solve crowding with time (another
scene) instead of shrinking type. Before rendering a sequence, look at one
representative frame (`motion render --frame N`) or the contact sheet.

## ffmpeg recipes

The `--mp4` pipe is the fast opaque path (`-f rawvideo -pix_fmt rgba` on
stdin → `yuv420p`). Alpha is deliberately *not* video-encoded here — render
a straight-alpha PNG sequence and encode:

```sh
# ProRes 4444 (.mov) — for video editors
ffmpeg -framerate 60 -i frames/frame_%05d.png \
  -c:v prores_ks -profile:v 4444 -pix_fmt yuva444p10le lower_third.mov

# VP9 (.webm) — for browsers
ffmpeg -framerate 60 -i frames/frame_%05d.png \
  -c:v libvpx-vp9 -pix_fmt yuva420p lower_third.webm
```

PNG paths never require ffmpeg; the pipe fails loudly, naming the binary,
when it's missing. `yuv420p` wants even dimensions — keep width and height
divisible by two.

## Demos

| demo | claim it stresses |
| --- | --- |
| `lower_third` (`examples/lower_third.ron` + example) | the data form, transparent background, straight-alpha delivery |
| `title_stagger` | per-word entrances via staggered clips, probe-measured layout |
| `chart_race` | `Clip::dynamic` + fenestra-charts rebuilt per frame from interpolated data |

`cargo run -p fenestra-motion --example <name>`; each demo lints clean,
asserts its own claim structurally, and is pinned by sentinel goldens.

## Out of scope (v1)

Lottie/bodymovin import, audio, stateful particles/physics, motion blur,
a GUI scrubber, video decode, per-glyph text animators, timeline diffing.

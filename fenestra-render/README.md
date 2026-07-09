# fenestra-render

The command-line renderer and verification harness for
[fenestra](https://github.com/richer-richard/fenestra): render a UI described as
JSON to pixels, drive it through scripted interactions, and compare against
baselines.

## Install

```sh
cargo install fenestra-render   # installs the `fenestra` binary
```

## Subcommands

| command | what it does |
| --- | --- |
| `render` | render to an access tree (stdout JSON), pixels (`--out`), and a11y warnings |
| `query` | find nodes by a semantic selector |
| `interact` | drive scripted interactions; report emitted intents + the after-tree |
| `check` | check contrast, labeling, and per-node legibility |
| `focus-order` | list the keyboard focus order: the refs a Tab cycle visits, in order |
| `layout` | report layout problems: small hit targets and off-screen nodes |
| `match-aria` | match an expected aria snapshot (partial / strict / regex) |
| `match-png` | compare against a baseline screenshot (tolerance, budget, `--mask`) |
| `vocabulary` | print the description grammar |
| `schema` | print the JSON Schema for the fenestra/1 description format |
| `validate` | validate a description without rendering |
| `verify` | run a scenario: drive steps, assert every expectation, one verdict |
| `preview` | open a live-reload window for a description file |
| `film` | render a filmstrip: drive optional steps, capture frames with real motion on, compose one strip PNG |

A description is read from a path or stdin (`-`); results are JSON on stdout, and
any image goes to `--out`. Exit codes: `0` ok, `1` a verification failed, `3` a
parse or IO error. Two exceptions: `preview` takes a real file path (there's
nothing to reload from a pipe) and blocks until the window closes; `film` never
exits `1` since it composes a filmstrip rather than checking a pass/fail
condition.

`match-png` ignores a rectangle when comparing with a repeatable
`--mask x,y,w,h` flag (logical pixels), e.g. `--mask 10,10,80,20 --mask
200,0,40,40` to exclude two volatile regions (a clock, a spinner) from the
pixel diff.

`preview <file.json>` opens a native window and re-renders whenever the file
changes on disk (polled, no filesystem-watcher dependency) — save-to-see
authoring instead of a render/inspect round trip. A parse error never crashes
or blanks the window: the last description that loaded cleanly keeps
rendering, with a themed error panel over it naming the problem; fix the file
and save again to clear it.

`film <desc.json> --frames N --interval-ms M --out strip.png` drives optional
`--steps` (applied first, so a click can trigger the transition to watch),
then captures `N` renders `M` milliseconds apart with real motion turned on —
every other subcommand stays reduced-motion for deterministic pixels; `film`
is the one place the point is watching motion play. The frames compose into
one captioned strip (`--scale` shrinks each cell, `0.05..=1.0`); JSON metadata
(the actual frame count/interval/scale used, after clamping, plus the strip's
pixel dimensions) goes to stdout. Hostile `--frames`/`--interval-ms`/`--scale`
values clamp rather than hang or crash; a legitimately oversized combination
(many frames at full scale) is a clean exit-3 error, never a panic.

## Example

```sh
echo '{"schema":"fenestra/1","root":{"button":{"label":"Add","on_click":"add"}}}' \
  | fenestra render --size 480x320 --out ui.png
```

## Determinism

Rendering goes through fenestra's headless path — scale 1.0, reduced motion,
embedded fonts only — so the same description renders identical pixels. Goldens
are referenced against macOS / Metal output (3/255 per-channel + 0.2% tolerance).

## License

Licensed under either of MIT or Apache-2.0 at your option.

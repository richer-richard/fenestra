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

A description is read from a path or stdin (`-`); results are JSON on stdout, and
any image goes to `--out`. Exit codes: `0` ok, `1` a verification failed, `3` a
parse or IO error. `preview` is the exception: it takes a real file path (there's
nothing to reload from a pipe) and blocks until the window closes.

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

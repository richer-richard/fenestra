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

A description is read from a path or stdin (`-`); results are JSON on stdout, and
any image goes to `--out`. Exit codes: `0` ok, `1` a verification failed, `3` a
parse or IO error.

`match-png` ignores a rectangle when comparing with a repeatable
`--mask x,y,w,h` flag (logical pixels), e.g. `--mask 10,10,80,20 --mask
200,0,40,40` to exclude two volatile regions (a clock, a spinner) from the
pixel diff.

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

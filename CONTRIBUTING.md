# Contributing to fenestra

Thanks for helping. fenestra aims to be the GUI framework both humans and
AI agents can build with and *verify* — pull requests are held to that
bar, and the tooling makes it easy to clear.

## Setup

```sh
git clone https://github.com/richer-richard/fenestra
cd fenestra
cargo test --workspace   # needs a GPU or software adapter (lavapipe/WARP)
```

Linux needs `libfontconfig1-dev pkg-config` and a Vulkan driver
(`mesa-vulkan-drivers` works headlessly). macOS and Windows work out of
the box.

## Quality gates (every PR)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- **Never suppress a warning that can be fixed.** No `#[allow]`,
  no env silencing. `#[expect(..., reason = "...")]` is acceptable only
  when the flagged behavior is genuinely intended.
- Bug fixes come with a regression test that fails before the fix.
- New widgets come with a golden test and accessible semantics
  (`.semantics(..)`, `.label(..)`).

## Visual snapshot workflow

PNG goldens live next to their tests in `tests/snapshots/`.

- `FENESTRA_UPDATE_SNAPSHOTS=1 cargo test` regenerates them — then **look
  at the images** before committing.
- macOS/Metal is the reference platform; CI's software rasterizers run
  with `FENESTRA_SNAPSHOT_BUDGET=0.006`.
- On failure, `<name>.actual.png` is written next to the golden for
  comparison.
- Text snapshots use `cargo insta review`.

## Architecture

Read [ARCHITECTURE.md](ARCHITECTURE.md) first — it records every
integration decision (pipeline, widget identity, transitions, overlays,
accessibility) milestone by milestone, including deliberate deviations.
The invariants that are not up for grabs:

- `fenestra-core` and `fenestra-kit` build and test **without a window**.
- Zero proc macros in the public API.
- Rendering stays a pure function of `(tree, theme, size, scale)` plus one
  retained `FrameState` keyed by stable `WidgetId`s.
- Kit widgets route every color through theme tokens.
- `unsafe_code = "forbid"` workspace-wide.

## Dependencies

Resolve the latest stable on crates.io and read the resolved version's
docs/source before integrating anything new — never pin from memory. CI
actions are pinned to full commit SHAs.

## Questions

Open a [discussion or issue](https://github.com/richer-richard/fenestra/issues).
Small, focused PRs land fastest.

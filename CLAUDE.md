# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

`fenestra` is a pure-Rust native GUI framework (winit + wgpu + vello + parley + taffy)
with web-grade aesthetics and first-class headless PNG rendering. The kickoff spec
defines the design tokens, builder vocabulary, and milestone order; ARCHITECTURE.md
records integration decisions as they are made.

- Workspace: `fenestra` (facade), `fenestra-core` (IR/theme/layout/paint),
  `fenestra-shell` (winit/wgpu/headless), `fenestra-kit` (widgets).
- `fenestra-core` and `fenestra-kit` must build and test without a window.
- Zero proc macros in the public API. Colors only through theme tokens in kit/examples.
- Do not publish to crates.io; Richard handles releases.

## Working rules

- You must use TaskCreate/TaskUpdate for any tasks unless they are very
  straightforward and require editing less than 3 files.
- Full CI/CD workflow is maintained in `.github/workflows/`. Keep it green.
- For warnings during tests/builds: DO NOT SUPPRESS THEM. If there are
  warnings/errors, FIX THEM. Never `#[allow(...)]` or env-silence your way past
  a warning that can be fixed properly.
- Quality gates for every milestone: `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Never pin dependency versions from memory: resolve latest stable on crates.io
  and read the docs/source of the resolved version before integrating.
- Snapshot updates: `FENESTRA_UPDATE_SNAPSHOTS=1` regenerates PNG goldens;
  `cargo insta review` for text snapshots.

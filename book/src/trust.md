# Trust and security

What you can verify about fenestra before staking anything on it — and
where the honest boundaries are.

## Code

- **No unsafe code.** `unsafe_code = "forbid"` across the workspace —
  not a guideline, a compile error. Memory-safety risk lives in the
  dependency tree (wgpu, vello, parley, winit), which is the explicit
  trust boundary: mature, widely-deployed Linebender/gfx-rs projects.
- **Totality under hostile input.** Property tests assert that any
  element tree at any viewport builds and paints without panicking;
  weekly fuzzing (libFuzzer) hammers theme-file parsing, layout, and
  the text-input pipeline. Scenario JSON and theme files reject unknown
  fields rather than guessing.
- **MSRV is declared and enforced**: `rust-version = "1.88"`, built in
  CI on exactly that toolchain. Minor releases may raise it; the
  CHANGELOG records when.

## Supply chain

- `cargo audit` (RustSec advisories) and `cargo deny` (license
  allowlist, registry pinning, ban rules) run on every push and weekly.
- Every GitHub Actions step is pinned to a full commit SHA, resolved at
  integration time.
- Releases publish from CI on tagged commits after the full gate suite
  re-runs. The packaged `.crate` files are attached to each GitHub
  release with **provenance attestations** — verify one with:

  ```sh
  gh attestation verify fenestra-core-*.crate --repo richer-richard/fenestra
  ```

## Quality gates

- rustfmt, clippy `-D warnings`, and the full test suite on macOS/Metal
  (the golden-reference platform), Linux/lavapipe, and Windows
  (compile + core tests; the WARP rasterizer's instability is
  documented, not hidden).
- Golden pixel tests with explicit budgets — the
  [determinism contract](determinism.md) states exactly what is
  guaranteed.
- A line-coverage floor on `fenestra-core` enforced in CI (raised as
  coverage grows; never lowered without a recorded decision), and
  performance gates with generous ceilings that catch order-of-
  magnitude regressions without flaking on shared runners.

## Reporting

Vulnerabilities: use GitHub's private reporting (Security → Report a
vulnerability). [SECURITY.md](https://github.com/richer-richard/fenestra/blob/main/SECURITY.md)
has scope and response expectations.

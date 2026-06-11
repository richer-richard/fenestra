## What

<!-- One paragraph: what this changes and why. -->

## Checklist

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` (no new suppressions)
- [ ] `cargo test --workspace`
- [ ] Bug fix → includes a regression test that fails without the fix
- [ ] New widget/visual change → golden test added/updated and the PNGs inspected
- [ ] Interactive widget → exposes `Semantics` and a label
- [ ] Decision worth recording → noted in ARCHITECTURE.md

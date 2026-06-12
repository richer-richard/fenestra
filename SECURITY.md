# Security policy

## Supported versions

The latest minor release line receives fixes. Pre-1.0, that means: only
the most recent `0.x` gets patches; upgrade forward rather than waiting
for backports.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on this repository
(Security → Report a vulnerability), which reaches the maintainer
privately. Please do not open public issues for suspected
vulnerabilities.

You can expect an acknowledgment within a week and an assessment or fix
plan within two. Credit is given in the release notes unless you ask
otherwise.

## Scope

In scope:

- Memory safety issues reachable through fenestra (the workspace is
  `unsafe_code = "forbid"`, so these arrive via dependencies — reports
  welcome either way; we coordinate upstream).
- Panics or unbounded resource use triggered by *untrusted input*:
  scenario JSON, theme files (`ThemeSpec`), hostile element trees,
  malicious font or image bytes routed through fenestra APIs.
- The release pipeline (crates.io publishing, GitHub releases,
  attestations) and CI supply chain.

Out of scope:

- Panics from API misuse that the documentation calls out.
- Issues in applications built with fenestra (report to those apps).

## What we run continuously

`cargo audit` (RustSec advisories) and `cargo deny` (advisories,
license allowlist, source pinning) on every push and weekly; fuzz
targets weekly; all workflow actions pinned to commit SHAs; releases
publish from CI with provenance attestations. See the book's trust
page for the full picture.

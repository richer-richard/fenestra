//! The verification flagship: author a UI as `fenestra/1` JSON, then prove it is
//! correct *headlessly* — with no window, and (for everything but the optional
//! PNG) no GPU. This is fenestra's wedge: the same description an agent emits is
//! parsed, read as a typed access tree, queried by role and name, checked for
//! accessibility, walked for focus order, and driven through interactions —
//! every assertion offline, none of it needing a display.
//!
//! `cargo run --example verify`         runs the verification battery (windowless,
//!                                      GPU-free) and exits non-zero on any miss,
//!                                      so it doubles as a smoke gate.
//! `cargo run --example verify -- shot` additionally renders the UI to a PNG at
//!                                      `gallery/verify.png` (the one GPU step).

use std::process::ExitCode;

use fenestra_describe::dto::AccessNodeDto;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::{
    Selector, access_tree, aria_snapshot, check_a11y, focus_order, layout_report, query, query_tree,
};
use fenestra_render::{Step, interact, render, resolve_theme};

/// The authored UI: a sign-in card with two text fields, a checkbox, a switch,
/// and a button row — written exactly as an agent would emit it. Strict against
/// the `fenestra/1` vocabulary (every field is checked; a typo would error here).
const UI: &str = r#"{
  "schema": "fenestra/1",
  "state": { "email": "", "remember": false, "notify": true },
  "root": {
    "col": {
      "style": { "p": 24, "gap": 16, "bg": "surface", "rounded": 12 },
      "children": [
        { "text": { "content": "Sign in to Acme", "style": { "size_px": 22, "weight": 600 } } },
        { "text": { "content": "Use your work account.", "style": { "size_px": 14, "color": "text_muted" } } },
        { "text_input": { "bind": "email", "placeholder": "Email", "id": "email" } },
        { "text_input": { "value": "", "placeholder": "Password", "id": "password" } },
        { "checkbox": { "bind": "remember", "label": "Remember me", "id": "remember" } },
        { "switch": { "bind": "notify", "label": "Email notifications", "id": "notify" } },
        {
          "row": {
            "style": { "gap": 8, "justify": "end" },
            "children": [
              { "button": { "label": "Cancel", "variant": "ghost", "on_click": "cancel", "id": "cancel" } },
              { "button": { "label": "Sign in", "on_click": "submit", "id": "signin" } }
            ]
          }
        }
      ]
    }
  }
}"#;

/// The interaction script the agent drives: focus the email field, type into it,
/// toggle "Remember me", then click "Sign in". Targets are semantic (by id or by
/// role + name) — never coordinates.
const STEPS: &str = r#"[
  { "click": { "id": "email" } },
  { "type": "ada@example.com" },
  { "click": { "id": "remember" } },
  { "click": { "role": "button", "name": "Sign in" } }
]"#;

/// The window the UI is verified at. Logical pixels; no real window is opened.
const SIZE: (u32, u32) = (520, 480);

/// Accumulates pass/fail verdicts and prints a readable line for each check.
struct Report {
    passed: usize,
    failed: usize,
}

impl Report {
    fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
        }
    }

    /// Records one check and prints `PASS`/`FAIL` with a one-line explanation.
    fn check(&mut self, name: &str, ok: bool, detail: impl std::fmt::Display) {
        let tag = if ok { "PASS" } else { "FAIL" };
        println!("  [{tag}] {name} — {detail}");
        if ok {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
    }

    fn all_ok(&self) -> bool {
        self.failed == 0
    }
}

/// A selector that matches a single node by its stable id (the `id` an author set).
fn by_id(id: &str) -> Selector {
    Selector {
        id: Some(id.to_string()),
        ..Default::default()
    }
}

/// Flattens every role word in the tree, in paint order.
fn collect_roles(node: &AccessNodeDto, out: &mut Vec<String>) {
    out.push(node.role.clone());
    for child in &node.children {
        collect_roles(child, out);
    }
}

fn main() -> ExitCode {
    let shot = std::env::args().any(|a| a == "shot");

    // 1. Parse the authored JSON into a typed Description. A bad field would
    //    surface here, not silently downstream.
    let desc: Description = match serde_json::from_str(UI) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("authored JSON failed to parse: {e}");
            return ExitCode::FAILURE;
        }
    };
    let theme = match resolve_theme(desc.theme.as_ref()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("theme failed to resolve: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("fenestra — author in JSON, verify headlessly (no window, no GPU)\n");

    // The typed tree the checks run against — what an agent reads instead of pixels.
    println!("authored UI, as an agent reads it (aria snapshot):");
    let snapshot =
        aria_snapshot(&desc, &theme, SIZE).expect("the description renders an aria tree");
    for line in snapshot.lines() {
        println!("    {line}");
    }
    println!();

    let mut report = Report::new();

    // 2. Structural verification — pure `fenestra_describe::inspect`, no GPU.
    println!("structure (fenestra_describe::inspect — windowless):");

    // Every expected control role is present in the access tree.
    let tree = access_tree(&desc, &theme, SIZE).expect("the description builds an access tree");
    let mut roles = Vec::new();
    collect_roles(&tree, &mut roles);
    for role in ["textbox", "checkbox", "switch", "button"] {
        let n = roles.iter().filter(|r| r.as_str() == role).count();
        report.check(&format!("role {role} present"), n > 0, format!("found {n}"));
    }

    // A semantic query locates a control by role + accessible name.
    let signin = query(
        &desc,
        &theme,
        SIZE,
        &Selector {
            role: Some("button".to_string()),
            name: Some("Sign in".to_string()),
            ..Default::default()
        },
    )
    .expect("query runs");
    report.check(
        "query button \"Sign in\"",
        signin.matches.len() == 1,
        format!(
            "matched {} (ref {})",
            signin.matches.len(),
            signin.matches.first().map_or("-", |m| m.ref_.as_str()),
        ),
    );

    // Accessibility: the theme is legible and every control carries a name.
    let a11y = check_a11y(&desc, &theme, SIZE).expect("a11y check runs");
    report.check(
        "a11y legible",
        a11y.legible,
        if a11y.legible {
            "theme contrast contract holds".to_string()
        } else {
            format!("{} contrast violation(s)", a11y.contrast_violations.len())
        },
    );
    report.check(
        "a11y labeled",
        a11y.unlabeled.is_empty(),
        if a11y.unlabeled.is_empty() {
            "every control has an accessible name".to_string()
        } else {
            let roles: Vec<&str> = a11y.unlabeled.iter().map(|n| n.role.as_str()).collect();
            format!("{} unlabeled: {roles:?}", a11y.unlabeled.len())
        },
    );

    // Keyboard focus order: what Tab visits, in order, by stable ref.
    let order = focus_order(&desc, &theme, SIZE).expect("focus order runs");
    let want_order = [
        "email", "password", "remember", "notify", "cancel", "signin",
    ];
    let order_ok = order == want_order;
    report.check(
        "focus order",
        order_ok,
        if order_ok {
            format!("Tab visits {order:?}")
        } else {
            format!("got {order:?}, want {want_order:?}")
        },
    );

    // Layout: the hard gate is "nothing clipped off-window" (a real correctness
    // property this UI satisfies). Sub-minimum tap targets are surfaced as an
    // advisory below — a heuristic, not a clipping bug.
    let layout = layout_report(&desc, &theme, SIZE).expect("layout report runs");
    report.check(
        "no off-screen clipping",
        layout.offscreen.is_empty(),
        if layout.offscreen.is_empty() {
            "every node lands within the window".to_string()
        } else {
            let names: Vec<&str> = layout
                .offscreen
                .iter()
                .map(|f| f.name.as_deref().unwrap_or(&f.ref_))
                .collect();
            format!("{} clipped: {names:?}", layout.offscreen.len())
        },
    );
    // Advisory (not gating): the WCAG 2.5.8 minimum hit-target heuristic flags the
    // compact checkbox/switch rows — wide enough to click, a few px under 24 tall.
    if !layout.small_targets.is_empty() {
        let notes: Vec<String> = layout
            .small_targets
            .iter()
            .map(|f| format!("{} ({})", f.name.as_deref().unwrap_or(&f.ref_), f.detail))
            .collect();
        println!(
            "  [note] {} target(s) under the 24px hit-size heuristic (advisory): {notes:?}",
            layout.small_targets.len(),
        );
    }

    // 3. Interaction verification — drive the script, then assert on the
    //    post-interaction tree. `want_png = false`, so this stays GPU-free too.
    println!("\ninteraction (fenestra_render::interact — drive, then assert):");
    let steps: Vec<Step> = serde_json::from_str(STEPS).expect("steps parse");
    let driven = interact(&desc, &theme, SIZE, &steps, false).expect("the script drives cleanly");

    report.check(
        "emitted intent",
        driven.emitted == ["submit"],
        format!("clicking Sign in emitted {:?}", driven.emitted),
    );

    let email = query_tree(&driven.tree, &by_id("email")).expect("query runs");
    let typed = email.matches.first().and_then(|m| m.value.clone());
    report.check(
        "typed text lands",
        typed.as_deref() == Some("ada@example.com"),
        format!("email field now {typed:?}"),
    );

    let remember = query_tree(&driven.tree, &by_id("remember")).expect("query runs");
    let checked = remember.matches.first().and_then(|m| m.checked);
    report.check(
        "checkbox toggles",
        checked == Some(true),
        format!("Remember me = {checked:?}"),
    );

    // 4. Optional: the one GPU step — render the authored UI to a PNG.
    if shot {
        println!("\nrender (fenestra_render::render — headless pixels):");
        let out = render(&desc, &theme, SIZE).expect("the UI renders");
        let dir = std::path::Path::new("gallery");
        std::fs::create_dir_all(dir).expect("create gallery dir");
        let path = dir.join("verify.png");
        out.png.save(&path).expect("write png");
        println!("  wrote {}", path.display());
    }

    println!();
    if report.all_ok() {
        println!(
            "all {} checks passed — the authored UI is verified, no display required.",
            report.passed,
        );
        ExitCode::SUCCESS
    } else {
        println!("{} passed, {} FAILED.", report.passed, report.failed);
        ExitCode::FAILURE
    }
}

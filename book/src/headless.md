# Headless rendering and testing

The thesis feature: a fenestra UI can be driven, inspected, and rendered
to pixels without a display server — by a test, a CI job, or an AI
agent. Assertions work at three levels, and one harness carries all of
them:

1. **Structure** — the accessibility tree (roles, names, values, rects).
2. **Behavior** — the messages the UI emits into `update`.
3. **Pixels** — deterministic PNGs compared against goldens.

## The harness and semantic queries

Find widgets the way users do — by role and accessible name — instead
of by coordinates:

```rust,ignore
use fenestra::prelude::*;
use fenestra::shell::Harness;

let mut h = Harness::new(app, Theme::light(), (480, 320));
h.click(&by::role(Semantics::Button).name("Add"));
h.type_text("buy milk");
h.key(KeyInput::plain(Key::Enter));

assert!(h.query(&by::label("buy milk")).is_some());     // structure
assert_eq!(h.take_messages().len(), 3);                  // behavior
let png = h.render();                                    // pixels
```

Queries follow the Testing Library priority: prefer `by::role(..)` +
`.name(..)`, then `by::label(..)`, then `by::value(..)`; `by::id(..)`
(your `.id("...")` keys) is the escape hatch users can't see. Lookups
are strict like Playwright locators — `get` panics on zero *or several*
matches, and the panic message contains the whole accessibility tree,
so the failure explains itself. `query` returns `Option` (assert
absence), `get_all` returns every match.

Verbs: `click`, `right_click`, `double_click`, `hover`, `type_text`,
`key`, `tab`/`shift_tab`, `focus`, `drag(from, to)`, `drop_file`,
`wheel`. The clock is explicit — `pump(ms)` advances animations exactly
that far; nothing is painted unless you call `render()`, so structural
tests stay fast.

Multi-window apps test whole: the harness reconciles [`App::windows`]
after every update; `activate_window(key)` scopes the verbs,
`render_window(key)` snapshots any window at its own size.

## The inspector

`h.frame().debug_tree()` dumps one line per node — kind, `#key`, layout
rect, scroll/focus flags, semantics, and `src=file:line` (the builder
call site, captured with `#[track_caller]`). It is the headless
equivalent of a visual-tree inspector; grep it.

`h.frame().access_yaml()` emits the accessibility tree in Playwright's
aria-snapshot grammar (`- button "Save"`), ready for insta snapshots:

```yaml
- text "Inbox"
- textbox [value="draft text"] #draft
- button "Send"
```

## Scenario scripts (no Rust required)

`run_scenario` drives a harness from JSON — the loop an agent reaches
for between code changes:

```json
{"steps": [
  {"click":  {"role": "button", "name": "Add"}},
  {"type":   "buy milk"},
  {"key":    "enter"},
  {"assert": {"exists": {"label": "buy milk"}}},
  {"shot":   "after-add"}
]}
```

Targets use the query vocabulary (`role`, `name`, `label`, `value`,
`id`, plus `_contains` forms); asserts cover `exists` / `absent` /
`count` / `value` / `windows`; `shot` writes named PNGs. Typos are
parse errors, not skipped steps, and every failure carries its step
index and the accessibility tree.

## Golden tests

`assert_png_snapshot(dir, name, &image)` compares against a committed
PNG (3/255 per channel, 0.2% pixel budget). On failure, three artifacts
land next to the golden: `<name>.actual.png`, `<name>.diff.png` (the
offending pixels in red over the dimmed golden — *where*, not just *how
much*), and `<name>.side.png` (golden | actual | diff). The panic
message carries the counts, the budget, and the worst pixel's
coordinates. `FENESTRA_UPDATE_SNAPSHOTS=1` regenerates — look at the
images before committing. Passing runs clean stale artifacts up.

The exact guarantees behind all of this — what "deterministic" means,
where the boundaries are — live in the
[determinism contract](determinism.md).

fenestra's own kit is verified this way: every widget, every state,
both themes, on every push, plus property tests that layout never
panics on arbitrary trees, Tab order is a permutation, and widget ids
stay unique per frame.

## The classic forms

`render_element(view, &theme, size)` renders one tree;
`render_element_with` takes custom fonts; `render_app(app, &events,
size, &theme)` replays coordinate-level `SyntheticEvent`s and renders a
settle frame — it is a thin wrapper over the harness, kept for simple
pixel probes.

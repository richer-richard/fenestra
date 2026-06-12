# Writing a widget crate

fenestra widgets are plain functions returning `Element`s — publishing
a widget crate needs no traits to implement, no macros, no registration.
[`fenestra-charts`](https://crates.io/crates/fenestra-charts) is the
reference: read its source next to this checklist.

## The contract

1. **Depend on `fenestra-core` only.** Widgets are pure tree builders;
   they never need the runner. Pull `fenestra-shell` in as a
   *dev-dependency* for golden tests.

   ```toml
   [dependencies]
   fenestra-core = "0.10"

   [dev-dependencies]
   fenestra-shell = "0.7"
   ```

2. **Take colors from the theme, never hardcode.** Style through
   `.themed(|t, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))`
   — your widget then works in light, dark, and every generated theme,
   including ones that don't exist yet.

3. **Stay Elm-pure.** Widgets own no state: take the current value and
   emit messages (`on_pick(impl Fn(..) -> Msg)`); the app stores. If
   your widget seems to need internal state, it needs a value + handler
   pair instead.

4. **Simple widgets are functions, configurable ones are builders.**
   `sparkline(values) -> Element<Msg>` for one-liners; a struct with
   methods + `impl From<W<Msg>> for Element<Msg>` once there are
   options (look at `combobox` or `data_table` in the kit).

5. **Name things for queries.** Set `.semantics(..)` and `.label(..)`
   on every meaningful node — that's what makes
   `by::role(..).name(..)` find your widget in users' tests, and what
   screen readers announce. Give stateful nodes stable `.id(..)`s.

6. **No panics, ever.** Hostile input (empty lists, NaN, negative
   sizes) renders something sane. Test it:

   ```rust,ignore
   let image = render_element(sparkline([f32::NAN]), &theme, (200, 50));
   ```

7. **Golden-test the look.** One snapshot per widget family:

   ```rust,ignore
   assert_png_snapshot(snapshot_dir(), "charts", &image);
   ```

   Goldens travel with the crate; users see exactly what they get, and
   your CI catches visual regressions on the reference platform.

8. **Drive behavior through the harness**, not coordinates:

   ```rust,ignore
   let mut h = Harness::new(app, Theme::light(), (400, 300));
   h.click(&by::role(Semantics::Button).name("sort by name"));
   ```

## Versioning

Track fenestra's minor version (`fenestra-core = "0.10"`) and re-test on
each release; the core IR and builder vocabulary are the stable
surface. After 1.0, semver does the rest.

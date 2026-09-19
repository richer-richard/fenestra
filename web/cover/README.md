# The cover

`gallery/cover.png` is the project's cover image: the README hero, the
GitHub social preview, and the link preview for the live demo.

The idea is the name. *Fenestra* is Latin for window, so the word is cut
out of a dark wall and a lit room shows through it. The room is one
fenestra-style dashboard, described once in `app.js` and styled once in
`app.css` with the framework's real theme tokens (accent hue 262, the
neutral and accent ramps from the theme snapshot, Inter from
`fenestra-core/assets`). It is rendered twice, dark theme and light
theme, and the light render is painted through the letters. Because both
passes share one layout, every card edge and chart line continues across
a letter boundary. That is the framework's own claim about themes, made
visible: nothing changed but the theme.

## Regenerate

```sh
sh web/cover/render.sh
git add -f gallery/cover.png
```

The script needs a Chrome or Chromium binary (set `CHROME=` to point at
one). It writes `light.png` next to this file as an intermediate, which
is gitignored, and then the cover itself. The word is set in Unbounded,
vendored in `fonts/` under the SIL Open Font License.

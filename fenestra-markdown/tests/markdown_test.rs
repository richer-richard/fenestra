//! The markdown widget: structure assertions through the semantic
//! layer, link clicks through the harness, a pixel-locked golden, and
//! the no-panic contract on hostile input.

use fenestra_core::{App, Element, Semantics, Theme, by, col};
use fenestra_markdown::markdown;
use fenestra_shell::testing::assert_png_snapshot;
use fenestra_shell::{Harness, render_element};

/// A long single paragraph: in a wide canvas it must wrap at the default
/// reading measure, not span the full width.
const LONG_PARA: &str = "The reading measure is the width of a text column, \
measured in characters per line; typographers have long held that roughly \
sixty-six characters makes the most comfortable line for sustained reading, \
because the eye tracks back to the start of the next line without losing its \
place, and the column stays narrow enough to read in a single relaxed sweep \
rather than a wide swing of the head from one margin to the other.";

const DOC: &str = "\
# Release notes

Some **bold**, some *italic*, some `inline code`, and a
[link to the book](https://example.com/book).

## Changes

- first item with `code`
- second item
1. ordered one
2. ordered two

> A quoted thought spanning a line.

```rust
fn main() { println!(\"hi\"); }
```

---

Done. ~~Struck.~~
";

const TABLE_DOC: &str = "\
| Fruit   | Count | Price  |
|:--------|:-----:|-------:|
| Apple   | 42    | $1.20  |
| Banana  | 13    | $0.50  |
";

const TASKLIST_DOC: &str = "\
- [x] completed task
- [ ] pending task
";

const IMAGE_DOC: &str = "![A diagram showing the layout](https://example.com/diagram.png)";

const FOOTNOTE_DOC: &str = "\
A statement with a footnote[^note1] and another[^note2].

[^note1]: First footnote definition.
[^note2]: Second footnote definition.
";

const GFM_DOC: &str = "\
# GFM Features

## Table

| Feature     | Status |
|:------------|:------:|
| Tables      | done   |
| Task lists  | done   |

## Tasks

- [x] implement tables
- [x] implement task lists
- [ ] implement something else

## Image

![Alt text for diagram](https://example.com/img.png)

## Footnote

See the spec[^ref].

[^ref]: GitHub Flavored Markdown specification.
";

fn snapshot_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

#[derive(Default)]
struct Viewer {
    opened: Option<String>,
}

#[derive(Clone)]
struct Open(String);

impl App for Viewer {
    type Msg = Open;

    fn update(&mut self, Open(url): Open) {
        self.opened = Some(url);
    }

    fn view(&self) -> Element<Open> {
        col().p(16.0).w(460.0).children([Element::from(
            markdown(DOC).on_link(|url| Open(url.to_owned())),
        )])
    }
}

#[test]
fn renders_structure_and_clicks_links() {
    let mut h = Harness::new(Viewer::default(), Theme::light(), (500, 560));
    // Headings and body text exist as accessible labels.
    assert!(h.query(&by::label("Release notes")).is_some());
    assert!(h.query(&by::label_contains("quoted thought")).is_some());
    // The link is a named clickable; clicking emits the URL.
    h.click(&by::role(Semantics::Button).name("https://example.com/book"));
    assert_eq!(h.app().opened.as_deref(), Some("https://example.com/book"));
}

#[test]
fn markdown_golden() {
    let view: Element<()> = col()
        .p(16.0)
        .w(460.0)
        .children([Element::from(markdown(DOC).on_link(|_| ()))]);
    let image = render_element(view, &Theme::light(), (500, 560));
    assert_png_snapshot(snapshot_dir(), "markdown", &image);
}

#[test]
fn measure_caps_prose_golden() {
    // A wide canvas: the default measure must cap the column, leaving an
    // empty gutter on the right rather than spanning the full 1000px.
    let view: Element<()> = col()
        .p(16.0)
        .children([Element::from(markdown(LONG_PARA).on_link(|_| ()))]);
    let image = render_element(view, &Theme::light(), (1000, 400));
    assert_png_snapshot(snapshot_dir(), "markdown_measure", &image);
}

#[derive(Default)]
struct Wide;

impl App for Wide {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col().p(16.0).children([Element::from(markdown(LONG_PARA))])
    }
}

#[test]
fn measure_caps_paragraph_width() {
    // The canvas is 1000px wide (content box ~968px), but the default measure
    // caps the prose column near the reading width (~525px at body size: 52ch ×
    // ~10.1px '0'), so the wrapped paragraph never approaches the full width.
    let h = Harness::new(Wide, Theme::light(), (1000, 400));
    let para = h
        .query(&by::label_contains("reading measure is the width"))
        .expect("paragraph leaf");
    let width = para.rect.width();
    assert!(
        width < 620.0,
        "paragraph width {width} should be capped near the measure"
    );
    assert!(
        width > 450.0,
        "paragraph width {width} should fill the reading column"
    );
}

#[test]
fn hostile_input_never_panics() {
    let cases = [
        "",
        "####### too deep",
        "[broken](",
        "``` no close",
        "> > > > deep quotes\n\n- - - -",
        "**unclosed bold *and emphasis",
        "| a | b |\n|---|---|\n| 1 | 2 |", // tables now render as a grid
        "\u{0000}\u{FFFF} control \u{202E}rtl-override",
    ];
    for case in cases {
        let view: Element<()> = col().children([Element::from(markdown(case))]);
        let image = render_element(view, &Theme::dark(), (300, 200));
        assert_eq!(image.dimensions(), (300, 200), "case {case:?}");
    }
}

// ── Table tests ───────────────────────────────────────────────────────────────

struct TableViewer;

impl App for TableViewer {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col()
            .p(16.0)
            .w(500.0)
            .children([Element::from(markdown(TABLE_DOC))])
    }
}

#[test]
fn table_projects_cell_text() {
    let h = Harness::new(TableViewer, Theme::light(), (540, 250));
    // Header cells are findable by label.
    assert!(
        h.query(&by::label_contains("Fruit")).is_some(),
        "header 'Fruit' not found"
    );
    assert!(
        h.query(&by::label_contains("Count")).is_some(),
        "header 'Count' not found"
    );
    assert!(
        h.query(&by::label_contains("Price")).is_some(),
        "header 'Price' not found"
    );
    // Body cells are findable.
    assert!(
        h.query(&by::label_contains("Apple")).is_some(),
        "body cell 'Apple' not found"
    );
    assert!(
        h.query(&by::label_contains("Banana")).is_some(),
        "body cell 'Banana' not found"
    );
    assert!(
        h.query(&by::label_contains("42")).is_some(),
        "body cell '42' not found"
    );
}

#[test]
fn table_golden() {
    let view: Element<()> = col()
        .p(16.0)
        .w(500.0)
        .children([Element::from(markdown(TABLE_DOC))]);
    let image = render_element(view, &Theme::light(), (540, 250));
    assert_png_snapshot(snapshot_dir(), "markdown_table", &image);
}

// ── Task list tests ───────────────────────────────────────────────────────────

struct TaskListViewer;

impl App for TaskListViewer {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col()
            .p(16.0)
            .w(400.0)
            .children([Element::from(markdown(TASKLIST_DOC))])
    }
}

#[test]
fn task_list_shows_checkboxes() {
    let h = Harness::new(TaskListViewer, Theme::light(), (440, 150));
    // Bracket forms, not ☑/☐: the ballot-box glyphs are outside the
    // embedded Inter coverage and rendered as tofu headlessly.
    assert!(
        h.query(&by::label("[x]")).is_some(),
        "checked checkbox [x] not found"
    );
    assert!(
        h.query(&by::label("[ ]")).is_some(),
        "unchecked checkbox [ ] not found"
    );
    // Item text should also be present.
    assert!(
        h.query(&by::label_contains("completed task")).is_some(),
        "task text not found"
    );
}

// ── Image tests ───────────────────────────────────────────────────────────────

struct ImageViewer;

impl App for ImageViewer {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col()
            .p(16.0)
            .w(400.0)
            .children([Element::from(markdown(IMAGE_DOC))])
    }
}

#[test]
fn image_alt_text_fallback() {
    // Images cannot be loaded headlessly; the alt text is shown as a placeholder.
    let h = Harness::new(ImageViewer, Theme::light(), (440, 100));
    // The placeholder text includes the alt text.
    assert!(
        h.query(&by::label_contains("A diagram")).is_some(),
        "image alt text placeholder not found"
    );
}

// ── Footnote tests ────────────────────────────────────────────────────────────

struct FootnoteViewer;

impl App for FootnoteViewer {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col()
            .p(16.0)
            .w(480.0)
            .children([Element::from(markdown(FOOTNOTE_DOC))])
    }
}

#[test]
fn footnote_renders_markers_and_definitions() {
    let h = Harness::new(FootnoteViewer, Theme::light(), (520, 250));
    // Inline reference markers [1] and [2] appear in the body text.
    assert!(
        h.query(&by::label_contains("[1]")).is_some(),
        "footnote reference [1] not found"
    );
    assert!(
        h.query(&by::label_contains("[2]")).is_some(),
        "footnote reference [2] not found"
    );
    // Definition text appears in the footnotes section.
    assert!(
        h.query(&by::label_contains("First footnote definition"))
            .is_some(),
        "footnote definition text not found"
    );
}

// ── GFM combined golden ───────────────────────────────────────────────────────

#[test]
fn gfm_features_golden() {
    let view: Element<()> = col()
        .p(16.0)
        .w(500.0)
        .children([Element::from(markdown(GFM_DOC))]);
    let image = render_element(view, &Theme::light(), (540, 700));
    assert_png_snapshot(snapshot_dir(), "markdown_gfm", &image);
}

// ── Code highlighting tests ─────────────────────────────────────────────────

const CODE_DOC: &str = "\
# Code samples

```rust
fn main() {
    let n = 42; // the answer
    let s = \"hi\";
}
```

```python
def greet(name):
    return f'hi {name}'  # greeting
```

```json
{ \"key\": \"value\", \"n\": 3.14, \"ok\": true }
```

```
plain untagged block stays as-is
```
";

struct CodeViewer;

impl App for CodeViewer {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col()
            .p(16.0)
            .w(520.0)
            .children([Element::from(markdown(CODE_DOC))])
    }
}

#[test]
fn code_block_shows_language_chips() {
    let h = Harness::new(CodeViewer, Theme::light(), (560, 660));
    // Every fenced block that declares a known language carries a muted chip.
    for lang in ["rust", "python", "json"] {
        assert!(
            h.query(&by::label(lang)).is_some(),
            "chip {lang:?} not found"
        );
    }
    // Tokenized code is still reachable as accessible leaves (per-piece).
    // `greet(name)` would also match the `# greeting` comment, so anchor on the
    // parenthesized form and assert presence (count-agnostic).
    assert!(
        !h.get_all(&by::label_contains("greet(name)")).is_empty(),
        "python identifier not found in tokenized body"
    );
}

const UNKNOWN_LANG_DOC: &str = "\
```haskell
main = putStrLn \"unknown stays plain\"
```
";

struct UnknownLangViewer;

impl App for UnknownLangViewer {
    type Msg = ();

    fn update(&mut self, (): ()) {}

    fn view(&self) -> Element<()> {
        col()
            .p(16.0)
            .w(420.0)
            .children([Element::from(markdown(UNKNOWN_LANG_DOC))])
    }
}

#[test]
fn unknown_language_chip_but_plain_body() {
    let h = Harness::new(UnknownLangViewer, Theme::light(), (460, 160));
    // The declared language is labeled even when it can't be highlighted…
    assert!(
        h.query(&by::label("haskell")).is_some(),
        "unknown-language chip not found"
    );
    // …and the body renders plain, as one selectable text leaf, so the whole
    // line is reachable by a single label.
    assert!(
        !h.get_all(&by::label_contains("main = putStrLn")).is_empty(),
        "plain unknown-language body not found as one leaf"
    );
}

#[test]
fn code_highlight_golden() {
    let view: Element<()> = col()
        .p(16.0)
        .w(520.0)
        .children([Element::from(markdown(CODE_DOC))]);
    let image = render_element(view, &Theme::light(), (560, 660));
    assert_png_snapshot(snapshot_dir(), "markdown_code", &image);
}

// ── Rich footnote golden ────────────────────────────────────────────────────

const FOOTNOTE_RICH_DOC: &str = "\
A claim with a rich footnote[^a].

[^a]: Body with **bold**, *italic*, and `code` that must survive.
";

#[test]
fn footnote_inline_styling_golden() {
    // The footnote definition's bold/italic/code must render as styled spans,
    // not flatten to plain muted text.
    let view: Element<()> = col()
        .p(16.0)
        .w(460.0)
        .children([Element::from(markdown(FOOTNOTE_RICH_DOC))]);
    let image = render_element(view, &Theme::light(), (500, 220));
    assert_png_snapshot(snapshot_dir(), "markdown_footnote_rich", &image);
}

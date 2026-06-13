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
        "| a | b |\n|---|---|\n| 1 | 2 |", // tables unsupported: plain text
        "\u{0000}\u{FFFF} control \u{202E}rtl-override",
    ];
    for case in cases {
        let view: Element<()> = col().children([Element::from(markdown(case))]);
        let image = render_element(view, &Theme::dark(), (300, 200));
        assert_eq!(image.dimensions(), (300, 200), "case {case:?}");
    }
}

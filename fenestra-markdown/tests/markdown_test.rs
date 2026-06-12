//! The markdown widget: structure assertions through the semantic
//! layer, link clicks through the harness, a pixel-locked golden, and
//! the no-panic contract on hostile input.

use fenestra_core::{App, Element, Semantics, Theme, by, col};
use fenestra_markdown::markdown;
use fenestra_shell::testing::assert_png_snapshot;
use fenestra_shell::{Harness, render_element};

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

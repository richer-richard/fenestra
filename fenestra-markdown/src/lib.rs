//! Markdown for fenestra: CommonMark rendered as native elements —
//! headings, paragraphs with inline styling, code blocks, lists,
//! blockquotes, links, and rules. Built on `fenestra-core`'s public API
//! only (rich text spans do the inline work), theme-token colors
//! throughout, no panics on hostile input.
//!
//! ```
//! use fenestra_markdown::markdown;
//!
//! let el: fenestra_core::Element<()> =
//!     markdown("# Title\n\nSome **bold** and `inline code`.").into();
//! ```

use fenestra_core::{
    Element, FamilyRole, Semantics, Span, TextSize, Theme, Weight, col, div, divider, rich_text,
    row, span, text,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Shared URL-to-message mapping for link clicks.
type LinkFn<Msg> = std::rc::Rc<dyn Fn(&str) -> Msg>;

/// A markdown document under construction; converts into an [`Element`].
pub struct Markdown<Msg> {
    source: String,
    on_link: Option<LinkFn<Msg>>,
}

/// Renders CommonMark (plus strikethrough) as native elements. Inline
/// emphasis/strong/code become rich-text spans; links are clickable
/// when [`Markdown::on_link`] is wired (they emit the URL — the app
/// decides what opening means).
pub fn markdown<Msg>(source: impl Into<String>) -> Markdown<Msg> {
    Markdown {
        source: source.into(),
        on_link: None,
    }
}

impl<Msg> Markdown<Msg> {
    /// Maps a clicked link's URL to a message.
    pub fn on_link(mut self, f: impl Fn(&str) -> Msg + 'static) -> Self {
        self.on_link = Some(std::rc::Rc::new(f));
        self
    }
}

/// Inline style state while walking events.
#[derive(Default, Clone, Copy)]
struct Inline {
    strong: bool,
    emphasis: bool,
    code: bool,
    strikethrough: bool,
    link: bool,
}

/// One flushable block being accumulated.
struct BlockBuilder {
    spans: Vec<SpanSpec>,
}

/// A span plus the link it belongs to (links split their own spans so
/// the whole link row can carry one click handler — v1 makes the whole
/// paragraph row clickable per link segment; see `flush`).
struct SpanSpec {
    text: String,
    inline: Inline,
    link: Option<String>,
}

fn heading_size(level: HeadingLevel) -> (f32, Weight) {
    match level {
        HeadingLevel::H1 => (28.0, Weight::Semibold),
        HeadingLevel::H2 => (22.0, Weight::Semibold),
        HeadingLevel::H3 => (18.0, Weight::Semibold),
        _ => (16.0, Weight::Medium),
    }
}

fn build_spans(specs: &[SpanSpec]) -> Vec<Span> {
    specs
        .iter()
        .map(|s| {
            let mut sp = span(s.text.clone());
            if s.inline.strong {
                sp = sp.weight(Weight::Semibold);
            }
            if s.inline.emphasis {
                sp = sp.italic();
            }
            if s.inline.code {
                sp = sp.family(FamilyRole::Mono);
            }
            if s.inline.link {
                sp = sp.weight(Weight::Medium);
            }
            // No line decoration in spans yet: struck text reads muted.
            if s.inline.strikethrough {
                sp = sp.color(fenestra_core::Color::from_rgba8(128, 128, 128, 255));
            }
            sp
        })
        .collect()
}

impl<Msg: Clone + 'static> From<Markdown<Msg>> for Element<Msg> {
    fn from(md: Markdown<Msg>) -> Self {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&md.source, options);

        let mut blocks: Vec<Element<Msg>> = Vec::new();
        let mut current = BlockBuilder { spans: Vec::new() };
        let mut inline = Inline::default();
        let mut link_target: Option<String> = None;
        let mut list_stack: Vec<Option<u64>> = Vec::new();
        let mut quote_depth = 0usize;
        let mut code: Option<String> = None;
        let mut heading: Option<HeadingLevel> = None;

        let flush_block = |current: &mut BlockBuilder,
                           blocks: &mut Vec<Element<Msg>>,
                           heading: Option<HeadingLevel>,
                           list_depth: usize,
                           list_marker: Option<String>,
                           quote_depth: usize,
                           on_link: &Option<LinkFn<Msg>>| {
            if current.spans.is_empty() {
                return;
            }
            let specs = std::mem::take(&mut current.spans);
            let has_links = specs.iter().any(|s| s.link.is_some());
            let style_spans = |seg: &[SpanSpec]| -> Vec<Span> {
                match heading {
                    Some(level) => {
                        let (px, weight) = heading_size(level);
                        build_spans(seg)
                            .into_iter()
                            .map(|s| s.size_px(px).weight(weight))
                            .collect()
                    }
                    None => build_spans(seg),
                }
            };
            let mut block: Element<Msg> = if !has_links {
                // Fast path: one paragraph, full text wrapping.
                rich_text(style_spans(&specs)).selectable()
            } else {
                // Inline emulation: word-level pieces flow in a wrap
                // row, so each link is its own correctly-sized
                // clickable. (Selection within link paragraphs is
                // per-piece — a documented v1 tradeoff.)
                let mut pieces: Vec<Element<Msg>> = Vec::new();
                let mut last_link: Option<String> = None;
                // Spaces attach to the FOLLOWING word: trailing-space
                // advances are trimmed by text measurement, leading
                // ones are kept — this carry preserves word gaps.
                let mut pending_space = false;
                for spec in &specs {
                    for (i, word) in spec.text.split(' ').enumerate() {
                        if i > 0 {
                            // A space sat between this item and the last.
                            pending_space = true;
                        }
                        if word.is_empty() {
                            continue;
                        }
                        let text = if std::mem::take(&mut pending_space) {
                            format!(" {word}")
                        } else {
                            word.to_owned()
                        };
                        let piece = SpanSpec {
                            text,
                            inline: spec.inline,
                            link: spec.link.clone(),
                        };
                        let mut el: Element<Msg> =
                            rich_text(style_spans(std::slice::from_ref(&piece))).shrink0();
                        if let (Some(url), Some(f)) = (&spec.link, on_link) {
                            el = el
                                .on_click(f(url))
                                .cursor(fenestra_core::Cursor::Pointer)
                                .themed(|t: &Theme, s| s.color(t.accent));
                            // One accessible button per link, not per
                            // word: only the run's first piece carries
                            // the semantic identity.
                            if last_link.as_deref() != Some(url) {
                                el = el.semantics(Semantics::Button).label(url.clone());
                            }
                        } else {
                            el = el.selectable();
                        }
                        last_link = spec.link.clone();
                        pieces.push(el);
                    }
                }
                row().wrap().items_baseline().children(pieces)
            };
            if let Some(marker) = list_marker {
                block = row().gap(6.0).pl(16.0 * list_depth as f32).children((
                    text(marker).themed(|t: &Theme, s| s.color(t.text_muted)),
                    block,
                ));
            }
            if quote_depth > 0 {
                block = row().children((
                    div()
                        .w(3.0)
                        .shrink0()
                        .themed(|t: &Theme, s| s.bg(t.border_subtle)),
                    col().pl(10.0).child(block),
                ));
            }
            blocks.push(block);
        };

        let mut pending_marker: Option<String> = None;
        let mut ordered_counters: Vec<u64> = Vec::new();

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => heading = Some(level),
                Event::End(TagEnd::Heading(_)) => {
                    flush_block(
                        &mut current,
                        &mut blocks,
                        heading.take(),
                        list_stack.len(),
                        None,
                        quote_depth,
                        &md.on_link,
                    );
                }
                Event::Start(Tag::Paragraph) => {}
                Event::End(TagEnd::Paragraph) => {
                    flush_block(
                        &mut current,
                        &mut blocks,
                        None,
                        list_stack.len(),
                        pending_marker.take(),
                        quote_depth,
                        &md.on_link,
                    );
                }
                Event::Start(Tag::BlockQuote(_)) => quote_depth += 1,
                Event::End(TagEnd::BlockQuote(_)) => quote_depth = quote_depth.saturating_sub(1),
                Event::Start(Tag::List(start)) => {
                    list_stack.push(start);
                    ordered_counters.push(start.unwrap_or(1));
                }
                Event::End(TagEnd::List(_)) => {
                    list_stack.pop();
                    ordered_counters.pop();
                }
                Event::Start(Tag::Item) => {
                    pending_marker = Some(match list_stack.last() {
                        Some(Some(_)) => {
                            let n = ordered_counters.last_mut().expect("counter");
                            let marker = format!("{n}.");
                            *n += 1;
                            marker
                        }
                        _ => "•".to_owned(),
                    });
                }
                Event::End(TagEnd::Item) => {
                    flush_block(
                        &mut current,
                        &mut blocks,
                        None,
                        list_stack.len(),
                        pending_marker.take(),
                        quote_depth,
                        &md.on_link,
                    );
                }
                Event::Start(Tag::CodeBlock(_)) => code = Some(String::new()),
                Event::End(TagEnd::CodeBlock) => {
                    if let Some(body) = code.take() {
                        let body = body.trim_end_matches('\n').to_owned();
                        blocks.push(
                            col()
                                .p(10.0)
                                .rounded(6.0)
                                .w_full()
                                .themed(|t: &Theme, s| {
                                    s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle)
                                })
                                .children([text(body).mono().size(TextSize::Sm).selectable()]),
                        );
                    }
                }
                Event::Start(Tag::Emphasis) => inline.emphasis = true,
                Event::End(TagEnd::Emphasis) => inline.emphasis = false,
                Event::Start(Tag::Strong) => inline.strong = true,
                Event::End(TagEnd::Strong) => inline.strong = false,
                Event::Start(Tag::Strikethrough) => inline.strikethrough = true,
                Event::End(TagEnd::Strikethrough) => inline.strikethrough = false,
                Event::Start(Tag::Link { dest_url, .. }) => {
                    inline.link = true;
                    link_target = Some(dest_url.to_string());
                }
                Event::End(TagEnd::Link) => {
                    inline.link = false;
                    link_target = None;
                }
                Event::Text(t) => match &mut code {
                    Some(body) => body.push_str(&t),
                    None => current.spans.push(SpanSpec {
                        text: t.to_string(),
                        inline,
                        link: link_target.clone(),
                    }),
                },
                Event::Code(t) => current.spans.push(SpanSpec {
                    text: t.to_string(),
                    inline: Inline {
                        code: true,
                        ..inline
                    },
                    link: link_target.clone(),
                }),
                Event::SoftBreak => current.spans.push(SpanSpec {
                    text: " ".to_owned(),
                    inline,
                    link: link_target.clone(),
                }),
                Event::HardBreak => {
                    flush_block(
                        &mut current,
                        &mut blocks,
                        None,
                        list_stack.len(),
                        None,
                        quote_depth,
                        &md.on_link,
                    );
                }
                Event::Rule => blocks.push(divider()),
                _ => {}
            }
        }
        // Trailing content without a closing block event.
        flush_block(
            &mut current,
            &mut blocks,
            heading.take(),
            list_stack.len(),
            pending_marker.take(),
            quote_depth,
            &md.on_link,
        );

        col().gap(10.0).items_start().children(blocks)
    }
}

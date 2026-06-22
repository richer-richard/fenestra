//! Markdown for fenestra: CommonMark + GFM rendered as native elements —
//! headings, paragraphs with inline styling, code blocks, lists,
//! blockquotes, links, rules, **tables**, **task lists**, **images** (alt
//! text fallback), **footnotes**, and autolinks. Built on `fenestra-core`'s
//! public API only (rich text spans do the inline work), theme-token colors
//! throughout, no panics on hostile input.
//!
//! Enabled GFM flags: TABLES, TASKLISTS, FOOTNOTES, STRIKETHROUGH, GFM.
//! Bare-URL autolinks are not yet supported by pulldown-cmark 0.13.4's
//! ENABLE_GFM (only blockquote admonitions); standard angle-bracket
//! autolinks (`<https://…>`) render as clickable links via CommonMark.
//! Code-block syntax highlighting is deferred (no syntax-highlighting dep).
//!
//! ```
//! use fenestra_markdown::markdown;
//!
//! let el: fenestra_core::Element<()> =
//!     markdown("# Title\n\nSome **bold** and `inline code`.").into();
//! ```

use fenestra_core::{
    Element, FamilyRole, MEASURE_CH, Semantics, Span, TextAlign, TextSize, Theme, Track, Weight,
    col, div, divider, rich_text, row, span, text,
};
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Shared URL-to-message mapping for link clicks.
type LinkFn<Msg> = std::rc::Rc<dyn Fn(&str) -> Msg>;

/// A markdown document under construction; converts into an [`Element`].
pub struct Markdown<Msg> {
    source: String,
    on_link: Option<LinkFn<Msg>>,
}

/// Renders CommonMark + GFM as native elements. Inline emphasis/strong/code
/// become rich-text spans; links are clickable when [`Markdown::on_link`] is
/// wired (they emit the URL — the app decides what opening means).
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
    /// Footnote reference marker — rendered slightly smaller.
    footnote_ref: bool,
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
            // Footnote references read smaller (superscript-ish).
            if s.inline.footnote_ref {
                sp = sp.size_px(9.0);
            }
            sp
        })
        .collect()
}

/// Build spans that are semibold (for table header cells).
fn header_spans(specs: &[SpanSpec]) -> Vec<Span> {
    build_spans(specs)
        .into_iter()
        .map(|s| s.weight(Weight::Semibold))
        .collect()
}

/// Convert a pulldown-cmark column [`Alignment`] to a fenestra [`TextAlign`].
fn column_text_align(alignment: Alignment) -> TextAlign {
    match alignment {
        Alignment::Center => TextAlign::Center,
        Alignment::Right => TextAlign::End,
        Alignment::Left | Alignment::None => TextAlign::Start,
    }
}

impl<Msg: Clone + 'static> From<Markdown<Msg>> for Element<Msg> {
    fn from(md: Markdown<Msg>) -> Self {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_FOOTNOTES);
        // ENABLE_GFM: GFM blockquote admonitions ([!NOTE] etc.) and future
        // GFM features. Bare-URL autolinks are NOT yet behind this flag in
        // pulldown-cmark 0.13.4; standard <url> autolinks work via CommonMark.
        options.insert(Options::ENABLE_GFM);
        let parser = Parser::new_ext(&md.source, options);

        let mut blocks: Vec<Element<Msg>> = Vec::new();
        let mut current = BlockBuilder { spans: Vec::new() };
        let mut inline = Inline::default();
        let mut link_target: Option<String> = None;
        let mut list_stack: Vec<Option<u64>> = Vec::new();
        let mut quote_depth = 0usize;
        let mut code: Option<String> = None;
        let mut heading: Option<HeadingLevel> = None;

        // Table state
        let mut table_alignments: Vec<Alignment> = Vec::new();
        let mut in_table = false;
        let mut in_table_head = false;
        let mut current_table_row: Vec<Vec<SpanSpec>> = Vec::new();
        let mut table_head_row: Vec<Vec<SpanSpec>> = Vec::new();
        let mut table_body_rows: Vec<Vec<Vec<SpanSpec>>> = Vec::new();

        // Image alt-text accumulator: (dest_url, alt_text)
        let mut image_state: Option<(String, String)> = None;

        // Footnote state
        let mut in_footnote_def: Option<String> = None;
        let mut footnote_defs: Vec<(String, Vec<SpanSpec>)> = Vec::new();
        let mut footnote_def_seen: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut footnote_refs: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut footnote_ref_counter = 0usize;

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
                // Fast path: one paragraph, full text wrapping. Headings
                // balance their line lengths (even lines instead of a
                // full-then-short ragged break); body text stays greedy.
                // (A heading with an inline link falls through to the
                // wrap-row path below, which has no single layout to
                // balance.)
                let para = rich_text(style_spans(&specs)).selectable();
                if heading.is_some() {
                    para.balance()
                } else {
                    para
                }
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
                    if in_footnote_def.is_none() {
                        flush_block(
                            &mut current,
                            &mut blocks,
                            heading.take(),
                            list_stack.len(),
                            None,
                            quote_depth,
                            &md.on_link,
                        );
                    } else {
                        heading = None;
                        // spans stay; collected when FootnoteDefinition ends
                    }
                }
                Event::Start(Tag::Paragraph) => {}
                Event::End(TagEnd::Paragraph) => {
                    if in_table {
                        // Paragraph inside table cell: spans stay for cell collection.
                    } else if in_footnote_def.is_some() {
                        // Paragraph inside footnote: separate with a space;
                        // spans are collected when FootnoteDefinition ends.
                        if !current.spans.is_empty() {
                            current.spans.push(SpanSpec {
                                text: " ".to_owned(),
                                inline,
                                link: None,
                            });
                        }
                    } else {
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
                        _ => "\u{2022}".to_owned(), // •
                    });
                }
                Event::End(TagEnd::Item) => {
                    if in_footnote_def.is_none() {
                        flush_block(
                            &mut current,
                            &mut blocks,
                            None,
                            list_stack.len(),
                            pending_marker.take(),
                            quote_depth,
                            &md.on_link,
                        );
                    } else {
                        pending_marker = None;
                    }
                }
                Event::Start(Tag::CodeBlock(_)) => code = Some(String::new()),
                Event::End(TagEnd::CodeBlock) => {
                    if let Some(body) = code.take() {
                        let body = body.trim_end_matches('\n').to_owned();
                        blocks.push(
                            col()
                                .p(10.0)
                                .w_full()
                                // Radius from the theme scale (was a hardcoded
                                // 6.0) so a sharp/soft theme re-rounds code
                                // blocks too; `radius.sm` defaults to R_SM = 6,
                                // so default output is unchanged.
                                .themed(|t: &Theme, s| {
                                    s.rounded(t.radius.sm)
                                        .bg(t.elevated_surface(1))
                                        .border(1.0, t.border_subtle)
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
                // Images: cannot load URLs headlessly, so fall back to alt text
                // in a styled placeholder box.
                Event::Start(Tag::Image { dest_url, .. }) => {
                    image_state = Some((dest_url.to_string(), String::new()));
                }
                Event::End(TagEnd::Image) => {
                    if let Some((url, alt)) = image_state.take() {
                        let label = if alt.is_empty() { url } else { alt };
                        blocks.push(
                            div()
                                .px(8.0)
                                .py(4.0)
                                .themed(|t: &Theme, s| {
                                    s.border(1.0, t.border_subtle).rounded(t.radius.sm)
                                })
                                .child(
                                    text(format!("[img: {label}]"))
                                        .themed(|t: &Theme, s| s.color(t.text_muted)),
                                ),
                        );
                    }
                }
                // Tables
                Event::Start(Tag::Table(aligns)) => {
                    table_alignments = aligns;
                    table_head_row.clear();
                    table_body_rows.clear();
                    in_table = true;
                }
                Event::End(TagEnd::Table) => {
                    in_table = false;
                    let n_cols = table_alignments.len().max(1);
                    let mut all_cells: Vec<Element<Msg>> = Vec::new();

                    // Header row: semibold text + elevated background + bottom rule.
                    for (i, cell_specs) in table_head_row.iter().enumerate() {
                        let ta = column_text_align(
                            table_alignments.get(i).copied().unwrap_or(Alignment::None),
                        );
                        let spans = header_spans(cell_specs);
                        let cell = div()
                            .px(8.0)
                            .py(6.0)
                            .themed(|t: &Theme, s| {
                                s.bg(t.elevated_surface(1))
                                    .border_bottom(1.0, t.border_subtle)
                            })
                            .child(rich_text(spans).text_align(ta).selectable());
                        all_cells.push(cell);
                    }

                    // Body rows.
                    for row_cells in &table_body_rows {
                        for (i, cell_specs) in row_cells.iter().enumerate() {
                            let ta = column_text_align(
                                table_alignments.get(i).copied().unwrap_or(Alignment::None),
                            );
                            let spans = build_spans(cell_specs);
                            let cell = div()
                                .px(8.0)
                                .py(4.0)
                                .themed(|t: &Theme, s| s.border_bottom(1.0, t.border_subtle))
                                .child(rich_text(spans).text_align(ta).selectable());
                            all_cells.push(cell);
                        }
                    }

                    blocks.push(
                        div()
                            .w_full()
                            .grid_cols(std::iter::repeat_n(Track::Fr(1.0), n_cols))
                            .themed(|t: &Theme, s| {
                                s.border(1.0, t.border_subtle).rounded(t.radius.sm)
                            })
                            .children(all_cells),
                    );
                    table_alignments.clear();
                }
                Event::Start(Tag::TableHead) => {
                    in_table_head = true;
                    table_head_row.clear();
                }
                Event::End(TagEnd::TableHead) => {
                    // The head row is now complete (cells were collected
                    // directly into table_head_row without a TableRow wrapper).
                    in_table_head = false;
                }
                // TableRow is emitted only for body rows; the header row's
                // cells are collected directly inside TableHead (no TableRow).
                Event::Start(Tag::TableRow) => {
                    current_table_row.clear();
                }
                Event::End(TagEnd::TableRow) => {
                    table_body_rows.push(std::mem::take(&mut current_table_row));
                }
                Event::Start(Tag::TableCell) => {}
                Event::End(TagEnd::TableCell) => {
                    let cell_spans = std::mem::take(&mut current.spans);
                    if in_table_head {
                        // Header cells go directly into table_head_row.
                        table_head_row.push(cell_spans);
                    } else {
                        // Body cells accumulate in the current row buffer.
                        current_table_row.push(cell_spans);
                    }
                }
                // Task list checkboxes: override the bullet with a glyph.
                Event::TaskListMarker(checked) => {
                    pending_marker = Some(if checked { "\u{2611}" } else { "\u{2610}" }.to_owned()); // ☑ / ☐
                }
                // Footnote references: inline [N] marker.
                Event::FootnoteReference(label) => {
                    let label_str = label.to_string();
                    let next = footnote_ref_counter + 1;
                    let n = *footnote_refs.entry(label_str).or_insert(next);
                    if n == next {
                        footnote_ref_counter = next;
                    }
                    current.spans.push(SpanSpec {
                        text: format!("[{n}]"),
                        inline: Inline {
                            footnote_ref: true,
                            ..inline
                        },
                        link: link_target.clone(),
                    });
                }
                // Footnote definitions: collect spans, render at document end.
                Event::Start(Tag::FootnoteDefinition(label)) => {
                    in_footnote_def = Some(label.to_string());
                }
                Event::End(TagEnd::FootnoteDefinition) => {
                    if let Some(label) = in_footnote_def.take() {
                        if !footnote_def_seen.contains(&label) {
                            footnote_def_seen.insert(label.clone());
                            // Trim trailing separator space added between paragraphs.
                            let mut spans = std::mem::take(&mut current.spans);
                            if spans.last().is_some_and(|s| s.text == " ") {
                                spans.pop();
                            }
                            footnote_defs.push((label, spans));
                        } else {
                            current.spans.clear();
                        }
                    }
                }
                Event::Text(t) => {
                    if let Some(body) = &mut code {
                        body.push_str(&t);
                    } else if let Some((_, alt)) = &mut image_state {
                        alt.push_str(&t);
                    } else {
                        current.spans.push(SpanSpec {
                            text: t.to_string(),
                            inline,
                            link: link_target.clone(),
                        });
                    }
                }
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
                Event::HardBreak if !in_table && in_footnote_def.is_none() => {
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

        // Footnote definitions section: a rule + numbered list at the end.
        if !footnote_defs.is_empty() {
            blocks.push(divider());
            let mut footnote_els: Vec<Element<Msg>> = Vec::new();
            for (label, def_spans) in &footnote_defs {
                let n = footnote_refs.get(label.as_str()).copied().unwrap_or(0);
                let marker = format!("{n}.");
                let body = if def_spans.is_empty() {
                    // Definition was referenced but has no body text.
                    text(String::new()).themed(|t: &Theme, s| s.color(t.text_muted))
                } else {
                    text(
                        def_spans
                            .iter()
                            .map(|s| s.text.as_str())
                            .collect::<String>(),
                    )
                    .themed(|t: &Theme, s| s.color(t.text_muted))
                };
                footnote_els.push(row().gap(6.0).items_start().children((
                    text(marker).themed(|t: &Theme, s| s.color(t.text_muted)),
                    body,
                )));
            }
            blocks.push(col().gap(4.0).items_start().children(footnote_els));
        }

        // Cap the document at the default reading measure (~66ch), resolved
        // against the body text size: a long paragraph wraps at the reading
        // column instead of spanning an arbitrarily wide canvas. Narrower
        // containers (the cap doesn't bind) are unaffected.
        col()
            .gap(10.0)
            .items_start()
            .measure(MEASURE_CH)
            .children(blocks)
    }
}

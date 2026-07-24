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
//! Fenced code blocks carry a muted language chip and dependency-free,
//! highlight-grade syntax coloring (keywords, strings, comments, numbers — all
//! through theme roles) for a handful of common languages; unknown or untagged
//! blocks render as plain mono, unchanged.
//!
//! ```
//! use fenestra_markdown::markdown;
//!
//! let el: fenestra_core::Element<()> =
//!     markdown("# Title\n\nSome **bold** and `inline code`.").into();
//! ```

use fenestra_core::{
    Color, Element, FamilyRole, MEASURE_CH, Semantics, Span, TextAlign, TextSize, Theme, Track,
    Weight, col, div, divider, rich_text, row, span, text,
};
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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
                sp = sp.color(Color::from_rgba8(128, 128, 128, 255));
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

// ── Code-block syntax highlighting ──────────────────────────────────────────
//
// A small, dependency-free, highlight-grade lexer: enough to color keywords,
// strings, comments, and numbers for a handful of common languages — not a full
// grammar. Token colors come only from theme roles (see [`role_color`]),
// matching how the renderer colors links. An unknown or missing fence language
// degrades to plain mono (the `CodeBlock` handler), so untagged blocks are
// visually unchanged.

/// The highlight role of a code token; mapped to a theme color by
/// [`role_color`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TokenRole {
    /// Default code text — mono, theme `text` (also whitespace and punctuation).
    Plain,
    /// A language keyword — theme `accent` (the same role links use).
    Keyword,
    /// A string or char literal — theme `success.text`.
    Str,
    /// A line or block comment — theme `text_muted`.
    Comment,
    /// A numeric literal — theme `warning.text`.
    Number,
}

/// Resolves a token role to a theme color — theme tokens only, never raw rgba.
fn role_color(role: TokenRole, t: &Theme) -> Color {
    match role {
        TokenRole::Plain => t.text,
        TokenRole::Keyword => t.accent,
        TokenRole::Str => t.success.text,
        TokenRole::Comment => t.text_muted,
        TokenRole::Number => t.warning.text,
    }
}

/// Per-language lexer configuration. Highlight-grade, not a full grammar.
struct LangSpec {
    /// Reserved words colored as keywords, space-separated (matched whole).
    keywords: &'static str,
    /// Line-comment prefixes (`//`, `#`, `--`), each running to end of line.
    line_comments: &'static [&'static str],
    /// Whether `/* … */` block comments are recognized.
    block_comment: bool,
    /// Whether single-quoted strings are recognized. Off for Rust, where `'`
    /// opens a lifetime, not a string literal.
    single_quote_string: bool,
    /// Whether keyword matching ignores ASCII case (SQL).
    case_insensitive: bool,
}

// Keyword sets, space-separated (compact and rustfmt-stable; split at match
// time). Highlight-grade — a representative core per language, not exhaustive.
const RUST_KW: &str = "as async await break const continue crate dyn else enum extern false fn for \
    if impl in let loop match mod move mut pub ref return self Self static struct super trait true \
    type unsafe use where while";

const JS_KW: &str = "async await break case catch class const continue debugger default delete do \
    else export extends false finally for function if import in instanceof let new null of return \
    super switch this throw true try typeof undefined var void while yield";

const TS_KW: &str = "abstract any as async await boolean break case catch class const continue \
    declare default delete do else enum export extends false finally for function if implements \
    import in instanceof interface let namespace never new null number of private protected public \
    readonly return static string super switch this throw true try type typeof undefined unknown \
    var void while yield";

const PY_KW: &str = "and as assert async await break class continue def del elif else except False \
    finally for from global if import in is lambda None nonlocal not or pass raise return True try \
    while with yield";

const JSON_KW: &str = "true false null";

const SH_KW: &str = "case declare do done echo elif else esac export fi for function if in local \
    readonly return select then until while";

const SQL_KW: &str = "add all alter and as between by column create delete distinct drop from group \
    having inner insert into is join key left like limit not null offset on or order outer primary \
    right select set table union update values where";

/// The lexer spec for a fenced language tag, or `None` for an unknown or empty
/// tag — the caller then renders the block as plain mono (unchanged).
fn lang_spec(lang: &str) -> Option<LangSpec> {
    let spec = match lang.trim().to_ascii_lowercase().as_str() {
        "rust" | "rs" => LangSpec {
            keywords: RUST_KW,
            line_comments: &["//"],
            block_comment: true,
            single_quote_string: false,
            case_insensitive: false,
        },
        "js" | "javascript" | "jsx" | "mjs" | "cjs" => LangSpec {
            keywords: JS_KW,
            line_comments: &["//"],
            block_comment: true,
            single_quote_string: true,
            case_insensitive: false,
        },
        "ts" | "typescript" | "tsx" => LangSpec {
            keywords: TS_KW,
            line_comments: &["//"],
            block_comment: true,
            single_quote_string: true,
            case_insensitive: false,
        },
        "python" | "py" => LangSpec {
            keywords: PY_KW,
            line_comments: &["#"],
            block_comment: false,
            single_quote_string: true,
            case_insensitive: false,
        },
        "json" | "jsonc" => LangSpec {
            keywords: JSON_KW,
            line_comments: &["//"],
            block_comment: false,
            single_quote_string: false,
            case_insensitive: false,
        },
        "sh" | "bash" | "shell" | "zsh" => LangSpec {
            keywords: SH_KW,
            line_comments: &["#"],
            block_comment: false,
            single_quote_string: true,
            case_insensitive: false,
        },
        "sql" => LangSpec {
            keywords: SQL_KW,
            line_comments: &["--"],
            block_comment: true,
            single_quote_string: true,
            case_insensitive: true,
        },
        _ => return None,
    };
    Some(spec)
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

fn is_keyword(spec: &LangSpec, word: &str) -> bool {
    spec.keywords.split_ascii_whitespace().any(|k| {
        if spec.case_insensitive {
            k.eq_ignore_ascii_case(word)
        } else {
            k == word
        }
    })
}

/// Byte length of a digit/underscore run from `i` in `b` (all ASCII).
fn digit_run(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
        i += 1;
    }
    i
}

/// Byte length of a quoted literal at the front of `rest` (which begins with
/// `quote`). Stops after the matching unescaped quote, before a newline (an
/// unterminated single-line literal), or at EOF.
fn string_len(rest: &str, quote: char) -> usize {
    let mut chars = rest.char_indices();
    chars.next(); // opening quote
    let mut escaped = false;
    for (idx, c) in chars {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '\n' {
            return idx;
        } else if c == quote {
            return idx + c.len_utf8();
        }
    }
    rest.len()
}

/// Byte length of a numeric literal at the front of `rest` (which begins with an
/// ASCII digit). Every recognized character is ASCII, so byte indexing is safe;
/// a `.` is only taken when a digit follows, so `0..10` never reads as one
/// number.
fn number_len(rest: &str) -> usize {
    let b = rest.as_bytes();
    // Hex / binary / octal prefix: 0x.., 0b.., 0o..
    if b.len() >= 2 && b[0] == b'0' && matches!(b[1], b'x' | b'X' | b'b' | b'B' | b'o' | b'O') {
        let mut i = 2;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
            i += 1;
        }
        return i;
    }
    let mut i = digit_run(b, 0);
    if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
        i = digit_run(b, i + 1);
    }
    if i < b.len() && matches!(b[i], b'e' | b'E') {
        let mut j = i + 1;
        if j < b.len() && matches!(b[j], b'+' | b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            i = digit_run(b, j);
        }
    }
    // Type suffix (Rust `f64`/`u32`, JS bigint `n`, …).
    while i < b.len() && (b[i].is_ascii_alphabetic() || b[i] == b'_') {
        i += 1;
    }
    i
}

/// Splits `code` into `(text, role)` tokens covering every byte in order
/// (lossless), coalescing adjacent same-role runs into one token.
fn tokenize(spec: &LangSpec, code: &str) -> Vec<(String, TokenRole)> {
    let mut raw: Vec<(usize, usize, TokenRole)> = Vec::new();
    let mut i = 0;
    while i < code.len() {
        let rest = &code[i..];
        let c = rest.chars().next().expect("non-empty remainder");
        let (len, role) = if c.is_whitespace() {
            let len = rest
                .find(|ch: char| !ch.is_whitespace())
                .unwrap_or(rest.len());
            (len, TokenRole::Plain)
        } else if let Some(prefix) = spec
            .line_comments
            .iter()
            .copied()
            .find(|p| rest.starts_with(*p))
        {
            // `#` only opens a comment at line start or after whitespace, so a
            // `#` inside a shell word stays plain (`//`/`--` are unambiguous).
            let opens = prefix != "#"
                || i == 0
                || code[..i]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace);
            if opens {
                (rest.find('\n').unwrap_or(rest.len()), TokenRole::Comment)
            } else {
                (c.len_utf8(), TokenRole::Plain)
            }
        } else if spec.block_comment && rest.starts_with("/*") {
            (
                rest.find("*/").map_or(rest.len(), |e| e + 2),
                TokenRole::Comment,
            )
        } else if c == '"' || (spec.single_quote_string && c == '\'') {
            (string_len(rest, c), TokenRole::Str)
        } else if c.is_ascii_digit() {
            (number_len(rest), TokenRole::Number)
        } else if is_ident_start(c) {
            let len = rest
                .find(|ch: char| !is_ident_continue(ch))
                .unwrap_or(rest.len());
            let role = if is_keyword(spec, &rest[..len]) {
                TokenRole::Keyword
            } else {
                TokenRole::Plain
            };
            (len, role)
        } else {
            (c.len_utf8(), TokenRole::Plain)
        };
        raw.push((i, i + len, role));
        i += len;
    }
    // Coalesce adjacent same-role runs into one token — but never across
    // whitespace, so the renderer can fold a whitespace run into the *leading*
    // edge of the next piece. (A piece's trailing whitespace collapses at
    // layout and would abut the next token.) Fewer elements, coarser selection.
    let mut out: Vec<(String, TokenRole)> = Vec::with_capacity(raw.len());
    for (s, e, role) in raw {
        let text = &code[s..e];
        let mergeable = !text.trim().is_empty();
        match out.last_mut() {
            Some(last) if last.1 == role && mergeable && !last.0.trim().is_empty() => {
                last.0.push_str(text);
            }
            _ => out.push((text.to_owned(), role)),
        }
    }
    out
}

/// Builds the body of a highlighted code block: one row of colored mono pieces
/// per source line, stacked tight so line height alone sets the rhythm.
/// Whitespace folds into the following piece, so indentation survives; like the
/// inline-link path, selection is per-piece (a documented v1 tradeoff).
///
/// Tokenized per line, so a `/* … */` comment or a multi-line string that spans
/// lines is re-scanned each line: its second and later lines highlight as code (a
/// highlight-grade limitation, not a full grammar).
fn highlighted_body<Msg: Clone + 'static>(spec: &LangSpec, body: &str) -> Element<Msg> {
    let mut lines: Vec<Element<Msg>> = Vec::new();
    for line in body.split('\n') {
        let mut pieces: Vec<Element<Msg>> = Vec::new();
        let mut pending = String::new();
        for (tok, role) in tokenize(spec, line) {
            // A pure-whitespace run carries forward as the next piece's leading
            // indentation (leading whitespace is measured; trailing collapses).
            if role == TokenRole::Plain && tok.trim().is_empty() {
                pending.push_str(&tok);
                continue;
            }
            let piece_text = if pending.is_empty() {
                tok
            } else {
                pending.push_str(&tok);
                std::mem::take(&mut pending)
            };
            let mut piece: Element<Msg> = text(piece_text)
                .mono()
                .size(TextSize::Sm)
                .selectable()
                .shrink0();
            if role != TokenRole::Plain {
                piece = piece.themed(move |t: &Theme, s| s.color(role_color(role, t)));
            }
            pieces.push(piece);
        }
        // Keep blank / whitespace-only lines occupying a line of height.
        if pieces.is_empty() {
            pieces.push(text(" ").mono().size(TextSize::Sm));
        }
        lines.push(row().items_baseline().children(pieces));
    }
    col().gap(0.0).items_start().children(lines)
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
        // The fenced language tag (first word of the info string), if any.
        let mut code_lang: Option<String> = None;
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
                // Fast path: one paragraph, one layout to refine. Headings
                // balance their line lengths (even lines instead of a
                // full-then-short ragged break); body prose uses pretty
                // wrapping to avoid a stranded one-word last line (orphan).
                // (A paragraph with an inline link falls through to the
                // wrap-row path below, which has no single layout to refine.)
                let para = rich_text(style_spans(&specs)).selectable();
                if heading.is_some() {
                    para.balance()
                } else {
                    para.pretty()
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
                Event::Start(Tag::CodeBlock(kind)) => {
                    code = Some(String::new());
                    // Fenced info string → the first whitespace/comma-delimited
                    // token (e.g. `rust` from ```rust,no_run); indented blocks
                    // have no language.
                    code_lang = match kind {
                        CodeBlockKind::Fenced(info) => info
                            .split(|c: char| c.is_whitespace() || c == ',')
                            .find(|s| !s.is_empty())
                            .map(str::to_owned),
                        CodeBlockKind::Indented => None,
                    };
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some(body) = code.take() {
                        let body = body.trim_end_matches('\n').to_owned();
                        let lang = code_lang.take();
                        let mut children: Vec<Element<Msg>> = Vec::new();
                        // A small, muted language chip for any fenced block that
                        // declares a language (known or not).
                        if let Some(lang) = lang.as_deref().filter(|l| !l.is_empty()) {
                            children.push(
                                row().w_full().justify_end().child(
                                    text(lang.to_owned())
                                        .size(TextSize::Xs)
                                        .themed(|t: &Theme, s| s.color(t.text_muted)),
                                ),
                            );
                        }
                        // Highlight a known language; an unknown or missing one
                        // degrades to plain mono (visually as before).
                        match lang.as_deref().and_then(lang_spec) {
                            Some(spec) => children.push(highlighted_body(&spec, &body)),
                            None => {
                                children.push(text(body).mono().size(TextSize::Sm).selectable());
                            }
                        }
                        blocks.push(
                            col()
                                .gap(6.0)
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
                                .children(children),
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
                    // Bracket forms, not ☑/☐: the ballot-box glyphs are
                    // outside the embedded Inter coverage and render as
                    // tofu in deterministic headless output.
                    pending_marker = Some(if checked { "[x]" } else { "[ ]" }.to_owned());
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
                let body: Element<Msg> = if def_spans.is_empty() {
                    // Definition was referenced but has no body text.
                    text(String::new()).themed(|t: &Theme, s| s.color(t.text_muted))
                } else {
                    // Route the body through the shared inline-span path so
                    // bold/italic/code/links inside a footnote survive instead
                    // of flattening to plain text. The muted base color is set
                    // on the paragraph; spans that set no color of their own
                    // inherit it.
                    rich_text(build_spans(def_spans)).themed(|t: &Theme, s| s.color(t.text_muted))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Just the token roles for `code`, in order.
    fn roles(spec: &LangSpec, code: &str) -> Vec<TokenRole> {
        tokenize(spec, code).into_iter().map(|(_, r)| r).collect()
    }

    #[test]
    fn lang_spec_known_and_unknown() {
        for known in [
            "rust",
            "rs",
            "js",
            "javascript",
            "ts",
            "typescript",
            "python",
            "py",
            "json",
            "sh",
            "bash",
            "sql",
        ] {
            assert!(lang_spec(known).is_some(), "{known} should be known");
        }
        for unknown in ["", "haskell", "brainfuck", "text", "plaintext", "ocaml"] {
            assert!(lang_spec(unknown).is_none(), "{unknown} should be unknown");
        }
    }

    #[test]
    fn tokenize_is_lossless() {
        // Every byte of the input must reappear exactly once, in order: the
        // renderer reconstructs the code from the tokens, so a drop or dup
        // would silently corrupt the displayed source.
        let spec = lang_spec("rust").unwrap();
        let code = "fn main() {\n    let x = 0xFF; // hi\n    println!(\"hi {x}\");\n}";
        let joined: String = tokenize(&spec, code).into_iter().map(|(t, _)| t).collect();
        assert_eq!(joined, code);
    }

    #[test]
    fn rust_block_has_multiple_token_roles() {
        let spec = lang_spec("rust").unwrap();
        let code = "let n = 42; // count\nlet s = \"hi\";";
        let mut seen: std::collections::HashSet<TokenRole> =
            roles(&spec, code).into_iter().collect();
        // Plain (whitespace/punctuation) is always present; the value is the
        // *colored* roles a single flat `text` could never express.
        seen.remove(&TokenRole::Plain);
        assert!(seen.contains(&TokenRole::Keyword), "keyword should color");
        assert!(seen.contains(&TokenRole::Number), "number should color");
        assert!(seen.contains(&TokenRole::Comment), "comment should color");
        assert!(seen.contains(&TokenRole::Str), "string should color");
        assert!(seen.len() >= 4, "≥4 distinct colored roles, got {seen:?}");
    }

    #[test]
    fn classifies_numbers_strings_comments() {
        let spec = lang_spec("rust").unwrap();
        assert_eq!(
            tokenize(&spec, "0xFF"),
            [("0xFF".into(), TokenRole::Number)]
        );
        assert_eq!(
            tokenize(&spec, "3.14"),
            [("3.14".into(), TokenRole::Number)]
        );
        assert_eq!(
            tokenize(&spec, "\"hi\""),
            [("\"hi\"".into(), TokenRole::Str)]
        );
        assert_eq!(
            tokenize(&spec, "// c"),
            [("// c".into(), TokenRole::Comment)]
        );
    }

    #[test]
    fn block_comment_spans_to_close() {
        let spec = lang_spec("rust").unwrap();
        assert_eq!(
            tokenize(&spec, "/* a\nb */x"),
            [
                ("/* a\nb */".into(), TokenRole::Comment),
                ("x".into(), TokenRole::Plain),
            ]
        );
    }

    #[test]
    fn rust_single_quote_is_not_a_string() {
        // Rust lifetimes use `'`, so single-quoted strings are disabled there:
        // `'a` must not be swallowed as a string (it would mis-color generics).
        let spec = lang_spec("rust").unwrap();
        let toks = tokenize(&spec, "&'a str");
        assert!(
            toks.iter().all(|(_, r)| *r != TokenRole::Str),
            "no string token expected in {toks:?}"
        );
    }

    #[test]
    fn python_single_quote_is_a_string() {
        let spec = lang_spec("python").unwrap();
        assert_eq!(tokenize(&spec, "'hi'"), [("'hi'".into(), TokenRole::Str)]);
    }

    #[test]
    fn number_does_not_overrun_a_range() {
        // `0..10` is two numbers around a `..`, never one giant token.
        let spec = lang_spec("rust").unwrap();
        let toks = tokenize(&spec, "0..10");
        assert_eq!(toks.first(), Some(&("0".into(), TokenRole::Number)));
        assert!(
            toks.iter()
                .any(|(t, r)| t == "10" && *r == TokenRole::Number),
            "trailing 10 stays a number in {toks:?}"
        );
    }

    #[test]
    fn sql_keywords_are_case_insensitive_and_use_dash_comments() {
        let spec = lang_spec("sql").unwrap();
        assert_eq!(roles(&spec, "SELECT"), [TokenRole::Keyword]);
        assert_eq!(roles(&spec, "select"), [TokenRole::Keyword]);
        assert_eq!(
            tokenize(&spec, "-- c"),
            [("-- c".into(), TokenRole::Comment)]
        );
    }

    #[test]
    fn inline_path_preserves_bold_italic_code() {
        // Footnote bodies now flow through `build_spans` (the shared inline
        // path); bold/italic/code must survive as span styling, not flatten to
        // plain text. Span derives `PartialEq`, so compare against the public
        // builder output.
        let specs = [
            SpanSpec {
                text: "b".into(),
                inline: Inline {
                    strong: true,
                    ..Inline::default()
                },
                link: None,
            },
            SpanSpec {
                text: "i".into(),
                inline: Inline {
                    emphasis: true,
                    ..Inline::default()
                },
                link: None,
            },
            SpanSpec {
                text: "c".into(),
                inline: Inline {
                    code: true,
                    ..Inline::default()
                },
                link: None,
            },
        ];
        let spans = build_spans(&specs);
        assert_eq!(spans[0], span("b").weight(Weight::Semibold));
        assert_eq!(spans[1], span("i").italic());
        assert_eq!(spans[2], span("c").family(FamilyRole::Mono));
    }
}

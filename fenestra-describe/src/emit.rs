//! The `fenestra/1` emitter: `Element` trees back into JSON [`Description`]s
//! — the inverse of [`crate::parse`], closing the round-trip so an agent can
//! import a builder-authored UI, edit it as data, and re-render it.
//!
//! The emitter covers the JSON-expressible subset and *reports* everything
//! else: the second return value lists, path-pointed, every feature of the
//! input tree that has no JSON projection (theme/hover/press style closures,
//! vector paths, virtualized rows, overlays, animation, exotic style
//! fields). An empty warning list therefore means the emitted document is a
//! faithful projection: parsing it back renders the same pixels for the
//! same theme.
//!
//! Colors emit as concrete OKLCH escape hatches, not theme roles — the
//! builder tree only holds resolved colors, so role names cannot be
//! recovered. Re-rendering under the same theme is exact; re-theming an
//! emitted document keeps the pinned colors.

use fenestra_core::{
    AlignItems, Direction, Element, JustifyContent, Kind, Length, Overflow, Paint, Position,
    Style as CoreStyle, TextAlign, TextStyle, oklch_of,
};

use crate::error::DescribeError;
use crate::format::{
    Border, ColorSpec, Container, Description, GrowSpec, ImageNode, InputNode, Leaf, Node,
    OklchColor, SCHEMA_V1, SizeSpec, Style, TextNode,
};
use crate::state::{Action, StateMap};

/// Emits a complete [`Description`] from a tree over [`Action`] — the
/// message type [`crate::parse`] itself produces — so click intents
/// round-trip. See the module docs for the coverage contract.
#[must_use]
pub fn emit_description(el: &Element<Action>) -> (Description, Vec<DescribeError>) {
    let mut warnings = Vec::new();
    let root = node(el, &intent_of_action, "/root", 1, &mut warnings);
    (
        Description {
            schema: SCHEMA_V1.to_owned(),
            root,
            theme: None,
            state: StateMap::default(),
        },
        warnings,
    )
}

/// Emits one [`Node`] from a tree over any message type. Handlers cannot be
/// read back from arbitrary `Msg` values, so every attached handler is
/// reported as a warning; use [`emit_description`] for [`Action`] trees.
#[must_use]
pub fn emit_element<Msg>(el: &Element<Msg>) -> (Node, Vec<DescribeError>) {
    let mut warnings = Vec::new();
    let node = node(el, &|_| None, "/root", 1, &mut warnings);
    (node, warnings)
}

/// Extracts the intent string from an [`Action`], where one exists: inert
/// intents round-trip; framework-owned state writes (`bind` handlers) do
/// not — re-author `bind` on the emitted node instead.
fn intent_of_action(a: &Action) -> Option<String> {
    match a {
        Action::Intent(s) => Some(s.clone()),
        Action::SetBool(..) | Action::SetText(..) | Action::SetNumber(..) => None,
    }
}

fn warn(warnings: &mut Vec<DescribeError>, path: &str, message: impl Into<String>) {
    warnings.push(DescribeError::new(path, message));
}

fn node<Msg>(
    el: &Element<Msg>,
    intent_of: &dyn Fn(&Msg) -> Option<String>,
    path: &str,
    depth: usize,
    warnings: &mut Vec<DescribeError>,
) -> Node {
    // Mirror the renderer's depth contract: never recurse past what
    // `build_frame` would accept (an emitter must not be the one traversal
    // that can blow the stack).
    if depth > fenestra_core::MAX_TREE_DEPTH {
        warn(
            warnings,
            path,
            format!(
                "subtree exceeds MAX_TREE_DEPTH ({}) and is emitted as an empty box",
                fenestra_core::MAX_TREE_DEPTH
            ),
        );
        return Node::Div(Container::default());
    }
    if el.has_dynamic_style() {
        warn(
            warnings,
            path,
            "theme/hover/press-dependent style closures resolve at render time and are not \
             emitted; the static style below is what could be captured",
        );
    }
    if el.has_generated_content() {
        warn(
            warnings,
            path,
            "generated content (virtual rows / container queries) is builder-only; the \
             generator itself is not emitted",
        );
    }
    let click = match el.click_msg() {
        Some(msg) => {
            let intent = intent_of(msg);
            if intent.is_none() {
                warn(
                    warnings,
                    path,
                    "click handler carries no inert intent (a bound state write or a non-Action \
                     message) and is not emitted",
                );
            }
            intent
        }
        None => None,
    };
    let style = style_spec(el.style(), path, warnings);
    let id = el.key().map(str::to_owned);

    match el.kind() {
        Kind::Box => {
            let children = el
                .children_ref()
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    node(
                        c,
                        intent_of,
                        &format!("{path}/children/{i}"),
                        depth + 1,
                        warnings,
                    )
                })
                .collect();
            let container = Container {
                children,
                style,
                on_click: click,
                id,
                ..Container::default()
            };
            if el.is_stack() {
                Node::Stack(container)
            } else if el.style().direction == Direction::Row {
                Node::Row(container)
            } else {
                Node::Col(container)
            }
        }
        Kind::Text(content) => Node::Text(TextNode {
            content: content.clone(),
            style,
            on_click: click,
            id,
            fallback: None,
        }),
        Kind::Rich(spans) => {
            warn(
                warnings,
                path,
                "rich text has no JSON node yet; spans are flattened to plain text (per-span \
                 weight/color/size is lost)",
            );
            Node::Text(TextNode {
                content: spans.iter().map(|s| s.content()).collect::<String>(),
                style,
                on_click: click,
                id,
                fallback: None,
            })
        }
        Kind::Divider => Node::Divider(Leaf {
            style,
            id,
            fallback: None,
        }),
        Kind::Path(_) => {
            warn(
                warnings,
                path,
                "vector path content has no JSON projection (JSON icons are name-based); \
                 emitted as an empty box of the same style",
            );
            Node::Div(Container {
                style,
                id,
                ..Container::default()
            })
        }
        Kind::Input(data) => {
            // Input handlers are closures; the value/placeholder state is
            // what can be captured.
            warn(
                warnings,
                path,
                "input handlers (on_input/on_key) are closures and are not emitted; \
                 re-author `on_input`/`bind` on the emitted node",
            );
            let styled =
                serde_json::to_value(&style).ok() != serde_json::to_value(Style::default()).ok();
            if styled || id.is_some() || click.is_some() {
                warn(
                    warnings,
                    path,
                    "the input node carries no style/id/on_click in the JSON grammar; wrap it \
                     in a styled container instead",
                );
            }
            let input = InputNode {
                value: data.value.clone(),
                placeholder: if data.placeholder.is_empty() {
                    None
                } else {
                    Some(data.placeholder.clone())
                },
                ..InputNode::default()
            };
            if data.multiline {
                Node::TextArea(input)
            } else {
                Node::TextInput(input)
            }
        }
        Kind::Image(data) => {
            let image = &data.image;
            let (w, h) = (image.width, image.height);
            let label = el
                .access_label()
                .map_or_else(|| "image".to_owned(), str::to_owned);
            if el.access_label().is_none() {
                warn(
                    warnings,
                    path,
                    "image has no accessible label; emitted with the placeholder alt text \
                     \"image\"",
                );
            }
            match encode_png(w, h, image.data.data()) {
                Some(png) => Node::Image(ImageNode {
                    png,
                    label,
                    style,
                    id,
                    on_click: click,
                    fallback: None,
                }),
                None => {
                    warn(
                        warnings,
                        path,
                        "image pixels could not be re-encoded as PNG",
                    );
                    Node::Div(Container {
                        style,
                        id,
                        ..Container::default()
                    })
                }
            }
        }
    }
}

/// Re-encodes straight-alpha RGBA8 pixels as a base64 PNG (RFC 4648
/// standard alphabet, the same one the parser's decoder accepts).
fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<String> {
    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec())?;
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(encode_base64(png.get_ref()))
}

/// RFC 4648 standard-alphabet base64 with padding (the encoder twin of the
/// parser's strict decoder).
fn encode_base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// A concrete color as the OKLCH escape hatch. Alpha is not expressible in
/// the format; the caller warns when it would be lost.
fn color_spec(c: fenestra_core::Color) -> ColorSpec {
    let [l, ch, h] = oklch_of(c);
    ColorSpec::Oklch(OklchColor { oklch: [l, ch, h] })
}

fn color_alpha(c: fenestra_core::Color) -> f32 {
    c.components[3]
}

fn size_spec(
    len: Length,
    path: &str,
    field: &str,
    warnings: &mut Vec<DescribeError>,
) -> Option<SizeSpec> {
    match len {
        Length::Auto => None,
        Length::Px(v) => Some(SizeSpec::Px(v)),
        Length::Pct(v) if (v - 100.0).abs() < f32::EPSILON => {
            Some(SizeSpec::Keyword("full".to_owned()))
        }
        Length::Pct(v) => Some(SizeSpec::Keyword(format!("{v}%"))),
        Length::Ch(_) => {
            warn(
                warnings,
                path,
                format!("{field}: ch-based sizes have no JSON projection; dropped"),
            );
            None
        }
    }
}

/// The core style's JSON projection. Every set field with no projection is
/// reported, so silence means fidelity.
#[expect(
    clippy::too_many_lines,
    reason = "a deliberate field-by-field inverse of apply_style; splitting it would \
              scatter the one place the projection is auditable"
)]
fn style_spec(s: &CoreStyle, path: &str, warnings: &mut Vec<DescribeError>) -> Style {
    let d = CoreStyle::default();
    let mut out = Style::default();

    // ── Padding / margin: minimal decomposition (all → axes → sides) ────────
    let pad = &s.padding;
    if pad.top == pad.bottom && pad.left == pad.right {
        if pad.top == pad.left {
            if pad.top != 0.0 {
                out.p = Some(pad.top);
            }
        } else {
            if pad.left != 0.0 {
                out.px = Some(pad.left);
            }
            if pad.top != 0.0 {
                out.py = Some(pad.top);
            }
        }
    } else {
        if pad.top != 0.0 {
            out.pt = Some(pad.top);
        }
        if pad.bottom != 0.0 {
            out.pb = Some(pad.bottom);
        }
        if pad.left != 0.0 {
            out.pl = Some(pad.left);
        }
        if pad.right != 0.0 {
            out.pr = Some(pad.right);
        }
    }
    let mar = &s.margin;
    if mar.top == mar.bottom && mar.left == mar.right {
        if mar.top == mar.left {
            if mar.top != 0.0 {
                out.m = Some(mar.top);
            }
        } else {
            if mar.left != 0.0 {
                out.mx = Some(mar.left);
            }
            if mar.top != 0.0 {
                out.my = Some(mar.top);
            }
        }
    } else {
        if mar.top != 0.0 {
            out.mt = Some(mar.top);
        }
        if mar.bottom != 0.0 {
            out.mb = Some(mar.bottom);
        }
        if mar.left != 0.0 {
            out.ml = Some(mar.left);
        }
        if mar.right != 0.0 {
            out.mr = Some(mar.right);
        }
    }

    // ── Gap / sizes / flex ──────────────────────────────────────────────────
    if s.gap != 0.0 {
        out.gap = Some(s.gap);
    }
    out.w = size_spec(s.width, path, "w", warnings);
    out.h = size_spec(s.height, path, "h", warnings);
    out.min_w = size_spec(s.min_width, path, "min_w", warnings);
    out.max_w = size_spec(s.max_width, path, "max_w", warnings);
    out.min_h = size_spec(s.min_height, path, "min_h", warnings);
    out.max_h = size_spec(s.max_height, path, "max_h", warnings);
    if s.flex_grow != 0.0 {
        out.grow = Some(if (s.flex_grow - 1.0).abs() < f32::EPSILON {
            GrowSpec::Flag(true)
        } else {
            GrowSpec::Factor(s.flex_grow)
        });
    }
    if s.flex_shrink != 1.0 {
        out.shrink = Some(s.flex_shrink);
    }
    if s.wrap {
        out.wrap = Some(true);
    }
    if s.flex_basis != d.flex_basis {
        warn(warnings, path, "flex_basis has no JSON projection; dropped");
    }
    out.scroll = match (s.overflow_x, s.overflow_y) {
        (Overflow::Visible, Overflow::Visible) => None,
        (Overflow::Scroll, Overflow::Scroll) => Some("both".to_owned()),
        (Overflow::Scroll, _) => Some("x".to_owned()),
        (_, Overflow::Scroll) => Some("y".to_owned()),
        (Overflow::Hidden, _) | (_, Overflow::Hidden) => Some("hidden".to_owned()),
    };

    // ── Alignment ───────────────────────────────────────────────────────────
    out.align = match s.align_items {
        AlignItems::Stretch => None,
        AlignItems::Start => Some("start".to_owned()),
        AlignItems::Center => Some("center".to_owned()),
        AlignItems::End => Some("end".to_owned()),
        AlignItems::Baseline => Some("baseline".to_owned()),
    };
    out.justify = match s.justify_content {
        JustifyContent::Start => None,
        JustifyContent::Center => Some("center".to_owned()),
        JustifyContent::End => Some("end".to_owned()),
        JustifyContent::SpaceBetween => Some("between".to_owned()),
    };
    if s.align_self.is_some() {
        warn(warnings, path, "align_self has no JSON projection; dropped");
    }
    if s.align_content != d.align_content {
        warn(
            warnings,
            path,
            "align_content has no JSON projection; dropped",
        );
    }

    // ── Position ────────────────────────────────────────────────────────────
    if s.position == Position::Absolute {
        out.absolute = Some(true);
        out.left = s.inset.left;
        out.top = s.inset.top;
        out.right = s.inset.right;
        out.bottom = s.inset.bottom;
    } else if s.inset.left.is_some()
        || s.inset.top.is_some()
        || s.inset.right.is_some()
        || s.inset.bottom.is_some()
    {
        warn(
            warnings,
            path,
            "relative inset offsets have no JSON projection; dropped",
        );
    }

    // ── Paint ───────────────────────────────────────────────────────────────
    match &s.fill {
        None => {}
        Some(Paint::Solid(c)) => {
            if color_alpha(*c) < 1.0 {
                warn(
                    warnings,
                    path,
                    "background alpha is not expressible in the color escape hatch; \
                     emitted opaque",
                );
            }
            out.bg = Some(color_spec(*c));
        }
        Some(_) => warn(
            warnings,
            path,
            "gradient/vibrancy backgrounds resample their stops at build time and do not \
             invert cleanly; dropped (re-author `gradient`/`material` on the emitted node)",
        ),
    }
    if let Some(b) = &s.border {
        out.border = Some(Border {
            width: b.width,
            color: color_spec(b.color),
        });
    }
    if s.side_borders != d.side_borders {
        warn(
            warnings,
            path,
            "per-side borders have no JSON projection; dropped",
        );
    }
    let cr = s.corner_radius;
    if cr.tl == cr.tr && cr.tr == cr.br && cr.br == cr.bl {
        if cr.tl.is_infinite() {
            out.rounded_full = Some(true);
        } else if cr.tl != 0.0 {
            out.rounded = Some(cr.tl);
        }
    } else {
        out.corners = Some([cr.tl, cr.tr, cr.br, cr.bl]);
    }
    if let Some(cs) = s.corner_smoothing {
        out.corner_smoothing = Some(cs);
    }
    if let Some(token) = s.shadow_token {
        use fenestra_core::ShadowToken;
        out.shadow = Some(
            match token {
                ShadowToken::Xs => "xs",
                ShadowToken::Sm => "sm",
                ShadowToken::Md => "md",
                ShadowToken::Lg => "lg",
                _ => "xl",
            }
            .to_owned(),
        );
    }
    if !s.shadows.is_empty() {
        warn(
            warnings,
            path,
            "custom shadow lists have no JSON projection; dropped",
        );
    }
    if s.highlight_top.is_some() {
        warn(
            warnings,
            path,
            "highlight_top has no JSON projection; dropped",
        );
    }

    // ── Glass optics ────────────────────────────────────────────────────────
    if let Some(e) = &s.specular_edge {
        out.specular_edge = Some(crate::format::EdgeSpec::Custom {
            light_deg: e.light_deg,
            intensity: e.intensity,
            shade: e.shade,
        });
    }
    if let Some(sh) = &s.sheen {
        out.sheen = Some(crate::format::SheenSpec::Custom {
            light_deg: sh.light_deg,
            top: sh.top,
            bottom: sh.bottom,
        });
    }
    if let Some(a) = &s.adaptive_tint {
        out.adaptive_tint = Some(crate::format::AdaptiveSpec::Custom {
            pivot: a.pivot,
            gain: a.gain,
        });
    }
    if let Some(b) = s.backdrop_blur {
        out.backdrop_blur = Some(b);
    }
    if s.element_filter.is_some() {
        warn(
            warnings,
            path,
            "element_filter has no emitter projection yet; re-author `element_filter` on \
             the emitted node",
        );
    }

    // ── Transforms / misc ───────────────────────────────────────────────────
    if s.opacity != 1.0 {
        out.opacity = Some(s.opacity);
    }
    if s.translate != (0.0, 0.0) {
        out.translate = Some([s.translate.0, s.translate.1]);
    }
    if s.rotate != 0.0 {
        out.rotate = Some(s.rotate);
    }
    if s.skew != (0.0, 0.0) {
        out.skew = Some([s.skew.0, s.skew.1]);
    }
    if s.scale != 1.0 || s.scale_xy != (1.0, 1.0) {
        warn(
            warnings,
            path,
            "scale transforms have no JSON projection; dropped",
        );
    }
    if s.path_trim != d.path_trim {
        warn(
            warnings,
            path,
            "path trim applies to vector paths, which do not emit",
        );
    }
    if s.sticky_top.is_some()
        || s.sticky_bottom.is_some()
        || s.sticky_left.is_some()
        || s.sticky_right.is_some()
    {
        warn(
            warnings,
            path,
            "sticky offsets have no JSON projection; dropped",
        );
    }

    // ── Grid ────────────────────────────────────────────────────────────────
    if !s.grid_template_columns.is_empty() || !s.grid_template_rows.is_empty() {
        warn(
            warnings,
            path,
            "grid templates have no emitter projection yet; re-author `grid_cols`/`grid_rows` \
             on the emitted node",
        );
    }
    if !s.grid_template_areas.is_empty() {
        out.grid_template_areas = Some(
            s.grid_template_areas
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|c| c.as_deref().unwrap_or("."))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect(),
        );
    }
    out.grid_area.clone_from(&s.grid_area);
    if let (Some(start), Some(end)) = (&s.grid_column_lines.start, &s.grid_column_lines.end) {
        out.grid_col_lines = Some([start.clone(), end.clone()]);
    }
    if let (Some(start), Some(end)) = (&s.grid_row_lines.start, &s.grid_row_lines.end) {
        out.grid_row_lines = Some([start.clone(), end.clone()]);
    }

    // ── Text ────────────────────────────────────────────────────────────────
    let t = &s.text;
    let td = TextStyle::default();
    if let Some(px) = t.size_px {
        out.size_px = Some(px);
    } else if t.size != td.size {
        out.size_px = Some(t.size.px());
    }
    if t.weight != td.weight {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "OpenType weights are small positive integers"
        )]
        {
            out.weight = Some(t.weight.value() as u16);
        }
    }
    if let Some(c) = t.color {
        if color_alpha(c) < 1.0 {
            warn(
                warnings,
                path,
                "text color alpha is not expressible in the color escape hatch; emitted opaque",
            );
        }
        out.color = Some(color_spec(c));
    }
    out.text_align = match t.align {
        TextAlign::Start => None,
        TextAlign::Center => Some("center".to_owned()),
        TextAlign::End => Some("end".to_owned()),
    };
    if t.family != td.family {
        warn(
            warnings,
            path,
            "font family roles have no JSON projection; dropped",
        );
    }
    if t.line_height != td.line_height
        || t.letter_spacing != td.letter_spacing
        || t.max_lines != td.max_lines
    {
        warn(
            warnings,
            path,
            "line_height/letter_spacing/max_lines have no JSON projection; dropped",
        );
    }

    out
}

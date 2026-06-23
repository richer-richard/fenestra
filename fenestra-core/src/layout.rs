//! Maps the fenestra `Style` layout group 1:1 onto `taffy::Style` and runs
//! layout. Text measurement is wired in M2; grid/scroll complete in M3.

use taffy::prelude::{Line, Size, TaffyGridLine, TaffyGridSpan, auto, fr, length, percent};
use taffy::style::{
    CheapCloneStr, GridPlacement, GridTemplateComponent, MaxTrackSizingFunction,
    MinTrackSizingFunction, RepetitionCount, TrackSizingFunction,
};
use taffy::style_helpers::{TaffyMaxContent, TaffyMinContent, fit_content, minmax, repeat};

use crate::style::{
    AlignContent, AlignItems, Direction, Display, GridPlace, GridTemplate, JustifyContent, Length,
    Overflow, Position, Repeat, Style, Track, TrackMax, TrackMin,
};

/// Translates one resolved style to taffy. `in_stack` forces the element
/// into grid cell (1, 1) so z-stack children overlap.
pub(crate) fn to_taffy(style: &Style, in_stack: bool) -> taffy::Style {
    let mut out = taffy::Style {
        display: match style.display {
            Display::Flex => taffy::style::Display::Flex,
            Display::Grid => taffy::style::Display::Grid,
            Display::None => taffy::style::Display::None,
        },
        flex_direction: match style.direction {
            Direction::Row => taffy::style::FlexDirection::Row,
            Direction::Column => taffy::style::FlexDirection::Column,
        },
        flex_wrap: if style.wrap {
            taffy::style::FlexWrap::Wrap
        } else {
            taffy::style::FlexWrap::NoWrap
        },
        align_items: Some(align_items(style.align_items)),
        align_self: style.align_self.map(align_items),
        justify_content: Some(match style.justify_content {
            JustifyContent::Start => taffy::style::JustifyContent::FlexStart,
            JustifyContent::Center => taffy::style::JustifyContent::Center,
            JustifyContent::End => taffy::style::JustifyContent::FlexEnd,
            JustifyContent::SpaceBetween => taffy::style::JustifyContent::SpaceBetween,
        }),
        align_content: Some(match style.align_content {
            AlignContent::Start => taffy::style::AlignContent::FlexStart,
            AlignContent::Center => taffy::style::AlignContent::Center,
            AlignContent::End => taffy::style::AlignContent::FlexEnd,
            AlignContent::Stretch => taffy::style::AlignContent::Stretch,
            AlignContent::SpaceBetween => taffy::style::AlignContent::SpaceBetween,
        }),
        gap: Size {
            width: length(finite(style.gap)),
            height: length(finite(style.gap)),
        },
        padding: taffy::geometry::Rect {
            left: length(finite(style.padding.left)),
            right: length(finite(style.padding.right)),
            top: length(finite(style.padding.top)),
            bottom: length(finite(style.padding.bottom)),
        },
        margin: taffy::geometry::Rect {
            left: length(finite(style.margin.left)),
            right: length(finite(style.margin.right)),
            top: length(finite(style.margin.top)),
            bottom: length(finite(style.margin.bottom)),
        },
        inset: taffy::geometry::Rect {
            left: inset(style.inset.left),
            right: inset(style.inset.right),
            top: inset(style.inset.top),
            bottom: inset(style.inset.bottom),
        },
        position: match style.position {
            Position::Relative => taffy::style::Position::Relative,
            Position::Absolute => taffy::style::Position::Absolute,
        },
        size: Size {
            width: dimension(style.width),
            height: dimension(style.height),
        },
        min_size: Size {
            width: dimension(style.min_width),
            height: dimension(style.min_height),
        },
        max_size: Size {
            width: dimension(style.max_width),
            height: dimension(style.max_height),
        },
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        flex_basis: dimension(style.flex_basis),
        grid_template_columns: style.grid_template_columns.iter().map(template).collect(),
        grid_template_rows: style.grid_template_rows.iter().map(template).collect(),
        overflow: taffy::geometry::Point {
            x: overflow(style.overflow_x),
            y: overflow(style.overflow_y),
        },
        scrollbar_width: 0.0,
        ..Default::default()
    };
    if in_stack {
        out.grid_row = taffy::prelude::line(1);
        out.grid_column = taffy::prelude::line(1);
    } else {
        out.grid_row = grid_line(style.grid_row);
        out.grid_column = grid_line(style.grid_column);
    }
    out
}

fn align_items(v: AlignItems) -> taffy::style::AlignItems {
    match v {
        AlignItems::Stretch => taffy::style::AlignItems::Stretch,
        AlignItems::Start => taffy::style::AlignItems::FlexStart,
        AlignItems::Center => taffy::style::AlignItems::Center,
        AlignItems::End => taffy::style::AlignItems::FlexEnd,
        // taffy never reports baselines for measured leaves (it would
        // synthesize bottom edges), so the frame pipeline lays baseline rows
        // out flex-start and shifts children by their true text baselines.
        AlignItems::Baseline => taffy::style::AlignItems::FlexStart,
    }
}

/// Hostile sizes are sanitized, not laid out: NaN spreads through
/// taffy and panics parley's line breaker, and near-f32::MAX values
/// overflow its advance arithmetic to infinity with the same result
/// (both found by the layout fuzzer). A billion logical px is beyond
/// any real canvas and leaves ~29 orders of magnitude before overflow.
const MAX_DIMENSION: f32 = 1.0e9;

fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, MAX_DIMENSION)
    } else {
        0.0
    }
}

fn dimension(l: Length) -> taffy::style::Dimension {
    match l {
        Length::Px(v) if !v.is_finite() => auto(),
        Length::Px(v) => length(v.clamp(0.0, MAX_DIMENSION)),
        Length::Pct(v) => percent(finite(v) / 100.0),
        // `Ch` reading measures are resolved to `Px` during `build` (where
        // font metrics are available) before taffy ever runs. A leaked value
        // is treated as `Auto` defensively — it cannot occur in the normal
        // pipeline.
        Length::Ch(_) => auto(),
        Length::Auto => auto(),
    }
}

fn inset(v: Option<f32>) -> taffy::style::LengthPercentageAuto {
    match v {
        // Insets are legitimately negative (offsets), so clamp both ways.
        Some(v) if v.is_finite() => length(v.clamp(-MAX_DIMENSION, MAX_DIMENSION)),
        Some(_) | None => auto(),
    }
}

fn overflow(v: Overflow) -> taffy::style::Overflow {
    match v {
        Overflow::Visible => taffy::style::Overflow::Visible,
        Overflow::Hidden => taffy::style::Overflow::Hidden,
        Overflow::Scroll => taffy::style::Overflow::Scroll,
    }
}

/// Maps one fenestra grid template entry to a taffy template component. Generic
/// over taffy's custom-ident type `S` (inferred from the collection target), so a
/// `repeat(...)` and a single track share one path.
fn template<S: CheapCloneStr>(g: &GridTemplate) -> GridTemplateComponent<S> {
    match g {
        GridTemplate::Single(t) => GridTemplateComponent::Single(track_sizing(*t)),
        GridTemplate::Repeat(rep, tracks) => repeat(
            repetition(*rep),
            tracks.iter().map(|t| track_sizing(*t)).collect(),
        ),
    }
}

/// One CSS `<track-size>` as a taffy track sizing function.
fn track_sizing(t: Track) -> TrackSizingFunction {
    match t {
        Track::Px(v) => length(finite(v)),
        Track::Fr(f) => fr(finite(f)),
        Track::Auto => auto(),
        Track::MinContent => TrackSizingFunction::MIN_CONTENT,
        Track::MaxContent => TrackSizingFunction::MAX_CONTENT,
        Track::FitContent(v) => fit_content(length(finite(v))),
        Track::MinMax(mn, mx) => minmax(track_min(mn), track_max(mx)),
    }
}

/// The `min` side of a `minmax()` as a taffy min track sizing function.
fn track_min(m: TrackMin) -> MinTrackSizingFunction {
    match m {
        TrackMin::Px(v) => length(finite(v)),
        TrackMin::Auto => auto(),
        TrackMin::MinContent => MinTrackSizingFunction::MIN_CONTENT,
        TrackMin::MaxContent => MinTrackSizingFunction::MAX_CONTENT,
    }
}

/// The `max` side of a `minmax()` as a taffy max track sizing function.
fn track_max(m: TrackMax) -> MaxTrackSizingFunction {
    match m {
        TrackMax::Px(v) => length(finite(v)),
        TrackMax::Fr(f) => fr(finite(f)),
        TrackMax::Auto => auto(),
        TrackMax::MinContent => MaxTrackSizingFunction::MIN_CONTENT,
        TrackMax::MaxContent => MaxTrackSizingFunction::MAX_CONTENT,
        TrackMax::FitContent(v) => fit_content(length(finite(v))),
    }
}

/// Maps a fenestra [`Repeat`] to taffy's repetition count.
fn repetition(r: Repeat) -> RepetitionCount {
    match r {
        Repeat::Count(n) => RepetitionCount::Count(n),
        Repeat::AutoFit => RepetitionCount::AutoFit,
        Repeat::AutoFill => RepetitionCount::AutoFill,
    }
}

fn grid_line(p: GridPlace) -> Line<GridPlacement> {
    match (p.start, p.span) {
        (None, None) => Line {
            start: GridPlacement::Auto,
            end: GridPlacement::Auto,
        },
        (Some(s), None) => taffy::prelude::line(s),
        (None, Some(n)) => taffy::prelude::span(n),
        (Some(s), Some(n)) => Line {
            start: GridPlacement::from_line_index(s),
            end: GridPlacement::from_span(n),
        },
    }
}

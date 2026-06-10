//! Maps the fenestra `Style` layout group 1:1 onto `taffy::Style` and runs
//! layout. Text measurement is wired in M2; grid/scroll complete in M3.

use taffy::prelude::{Line, Size, TaffyGridLine, TaffyGridSpan, auto, fr, length, percent};
use taffy::style::GridPlacement;

use crate::style::{
    AlignContent, AlignItems, Direction, Display, GridPlace, JustifyContent, Length, Overflow,
    Position, Style, Track,
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
            width: length(style.gap),
            height: length(style.gap),
        },
        padding: taffy::geometry::Rect {
            left: length(style.padding.left),
            right: length(style.padding.right),
            top: length(style.padding.top),
            bottom: length(style.padding.bottom),
        },
        margin: taffy::geometry::Rect {
            left: length(style.margin.left),
            right: length(style.margin.right),
            top: length(style.margin.top),
            bottom: length(style.margin.bottom),
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
        grid_template_columns: style
            .grid_template_columns
            .iter()
            .map(|t| track(*t))
            .collect(),
        grid_template_rows: style.grid_template_rows.iter().map(|t| track(*t)).collect(),
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

fn dimension(l: Length) -> taffy::style::Dimension {
    match l {
        Length::Px(v) => length(v),
        Length::Pct(v) => percent(v / 100.0),
        Length::Auto => auto(),
    }
}

fn inset(v: Option<f32>) -> taffy::style::LengthPercentageAuto {
    match v {
        Some(v) => length(v),
        None => auto(),
    }
}

fn overflow(v: Overflow) -> taffy::style::Overflow {
    match v {
        Overflow::Visible => taffy::style::Overflow::Visible,
        Overflow::Hidden => taffy::style::Overflow::Hidden,
        Overflow::Scroll => taffy::style::Overflow::Scroll,
    }
}

fn track<T: taffy::style_helpers::FromLength + taffy::style_helpers::FromFr>(t: Track) -> T {
    match t {
        Track::Px(v) => length(v),
        Track::Fr(f) => fr(f),
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

//! The widget kit: every widget is built only on core's public API, colors
//! route through theme tokens via deferred `themed` styling, and color
//! changes animate with Fast transitions.

mod button;
mod checkbox;
mod combobox;
mod data_table;
mod date_picker;
mod display;
mod menu;
mod overlay_widgets;
mod palette;
mod panes;
mod radio;
mod select;
mod slider;
mod switch;
mod text_area;
mod text_input;
mod toast;
mod tree;
mod virtual_list;

pub use button::{Button, ButtonVariant, IconButton, button, icon_button};
pub use checkbox::{Checkbox, checkbox};
pub use combobox::{Combobox, combobox};
pub use data_table::{DataTable, data_table};
pub use date_picker::{Date, DatePicker, date_picker};
pub use display::{
    StatCard, Status, avatar, badge, badge_dot, callout, card, progress, progress_indeterminate,
    spinner, stat_card, table, tabs,
};
pub use menu::{context_menu, dropdown_menu, menu, popover};
pub use overlay_widgets::{Modal, modal, tooltip};
pub use palette::{CommandPalette, command_palette};
pub use panes::{SplitPane, split_pane};
pub use radio::{Radio, radio};
pub use select::{Select, select};
pub use slider::{Slider, slider};
pub use switch::{Switch, switch};
pub use text_area::{TextArea, text_area};
pub use text_input::{TextInput, text_input};
pub use toast::{ToastStack, toast_stack};
pub use tree::{TreeNode, TreeView, tree_view};
pub use virtual_list::{virtual_list, virtual_list_variable};

/// Control sizes on a shared height grid (24 / 32 / 36 / 40 logical px), so a
/// row of mixed controls — button, input, select — lines up on one baseline.
/// Each size resolves to a [`ControlMetrics`] bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlSize {
    /// 24px tall: dense toolbars, inline chips.
    Xs,
    /// 32px tall: compact forms.
    Sm,
    /// 36px tall (default).
    #[default]
    Md,
    /// 40px tall: prominent primary actions.
    Lg,
}

/// The resolved dimensions of a [`ControlSize`] — every measure a control needs
/// to sit on the shared height grid. One struct so widgets pull height,
/// padding, gap, font, and icon size from a single source instead of guessing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlMetrics {
    /// Control height in logical px.
    pub height: f32,
    /// Horizontal padding in logical px.
    pub pad_x: f32,
    /// Internal gap between icon and label in logical px.
    pub gap: f32,
    /// Label text size.
    pub font: fenestra_core::TextSize,
    /// Icon edge length in logical px.
    pub icon: f32,
}

impl ControlSize {
    /// The resolved metrics for this size.
    pub const fn metrics(self) -> ControlMetrics {
        use fenestra_core::TextSize;
        match self {
            Self::Xs => ControlMetrics {
                height: 24.0,
                pad_x: 8.0,
                gap: 4.0,
                font: TextSize::Xs,
                icon: 14.0,
            },
            Self::Sm => ControlMetrics {
                height: 32.0,
                pad_x: 12.0,
                gap: 6.0,
                font: TextSize::Sm,
                icon: 16.0,
            },
            Self::Md => ControlMetrics {
                height: 36.0,
                pad_x: 16.0,
                gap: 8.0,
                font: TextSize::Sm,
                icon: 18.0,
            },
            Self::Lg => ControlMetrics {
                height: 40.0,
                pad_x: 20.0,
                gap: 8.0,
                font: TextSize::Base,
                icon: 20.0,
            },
        }
    }

    /// Control height in logical px.
    pub const fn height(self) -> f32 {
        self.metrics().height
    }

    /// Horizontal padding in logical px.
    pub(crate) const fn padding_x(self) -> f32 {
        self.metrics().pad_x
    }

    /// Label text size.
    pub(crate) const fn text_size(self) -> fenestra_core::TextSize {
        self.metrics().font
    }

    /// Internal icon/label gap in logical px.
    pub(crate) const fn gap(self) -> f32 {
        self.metrics().gap
    }

    /// Icon edge length in logical px.
    pub(crate) const fn icon(self) -> f32 {
        self.metrics().icon
    }
}

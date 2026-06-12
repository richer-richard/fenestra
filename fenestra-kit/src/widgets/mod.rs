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
pub use virtual_list::virtual_list;

/// Control heights shared by buttons and inputs, logical px.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlSize {
    /// 28px tall.
    Sm,
    /// 36px tall (default).
    #[default]
    Md,
    /// 44px tall.
    Lg,
}

impl ControlSize {
    /// Control height in logical px.
    pub const fn height(self) -> f32 {
        match self {
            Self::Sm => 28.0,
            Self::Md => 36.0,
            Self::Lg => 44.0,
        }
    }

    /// Horizontal padding (1.5x the implied vertical padding).
    pub(crate) const fn padding_x(self) -> f32 {
        match self {
            Self::Sm => 12.0,
            Self::Md => 16.0,
            Self::Lg => 20.0,
        }
    }

    /// Label text size.
    pub(crate) const fn text_size(self) -> fenestra_core::TextSize {
        match self {
            Self::Sm | Self::Md => fenestra_core::TextSize::Sm,
            Self::Lg => fenestra_core::TextSize::Base,
        }
    }
}

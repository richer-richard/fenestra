//! The widget kit: every widget is built only on core's public API, colors
//! route through theme tokens via deferred `themed` styling, and color
//! changes animate with Fast transitions.

mod button;
mod checkbox;
mod combobox;
mod data_table;
mod date_picker;
mod disclosure;
mod display;
mod field;
mod glass;
mod kbd;
mod menu;
mod multi_select;
mod navigation;
mod overlay_widgets;
mod palette;
mod panes;
mod radio;
mod segmented;
mod select;
mod skeleton;
mod slider;
mod spin_button;
mod switch;
mod tag_input;
mod text_area;
mod text_input;
mod toast;
mod toolbar;
mod tree;
pub mod validation;
mod virtual_list;

pub use button::{Button, ButtonVariant, IconButton, button, icon_button};
pub use checkbox::{Checkbox, checkbox};
pub use combobox::{Combobox, combobox};
pub use data_table::{DataTable, data_table};
pub use date_picker::{Date, DatePicker, date_picker, date_range_picker};
pub use disclosure::{Accordion, AccordionItem, accordion, accordion_item};
pub use display::{
    Meter, StatCard, Status, StatusIndicator, WavyProgress, avatar, badge, badge_dot, callout,
    card, meter, progress, progress_indeterminate, reading_column, responsive_grid, spinner,
    stat_card, status, table, tabs, wavy_progress,
};
pub use field::{Field, field};
pub use glass::{glass_panel, glass_surface};
pub use kbd::{kbd, kbd_raised};
pub use menu::{
    Menu, MenuItem, Menubar, context_menu, dropdown_menu, menu, menu_item, menu_items,
    menu_separator, menubar, popover,
};
pub use multi_select::{MultiSelect, multi_select};
pub use navigation::{
    Breadcrumbs, Crumb, Pagination, Stepper, breadcrumbs, crumb, pagination, stepper,
};
pub use overlay_widgets::{Drawer, Modal, drawer, modal, tooltip};
pub use palette::{CommandPalette, command_palette};
pub use panes::{SplitPane, split_pane};
pub use radio::{Radio, radio, radio_group};
pub use segmented::{Segmented, segmented};
pub use select::{Select, select};
pub use skeleton::{skeleton, skeleton_circle, skeleton_text};
pub use slider::{RangeSlider, Slider, range_slider, slider};
pub use spin_button::{SpinButton, spin_button};
pub use switch::{Switch, switch};
pub use tag_input::{TagInput, tag_input};
pub use text_area::{TextArea, text_area};
pub use text_input::{TextInput, text_input};
pub use toast::{ToastStack, toast_stack};
pub use toolbar::{Toolbar, toolbar};
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

/// How tightly controls pack on the shared height grid. `Comfortable` (the
/// default) is the stock metrics — byte-identical to before density existed, so
/// the kit looks unchanged unless you opt in. `Compact` tightens height,
/// padding, gap, and icon for dense, information-rich pro-tool UIs; `Spacious`
/// loosens them. The label font stays tied to the [`ControlSize`] across every
/// density — density scales *spacing*, not *type*, so a control's text never
/// shrinks below its legible size. Resolve via [`ControlSize::metrics_at`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    /// Tighter height / padding / gap / icon — dense forms and toolbars.
    Compact,
    /// The stock metrics (byte-identical to pre-density). The default.
    #[default]
    Comfortable,
    /// Roomier height / padding / gap / icon.
    Spacious,
}

impl ControlSize {
    /// The resolved metrics for this size (at [`Density::Comfortable`]).
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

    /// The metrics for this size at `density`. [`Density::Comfortable`] is
    /// exactly [`metrics`](Self::metrics) (so existing layouts are unchanged);
    /// `Compact` and `Spacious` scale height, padding, gap, and icon while
    /// keeping the same label `font` (density is spacing, not type).
    pub const fn metrics_at(self, density: Density) -> ControlMetrics {
        use fenestra_core::TextSize;
        match density {
            Density::Comfortable => self.metrics(),
            Density::Compact => match self {
                Self::Xs => ControlMetrics {
                    height: 22.0,
                    pad_x: 6.0,
                    gap: 3.0,
                    font: TextSize::Xs,
                    icon: 14.0,
                },
                Self::Sm => ControlMetrics {
                    height: 28.0,
                    pad_x: 10.0,
                    gap: 4.0,
                    font: TextSize::Sm,
                    icon: 14.0,
                },
                Self::Md => ControlMetrics {
                    height: 32.0,
                    pad_x: 12.0,
                    gap: 6.0,
                    font: TextSize::Sm,
                    icon: 16.0,
                },
                Self::Lg => ControlMetrics {
                    height: 36.0,
                    pad_x: 16.0,
                    gap: 6.0,
                    font: TextSize::Base,
                    icon: 18.0,
                },
            },
            Density::Spacious => match self {
                Self::Xs => ControlMetrics {
                    height: 28.0,
                    pad_x: 10.0,
                    gap: 6.0,
                    font: TextSize::Xs,
                    icon: 16.0,
                },
                Self::Sm => ControlMetrics {
                    height: 36.0,
                    pad_x: 16.0,
                    gap: 8.0,
                    font: TextSize::Sm,
                    icon: 18.0,
                },
                Self::Md => ControlMetrics {
                    height: 40.0,
                    pad_x: 20.0,
                    gap: 10.0,
                    font: TextSize::Sm,
                    icon: 20.0,
                },
                Self::Lg => ControlMetrics {
                    height: 48.0,
                    pad_x: 24.0,
                    gap: 12.0,
                    font: TextSize::Base,
                    icon: 24.0,
                },
            },
        }
    }

    /// Control height in logical px (at [`Density::Comfortable`]). The nominal
    /// grid height for this size; density-aware widgets resolve the full
    /// [`ControlMetrics`] via [`metrics_at`](Self::metrics_at) instead.
    pub const fn height(self) -> f32 {
        self.metrics().height
    }
}

#[cfg(test)]
mod density_tests {
    use super::{ControlSize, Density};

    const SIZES: [ControlSize; 4] = [
        ControlSize::Xs,
        ControlSize::Sm,
        ControlSize::Md,
        ControlSize::Lg,
    ];

    #[test]
    fn comfortable_is_byte_identical_to_today() {
        // Comfortable must equal the pre-density `metrics()` exactly, so every
        // existing widget golden stays unchanged. Pin the literals too, so a
        // future table edit trips this test rather than silently moving pixels.
        for size in SIZES {
            assert_eq!(size.metrics_at(Density::Comfortable), size.metrics());
        }
        let sm = ControlSize::Sm.metrics();
        assert_eq!(
            (sm.height, sm.pad_x, sm.gap, sm.icon),
            (32.0, 12.0, 6.0, 16.0)
        );
        let lg = ControlSize::Lg.metrics();
        assert_eq!(
            (lg.height, lg.pad_x, lg.gap, lg.icon),
            (40.0, 20.0, 8.0, 20.0)
        );
    }

    #[test]
    fn density_scales_box_monotonically() {
        for size in SIZES {
            let c = size.metrics_at(Density::Compact);
            let m = size.metrics_at(Density::Comfortable);
            let s = size.metrics_at(Density::Spacious);
            assert!(
                c.height < m.height && m.height < s.height,
                "{size:?} height"
            );
            assert!(c.pad_x <= m.pad_x && m.pad_x <= s.pad_x, "{size:?} pad_x");
            assert!(c.gap <= m.gap && m.gap <= s.gap, "{size:?} gap");
            assert!(c.icon <= m.icon && m.icon <= s.icon, "{size:?} icon");
        }
    }

    #[test]
    fn density_preserves_legible_font() {
        // Density scales spacing, not type: the label font is tied to the
        // control size and never shrinks (or grows) with density.
        for size in SIZES {
            let font = size.metrics().font;
            assert_eq!(size.metrics_at(Density::Compact).font, font, "{size:?}");
            assert_eq!(size.metrics_at(Density::Spacious).font, font, "{size:?}");
        }
    }
}

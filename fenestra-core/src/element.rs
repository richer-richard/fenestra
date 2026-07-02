//! The element IR: a plain tree of boxes and text, with typed styles,
//! interaction variant overlays, and `Msg`-carrying handlers. `view()`
//! rebuilds this tree on every redraw; it must stay cheap to construct.

use peniko::Color;

use crate::events::KeyInput;
use crate::style::{Length, Paint, Style, TextAlign, TextWrap, ThemedFn, Transition};
use crate::theme::Theme;
use crate::tokens::{ShadowToken, TextSize, Weight};

/// Maps a key press on a focused element to an optional message.
pub(crate) type TypeAheadFn<Msg> = Box<dyn Fn(&str) -> Option<Msg>>;
pub(crate) type KeyFn<Msg> = Box<dyn Fn(&KeyInput) -> Option<Msg>>;
/// Maps a pointer position (as fractions of the element rect) to a message.
pub(crate) type DragFn<Msg> = Box<dyn Fn(f32, f32) -> Option<Msg>>;
/// Maps a recognized [`SwipeDir`] to a message.
pub(crate) type SwipeFn<Msg> = Box<dyn Fn(SwipeDir) -> Msg>;
/// Maps the edited text to a message.
pub(crate) type InputFn<Msg> = Box<dyn Fn(&str) -> Msg>;
/// Maps a dropped OS file path to a message.
pub(crate) type FileDropFn<Msg> = Box<dyn Fn(&std::path::Path) -> Msg>;
/// Maps an internal drag payload to an optional message on drop.
pub(crate) type DropFn<Msg> = Box<dyn Fn(&str) -> Option<Msg>>;
/// Resolves a control's content color (the color drawn *on* it) from the
/// theme, for the uniform state-layer engine.
pub(crate) type ContentFn = Box<dyn Fn(&Theme) -> Color>;

/// What an element fundamentally is.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A container box.
    Box,
    /// A text run (one style for the whole paragraph).
    Text(String),
    /// A paragraph of styled runs ([`rich_text`]): one wrapped layout,
    /// per-span weight/color/size/family/italic.
    Rich(Vec<Span>),
    /// A themed hairline rule (resolved to `border_subtle`).
    Divider,
    /// A vector path (icons, check marks), scaled from its viewbox to the
    /// element rect and painted in the resolved text color.
    Path(PathData),
    /// A single-line editable text field driven by parley's `PlainEditor`.
    /// The app's value is the source of truth; edits emit `on_input`.
    Input(InputData),
    /// An RGBA8 image stretched to the element rect and clipped to its
    /// corner radius.
    Image(ImageData),
}

/// Payload for virtualized rows ([`Element::virtual_rows`]): only the
/// scrolled-into-view window of rows is materialized each frame.
pub struct VirtualData<Msg> {
    pub(crate) count: usize,
    pub(crate) row_height: f32,
    /// Rows size themselves; `row_height` is the estimate for
    /// unmaterialized ones (heights are measured and corrected).
    pub(crate) variable: bool,
    pub(crate) builder: std::rc::Rc<dyn Fn(usize) -> Element<Msg>>,
}

impl<Msg> Clone for VirtualData<Msg> {
    fn clone(&self) -> Self {
        Self {
            count: self.count,
            row_height: self.row_height,
            variable: self.variable,
            builder: std::rc::Rc::clone(&self.builder),
        }
    }
}

/// Payload for a container query ([`responsive`]): the closure rebuilds this
/// container's subtree from its own available size, and `hint` is the size the
/// first frame uses before any measurement exists.
pub(crate) struct ResponsiveData<Msg> {
    pub(crate) hint: (f32, f32),
    pub(crate) f: std::rc::Rc<dyn Fn((f32, f32)) -> Element<Msg>>,
}

/// Payload for [`Kind::Image`].
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Decoded straight-alpha RGBA8 pixels, shared cheaply across frames.
    pub image: peniko::ImageData,
}

impl PartialEq for ImageData {
    /// Identity comparison (same pixel allocation and dimensions), not a
    /// byte-by-byte one: rebuilt views share the `Arc`'d blob.
    fn eq(&self, other: &Self) -> bool {
        self.image.data.id() == other.image.data.id()
            && self.image.width == other.image.width
            && self.image.height == other.image.height
    }
}

/// Payload for [`Kind::Input`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputData {
    /// The current value (app state).
    pub value: String,
    /// Placeholder shown when the value is empty.
    pub placeholder: String,
    /// Multiline editing: text wraps to the element width, Enter inserts a
    /// newline, and the measured height grows with the content.
    pub multiline: bool,
}

/// Optical corrections for a [`Kind::Path`] (see [`crate::optical`]): geometric
/// nudges that make a shape *look* right even though it measures "wrong". Both
/// default to off, so a path renders byte-identically until opted in — set them
/// per icon where the shape needs it, rather than auto-detecting (which would
/// silently shift every existing icon).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OpticalCorrection {
    /// Scale the drawn path up by [`crate::optical::CIRCLE_OVERSHOOT`] about the
    /// viewbox center, so a round or pointed icon reads the same visual size as
    /// square-edged neighbors.
    pub overshoot: bool,
    /// Translate the path so its centroid (visual mass, [`crate::optical::centroid`])
    /// sits at the viewbox center instead of its bounding-box center — the
    /// classic "nudge the play triangle toward its point" correction.
    pub center: bool,
}

/// Path payload for [`Kind::Path`].
#[derive(Debug, Clone, PartialEq)]
pub struct PathData {
    /// The path, in viewbox coordinates.
    pub path: std::sync::Arc<kurbo::BezPath>,
    /// Design-space size the path was drawn in.
    pub viewbox: (f64, f64),
    /// Stroke width in viewbox units; `None` fills the path instead.
    pub stroke: Option<f64>,
    /// Optical corrections (overshoot / centroid centering); both off by default.
    pub optical: OpticalCorrection,
}

/// How an overlay child opens and closes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayMode {
    /// Open while present in the tree; the app controls it (modals).
    /// Outside clicks and Esc emit the overlay's `on_close` message.
    Open,
    /// Clicking the anchor (parent) toggles it; outside clicks and Esc
    /// close it, and a click on any clickable inside closes it (menus).
    Toggle,
    /// Shows after hovering the anchor for the delay; never hit-tested.
    Hover {
        /// Hover delay in milliseconds.
        delay_ms: f32,
    },
}

/// Which screen edge a drawer/sheet overlay is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerSide {
    /// The left edge, filling the full canvas height.
    Left,
    /// The right edge, filling the full canvas height.
    Right,
    /// The top edge, filling the full canvas width.
    Top,
    /// The bottom edge, filling the full canvas width — a bottom sheet.
    Bottom,
}

/// Where an overlay is positioned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayPlacement {
    /// Below the anchor, left edges aligned, with a vertical gap. Flips
    /// above when there is no room below.
    Below {
        /// Gap in logical px.
        gap: f32,
    },
    /// Below the anchor, horizontally centered on it.
    BelowCenter {
        /// Gap in logical px.
        gap: f32,
    },
    /// Centered in the canvas.
    Center,
    /// Pinned to the top-right of the canvas (toast stacks).
    TopRight {
        /// Margin from the canvas edges in logical px.
        margin: f32,
    },
    /// At the pointer position when the overlay opened (context menus);
    /// pinned there until it closes.
    Pointer {
        /// Offset from the pointer in logical px.
        gap: f32,
    },
    /// Flush against a screen edge, filling that edge's full span (drawers and
    /// sheets); slides in from off-canvas as it opens.
    Edge {
        /// Which edge to anchor to.
        side: DrawerSide,
    },
    /// To the right of the anchor with top edges aligned (submenu flyouts);
    /// flips to the anchor's left when there is no room on the right.
    RightStart {
        /// Gap from the anchor's right (or left, when flipped) edge in px.
        gap: f32,
    },
}

/// Marks an element as an overlay child of its parent (the anchor):
/// excluded from normal layout, laid out against the canvas, painted after
/// the root, and hit-tested first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlay {
    /// Open/close behavior.
    pub mode: OverlayMode,
    /// Positioning relative to the anchor or canvas.
    pub placement: OverlayPlacement,
    /// Dim everything beneath with black at 0.4 alpha.
    pub backdrop: bool,
    /// Tab cycles only inside this overlay while it is open.
    pub trap_focus: bool,
}

impl Overlay {
    /// A click-toggled menu below its anchor (select listboxes).
    pub fn menu() -> Self {
        Self {
            mode: OverlayMode::Toggle,
            placement: OverlayPlacement::Below { gap: 4.0 },
            backdrop: false,
            trap_focus: false,
        }
    }

    /// A hover tooltip: 400ms delay, 6px below, centered, untouchable.
    pub fn tooltip() -> Self {
        Self {
            mode: OverlayMode::Hover { delay_ms: 400.0 },
            placement: OverlayPlacement::BelowCenter { gap: 6.0 },
            backdrop: false,
            trap_focus: false,
        }
    }

    /// An app-driven centered modal with backdrop and focus trap.
    pub fn modal() -> Self {
        Self {
            mode: OverlayMode::Open,
            placement: OverlayPlacement::Center,
            backdrop: true,
            trap_focus: true,
        }
    }

    /// An app-driven context menu pinned at the right-click position:
    /// pair with `.on_right_click(open_msg)` on the target and
    /// `.on_close(close_msg)` on the menu; item clicks are the app's cue
    /// to close.
    pub fn context() -> Self {
        Self {
            mode: OverlayMode::Open,
            placement: OverlayPlacement::Pointer { gap: 2.0 },
            backdrop: false,
            trap_focus: false,
        }
    }

    /// An app-driven toast stack pinned to the top-right: no backdrop, no
    /// focus trap, and nothing closes it from outside — dismissal is the
    /// stack's own buttons (or the app removing items).
    pub fn toasts() -> Self {
        Self {
            mode: OverlayMode::Open,
            placement: OverlayPlacement::TopRight { margin: 16.0 },
            backdrop: false,
            trap_focus: false,
        }
    }

    /// An app-driven drawer/sheet flush to a screen `side`, with a backdrop and
    /// focus trap; it slides in from that edge. Render it only while open;
    /// `on_close` fires on Esc and an outside (scrim) click.
    pub fn drawer(side: DrawerSide) -> Self {
        Self {
            mode: OverlayMode::Open,
            placement: OverlayPlacement::Edge { side },
            backdrop: true,
            trap_focus: true,
        }
    }

    /// A click-toggled submenu flyout to the right of its anchoring menu item
    /// (flipping left at the canvas edge). Clicking or pressing Enter on the
    /// anchor opens/closes it; outside clicks and Escape close it.
    pub fn submenu() -> Self {
        Self {
            mode: OverlayMode::Toggle,
            placement: OverlayPlacement::RightStart { gap: 2.0 },
            backdrop: false,
            trap_focus: false,
        }
    }
}

/// Accessible role and state of an element, projected into the platform
/// accessibility tree by the shell (AccessKit) and exposed headlessly via
/// `Frame::access_tree`. Text, image, and input leaves project
/// automatically; kit widgets set the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Semantics {
    /// An activatable button.
    Button,
    /// A checkbox: two-state, or tri-state when `mixed`.
    Checkbox {
        /// Whether it is checked.
        checked: bool,
        /// Whether it is in the indeterminate (mixed) state — projects
        /// `aria-checked="mixed"`.
        mixed: bool,
    },
    /// An on/off switch.
    Switch {
        /// Whether it is on.
        on: bool,
    },
    /// One option of a radio group.
    Radio {
        /// Whether it is the selected option.
        selected: bool,
    },
    /// A numeric slider.
    Slider {
        /// Current value.
        value: f32,
        /// Minimum value.
        min: f32,
        /// Maximum value.
        max: f32,
    },
    /// An editable text field (value comes from the input element).
    TextInput {
        /// Whether it accepts newlines.
        multiline: bool,
    },
    /// A button that opens a listbox of options.
    ComboBox,
    /// A modal dialog.
    Dialog,
    /// One tab of a tab strip.
    Tab {
        /// Whether it is the active tab.
        selected: bool,
    },
    /// A transient notification (toasts).
    Alert,
    /// Static text (automatic for text leaves).
    Label,
    /// An image (automatic for image leaves).
    Image,
    /// A numeric input stepped by buttons / arrow keys (CSS `<input type=number>`).
    Spinbutton {
        /// Current value.
        value: f32,
        /// Minimum value.
        min: f32,
        /// Maximum value.
        max: f32,
    },
    /// A scalar measurement within a known range (the HTML `<meter>`).
    Meter {
        /// Current value.
        value: f32,
        /// Minimum value.
        min: f32,
        /// Maximum value.
        max: f32,
    },
    /// Task completion. `value` is the fraction `0.0..=1.0`, or `None` when the
    /// progress is indeterminate.
    ProgressBar {
        /// Completed fraction `0.0..=1.0`, or `None` for indeterminate.
        value: Option<f32>,
    },
}

impl Semantics {
    /// The ARIA role word for this role (`"button"`, `"textbox"`, …) — the same
    /// vocabulary [`Frame::access_yaml`](crate::Frame::access_yaml) emits. The
    /// public name for a node's role, stable across versions.
    #[must_use]
    pub fn aria_role(&self) -> &'static str {
        crate::query::role_name(self)
    }
}

/// One styled run of a [`rich_text`] paragraph. Unset properties
/// inherit the paragraph's text style.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub(crate) text: String,
    pub(crate) weight: Option<crate::tokens::Weight>,
    pub(crate) color: Option<Color>,
    pub(crate) size_px: Option<f32>,
    pub(crate) family: Option<crate::tokens::FamilyRole>,
    pub(crate) italic: bool,
}

/// A styled run for [`rich_text`].
pub fn span(text: impl Into<String>) -> Span {
    Span {
        text: text.into(),
        weight: None,
        color: None,
        size_px: None,
        family: None,
        italic: false,
    }
}

impl Span {
    /// Font weight for this run.
    pub fn weight(mut self, weight: crate::tokens::Weight) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Color for this run (route through theme tokens in app code).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Font size in logical px for this run.
    pub fn size_px(mut self, px: f32) -> Self {
        self.size_px = Some(px);
        self
    }

    /// Family role for this run.
    pub fn family(mut self, family: crate::tokens::FamilyRole) -> Self {
        self.family = Some(family);
        self
    }

    /// Italic (synthesized when the face has no italic variant).
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}

/// A recognized swipe (flick) direction, delivered by
/// [`Element::on_swipe`]. Screen axes: `Down` is toward the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDir {
    /// A flick toward the top.
    Up,
    /// A flick toward the bottom.
    Down,
    /// A flick toward the start of the line (left in LTR).
    Left,
    /// A flick toward the end of the line (right in LTR).
    Right,
}

/// Mouse cursor shown while hovering an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    /// The platform default arrow.
    Default,
    /// Pointing hand (buttons, links).
    Pointer,
    /// Text I-beam.
    Text,
    /// Action not available.
    NotAllowed,
}

/// How an element animates *out* when it leaves the tree (the counterpart of
/// [`Element::enter`]). When an element tagged with [`Element::exit`] is
/// removed, a paint-only snapshot ("ghost") is left in its place and animates
/// toward these targets over `transition`, then is dropped — there is no live
/// widget behind it (inputs collapse to a plain box). Inert under reduced
/// motion: the element is removed immediately, so headless renders are
/// unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExitAnim {
    /// Timing of the exit (spring, or duration + easing).
    pub transition: Transition,
    /// Opacity the ghost fades to (0.0 = fully gone, the default).
    pub opacity_to: f32,
    /// Scale the ghost reaches about its center (1.0 = no scale change).
    pub scale_to: f32,
    /// Translation the ghost drifts by as it leaves, logical px `(dx, dy)`.
    pub translate_to: (f32, f32),
}

/// One node in the view tree. `Msg` is the app's message type; handlers
/// carry `Msg` values, not closures over state.
pub struct Element<Msg> {
    pub(crate) kind: Kind,
    pub(crate) style: Style,
    pub(crate) children: Vec<Element<Msg>>,
    /// User key for stable identity (`.id()`).
    pub(crate) key: Option<String>,
    /// Where this element was constructed (file:line of the builder
    /// call) — surfaced by `Frame::debug_tree` so dumps map back to
    /// source. Captured via `#[track_caller]`, zero proc macros.
    pub(crate) source: &'static std::panic::Location<'static>,
    /// Forces children of a z-stack into the same grid cell.
    pub(crate) stack: bool,
    pub(crate) focusable: bool,
    pub(crate) autofocus: bool,
    /// Scroll containers only: keep pinned to the bottom while content
    /// grows (chat/log pattern).
    pub(crate) stick_bottom: bool,
    pub(crate) cursor: Option<Cursor>,
    pub(crate) disabled: bool,
    pub(crate) on_click: Option<Msg>,
    pub(crate) on_double_click: Option<Msg>,
    pub(crate) on_right_click: Option<Msg>,
    pub(crate) on_hover: Option<Msg>,
    pub(crate) on_key: Option<KeyFn<Msg>>,
    pub(crate) on_drag: Option<DragFn<Msg>>,
    /// Fired once on pointer release after a captured drag (the gesture
    /// that drove [`Self::on_drag`] ended). Only meaningful alongside
    /// `on_drag`; used for drag lifecycles like column-resize commit.
    pub(crate) on_drag_end: Option<Msg>,
    /// Fired when a press-drag-release on this element is recognized as a swipe
    /// (a fast flick past a small distance), with the dominant [`SwipeDir`].
    pub(crate) on_swipe: Option<SwipeFn<Msg>>,
    pub(crate) on_input: Option<InputFn<Msg>>,
    pub(crate) on_close: Option<Msg>,
    pub(crate) on_file_drop: Option<FileDropFn<Msg>>,
    /// Payload announced when a pointer drag starts on this element.
    pub(crate) drag_source: Option<String>,
    pub(crate) on_drop: Option<DropFn<Msg>>,
    pub(crate) overlay: Option<Overlay>,
    /// Continuous rotation period in ms (spinners); paint-time, clock-driven.
    pub(crate) spin: Option<f32>,
    /// Looping keyframe timeline sampled from the frame clock.
    pub(crate) keyframes: Option<crate::style::Keyframes>,
    /// Virtualized rows: materialized from scroll state at build time.
    pub(crate) virtual_rows: Option<VirtualData<Msg>>,
    /// Container query: a transparent wrapper that `build` replaces with the
    /// element its closure builds from this container's own measured size.
    pub(crate) responsive: Option<ResponsiveData<Msg>>,
    /// Accessible role and state (kit widgets set it; leaves auto-project).
    pub(crate) semantics: Option<Semantics>,
    /// Explicit accessible value (ARIA `valuetext`), overriding the input-derived
    /// one. Widgets like spinbutton/meter set it to a formatted value string.
    pub(crate) access_value: Option<String>,
    /// Announce content changes to assistive technology (polite).
    pub(crate) live: bool,
    /// Static text: users can drag-select and copy it.
    pub(crate) selectable: bool,
    /// Fade-in transition seeded when the id first appears.
    pub(crate) enter: Option<crate::style::Transition>,
    /// Exit animation: a paint-only ghost lingers and animates out when the
    /// id is removed from the tree (the counterpart of [`Self::enter`]).
    pub(crate) exit: Option<ExitAnim>,
    /// FLIP/shared-element layout animation: when the element's measured rect
    /// moves between frames it slides from the old position to the new.
    pub(crate) animate_layout: bool,
    /// Buffered type-ahead while focused (1s window per keystroke).
    pub(crate) on_type_ahead: Option<TypeAheadFn<Msg>>,
    /// Accessible name (screen-reader label).
    pub(crate) label: Option<String>,
    pub(crate) themed: Option<ThemedFn>,
    pub(crate) hover_style: Option<ThemedFn>,
    pub(crate) active_style: Option<ThemedFn>,
    pub(crate) focus_style: Option<ThemedFn>,
    /// Uniform Material state layer: resolves the content color veiled over the
    /// container on hover/focus/press/drag (replaces per-state color swaps).
    pub(crate) state_layer: Option<ContentFn>,
    /// Dip to [`crate::tokens::PRESS_SCALE`] while pressed.
    pub(crate) press_scale: bool,
    /// Recolor the focus ring (and swapped border) to the danger hue.
    pub(crate) invalid: bool,
    pub(crate) transition: Option<Transition>,
}

impl<Msg> Element<Msg> {
    #[track_caller]
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            style: Style::default(),
            children: Vec::new(),
            key: None,
            source: std::panic::Location::caller(),
            stack: false,
            focusable: false,
            autofocus: false,
            stick_bottom: false,
            cursor: None,
            disabled: false,
            on_click: None,
            on_double_click: None,
            on_right_click: None,
            on_hover: None,
            on_key: None,
            on_drag: None,
            on_drag_end: None,
            on_swipe: None,
            on_input: None,
            on_close: None,
            on_file_drop: None,
            drag_source: None,
            on_drop: None,
            overlay: None,
            spin: None,
            keyframes: None,
            virtual_rows: None,
            responsive: None,
            semantics: None,
            access_value: None,
            live: false,
            selectable: false,
            enter: None,
            exit: None,
            animate_layout: false,
            on_type_ahead: None,
            label: None,
            themed: None,
            hover_style: None,
            active_style: None,
            focus_style: None,
            state_layer: None,
            press_scale: false,
            invalid: false,
            transition: None,
        }
    }

    /// The element's style (read access for tests and tooling).
    pub fn style(&self) -> &Style {
        &self.style
    }

    /// Appends children. Anything convertible to an element works, so kit
    /// widget builders drop in next to `text()`/`div()` trees.
    pub fn children<M>(mut self, children: impl crate::IntoChildren<Msg, M>) -> Self {
        self.children.extend(children.into_children());
        self
    }

    /// Appends one child.
    pub fn child(mut self, child: impl Into<Element<Msg>>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Sets a user key for stable identity across reorders.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }

    /// Emits this message when the element is clicked.
    pub fn on_click(mut self, msg: Msg) -> Self {
        self.on_click = Some(msg);
        self.focusable = true;
        if self.cursor.is_none() {
            self.cursor = Some(Cursor::Pointer);
        }
        self
    }

    /// Style overlay applied while hovered.
    pub fn hover(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.hover_style = Some(Box::new(move |_, s| f(s)));
        self
    }

    /// Theme-aware hover overlay (used by kit widgets, which have no theme
    /// in scope at build time).
    pub fn hover_themed(mut self, f: impl Fn(&Theme, Style) -> Style + 'static) -> Self {
        self.hover_style = Some(Box::new(f));
        self
    }

    /// Style overlay applied while pressed.
    pub fn active(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.active_style = Some(Box::new(move |_, s| f(s)));
        self
    }

    /// Theme-aware active overlay.
    pub fn active_themed(mut self, f: impl Fn(&Theme, Style) -> Style + 'static) -> Self {
        self.active_style = Some(Box::new(f));
        self
    }

    /// Style overlay applied while focused.
    pub fn focus(mut self, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.focus_style = Some(Box::new(move |_, s| f(s)));
        self
    }

    /// Theme-aware focus overlay.
    pub fn focus_themed(mut self, f: impl Fn(&Theme, Style) -> Style + 'static) -> Self {
        self.focus_style = Some(Box::new(f));
        self
    }

    /// Drives the uniform Material state layer for this control. `content`
    /// resolves the color drawn *on* the control (its label/icon color); a
    /// translucent veil of it is laid over the container on hover, keyboard
    /// focus, press, and drag at the [`crate::tokens::STATE_LAYER`] opacities —
    /// one recipe instead of per-state color swaps. A disabled control fades
    /// its container toward the resting surface. The veil animates through the
    /// element's transition like any fill change.
    pub fn state_layer(mut self, content: impl Fn(&Theme) -> Color + 'static) -> Self {
        self.state_layer = Some(Box::new(content));
        self
    }

    /// Dips the control to [`crate::tokens::PRESS_SCALE`] while pressed
    /// (pointer down) — a tactile shrink that animates and never disturbs
    /// layout or hit-testing.
    pub fn press_scale(mut self) -> Self {
        self.press_scale = true;
        self
    }

    /// Marks the control invalid: its keyboard focus ring and swapped border
    /// recolor to the danger hue (shadcn's `aria-invalid` ring).
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Theme-deferred base styling, applied during style resolution. This is
    /// how kit widgets route every color through tokens without a theme in
    /// scope: `view()` has no theme parameter.
    pub fn themed(mut self, f: impl Fn(&Theme, Style) -> Style + 'static) -> Self {
        self.themed = Some(match self.themed.take() {
            Some(prev) => Box::new(move |t, s| f(t, prev(t, s))),
            None => Box::new(f),
        });
        self
    }

    /// Emits this message when the element is clicked twice within 400ms
    /// (the single-click message fires for both clicks too).
    pub fn on_double_click(mut self, msg: Msg) -> Self {
        self.on_double_click = Some(msg);
        self
    }

    /// Emits this message on a right-button press over the element (the
    /// context-menu gesture; fires on press, like macOS).
    pub fn on_right_click(mut self, msg: Msg) -> Self {
        self.on_right_click = Some(msg);
        self
    }

    /// Emits this message when the pointer enters the element.
    pub fn on_hover(mut self, msg: Msg) -> Self {
        self.on_hover = Some(msg);
        self
    }

    /// Maps key presses (while focused) to messages.
    pub fn on_key(mut self, f: impl Fn(&KeyInput) -> Option<Msg> + 'static) -> Self {
        self.on_key = Some(Box::new(f));
        self.focusable = true;
        self
    }

    /// Maps pointer presses and captured drags to messages. The callback
    /// receives the pointer position as fractions (0..=1) of the element
    /// rect on both axes.
    pub fn on_drag(mut self, f: impl Fn(f32, f32) -> Option<Msg> + 'static) -> Self {
        self.on_drag = Some(Box::new(f));
        self
    }

    /// Emits a message once when a captured drag (see [`Self::on_drag`])
    /// ends on pointer release. It fires only when this element actually
    /// captured the press as a drag, so a plain click on a different
    /// (click-only) element never triggers it. Pairs with `on_drag` to
    /// model gesture lifecycles — e.g. committing a column resize.
    pub fn on_drag_end(mut self, msg: Msg) -> Self {
        self.on_drag_end = Some(msg);
        self
    }

    /// Recognizes a swipe (flick) on this element: a press, a quick drag past a
    /// small threshold, and release fire the closure with the dominant
    /// [`SwipeDir`]. Good for carousels, dismissible cards, and back gestures —
    /// the element captures the press, so it works without `on_drag`.
    pub fn on_swipe(mut self, f: impl Fn(SwipeDir) -> Msg + 'static) -> Self {
        self.on_swipe = Some(Box::new(f));
        self
    }

    /// Maps each text edit of an input element to a message.
    pub fn on_input(mut self, f: impl Fn(&str) -> Msg + 'static) -> Self {
        self.on_input = Some(Box::new(f));
        self
    }

    /// Maps an OS file dropped onto this element to a message (delivered
    /// at the last pointer position; with no hit, the first handler in the
    /// tree receives it). The OS sends one event per dropped file.
    pub fn on_file_drop(mut self, f: impl Fn(&std::path::Path) -> Msg + 'static) -> Self {
        self.on_file_drop = Some(Box::new(f));
        self
    }

    /// Marks this element as an internal drag source carrying a string
    /// payload: pressing on it starts a drag, and releasing over an
    /// element with [`Self::on_drop`] delivers the payload there.
    pub fn drag_source(mut self, payload: impl Into<String>) -> Self {
        self.drag_source = Some(payload.into());
        self
    }

    /// Receives internal drag payloads released over this element; return
    /// `None` to reject. Style the drag with `.active(..)` on the source.
    pub fn on_drop(mut self, f: impl Fn(&str) -> Option<Msg> + 'static) -> Self {
        self.on_drop = Some(Box::new(f));
        self
    }

    /// Marks this element as an overlay child of its parent (the anchor).
    pub fn overlay(mut self, overlay: Overlay) -> Self {
        self.overlay = Some(overlay);
        self
    }

    /// Emitted when an app-driven overlay is dismissed (outside click, Esc).
    pub fn on_close(mut self, msg: Msg) -> Self {
        self.on_close = Some(msg);
        self
    }

    /// Rotates the element's painted content continuously (one turn per
    /// `period_ms`). Only path elements rotate; used by spinners.
    pub fn spin(mut self, period_ms: f32) -> Self {
        self.spin = Some(period_ms);
        self
    }

    /// Optically overshoots a path icon ([`crate::optical::CIRCLE_OVERSHOOT`]):
    /// scales the drawn path up about the viewbox center so a round or pointed
    /// glyph reads the same visual size as square-edged neighbors. No-op on
    /// non-path elements; off by default (existing icons are unchanged).
    pub fn optical_overshoot(mut self) -> Self {
        if let Kind::Path(p) = &mut self.kind {
            p.optical.overshoot = true;
        }
        self
    }

    /// Centers a path icon on its visual mass ([`crate::optical::centroid`])
    /// rather than its bounding box — the play-triangle correction. No-op on
    /// non-path elements; off by default.
    pub fn optical_center(mut self) -> Self {
        if let Kind::Path(p) = &mut self.kind {
            p.optical.center = true;
        }
        self
    }

    /// Attaches a looping [`Keyframes`](crate::style::Keyframes) timeline,
    /// sampled from the frame clock after `themed`, interaction variants,
    /// and transitions resolve. Reduced motion pins the first stop.
    pub fn keyframes(mut self, keyframes: crate::style::Keyframes) -> Self {
        self.keyframes = Some(keyframes);
        self
    }

    /// Virtualizes this container's rows: `builder(i)` is called only for
    /// the rows inside the scrolled-into-view window (plus overscan), with
    /// spacers standing in for the rest, so a 100k-row list builds a
    /// screenful of nodes per frame. Rows are forced to `row_height` and
    /// keyed by index. Pair with `.scroll_y()` and a stable `.id(..)` (the
    /// kit's `virtual_list` does both). Overlays inside virtual rows are
    /// not supported.
    pub fn virtual_rows(
        mut self,
        count: usize,
        row_height: f32,
        builder: impl Fn(usize) -> Element<Msg> + 'static,
    ) -> Self {
        self.virtual_rows = Some(VirtualData {
            count,
            row_height,
            variable: false,
            builder: std::rc::Rc::new(builder),
        });
        self
    }

    /// Like [`Self::virtual_rows`], but rows size themselves:
    /// `estimated_height` positions unmaterialized rows, and real
    /// heights are measured as rows appear (offsets self-correct over
    /// the next frame). Constraints: no overlays inside rows.
    pub fn virtual_rows_variable(
        mut self,
        count: usize,
        estimated_height: f32,
        builder: impl Fn(usize) -> Element<Msg> + 'static,
    ) -> Self {
        self.virtual_rows = Some(VirtualData {
            count,
            row_height: estimated_height,
            variable: true,
            builder: std::rc::Rc::new(builder),
        });
        self
    }

    /// Maps the focused element's accumulated type-ahead buffer to a
    /// message: printable keystrokes append (1s window between them,
    /// Escape clears), and the handler sees the whole buffer — "che"
    /// jumps a select to "Cherry" instead of cycling C-entries.
    pub fn on_type_ahead(mut self, f: impl Fn(&str) -> Option<Msg> + 'static) -> Self {
        self.on_type_ahead = Some(Box::new(f));
        self
    }

    /// Animates the element in when it first appears (a fade from
    /// opacity 0 through the given transition — give stateful entries a
    /// stable `.id` so reorders don't retrigger it). Pair with
    /// [`Self::exit`] to also animate removal.
    pub fn enter(mut self, transition: crate::style::Transition) -> Self {
        self.enter = Some(crate::style::Transition {
            opacity: true,
            ..transition
        });
        self
    }

    /// Animates the element *out* when it is removed from the tree: a
    /// paint-only snapshot ("ghost") is left in place and fades to transparent
    /// over `transition`, then dropped (the counterpart of [`Self::enter`]).
    /// Give stateful entries a stable [`Self::id`] so removal is detected by
    /// identity, not list position. Inert under reduced motion (removal is
    /// immediate). Use [`Self::exit_to`] to also scale or slide the ghost.
    pub fn exit(mut self, transition: crate::style::Transition) -> Self {
        self.exit = Some(ExitAnim {
            transition: crate::style::Transition {
                opacity: true,
                ..transition
            },
            opacity_to: 0.0,
            scale_to: 1.0,
            translate_to: (0.0, 0.0),
        });
        self
    }

    /// Like [`Self::exit`], but the leaving ghost animates toward an explicit
    /// `opacity`, `scale` (about its center), and `(dx, dy)` translation over
    /// the standard exit timing (a brisk accelerate-eased
    /// [`MotionDuration::exit_ms`](crate::tokens::MotionDuration::exit_ms)).
    /// For example `exit_to(0.0, 0.96, 0.0, 8.0)` fades a toast out while it
    /// shrinks slightly and drops away.
    pub fn exit_to(mut self, opacity: f32, scale: f32, dx: f32, dy: f32) -> Self {
        self.exit = Some(ExitAnim {
            transition: crate::style::Transition::all()
                .duration_ms(crate::tokens::MotionDuration::Base.exit_ms())
                .easing(crate::tokens::EASE_EXIT),
            opacity_to: opacity,
            scale_to: scale,
            translate_to: (dx, dy),
        });
        self
    }

    /// Animates this element's *position* when layout moves it between frames
    /// (FLIP / shared-element). Its measured rect is compared with the
    /// previous frame's; when the center moves, the element is painted
    /// starting at the old position and springs to the new one, so reordering
    /// a list (or a resized sibling pushing it) glides instead of jumping.
    /// Pair with a stable [`Self::id`] — without one, reordering changes the
    /// `WidgetId`, the previous position is lost under the new identity, and
    /// the slide never fires. Composes with any static or animated
    /// `translate`. Inert under reduced motion (the element snaps).
    ///
    /// With no explicit [`Self::transition`]/[`Self::enter`] the slide rides an
    /// implicit spatial spring; a declared transition wins and drives the slide
    /// at its own timing instead. Either way that transition is the element's
    /// general one, so an incidental style change animates through it too
    /// (colors clamp; only position springs here).
    pub fn animate_layout(mut self) -> Self {
        self.animate_layout = true;
        self
    }

    /// Makes static text selectable: drag (or double/triple-click)
    /// selects, Cmd/Ctrl+C copies. For text and rich-text elements.
    pub fn selectable(mut self) -> Self {
        self.selectable = true;
        self
    }

    /// Marks a live region: assistive technology announces content
    /// changes inside it without focus moving there (status lines,
    /// toasts — the kit's toast stack sets this itself).
    pub fn live(mut self) -> Self {
        self.live = true;
        self
    }

    /// Sets the accessible role and state projected into the accessibility
    /// tree. Text, image, and input leaves project their role automatically;
    /// use this to set one on a custom element, or to override the default.
    pub fn semantics(mut self, semantics: Semantics) -> Self {
        self.semantics = Some(semantics);
        self
    }

    /// Sets the accessible name announced by screen readers (text leaves
    /// use their content automatically).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the accessible value (ARIA `valuetext`) for a non-input control —
    /// e.g. a spinbutton's formatted "$5.00" or a meter's "75%". Text inputs
    /// derive their value from the edited text; this overrides it.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.access_value = Some(value.into());
        self
    }

    /// Disables interaction: handlers and variants stop applying.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Declares which properties animate between style states.
    pub fn transition(mut self, t: Transition) -> Self {
        self.transition = Some(t);
        self
    }

    /// Marks the element keyboard-focusable.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Focuses this element when it newly appears in the tree (opening a
    /// modal focuses its input). It does not steal focus while it stays
    /// mounted, and refocuses when it disappears and reappears. Give at
    /// most one element autofocus per view state.
    pub fn autofocus(mut self) -> Self {
        self.autofocus = true;
        self.focusable = true;
        self
    }

    /// Sets the hover cursor.
    pub fn cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    // ----- style delegation: every fluent `Style` method, on the element -----

    /// Padding on all edges.
    pub fn p(mut self, v: f32) -> Self {
        self.style = self.style.p(v);
        self
    }

    /// Horizontal padding.
    pub fn px(mut self, v: f32) -> Self {
        self.style = self.style.px(v);
        self
    }

    /// Vertical padding.
    pub fn py(mut self, v: f32) -> Self {
        self.style = self.style.py(v);
        self
    }

    /// Top padding.
    pub fn pt(mut self, v: f32) -> Self {
        self.style = self.style.pt(v);
        self
    }

    /// Right padding.
    pub fn pr(mut self, v: f32) -> Self {
        self.style = self.style.pr(v);
        self
    }

    /// Bottom padding.
    pub fn pb(mut self, v: f32) -> Self {
        self.style = self.style.pb(v);
        self
    }

    /// Left padding.
    pub fn pl(mut self, v: f32) -> Self {
        self.style = self.style.pl(v);
        self
    }

    /// Margin on all edges.
    pub fn m(mut self, v: f32) -> Self {
        self.style = self.style.m(v);
        self
    }

    /// Horizontal margin.
    pub fn mx(mut self, v: f32) -> Self {
        self.style = self.style.mx(v);
        self
    }

    /// Vertical margin.
    pub fn my(mut self, v: f32) -> Self {
        self.style = self.style.my(v);
        self
    }

    /// Top margin.
    pub fn mt(mut self, v: f32) -> Self {
        self.style = self.style.mt(v);
        self
    }

    /// Right margin.
    pub fn mr(mut self, v: f32) -> Self {
        self.style = self.style.mr(v);
        self
    }

    /// Bottom margin.
    pub fn mb(mut self, v: f32) -> Self {
        self.style = self.style.mb(v);
        self
    }

    /// Left margin.
    pub fn ml(mut self, v: f32) -> Self {
        self.style = self.style.ml(v);
        self
    }

    /// Gap between children.
    pub fn gap(mut self, v: f32) -> Self {
        self.style = self.style.gap(v);
        self
    }

    /// Preferred width (raw `f32` = logical px).
    pub fn w(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.w(v);
        self
    }

    /// Preferred height.
    pub fn h(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.h(v);
        self
    }

    /// Minimum width.
    pub fn min_w(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.min_w(v);
        self
    }

    /// Maximum width.
    pub fn max_w(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.max_w(v);
        self
    }

    /// Minimum height.
    pub fn min_h(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.min_h(v);
        self
    }

    /// Maximum height.
    pub fn max_h(mut self, v: impl Into<Length>) -> Self {
        self.style = self.style.max_h(v);
        self
    }

    /// Caps this element's width at a reading measure of `chars` characters
    /// (a `ch`-based `max-width`); see [`Style::measure`](crate::Style::measure).
    pub fn measure(mut self, chars: f32) -> Self {
        self.style = self.style.measure(chars);
        self
    }

    /// Preferred width in `ch` units (see [`Length::Ch`](crate::Length::Ch)).
    pub fn w_ch(mut self, chars: f32) -> Self {
        self.style = self.style.w_ch(chars);
        self
    }

    /// Minimum width in `ch` units.
    pub fn min_w_ch(mut self, chars: f32) -> Self {
        self.style = self.style.min_w_ch(chars);
        self
    }

    /// Maximum width in `ch` units (alias of [`Element::measure`]).
    pub fn max_w_ch(mut self, chars: f32) -> Self {
        self.style = self.style.max_w_ch(chars);
        self
    }

    /// Width 100%.
    pub fn w_full(mut self) -> Self {
        self.style = self.style.w_full();
        self
    }

    /// Height 100%.
    pub fn h_full(mut self) -> Self {
        self.style = self.style.h_full();
        self
    }

    /// Flex grow 1.
    pub fn grow(mut self) -> Self {
        self.style = self.style.grow();
        self
    }

    /// Flex shrink 0.
    pub fn shrink0(mut self) -> Self {
        self.style = self.style.shrink0();
        self
    }

    /// Align children to the cross-axis start.
    pub fn items_start(mut self) -> Self {
        self.style = self.style.items_start();
        self
    }

    /// Center children on the cross axis.
    pub fn items_center(mut self) -> Self {
        self.style = self.style.items_center();
        self
    }

    /// Align children to the cross-axis end.
    pub fn items_end(mut self) -> Self {
        self.style = self.style.items_end();
        self
    }

    /// Align children on their first text baseline (rows only).
    pub fn items_baseline(mut self) -> Self {
        self.style = self.style.items_baseline();
        self
    }

    /// Override the parent's cross-axis alignment for this element alone,
    /// hugging its content at the cross-axis start instead of stretching.
    pub fn self_start(mut self) -> Self {
        self.style = self.style.self_start();
        self
    }

    /// Override the parent's cross-axis alignment for this element alone,
    /// centering it on the cross axis.
    pub fn self_center(mut self) -> Self {
        self.style = self.style.self_center();
        self
    }

    /// Override the parent's cross-axis alignment for this element alone,
    /// packing it toward the cross-axis end.
    pub fn self_end(mut self) -> Self {
        self.style = self.style.self_end();
        self
    }

    /// Override the parent's cross-axis alignment for this element alone,
    /// stretching it to fill the cross axis.
    pub fn self_stretch(mut self) -> Self {
        self.style = self.style.self_stretch();
        self
    }

    /// Pack children toward the main-axis start.
    pub fn justify_start(mut self) -> Self {
        self.style = self.style.justify_start();
        self
    }

    /// Center children on the main axis.
    pub fn justify_center(mut self) -> Self {
        self.style = self.style.justify_center();
        self
    }

    /// Pack children toward the main-axis end.
    pub fn justify_end(mut self) -> Self {
        self.style = self.style.justify_end();
        self
    }

    /// Distribute children with space between.
    pub fn justify_between(mut self) -> Self {
        self.style = self.style.justify_between();
        self
    }

    /// Allow flex children to wrap.
    pub fn wrap(mut self) -> Self {
        self.style = self.style.wrap();
        self
    }

    /// Position absolutely against the nearest relative ancestor.
    pub fn absolute(mut self) -> Self {
        self.style = self.style.absolute();
        self
    }

    /// Offset from the top.
    pub fn top(mut self, v: f32) -> Self {
        self.style = self.style.top(v);
        self
    }

    /// Offset from the right.
    pub fn right(mut self, v: f32) -> Self {
        self.style = self.style.right(v);
        self
    }

    /// Offset from the bottom.
    pub fn bottom(mut self, v: f32) -> Self {
        self.style = self.style.bottom(v);
        self
    }

    /// Offset from the left.
    pub fn left(mut self, v: f32) -> Self {
        self.style = self.style.left(v);
        self
    }

    /// Clip children to the bounds.
    pub fn overflow_hidden(mut self) -> Self {
        self.style = self.style.overflow_hidden();
        self
    }

    /// Vertical scrolling with clipped content.
    pub fn scroll_y(mut self) -> Self {
        self.style = self.style.scroll_y();
        self
    }

    /// Horizontal scrolling with clipped content.
    pub fn scroll_x(mut self) -> Self {
        self.style = self.style.scroll_x();
        self
    }

    /// Scrolling on both axes with clipped content.
    pub fn scroll_xy(mut self) -> Self {
        self.style = self.style.scroll_xy();
        self
    }

    /// Sticks `offset` px below the scroll viewport's top edge (`position: sticky`).
    pub fn sticky_top(mut self, offset: f32) -> Self {
        self.style = self.style.sticky_top(offset);
        self
    }

    /// Sticks `offset` px above the scroll viewport's bottom edge.
    pub fn sticky_bottom(mut self, offset: f32) -> Self {
        self.style = self.style.sticky_bottom(offset);
        self
    }

    /// Sticks `offset` px right of the scroll viewport's left edge.
    pub fn sticky_left(mut self, offset: f32) -> Self {
        self.style = self.style.sticky_left(offset);
        self
    }

    /// Sticks `offset` px left of the scroll viewport's right edge.
    pub fn sticky_right(mut self, offset: f32) -> Self {
        self.style = self.style.sticky_right(offset);
        self
    }

    /// Keeps a scroll container pinned to its bottom edge while content
    /// grows — until the user scrolls away, and again once they return to
    /// the bottom (the chat/log pattern). Starts at the bottom.
    pub fn stick_to_bottom(mut self) -> Self {
        self.stick_bottom = true;
        self
    }

    /// Background fill.
    pub fn bg(mut self, paint: impl Into<Paint>) -> Self {
        self.style = self.style.bg(paint);
        self
    }

    /// Uniform border (a stroke on the element's edge).
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.style = self.style.border(width, color);
        self
    }

    /// A border stroke on just the top edge — a straight hairline (square
    /// corners) for ruled layouts, no manual divider child needed. See
    /// [`Style::border_top`](crate::Style::border_top).
    pub fn border_top(mut self, width: f32, color: Color) -> Self {
        self.style = self.style.border_top(width, color);
        self
    }

    /// A border stroke on just the right edge. See
    /// [`Style::border_right`](crate::Style::border_right).
    pub fn border_right(mut self, width: f32, color: Color) -> Self {
        self.style = self.style.border_right(width, color);
        self
    }

    /// A border stroke on just the bottom edge — a header/row rule. See
    /// [`Style::border_bottom`](crate::Style::border_bottom).
    pub fn border_bottom(mut self, width: f32, color: Color) -> Self {
        self.style = self.style.border_bottom(width, color);
        self
    }

    /// A border stroke on just the left edge — an accent rail. See
    /// [`Style::border_left`](crate::Style::border_left).
    pub fn border_left(mut self, width: f32, color: Color) -> Self {
        self.style = self.style.border_left(width, color);
        self
    }

    /// A crisp `width`-px ring just outside the box, hugging the corner radius
    /// (see [`Style::ring`](crate::Style::ring)) — the "ring, not border" look:
    /// outside the element, zero layout cost, ideal for selection rings and
    /// sub-pixel hairlines.
    pub fn ring(mut self, width: f32, color: Color) -> Self {
        self.style = self.style.ring(width, color);
        self
    }

    /// The same corner radius on all corners.
    pub fn rounded(mut self, r: f32) -> Self {
        self.style = self.style.rounded(r);
        self
    }

    /// Fully-rounded corners.
    pub fn rounded_full(mut self) -> Self {
        self.style = self.style.rounded_full();
        self
    }

    /// Rounds the top two corners only.
    pub fn rounded_t(mut self, r: f32) -> Self {
        self.style = self.style.rounded_t(r);
        self
    }

    /// Rounds the bottom two corners only.
    pub fn rounded_b(mut self, r: f32) -> Self {
        self.style = self.style.rounded_b(r);
        self
    }

    /// Rounds the left two corners only.
    pub fn rounded_l(mut self, r: f32) -> Self {
        self.style = self.style.rounded_l(r);
        self
    }

    /// Rounds the right two corners only.
    pub fn rounded_r(mut self, r: f32) -> Self {
        self.style = self.style.rounded_r(r);
        self
    }

    /// Sets each corner radius independently (top-left, top-right,
    /// bottom-right, bottom-left).
    pub fn corners(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        self.style = self.style.corners(tl, tr, br, bl);
        self
    }

    /// Paint-time translation in logical px (never affects layout). Animatable.
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.style = self.style.translate(x, y);
        self
    }

    /// Paint-time rotation in degrees about the element center. Animatable.
    pub fn rotate(mut self, degrees: f32) -> Self {
        self.style = self.style.rotate(degrees);
        self
    }

    /// Paint-time skew in degrees `(x, y)` about the element center. Animatable.
    pub fn skew(mut self, x_degrees: f32, y_degrees: f32) -> Self {
        self.style = self.style.skew(x_degrees, y_degrees);
        self
    }

    /// Non-uniform paint-time scale `(x, y)`, composed with the uniform
    /// press scale. Never disturbs layout; animatable.
    pub fn scale_xy(mut self, x: f32, y: f32) -> Self {
        self.style = self.style.scale_xy(x, y);
        self
    }

    /// The pivot for this element's paint-time transforms, as a fraction of
    /// its rect (CSS `transform-origin`): `(0, 0)` top-left, `(0.5, 0.5)`
    /// center (the default).
    pub fn transform_origin(mut self, fx: f32, fy: f32) -> Self {
        self.style = self.style.transform_origin(fx, fy);
        self
    }

    /// Continuous-curvature corner smoothing, `0.0..=1.0` (see
    /// [`Style::corner_smoothing`](crate::Style::corner_smoothing)). `0.0`
    /// (default) keeps exact circular arcs; higher values blend toward a
    /// fuller squircle.
    pub fn corner_smoothing(mut self, s: f32) -> Self {
        self.style = self.style.corner_smoothing(s);
        self
    }

    /// A themed shadow elevation token.
    pub fn shadow(mut self, token: ShadowToken) -> Self {
        self.style = self.style.shadow(token);
        self
    }

    /// Applies a [`Surface`](crate::Surface) material — the kit ergonomic. The
    /// fill, border, radius, shadow, and highlight resolve at theme-resolution
    /// time (so it needs no theme in `view()`), replacing the
    /// `.rounded(..).shadow(..).themed(|t, s| s.bg(..).border(..))` combo. Call
    /// it once as the element's material; chain a `.themed` after it to tweak a
    /// single property.
    pub fn surface(self, role: crate::surface::Surface) -> Self {
        self.themed(move |t, s| {
            let mut s = role.bundle().apply(t, s).rounded(role.radius_px(&t.radius));
            // Flat elevation drops the shadow on resting cards (border + tone
            // carry separation); floating roles always keep theirs.
            if t.elevation == crate::Elevation::Flat
                && matches!(
                    role,
                    crate::surface::Surface::Card | crate::surface::Surface::Raised
                )
            {
                s.shadow_token = None;
            }
            s
        })
    }

    /// Subtree opacity.
    pub fn opacity(mut self, v: f32) -> Self {
        self.style = self.style.opacity(v);
        self
    }

    /// Draw progress for path elements (0 = nothing, 1 = full path).
    pub fn trim(mut self, v: f32) -> Self {
        self.style = self.style.trim(v);
        self
    }

    /// Frosted-glass backdrop blur in logical px: blurs the content *behind*
    /// this translucent element (see [`Style::backdrop_blur`](crate::Style::backdrop_blur)).
    /// [`Surface::Glass`](crate::Surface::Glass) sets this for you; reach for
    /// this builder to tune the radius or frost a custom translucent pane.
    pub fn backdrop_blur(mut self, radius: f32) -> Self {
        self.style = self.style.backdrop_blur(radius);
        self
    }

    /// A foreground [`ElementFilter`](crate::ElementFilter) on this element's
    /// own content (blur / brightness / saturate).
    pub fn element_filter(mut self, filter: crate::ElementFilter) -> Self {
        self.style = self.style.element_filter(filter);
        self
    }

    /// A luminous specular edge rim — the Liquid Glass perimeter light — on this
    /// element. [`Surface::Glass`](crate::Surface::Glass) sets it for you; reach
    /// for this builder to put the rim on a custom translucent pane. See
    /// [`SpecularEdge`](crate::SpecularEdge) (and [`SpecularEdge::glass`]).
    pub fn specular_edge(mut self, edge: crate::style::SpecularEdge) -> Self {
        self.style = self.style.specular_edge(edge);
        self
    }

    /// A directional body sheen — the raking glass light — across this element's
    /// face. See [`Sheen`](crate::Sheen) (and [`Sheen::glass`]).
    pub fn sheen(mut self, sheen: crate::style::Sheen) -> Self {
        self.style = self.style.sheen(sheen);
        self
    }

    /// Backdrop-adaptive vibrancy — shift the glass tint's lightness by the mean
    /// luminance of the frosted backdrop behind it (headless-only). See
    /// [`AdaptiveTint`](crate::AdaptiveTint) (and [`AdaptiveTint::glass`]).
    pub fn adaptive_tint(mut self, adaptive: crate::style::AdaptiveTint) -> Self {
        self.style = self.style.adaptive_tint(adaptive);
        self
    }

    /// Text size.
    pub fn size(mut self, size: TextSize) -> Self {
        self.style = self.style.size(size);
        self
    }

    /// Free-form text size in logical px (editorial display sizes).
    pub fn size_px(mut self, px: f32) -> Self {
        self.style = self.style.size_px(px);
        self
    }

    /// Letter spacing in em (tracked-out eyebrows, small caps).
    pub fn tracking(mut self, em: f32) -> Self {
        self.style = self.style.tracking(em);
        self
    }

    /// Line height as a multiple of the font size.
    pub fn leading(mut self, multiple: f32) -> Self {
        self.style = self.style.leading(multiple);
        self
    }

    /// Tabular (fixed-width) numerals (`tnum`) — digits align in columns. For
    /// tables, timers, charts, and numeric data.
    pub fn tabular(mut self) -> Self {
        self.style = self.style.tabular();
        self
    }

    /// Proportional numerals — individually spaced for prose (`pnum`).
    pub fn proportional_nums(mut self) -> Self {
        self.style = self.style.proportional_nums();
        self
    }

    /// Old-style / text figures (`onum`): ascending and descending digits
    /// that sit naturally in serif prose.
    pub fn oldstyle_nums(mut self) -> Self {
        self.style = self.style.oldstyle_nums();
        self
    }

    /// Lining figures (`lnum`): uniform cap-height digits for data and UI.
    pub fn lining_nums(mut self) -> Self {
        self.style = self.style.lining_nums();
        self
    }

    /// Render lowercase letters as small capitals (`smcp`).
    pub fn small_caps(mut self) -> Self {
        self.style = self.style.small_caps();
        self
    }

    /// Enable or disable standard ligatures (`liga`); most fonts default on.
    pub fn ligatures(mut self, on: bool) -> Self {
        self.style = self.style.ligatures(on);
        self
    }

    /// Common fractions (`frac`): `1/2` becomes a single fraction glyph.
    pub fn fractions(mut self) -> Self {
        self.style = self.style.fractions();
        self
    }

    /// Font family role (Sans, Mono, or a registered Display/Serif face).
    pub fn family(mut self, family: crate::tokens::FamilyRole) -> Self {
        self.style = self.style.family(family);
        self
    }

    /// Font weight.
    pub fn weight(mut self, weight: Weight) -> Self {
        self.style = self.style.weight(weight);
        self
    }

    /// Text color.
    pub fn color(mut self, color: Color) -> Self {
        self.style = self.style.color(color);
        self
    }

    /// Mono family role.
    pub fn mono(mut self) -> Self {
        self.style = self.style.mono();
        self
    }

    /// Truncate to one line with an ellipsis.
    pub fn truncate(mut self) -> Self {
        self.style = self.style.truncate();
        self
    }

    /// Horizontal text alignment.
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.style = self.style.text_align(align);
        self
    }

    /// Balance line lengths ([`TextWrap`](crate::TextWrap)`::Balance`) — for
    /// headings and titles.
    pub fn balance(mut self) -> Self {
        self.style = self.style.balance();
        self
    }

    /// Avoid a stranded last word ([`TextWrap`](crate::TextWrap)`::Pretty`) —
    /// best-effort for paragraphs.
    pub fn pretty(mut self) -> Self {
        self.style = self.style.pretty();
        self
    }

    /// Sets the line-breaking mode explicitly ([`TextWrap`](crate::TextWrap)).
    pub fn text_wrap(mut self, wrap: TextWrap) -> Self {
        self.style = self.style.text_wrap(wrap);
        self
    }

    /// Sets optical sizing explicitly
    /// ([`OpticalSizing`](crate::OpticalSizing)) — how a variable font's
    /// `opsz` axis is driven.
    pub fn optical(mut self, optical: crate::style::OpticalSizing) -> Self {
        self.style = self.style.optical(optical);
        self
    }

    /// Tracks the `opsz` axis to the rendered size
    /// ([`OpticalSizing::Auto`](crate::OpticalSizing::Auto)) — small text gets
    /// the text-optical master, large sizes the display master. A no-op on
    /// static faces (the embedded Inter / mono).
    pub fn optical_auto(mut self) -> Self {
        self.style = self.style.optical_auto();
        self
    }
}

/// A plain container box (flex row by default, like taffy).
#[track_caller]
pub fn div<Msg>() -> Element<Msg> {
    Element::new(Kind::Box)
}

/// A flex row.
#[track_caller]
pub fn row<Msg>() -> Element<Msg> {
    Element::new(Kind::Box)
}

/// A flex column.
#[track_caller]
pub fn col<Msg>() -> Element<Msg> {
    let mut el = Element::new(Kind::Box);
    el.style.direction = crate::style::Direction::Column;
    el
}

/// A z-stack: children occupy the same rect and paint in order.
#[track_caller]
pub fn stack<Msg>() -> Element<Msg> {
    let mut el = Element::new(Kind::Box);
    el.style.display = crate::style::Display::Grid;
    el.stack = true;
    el
}

/// A paragraph of styled runs: spans wrap together as one layout, and
/// each [`span`] may override weight, color, size, family, or italic.
#[track_caller]
pub fn rich_text<Msg>(spans: impl IntoIterator<Item = Span>) -> Element<Msg> {
    Element::new(Kind::Rich(spans.into_iter().collect()))
}

/// A text run.
#[track_caller]
pub fn text<Msg>(content: impl Into<String>) -> Element<Msg> {
    Element::new(Kind::Text(content.into()))
}

/// Flexible empty space (flex grow 1).
#[track_caller]
pub fn spacer<Msg>() -> Element<Msg> {
    Element::new(Kind::Box).grow()
}

/// A horizontal hairline rule in `border_subtle`, 1px tall and full width.
/// For a vertical rule, override with `.w(1.0)` and `.h_full()`.
#[track_caller]
pub fn divider<Msg>() -> Element<Msg> {
    Element::new(Kind::Divider).w_full().h(1.0).shrink0()
}

/// A container that chooses its own layout from its measured size — a
/// **container query**, the counterpart of window-size
/// [`App::view_at`](crate::App::view_at). `f(available)` receives this
/// container's own content size in logical px *from the previous frame* (the
/// layout the motion system already records) and returns the element to lay
/// out in its place.
///
/// It is **one frame deferred**: the very first frame has no measured size, so
/// it builds at the hint `(0.0, 0.0)` — the "smallest" branch — and converges
/// on the next frame; a later resize re-converges one frame after it lands.
/// Reach for [`responsive_hinted`] to seed a first-frame size and skip that
/// flash.
///
/// The returned element is **transparent**: it replaces the `responsive(..)`
/// wrapper entirely, so style the element you return, not this call. Two rules
/// keep the feedback loop well-behaved:
/// - **Stable identity.** Give the wrapper a stable [`Element::id`] if its
///   position among its siblings can change, so the query keys off a stable
///   [`WidgetId`](crate::WidgetId) across frames (a fixed tree position is
///   already stable).
/// - **Monotone in width, and never self-wrapping.** `f` should not return
///   content that shrinks the container back below a threshold a wider size
///   crossed (otherwise it can flip every frame at a boundary — prefer
///   width-driven breakpoints on a parent-sized container). Nest a finer
///   `responsive(..)` as a *child* of the returned element, never as its root:
///   a closure that directly returns another `responsive(..)` chains under the
///   same id, and is expanded only a bounded number of times before being
///   flattened to empty (it degrades, it never overflows the stack).
///
/// Like a virtualized list, the generated subtree is not part of the *declared*
/// tree, so an [`overlay`](Element::overlay) inside it is silently skipped (the
/// overlay machinery indexes the declared tree). Mount overlays outside the
/// `responsive` wrapper.
#[track_caller]
pub fn responsive<Msg>(f: impl Fn((f32, f32)) -> Element<Msg> + 'static) -> Element<Msg> {
    responsive_hinted((0.0, 0.0), f)
}

/// [`responsive`] with an explicit first-frame size: `f(hint)` builds the
/// initial frame (before any measurement exists), removing the one-frame
/// "smallest branch" flash when the rough size is known up front. See
/// [`responsive`] for the convergence model and the identity / monotonicity
/// caveats.
#[track_caller]
pub fn responsive_hinted<Msg>(
    hint: (f32, f32),
    f: impl Fn((f32, f32)) -> Element<Msg> + 'static,
) -> Element<Msg> {
    let mut el = Element::new(Kind::Box);
    el.responsive = Some(ResponsiveData {
        hint,
        f: std::rc::Rc::new(f),
    });
    el
}

/// A bare single-line text input leaf. Most apps want the styled
/// `fenestra_kit` `text_input` instead; this is the primitive it wraps.
/// Focusable, shows the text I-beam, and emits `on_input` per edit.
#[track_caller]
pub fn raw_input<Msg>(value: impl Into<String>, placeholder: impl Into<String>) -> Element<Msg> {
    Element::new(Kind::Input(InputData {
        value: value.into(),
        placeholder: placeholder.into(),
        multiline: false,
    }))
    .focusable(true)
    .cursor(Cursor::Text)
}

/// A bare multiline text area leaf: text wraps to the element width, Enter
/// inserts a newline, arrows move by line, and the measured height grows
/// with the wrapped content (constrain it with `.min_h`/`.max_h` plus an
/// outer scroll container). Most apps want the styled `fenestra_kit`
/// `text_area` instead; this is the primitive it wraps.
#[track_caller]
pub fn raw_text_area<Msg>(
    value: impl Into<String>,
    placeholder: impl Into<String>,
) -> Element<Msg> {
    Element::new(Kind::Input(InputData {
        value: value.into(),
        placeholder: placeholder.into(),
        multiline: true,
    }))
    .focusable(true)
    .cursor(Cursor::Text)
}

/// An image leaf showing straight-alpha RGBA8 pixels (row-major, 4 bytes
/// per pixel). Sized to the image by default, stretched when styled
/// otherwise, and painted clipped to the corner radius — so
/// `.rounded_full()` crops a square source into a round avatar. If `pixels`
/// holds fewer than `width * height` complete rows, the element shrinks to
/// the rows actually provided instead of panicking.
#[track_caller]
pub fn image_rgba8<Msg>(width: u32, height: u32, mut pixels: Vec<u8>) -> Element<Msg> {
    let row = width as usize * 4;
    let rows = pixels
        .len()
        .checked_div(row)
        .unwrap_or(0)
        .min(height as usize);
    pixels.truncate(row * rows);
    #[expect(clippy::cast_possible_truncation, reason = "rows <= height: u32")]
    let height = rows as u32;
    let image = peniko::ImageData {
        data: pixels.into(),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width,
        height,
    };
    #[expect(clippy::cast_precision_loss, reason = "image sizes fit in f32")]
    Element::new(Kind::Image(ImageData { image }))
        .w(width as f32)
        .h(height as f32)
        .shrink0()
}

/// A vector path drawn in `viewbox` coordinates and scaled to the element
/// rect (sized to the viewbox by default). `stroke` is a width in viewbox
/// units; `None` fills instead. Painted in the resolved text color.
#[track_caller]
pub fn path<Msg>(bez: kurbo::BezPath, viewbox: (f64, f64), stroke: Option<f64>) -> Element<Msg> {
    #[expect(clippy::cast_possible_truncation, reason = "viewbox sizes are small")]
    Element::new(Kind::Path(PathData {
        path: std::sync::Arc::new(bez),
        viewbox,
        stroke,
        optical: OpticalCorrection::default(),
    }))
    .w(viewbox.0 as f32)
    .h(viewbox.1 as f32)
    .shrink0()
}

impl<Msg: 'static> Element<Msg> {
    /// Converts every message this subtree can emit with `f`, so a
    /// component written around its own message type drops into any parent:
    /// the Elm composition tool.
    ///
    /// ```
    /// use fenestra_core::{Element, div, text};
    ///
    /// #[derive(Clone)]
    /// enum CardMsg { Open }
    /// #[derive(Clone)]
    /// enum AppMsg { Card(usize, CardMsg) }
    ///
    /// fn card() -> Element<CardMsg> {
    ///     div().on_click(CardMsg::Open).child(text("open"))
    /// }
    ///
    /// let el: Element<AppMsg> = card().map(|m| AppMsg::Card(0, m));
    /// ```
    pub fn map<B: 'static>(self, f: impl Fn(Msg) -> B + Clone + 'static) -> Element<B> {
        Element {
            kind: self.kind,
            style: self.style,
            children: self
                .children
                .into_iter()
                .map(|c| c.map(f.clone()))
                .collect(),
            key: self.key,
            source: self.source,
            stack: self.stack,
            focusable: self.focusable,
            autofocus: self.autofocus,
            stick_bottom: self.stick_bottom,
            cursor: self.cursor,
            disabled: self.disabled,
            on_click: self.on_click.map(&f),
            on_double_click: self.on_double_click.map(&f),
            on_right_click: self.on_right_click.map(&f),
            on_hover: self.on_hover.map(&f),
            on_key: self.on_key.map(|k| {
                let f = f.clone();
                Box::new(move |key: &KeyInput| k(key).map(&f)) as KeyFn<B>
            }),
            on_type_ahead: self.on_type_ahead.map(|k| {
                let f = f.clone();
                Box::new(move |buffer: &str| k(buffer).map(&f)) as TypeAheadFn<B>
            }),
            on_drag: self.on_drag.map(|d| {
                let f = f.clone();
                Box::new(move |x: f32, y: f32| d(x, y).map(&f)) as DragFn<B>
            }),
            on_drag_end: self.on_drag_end.map(&f),
            on_swipe: self.on_swipe.map(|s| {
                let f = f.clone();
                Box::new(move |d: SwipeDir| f(s(d))) as SwipeFn<B>
            }),
            on_input: self.on_input.map(|i| {
                let f = f.clone();
                Box::new(move |s: &str| f(i(s))) as InputFn<B>
            }),
            on_close: self.on_close.map(&f),
            on_file_drop: self.on_file_drop.map(|d| {
                let f = f.clone();
                Box::new(move |p: &std::path::Path| f(d(p))) as FileDropFn<B>
            }),
            drag_source: self.drag_source,
            on_drop: self.on_drop.map(|d| {
                let f = f.clone();
                Box::new(move |payload: &str| d(payload).map(&f)) as DropFn<B>
            }),
            overlay: self.overlay,
            spin: self.spin,
            keyframes: self.keyframes,
            virtual_rows: self.virtual_rows.map(|v| {
                let f = f.clone();
                let builder = v.builder;
                VirtualData {
                    count: v.count,
                    row_height: v.row_height,
                    variable: v.variable,
                    builder: std::rc::Rc::new(move |i| builder(i).map(f.clone())),
                }
            }),
            responsive: self.responsive.map(|r| {
                let f = f.clone();
                let inner = r.f;
                ResponsiveData {
                    hint: r.hint,
                    f: std::rc::Rc::new(move |sz| inner(sz).map(f.clone())),
                }
            }),
            semantics: self.semantics,
            access_value: self.access_value,
            live: self.live,
            selectable: self.selectable,
            enter: self.enter,
            exit: self.exit,
            animate_layout: self.animate_layout,
            label: self.label,
            themed: self.themed,
            hover_style: self.hover_style,
            active_style: self.active_style,
            focus_style: self.focus_style,
            state_layer: self.state_layer,
            press_scale: self.press_scale,
            invalid: self.invalid,
            transition: self.transition,
        }
    }
}

impl<Msg> Element<Msg> {
    /// Grid template columns (switches display to grid). Accepts plain
    /// [`Track`](crate::style::Track)s or full
    /// [`GridTemplate`](crate::style::GridTemplate) entries (e.g. `repeat(...)`).
    pub fn grid_cols<T: Into<crate::style::GridTemplate>>(
        mut self,
        tracks: impl IntoIterator<Item = T>,
    ) -> Self {
        self.style = self.style.grid_cols(tracks);
        self
    }

    /// Grid template rows (switches display to grid). Accepts plain
    /// [`Track`](crate::style::Track)s or full
    /// [`GridTemplate`](crate::style::GridTemplate) entries (e.g. `repeat(...)`).
    pub fn grid_rows<T: Into<crate::style::GridTemplate>>(
        mut self,
        tracks: impl IntoIterator<Item = T>,
    ) -> Self {
        self.style = self.style.grid_rows(tracks);
        self
    }

    /// Places this element at a 1-based grid column, spanning `span` tracks.
    pub fn grid_col(mut self, start: i16, span: u16) -> Self {
        self.style = self.style.grid_col(start, span);
        self
    }

    /// Places this element at a 1-based grid row, spanning `span` tracks.
    pub fn grid_row(mut self, start: i16, span: u16) -> Self {
        self.style = self.style.grid_row(start, span);
        self
    }

    /// `grid-template-areas` (CSS): each row is a string of whitespace-separated
    /// area names, `.` for an empty cell. Place children with
    /// [`Element::grid_area`]. Implies a grid of `auto` tracks matching the area
    /// shape when no explicit tracks are given.
    pub fn grid_template_areas<R: AsRef<str>>(mut self, rows: impl IntoIterator<Item = R>) -> Self {
        self.style = self.style.grid_template_areas(rows);
        self
    }

    /// Places this element in a named grid area (CSS `grid-area`).
    pub fn grid_area(mut self, name: impl Into<String>) -> Self {
        self.style = self.style.grid_area(name);
        self
    }

    /// Places this element's columns between two named grid lines
    /// (CSS `grid-column: start / end`).
    pub fn grid_col_lines(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
        self.style = self.style.grid_col_lines(start, end);
        self
    }

    /// Places this element's rows between two named grid lines
    /// (CSS `grid-row: start / end`).
    pub fn grid_row_lines(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
        self.style = self.style.grid_row_lines(start, end);
        self
    }

    /// Names the column grid lines positionally: the i-th name labels the
    /// (i+1)-th line. Reference them from [`Element::grid_col_lines`].
    pub fn grid_col_names<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.style = self.style.grid_col_names(names);
        self
    }

    /// Names the row grid lines positionally: the i-th name labels the (i+1)-th
    /// line. Reference them from [`Element::grid_row_lines`].
    pub fn grid_row_names<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.style = self.style.grid_row_names(names);
        self
    }
}

#[cfg(test)]
mod surface_tests {
    use super::div;
    use crate::surface::Surface;

    #[test]
    fn element_surface_defers_to_resolution() {
        // The fill is installed by the deferred `themed`, not the base style,
        // so `.surface(..)` needs no theme at build time.
        let el = div::<()>().surface(Surface::Card);
        assert!(el.style().fill.is_none());
    }
}

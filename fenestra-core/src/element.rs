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
}

/// Accessible role and state of an element, projected into the platform
/// accessibility tree by the shell (AccessKit) and exposed headlessly via
/// `Frame::access_tree`. Text, image, and input leaves project
/// automatically; kit widgets set the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Semantics {
    /// An activatable button.
    Button,
    /// A two-state checkbox.
    Checkbox {
        /// Whether it is checked.
        checked: bool,
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
    /// Accessible role and state (kit widgets set it; leaves auto-project).
    pub(crate) semantics: Option<Semantics>,
    /// Announce content changes to assistive technology (polite).
    pub(crate) live: bool,
    /// Static text: users can drag-select and copy it.
    pub(crate) selectable: bool,
    /// Fade-in transition seeded when the id first appears.
    pub(crate) enter: Option<crate::style::Transition>,
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
            on_input: None,
            on_close: None,
            on_file_drop: None,
            drag_source: None,
            on_drop: None,
            overlay: None,
            spin: None,
            keyframes: None,
            virtual_rows: None,
            semantics: None,
            live: false,
            selectable: false,
            enter: None,
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

    /// Sets the accessible role and state projected into the accessibility
    /// tree. Text, image, and input leaves project automatically.
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
    /// stable `.id` so reorders don't retrigger it). Exit animations
    /// are not supported yet: removal is immediate.
    pub fn enter(mut self, transition: crate::style::Transition) -> Self {
        self.enter = Some(crate::style::Transition {
            opacity: true,
            ..transition
        });
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
            semantics: self.semantics,
            live: self.live,
            selectable: self.selectable,
            enter: self.enter,
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
    /// Grid template columns (switches display to grid).
    pub fn grid_cols(mut self, tracks: impl IntoIterator<Item = crate::style::Track>) -> Self {
        self.style = self.style.grid_cols(tracks);
        self
    }

    /// Grid template rows (switches display to grid).
    pub fn grid_rows(mut self, tracks: impl IntoIterator<Item = crate::style::Track>) -> Self {
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

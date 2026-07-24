//! A native Rust renderer for [A2UI](https://a2ui.org) v0.9 — the open
//! Agent-to-UI standard where agents send declarative JSON surfaces and
//! the client renders them with its own component library.
//!
//! fenestra is that component library here: an A2UI message stream folds
//! into a [`Client`] of [`Surface`]s, each surface renders to a fenestra
//! [`Element`](fenestra_core::Element) tree through the v0.9 *basic
//! catalog* mapping, and everything downstream of that — windowed
//! running, deterministic headless PNGs, the accessibility tree, golden
//! testing — is the ordinary fenestra pipeline. That last part is the
//! point: this is an A2UI client whose output an agent can verify
//! headlessly, byte-for-byte, in CI.
//!
//! ```no_run
//! use fenestra_a2ui::{Client, messages::parse_stream};
//!
//! let msgs = parse_stream(r#"{ "messages": [] }"#).unwrap();
//! let mut client = Client::new();
//! client.apply_all(&msgs).unwrap();
//! if let Some(surface) = client.single_surface() {
//!     let rendered = surface.render(&fenestra_core::Theme::light());
//!     // rendered.element → any fenestra runner or headless render;
//!     // rendered.notes  → what (if anything) didn't map faithfully.
//! }
//! ```
//!
//! Coverage is the whole 18-component basic catalog, with the gaps
//! *reported* per render (`Rendered::notes` / `Surface::notes`): remote
//! images/video/audio render as labeled placeholders (deterministic
//! renders never fetch the network), `DateTimeInput` is an ISO text field
//! for now, obscured text renders unmasked, and `checks` validation rules
//! parse but do not yet gate actions. Data bindings (absolute and
//! template-relative JSON Pointers), templated children, two-way input
//! binding, `formatString`/`formatNumber`/`formatCurrency`/`formatDate`/
//! `pluralize`, and server-bound actions with resolved context all work.

pub mod catalog;
pub mod functions;
pub mod messages;
pub mod render;
pub mod surface;

pub use messages::{Envelope, MessageStream, parse_stream};
pub use render::{A2uiMsg, A2uiSignal, Rendered};
pub use surface::{A2uiError, Client, Surface};

//! The Elm-shaped application contract.

use crate::element::Element;
use crate::theme::Theme;

/// An application: state, a pure view of it, and a message-driven update.
///
/// ```
/// use fenestra_core::*;
///
/// struct Counter {
///     n: i64,
/// }
///
/// #[derive(Clone)]
/// enum Msg {
///     Inc,
///     Dec,
/// }
///
/// impl App for Counter {
///     type Msg = Msg;
///
///     fn update(&mut self, msg: Msg) {
///         match msg {
///             Msg::Inc => self.n += 1,
///             Msg::Dec => self.n -= 1,
///         }
///     }
///
///     fn view(&self) -> Element<Msg> {
///         col().items_center().children([text(self.n.to_string())])
///     }
/// }
/// ```
pub trait App {
    /// The message type carried by handlers. Cloned on dispatch.
    type Msg: Clone + 'static;

    /// Applies one message to the state.
    fn update(&mut self, msg: Self::Msg);

    /// Builds the view. Pure and cheap: called on every redraw, the whole
    /// tree is rebuilt, laid out, and repainted (no diffing).
    fn view(&self) -> Element<Self::Msg>;

    /// The theme to render with. Override to store the theme in app state
    /// (e.g. for a light/dark toggle).
    fn theme(&self) -> Theme {
        Theme::light()
    }
}

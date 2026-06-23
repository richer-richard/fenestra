//! A navigation stack — the Elm-native router. The app holds a [`Nav<Route>`] in
//! its model, matches [`Nav::current`] in `view`, and drives it from `update`
//! (push on navigate, pop on back). There is no framework plumbing: it is a stack
//! of *your own* route values, so it composes with multi-window
//! [`App::view_for`](crate::App::view_for) and with `.enter()` / `.exit()`
//! transitions on the screens it swaps.
//!
//! ```
//! use fenestra_core::Nav;
//!
//! #[derive(Clone, PartialEq, Eq, Debug)]
//! enum Route {
//!     Home,
//!     Settings,
//!     Profile(u32),
//! }
//!
//! let mut nav = Nav::new(Route::Home);
//! nav.push(Route::Settings);
//! nav.push(Route::Profile(7));
//! assert_eq!(nav.depth(), 3);
//! assert_eq!(nav.current(), &Route::Profile(7));
//!
//! assert_eq!(nav.pop(), Some(Route::Profile(7)));
//! assert_eq!(nav.current(), &Route::Settings);
//!
//! nav.pop_to_root();
//! assert_eq!(nav.current(), &Route::Home);
//! assert!(!nav.can_pop()); // the root never pops
//! ```

/// A non-empty stack of routes. The bottom entry is the root and is never popped,
/// so [`current`](Self::current) is always valid — `Nav` cannot be empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nav<R> {
    stack: Vec<R>,
}

impl<R> Nav<R> {
    /// A new stack rooted at `root`.
    #[must_use]
    pub fn new(root: R) -> Self {
        Self { stack: vec![root] }
    }

    /// The route on top of the stack — the screen to render. Always present.
    #[must_use]
    pub fn current(&self) -> &R {
        self.stack.last().expect("Nav is never empty")
    }

    /// The top route, mutably (e.g. to update a screen's in-place state).
    pub fn current_mut(&mut self) -> &mut R {
        self.stack.last_mut().expect("Nav is never empty")
    }

    /// The root route (the bottom of the stack).
    #[must_use]
    pub fn root(&self) -> &R {
        &self.stack[0]
    }

    /// The number of routes on the stack (always ≥ 1).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Whether there is a screen to go back to (depth > 1).
    #[must_use]
    pub fn can_pop(&self) -> bool {
        self.stack.len() > 1
    }

    /// The full stack, root-first — for a breadcrumb or a back affordance.
    #[must_use]
    pub fn stack(&self) -> &[R] {
        &self.stack
    }

    /// Pushes a new screen on top.
    pub fn push(&mut self, route: R) {
        self.stack.push(route);
    }

    /// Pops the top screen, returning it — but never the root, so this returns
    /// `None` when already at the root.
    pub fn pop(&mut self) -> Option<R> {
        if self.can_pop() {
            self.stack.pop()
        } else {
            None
        }
    }

    /// Replaces the top screen in place (CSS-router "replace", no new history
    /// entry).
    pub fn replace(&mut self, route: R) {
        *self.current_mut() = route;
    }

    /// Pops every screen back to the root. Returns whether anything was popped.
    pub fn pop_to_root(&mut self) -> bool {
        let popped = self.can_pop();
        self.stack.truncate(1);
        popped
    }

    /// Clears the whole stack and starts over rooted at `root`.
    pub fn reset(&mut self, root: R) {
        self.stack.clear();
        self.stack.push(root);
    }
}

impl<R: PartialEq> Nav<R> {
    /// Pops back to the topmost occurrence of `route` (inclusive), or does
    /// nothing if it is not on the stack. Returns whether it moved.
    pub fn pop_to(&mut self, route: &R) -> bool {
        if let Some(idx) = self.stack.iter().rposition(|r| r == route) {
            let moved = idx + 1 < self.stack.len();
            self.stack.truncate(idx + 1);
            moved
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq, Eq, Debug)]
    enum Route {
        Home,
        List,
        Detail(u32),
    }

    #[test]
    fn root_is_never_popped() {
        let mut nav = Nav::new(Route::Home);
        assert_eq!(nav.depth(), 1);
        assert!(!nav.can_pop());
        assert_eq!(nav.pop(), None);
        assert_eq!(nav.current(), &Route::Home);
    }

    #[test]
    fn push_pop_replace() {
        let mut nav = Nav::new(Route::Home);
        nav.push(Route::List);
        nav.push(Route::Detail(3));
        assert_eq!(nav.depth(), 3);
        assert_eq!(nav.current(), &Route::Detail(3));
        assert!(nav.can_pop());

        assert_eq!(nav.pop(), Some(Route::Detail(3)));
        assert_eq!(nav.current(), &Route::List);

        nav.replace(Route::Detail(9));
        assert_eq!(nav.current(), &Route::Detail(9));
        assert_eq!(nav.depth(), 2, "replace doesn't grow the stack");
    }

    #[test]
    fn pop_to_root_and_reset() {
        let mut nav = Nav::new(Route::Home);
        nav.push(Route::List);
        nav.push(Route::Detail(1));
        assert!(nav.pop_to_root());
        assert_eq!(nav.current(), &Route::Home);
        assert_eq!(nav.depth(), 1);
        assert!(!nav.pop_to_root(), "already at root");

        nav.push(Route::List);
        nav.reset(Route::Detail(5));
        assert_eq!(nav.current(), &Route::Detail(5));
        assert_eq!(nav.depth(), 1);
    }

    #[test]
    fn pop_to_a_route() {
        let mut nav = Nav::new(Route::Home);
        nav.push(Route::List);
        nav.push(Route::Detail(1));
        nav.push(Route::Detail(2));
        assert!(nav.pop_to(&Route::List));
        assert_eq!(nav.current(), &Route::List);
        assert_eq!(nav.depth(), 2);
        // Not on the stack any more → no-op.
        assert!(!nav.pop_to(&Route::Detail(9)));
        // Already current → no movement.
        assert!(!nav.pop_to(&Route::List));
    }

    #[test]
    fn stack_and_root_accessors() {
        let mut nav = Nav::new(Route::Home);
        nav.push(Route::List);
        assert_eq!(nav.root(), &Route::Home);
        assert_eq!(nav.stack(), &[Route::Home, Route::List]);
    }
}

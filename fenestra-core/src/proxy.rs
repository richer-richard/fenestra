//! Message delivery into a running app from outside the view tree.

use std::sync::Arc;

/// Delivers messages into the running app's `update` from any thread:
/// background work, timers, IO completion. Cloneable and cheap; apps
/// receive one in [`crate::App::init`] and move clones into threads.
pub struct Proxy<Msg> {
    send: Arc<dyn Fn(Msg) + Send + Sync>,
}

impl<Msg> Clone for Proxy<Msg> {
    fn clone(&self) -> Self {
        Self {
            send: Arc::clone(&self.send),
        }
    }
}

impl<Msg> Proxy<Msg> {
    /// Wraps a raw sink. Runners construct this; apps only consume it.
    pub fn new(send: impl Fn(Msg) + Send + Sync + 'static) -> Self {
        Self {
            send: Arc::new(send),
        }
    }

    /// Sends a message to the app. Non-blocking and safe from any thread;
    /// messages sent after the runner stops are dropped silently.
    pub fn send(&self, msg: Msg) {
        (self.send)(msg);
    }
}

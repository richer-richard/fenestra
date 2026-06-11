//! The command channel: `App::init` hands the app a `Proxy` that delivers
//! messages into `update` from outside the view.

use std::sync::{Arc, Mutex, PoisonError};

use fenestra_core::Proxy;

/// A proxy is a cloneable, thread-safe sink: every clone delivers into the
/// same place, from any thread.
#[test]
fn proxy_delivers_from_any_thread() {
    let seen: Arc<Mutex<Vec<u32>>> = Arc::default();
    let sink = seen.clone();
    let proxy = Proxy::new(move |m: u32| {
        sink.lock().unwrap_or_else(PoisonError::into_inner).push(m);
    });

    proxy.send(1);
    let off_thread = proxy.clone();
    std::thread::spawn(move || off_thread.send(2))
        .join()
        .expect("sender thread");

    let mut got = seen.lock().unwrap_or_else(PoisonError::into_inner).clone();
    got.sort_unstable();
    assert_eq!(got, vec![1, 2]);
}

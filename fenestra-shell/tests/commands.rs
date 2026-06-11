//! Headless command-channel semantics: messages sent through the
//! `App::init` proxy are applied at deterministic drain points.

use fenestra_core::{App, Element, Proxy, SP4, Theme, col, text};
use fenestra_shell::render_app;

struct Booted {
    label: String,
    proxy: Option<Proxy<BootMsg>>,
}

#[derive(Clone)]
enum BootMsg {
    Ready,
}

impl App for Booted {
    type Msg = BootMsg;

    fn init(&mut self, proxy: Proxy<BootMsg>) {
        // Synchronous send during init: must be visible in the first frame.
        proxy.send(BootMsg::Ready);
        self.proxy = Some(proxy);
    }

    fn update(&mut self, msg: BootMsg) {
        match msg {
            BootMsg::Ready => self.label = "ready".to_owned(),
        }
    }

    fn view(&self) -> Element<BootMsg> {
        col().p(SP4).children([text(&self.label)])
    }
}

/// `render_app` calls `init` and drains proxied messages before rendering,
/// so init-time sends behave deterministically in tests.
#[test]
fn init_proxy_messages_apply_headlessly() {
    let theme = Theme::light();
    let mut app = Booted {
        label: "booting".to_owned(),
        proxy: None,
    };
    let _ = render_app(&mut app, &[], (200, 60), &theme);
    assert_eq!(app.label, "ready");
    assert!(app.proxy.is_some(), "app keeps the proxy for later sends");
}

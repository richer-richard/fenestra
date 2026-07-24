//! The declarative native menu bar: `App::menu` describes it from state,
//! chosen items come back as messages, and structural changes (here the
//! enabled state of Save) reconcile automatically. Attaches on macOS; on
//! other platforms the same app runs with the kit's in-window patterns.
//!
//! `cargo run --example native_menu`

use fenestra::prelude::*;

struct Editor {
    dirty: bool,
    last_action: String,
}

#[derive(Clone)]
enum Msg {
    New,
    Save,
    Edit,
    ToggleTheme,
}

impl App for Editor {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::New => {
                self.dirty = false;
                self.last_action = "File → New".into();
            }
            Msg::Save => {
                self.dirty = false;
                self.last_action = "File → Save".into();
            }
            Msg::Edit => {
                self.dirty = true;
                self.last_action = "edited (Save enables)".into();
            }
            Msg::ToggleTheme => {
                self.last_action = "View → Toggle theme".into();
            }
        }
    }

    fn menu(&self) -> Option<MenuSpec<Msg>> {
        Some(MenuSpec::new([
            MenuDesc::new(
                "File",
                [
                    MenuItemDesc::item("New", Msg::New).accelerator("CmdOrCtrl+N"),
                    MenuItemDesc::Separator,
                    if self.dirty {
                        MenuItemDesc::item("Save", Msg::Save).accelerator("CmdOrCtrl+S")
                    } else {
                        MenuItemDesc::item("Save", Msg::Save).disabled()
                    },
                ],
            ),
            MenuDesc::new(
                "View",
                [MenuItemDesc::item("Toggle theme", Msg::ToggleTheme)],
            ),
        ]))
    }

    fn view(&self) -> Element<Msg> {
        col()
            .p(SP6)
            .gap(SP4)
            .items_center()
            .justify_center()
            .children((
                text(if self.dirty {
                    "Unsaved changes (Save is enabled)"
                } else {
                    "Saved (Save is greyed out)"
                }),
                text(format!("last action: {}", self.last_action))
                    .size(TextSize::Sm)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
                button("Edit something").on_click(Msg::Edit),
            ))
    }
}

fn main() {
    fenestra::run(
        Editor {
            dirty: false,
            last_action: "none yet".into(),
        },
        WindowOptions::titled("Native menu").with_size(480.0, 260.0),
    );
}

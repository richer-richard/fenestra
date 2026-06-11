//! Native file dialogs (rfd) composed with the command proxy: a tiny text
//! editor that opens and saves through the OS pickers. The dialogs run on
//! a worker thread (`rfd::AsyncFileDialog` hops to the main run loop where
//! the platform requires it) and report back as messages.
//!
//! `cargo run --example file_dialog`

use std::path::PathBuf;

use fenestra::prelude::*;

struct Editor {
    path: Option<PathBuf>,
    text: String,
    status: String,
    proxy: Option<Proxy<Msg>>,
}

#[derive(Clone)]
enum Msg {
    Open,
    Save,
    Edit(String),
    Loaded(PathBuf, String),
    Saved(PathBuf),
    Failed(String),
}

impl App for Editor {
    type Msg = Msg;

    fn init(&mut self, proxy: Proxy<Msg>) {
        self.proxy = Some(proxy);
    }

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Open => {
                let Some(proxy) = self.proxy.clone() else {
                    return;
                };
                std::thread::spawn(move || {
                    let picked = pollster::block_on(
                        rfd::AsyncFileDialog::new()
                            .add_filter("text", &["txt", "md", "rs", "toml"])
                            .pick_file(),
                    );
                    if let Some(file) = picked {
                        let path = file.path().to_path_buf();
                        match std::fs::read_to_string(&path) {
                            Ok(text) => proxy.send(Msg::Loaded(path, text)),
                            Err(e) => proxy.send(Msg::Failed(e.to_string())),
                        }
                    }
                });
            }
            Msg::Save => {
                let Some(proxy) = self.proxy.clone() else {
                    return;
                };
                let text = self.text.clone();
                let suggested = self.path.as_ref().and_then(|p| p.file_name()).map_or_else(
                    || "untitled.txt".into(),
                    |n| n.to_string_lossy().into_owned(),
                );
                std::thread::spawn(move || {
                    let picked = pollster::block_on(
                        rfd::AsyncFileDialog::new()
                            .set_file_name(&suggested)
                            .save_file(),
                    );
                    if let Some(file) = picked {
                        let path = file.path().to_path_buf();
                        match std::fs::write(&path, text) {
                            Ok(()) => proxy.send(Msg::Saved(path)),
                            Err(e) => proxy.send(Msg::Failed(e.to_string())),
                        }
                    }
                });
            }
            Msg::Edit(s) => self.text = s,
            Msg::Loaded(path, text) => {
                self.status = format!("opened {}", path.display());
                self.path = Some(path);
                self.text = text;
            }
            Msg::Saved(path) => {
                self.status = format!("saved {}", path.display());
                self.path = Some(path);
            }
            Msg::Failed(e) => self.status = format!("error: {e}"),
        }
    }

    fn view(&self) -> Element<Msg> {
        col()
            .w_full()
            .h_full()
            .p(SP6)
            .gap(SP4)
            .items_start()
            .children([
                row().items_center().gap(SP3).children([
                    Element::from(button("Open…").on_click(Msg::Open).id("open")),
                    Element::from(
                        button("Save as…")
                            .variant(ButtonVariant::Secondary)
                            .on_click(Msg::Save)
                            .id("save"),
                    ),
                    text(&self.status)
                        .size(TextSize::Sm)
                        .themed(|t: &Theme, s| s.color(t.text_muted)),
                ]),
                text_area(&self.text)
                    .placeholder("Open a file, or just start typing…")
                    .width(640.0)
                    .min_height(360.0)
                    .on_input(Msg::Edit)
                    .id("editor")
                    .into(),
            ])
    }
}

fn main() {
    fenestra::run(
        Editor {
            path: None,
            text: String::new(),
            status: "no file open".to_owned(),
            proxy: None,
        },
        WindowOptions::titled("fenestra file dialogs").with_size(720.0, 520.0),
    )
}

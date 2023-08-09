use std::sync::mpsc::{channel, Receiver, Sender};

use egui::TextEdit;

use crate::handle::Handle;

pub struct App {
    state: State,
    storage: Storage,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
struct Storage {
    url: String,
    target: Handle,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            url: String::new(),
            target: Handle::from_hex("0-0-0-2400000000000000").unwrap(),
        }
    }
}

struct State {
    target_input: String,
    response: String,
    error: String,
    first_render: bool,
    response_tx: Sender<ehttp::Result<ehttp::Response>>,
    response_rx: Receiver<ehttp::Result<ehttp::Response>>,
}

impl Default for State {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            target_input: String::new(),
            response: String::new(),
            error: String::new(),
            first_render: true,
            response_tx: tx,
            response_rx: rx,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return Self {
                state: State::default(),
                storage: eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default(),
            };
        }

        App {
            state: State::default(),
            storage: Storage::default(),
        }
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.storage);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let storage = &mut self.storage;
        let State {
            target_input,
            response,
            error,
            first_render,
            response_tx: tx,
            response_rx: rx,
        } = &mut self.state;

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("URL: ");
                TextEdit::singleline(&mut storage.url)
                    .hint_text("127.0.0.1:9090")
                    .desired_width(f32::INFINITY)
                    .show(ui);
            });

            if *first_render {
                *target_input = storage.target.to_hex();
            }
            ui.horizontal(|ui| {
                ui.label("Target: ");
                if TextEdit::singleline(target_input)
                    .desired_width(f32::INFINITY)
                    .show(ui)
                    .response
                    .changed()
                {
                    match Handle::from_hex(&target_input) {
                        Ok(h) => {
                            error.clear();
                            storage.target = h;
                        }
                        Err(_) => todo!(),
                    }
                }
            });

            if ui.button("Get Parent").clicked() {
                let request = ehttp::Request::get(format!(
                    "http://{}/parents?handle={}",
                    storage.url,
                    storage.target.to_hex(),
                ));
                let tx_clone = tx.clone();
                ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
                    tx_clone.send(result).unwrap();
                })
            }

            if let Ok(http_result) = rx.try_recv() {
                match http_result {
                    Ok(v) => *response = v.text().unwrap().to_string(),
                    Err(e) => *error = e,
                }
            }

            ui.separator();
            ui.label(response.as_str());

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            ui.heading("eframe template");
            ui.hyperlink("https://github.com/emilk/eframe_template");
            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/master/",
                "Source code."
            ));
            egui::warn_if_debug_build(ui);
        });

        *first_render = false;

        if false {
            egui::Window::new("Window").show(ctx, |ui| {
                ui.label("Windows can be moved by dragging them.");
                ui.label("They are automatically sized based on contents.");
                ui.label("You can turn on resizing and scrolling if you like.");
                ui.label("You would normally choose either panels OR windows.");
            });
        }
    }
}

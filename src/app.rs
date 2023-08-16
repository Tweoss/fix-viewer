use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc,
};

use anyhow::Result;
use egui::{
    plot::{Line, Plot},
    Color32, Stroke, TextEdit,
};
use reqwest::Client;

use crate::{
    handle::Handle,
    http::{self, Response},
    plot::Graph,
};

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
    client: Arc<Client>,
    response_tx: Sender<Result<http::Response>>,
    response_rx: Receiver<Result<http::Response>>,
    graph: Graph,
}

impl Default for State {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            target_input: String::new(),
            response: String::new(),
            error: String::new(),
            first_render: true,
            client: Arc::new(Client::new()),
            response_tx: tx,
            response_rx: rx,
            graph: Graph::new("dependency_graph"),
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
            client,
            response_tx: tx,
            response_rx: rx,
            graph,
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
                    || *first_render
                {
                    match Handle::from_hex(&target_input) {
                        Ok(h) => {
                            error.clear();
                            storage.target = h.clone();
                            log::error!("BONNNNJOUR valid handle");
                            graph.set_main_handle(ui, h);
                            // objects.truncate(1);
                            // objects[0].update_text(ui, storage.target.to_hex());
                        }
                        Err(e) => *error = format!("{:#}", e),
                    }
                }
            });

            if ui.button("Get Parent").clicked() {
                http::get_parents(
                    client.clone(),
                    ctx.clone(),
                    storage.target.clone(),
                    tx.clone(),
                    &storage.url,
                );
            }

            if let Ok(http_result) = rx.try_recv() {
                match http_result {
                    Ok(Response::Parents(tasks)) => {
                        *response = tasks
                            .iter()
                            .map(|task| task.to_string())
                            .collect::<Vec<_>>()
                            .join("\n");
                        for (i, task) in tasks.iter().enumerate() {
                            todo!()
                        }
                    }
                    Err(e) => *error = format!("{:#}", e),
                    _ => todo!(),
                }
            }

            ui.separator();
            ui.label(response.as_str());
            ui.label(error.as_str());

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
            Plot::new("view_plot").data_aspect(1.0).show(ui, |plot_ui| {
                plot_ui.add(graph.clone());
                plot_ui.line(Line::new(vec![[1.0, 1.0], [0.5, 0.0]]));
                // Give the stroke an automatic color if no color has been assigned.
                let line = Line::new(vec![[1.0, 1.0], [0.0, 0.0]])
                    .stroke(Stroke::new(0.2, Color32::WHITE));
                plot_ui.add(line);
                plot_ui.transform().dpos_dvalue_x() as f32
            });

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

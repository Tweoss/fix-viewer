use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc,
};

use anyhow::Result;
use egui::{
    plot::{items::PlotItem, Plot},
    Color32, TextEdit, Visuals,
};
use reqwest::Client;

use crate::{
    graph::Graph,
    handle::{Handle, Operation},
    http::{self, Response},
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
    response_tx: Sender<(Handle, Result<http::Response>)>,
    response_rx: Receiver<(Handle, Result<http::Response>)>,
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

        cc.egui_ctx.set_visuals(Visuals::dark());

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
                    match Handle::from_hex(target_input) {
                        Ok(h) => {
                            error.clear();
                            storage.target = h.clone();
                            graph.set_main_handle(ui, h);
                        }
                        Err(e) => *error = format!("{:#}", e),
                    }
                }
            });

            if ui.button("Get Parent").clicked() {
                http::get_parents(
                    client.clone(),
                    ctx.clone(),
                    &storage.target,
                    tx.clone(),
                    &storage.url,
                );
            }

            if let Ok(http_result) = rx.try_recv() {
                let handle = http_result.0;
                match http_result.1 {
                    Ok(Response::Parents(tasks)) => {
                        if let Some(tasks) = tasks {
                            log::info!("Received tasks {:?}", tasks);
                            graph.set_parents(ui, handle, tasks);
                        }
                    }
                    Err(e) => *error = format!("Failed http request: {}.", e.root_cause()),
                    _ => todo!(),
                }
            }

            ui.separator();
            ui.colored_label(Operation::Apply.get_color(), Operation::Apply.to_string());
            ui.colored_label(Operation::Eval.get_color(), Operation::Eval.to_string());
            ui.colored_label(Operation::Fill.get_color(), Operation::Fill.to_string());
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
            let hovered_elem = Plot::new("view_plot")
                .data_aspect(1.0)
                .auto_bounds_x()
                .auto_bounds_y()
                .show_axes([true; 2])
                .show_x(false)
                .show_y(false)
                .show(ui, |plot_ui| {
                    plot_ui.add(graph.clone());
                    let (Some(coords), true) = (plot_ui.pointer_coordinate(), plot_ui.plot_clicked()) else {
                        return None
                    };
                    let closest_elem = graph
                        .find_closest(plot_ui.screen_from_plot(coords), plot_ui.transform())?;
                    Some((coords, closest_elem))
                }).inner;

            if let Some((coords, closest_elem)) = hovered_elem {
                graph.handle_nearby_click(ui, coords, closest_elem, |handle| {
                    http::get_parents(
                        client.clone(),
                        ctx.clone(),
                        handle,
                        tx.clone(),
                        &storage.url,
                    );
                });
            }

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

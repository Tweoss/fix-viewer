use std::sync::mpsc::Sender;
use std::sync::Arc;

use anyhow::Result;
use egui::plot::Plot;
use egui::Context;
use egui::{plot::items::PlotItem, Ui};

use crate::handle::Task;
use crate::http;
use crate::{handle::Handle, plot::Element};

mod ancestors;

#[derive(Clone)]
pub(crate) struct GraphsContainer {
    ancestry: Option<ancestors::AncestorGraph>,
}

impl GraphsContainer {
    pub fn new() -> Self {
        Self { ancestry: None }
    }

    pub fn view(
        &self,
        ctx: &Context,
        client: Arc<reqwest::Client>,
        url: &str,
        tx: Sender<(Handle, Result<http::Response>)>,
    ) {
        egui::Window::new("Ancestry Tree").resizable(true).show(ctx, |ui| {
            let hovered_elem = Plot::new("view_plot")
                .data_aspect(1.0)
                .auto_bounds_x()
                .auto_bounds_y()
                .show_axes([true; 2])
                .show_x(false)
                .show_y(false)
                .show(ui, |plot_ui| {
                    let graph = self.ancestry.as_ref()?;
                    plot_ui.add(graph.clone());
                    let (Some(coords), true) = (plot_ui.pointer_coordinate(), plot_ui.plot_clicked()) else {
                        return None
                    };
                    let closest_elem = graph
                        .find_closest(plot_ui.screen_from_plot(coords), plot_ui.transform())?;
                    Some((coords, closest_elem))
                }).inner;

            if let Some((coords, closest_elem)) = hovered_elem {
                if let Some(graph) = &self.ancestry {
                    graph.handle_nearby_click(ui, coords, closest_elem, |handle| {
                        http::get_parents(
                            client.clone(),
                            ctx.clone(),
                            handle,
                            tx.clone(),
                            url,
                        );
                    });
                }
            }
        });
    }

    /// Resets the main ancestor and deletes all of its ancestors.
    pub fn set_main_handle(&mut self, ui: &Ui, handle: Handle) {
        self.ancestry = Some(ancestors::AncestorGraph::new(Element::new(ui, handle)));
    }

    /// Set the parents of a specific handle
    pub fn set_parents(&mut self, ui: &Ui, handle: Handle, parents: Vec<Task>) {
        // Merge into the ancestry tree.
        if let Some(main) = &mut self.ancestry {
            main.merge_new_parents(ui, handle, &parents);
        }
    }
}

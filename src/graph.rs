use eframe::epaint::util::FloatOrd;
use egui::plot::LabelFormatter;
use egui::{
    plot::{
        items::{
            values::{ClosestElem, PlotGeometry},
            PlotConfig, PlotItem,
        },
        PlotBounds, PlotPoint, PlotTransform,
    },
    Color32, Pos2, Shape, Ui,
};

use crate::handle::Task;
use crate::{handle::Handle, plot::Element};

mod ancestors;

#[derive(Clone)]
pub(crate) struct Graph {
    name: String,
    main: Option<ancestors::AncestorGraph>,
}

impl Graph {
    /// Defines an ordering of the elements in a graph. Used for referencing
    /// from a ClosestElem.
    /// ```
    ///     pp0 = 3  pp1 = 4
    ///          \    /
    ///  p0 = 1  p1 = 2  p2 = 5
    ///      \     |    /  
    ///        main = 0
    ///
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &Element> {
        let iter = std::iter::empty();
        // Main handle and associated ancestors.
        let iter = iter.chain(self.main.iter().flat_map(|a| a.iter()));

        iter
    }

    pub fn new(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            main: None,
        }
    }

    /// Resets the main ancestor and deletes all of its ancestors.
    pub fn set_main_handle(&mut self, ui: &Ui, handle: Handle) {
        self.main = Some(ancestors::AncestorGraph::new(Element::new(ui, handle)));
    }

    /// Set the parents of a specifc handle (which must be either a MainAncestor)
    /// or an Ancestor of the MainAncestor
    pub fn set_parents(&mut self, ui: &Ui, handle: Handle, parents: Vec<Task>) {
        // Merge into the ancestry tree.
        if let Some(main) = &mut self.main {
            main.merge_new_parents(ui, handle, &parents);
        }
    }

    /// Categorises an index as a `GraphIndex`.
    pub fn get_graph_index(&self, index: usize) -> Option<GraphIndex> {
        if let Some(main) = &self.main {
            let ancestor_length = main.iter().count();
            if (0..ancestor_length).contains(&index) {
                return Some(GraphIndex::Ancestor(index));
            }
        }
        None
    }

    /// Calculates the draw parameters of an element based on its GraphIndex.
    /// Linear in the size of the ancestry tree.
    pub fn get_draw_parameters(&self, index: GraphIndex) -> (PlotPoint, f32) {
        match index {
            GraphIndex::Ancestor(index) => self
                .main
                .as_ref()
                .expect("Should have main when getting draw parameters")
                .get_draw_parameters(index),
        }
    }

    /// Handle a click that is near to a ClosestElem. May send an http request
    /// that is specified by the `request` parameter.
    pub(crate) fn handle_nearby_click(
        &self,
        ui: &Ui,
        coords: PlotPoint,
        closest_elem: ClosestElem,
        request: impl FnOnce(&Handle),
    ) {
        let Some(elem) = self.iter().nth(closest_elem.index) else {
            log::error!("Handling a click near to an element whose index no longer exists");
            return;
        };

        let params = self.get_draw_parameters(self.get_graph_index(closest_elem.index).unwrap());
        let [min_x, min_y] = elem.bounds(params).min();
        let [max_x, max_y] = elem.bounds(params).max();
        let p = coords;
        let elem_contains_p = min_x <= p.x && p.x <= max_x && min_y <= p.y && p.y <= max_y;
        if elem_contains_p {
            ui.output_mut(|o| o.copied_text = elem.get_text());
            log::info!("Requesting parents");
            request(elem.get_handle());
        };
    }
}

pub(crate) enum GraphIndex {
    Ancestor(usize),
}

impl PlotItem for Graph {
    fn shapes(&self, _ui: &mut Ui, transform: &PlotTransform, shapes: &mut Vec<Shape>) {
        for (index, el) in self.iter().enumerate() {
            el.add_shapes(
                transform,
                shapes,
                self.get_draw_parameters(self.get_graph_index(index).unwrap()),
                false,
            );
        }
    }

    fn initialize(&mut self, _x_range: std::ops::RangeInclusive<f64>) {}

    fn name(&self) -> &str {
        &self.name
    }

    fn color(&self) -> Color32 {
        Color32::RED
    }

    fn highlight(&mut self) {}

    fn highlighted(&self) -> bool {
        false
    }

    fn geometry(&self) -> PlotGeometry<'_> {
        PlotGeometry::Rects
    }

    /// Search for the closest element in the graph based on squared distance to bounds.
    fn find_closest(&self, point: Pos2, transform: &PlotTransform) -> Option<ClosestElem> {
        self.iter()
            .enumerate()
            .map(|(index, el)| {
                let bounds =
                    el.bounds(self.get_draw_parameters(self.get_graph_index(index).unwrap()));
                let rect = transform.rect_from_values(&bounds.min().into(), &bounds.max().into());
                ClosestElem {
                    index,
                    dist_sq: rect.distance_sq_to_pos(point),
                }
            })
            .min_by_key(|e| e.dist_sq.ord())
    }

    fn on_hover(
        &self,
        elem: ClosestElem,
        shapes: &mut Vec<Shape>,
        _: &mut Vec<egui::plot::Cursor>,
        plot: &PlotConfig<'_>,
        _: &LabelFormatter,
    ) {
        let entry = self.iter().nth(elem.index);
        let Some(entry) = entry else { return };
        entry.add_highlight(
            plot.transform,
            self.get_draw_parameters(self.get_graph_index(elem.index).unwrap()),
            shapes,
        );
    }

    fn bounds(&self) -> PlotBounds {
        let mut bounds = PlotBounds::NOTHING;
        for (index, el) in self.iter().enumerate() {
            bounds
                .merge(&el.bounds(self.get_draw_parameters(self.get_graph_index(index).unwrap())));
        }
        bounds
    }
}

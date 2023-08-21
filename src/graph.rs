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
use crate::plot::ElementContent;
use crate::{handle::Handle, plot::Element};

#[derive(Clone)]
pub(crate) struct Graph {
    name: String,
    main: Option<ancestors::MainAncestor>,
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
        self.main = Some(ancestors::MainAncestor::new(Element::new(
            ui,
            ElementContent::Handle(handle),
        )));
    }

    /// Set the parents of a specifc handle (which must be either a MainAncestor)
    /// or an Ancestor of the MainAncestor
    pub fn set_parents(&mut self, ui: &Ui, handle: Handle, parents: Vec<Task>) {
        // Wrap the new Tasks in Elements.
        let elements: Vec<_> = parents
            .iter()
            .map(|task| Element::new(ui, ElementContent::Task(task.clone())))
            .collect();
        // Merge into the ancestry tree.
        if let Some(main) = &mut self.main {
            main.merge_new_parents(handle, &elements);
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

mod ancestors {
    use std::fmt::Display;

    use egui::plot::PlotPoint;

    use crate::{handle::Handle, plot::Element};

    /// An element and all of its ancestors
    #[derive(Clone)]
    pub(super) struct MainAncestor {
        inner: [Ancestor; 1],
    }

    /// An element and all of its ancestors
    #[derive(Clone)]
    pub struct Ancestor {
        content: Element,
        parents: Vec<Ancestor>,
    }

    impl Display for Ancestor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!(
                "{{content: {}, parents: [",
                self.content.get_text()
            ))?;
            for parent in &self.parents {
                f.write_fmt(format_args!("{}, ", parent))?;
            }
            f.write_str("]}")
        }
    }

    impl MainAncestor {
        pub fn new(element: Element) -> Self {
            Self {
                inner: [Ancestor {
                    content: element,
                    parents: vec![],
                }],
            }
        }

        pub fn iter(&self) -> impl Iterator<Item = &Element> {
            let mut list = vec![];
            // Prefix, depth-first search
            fn push_elements<'a>(current_list: &'a [Ancestor], list: &mut Vec<&'a Element>) {
                for el in current_list {
                    list.push(&el.content);
                    push_elements(&el.parents, list)
                }
            }
            push_elements(&self.inner, &mut list);
            list.into_iter()
        }

        fn find(&mut self, handle: Handle) -> Option<&mut Ancestor> {
            fn find_rec<'a>(
                current_list: &'a mut [Ancestor],
                handle: &Handle,
            ) -> Option<&'a mut Ancestor> {
                current_list.iter_mut().find_map(|el| {
                    if el.content.get_handle() == handle {
                        return Some(el);
                    }
                    find_rec(&mut el.parents, handle)
                })
            }
            find_rec(&mut self.inner, &handle)
        }

        pub fn get_draw_parameters(&self, mut index: usize) -> (PlotPoint, f32) {
            // Convert this singular index into indices into each of the generations.
            fn get_location_rec(
                current_list: &[Ancestor],
                index: &mut usize,
            ) -> Option<Vec<usize>> {
                current_list.iter().enumerate().find_map(|(i, el)| {
                    if *index == 0 {
                        return Some(vec![i]);
                    }
                    // Prefix
                    *index -= 1;
                    // Depth first
                    if let Some(mut list) = get_location_rec(&el.parents, index) {
                        list.push(i);
                        return Some(list);
                    }
                    None
                })
            }

            let lineage = {
                let mut vec = get_location_rec(&self.inner, &mut index)
                    .expect("Need index that is contained in ancestors to get_location");
                vec.reverse();
                vec
            };

            let mut pos = [0.0, 0.0];
            let mut scale = 1.0;
            let mut current_generation = self.inner.as_slice();
            for lineage_index in &lineage {
                // Scale y for this generation
                scale /= current_generation.len() as f32;
                // Increase y
                pos[1] += scale;
                // Offset x
                // |   0   |   1   |   2   |
                // |  0  |  1  |  2  |  3  |
                let step_size = scale;
                let x_step_offset_to_left_edge =
                    *lineage_index as f32 - (current_generation.len() as f32) * 0.5;
                let x_step_offset_to_center = x_step_offset_to_left_edge + 0.5;
                pos[0] += step_size * x_step_offset_to_center;

                current_generation = current_generation[*lineage_index].parents.as_slice();
            }

            (PlotPoint::new(pos[0], pos[1]), scale)
        }

        pub fn merge_new_parents(&mut self, handle: Handle, incoming_parents: &[Element]) {
            if let Some(ancestor) = self.find(handle) {
                ancestor.merge_parents(incoming_parents);
            }
        }
    }

    impl Ancestor {
        fn merge_parents(&mut self, incoming_parents: &[Element]) {
            for element in incoming_parents {
                // Linear scan, performance irrelevant for small lists of parents.
                if self
                    .parents
                    .iter()
                    .map(|p| &p.content)
                    .all(|p_el| p_el != element)
                {
                    self.parents.push(Ancestor {
                        content: element.clone(),
                        parents: vec![],
                    });
                }
            }
        }
    }
}

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

use crate::{
    handle::{Handle, Task},
    plot::Element,
};

#[derive(Clone)]
pub struct Graph {
    name: String,
    main: Option<Element>,
    parents: Vec<Element>,
}

impl Graph {
    fn iter(
        &self,
    ) -> std::iter::Chain<std::option::Iter<'_, Element>, std::slice::Iter<'_, Element>> {
        self.main.iter().chain(self.parents.iter())
    }
}
impl Graph {
    pub(crate) fn new(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            main: None,
            parents: Vec::new(),
        }
    }

    pub(crate) fn set_main_handle(&mut self, ui: &Ui, handle: Handle) {
        self.main = Some(Element::new(
            ui,
            handle.to_hex(),
            PlotPoint::new(0.0, 10.5),
            6.0,
        ));
    }

    pub(crate) fn set_parents(&mut self, ui: &Ui, tasks: Vec<Task>) {
        self.parents = tasks
            .iter()
            .enumerate()
            .map(|(i, task)| {
                Element::new(
                    ui,
                    task.to_string(),
                    PlotPoint::new(i as f32 * 4.0, -5.0),
                    2.0,
                )
            })
            .collect();
    }
}

impl PlotItem for Graph {
    fn shapes(&self, _ui: &mut Ui, transform: &PlotTransform, shapes: &mut Vec<Shape>) {
        for el in self.iter() {
            el.add_shapes(transform, shapes, false);
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

    fn find_closest(&self, point: Pos2, transform: &PlotTransform) -> Option<ClosestElem> {
        self.iter()
            .enumerate()
            .map(|(index, el)| {
                let bounds = el.bounds();
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
        entry.add_highlight(plot.transform, shapes);
    }

    fn bounds(&self) -> PlotBounds {
        let mut bounds = PlotBounds::NOTHING;
        if let Some(main) = &self.main {
            bounds.merge(&main.bounds());
        }
        for parent in self.parents.iter() {
            bounds.merge(&parent.bounds());
        }
        bounds
    }
}

use eframe::epaint::{ClippedShape, Primitive, TextShape};
use egui::{
    plot::{
        items::{
            values::{ClosestElem, PlotGeometry},
            PlotConfig, PlotItem,
        },
        LabelFormatter, PlotBounds, PlotPoint, PlotTransform,
    },
    Color32, Mesh, Pos2, Rect, RichText, Shape, Stroke, TextStyle, Ui, WidgetText,
};

use crate::handle::Handle;

impl Graph {
    pub(crate) fn new(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            main: None,
            parents: Vec::new(),
        }
    }

    pub(crate) fn set_main_handle(&mut self, ui: &Ui, handle: Handle) {
        self.main = Some(HandleBox::new(ui, handle));
    }
}

#[derive(Clone)]
pub struct Graph {
    name: String,
    main: Option<HandleBox>,
    parents: Vec<HandleBox>,
}

impl PlotItem for Graph {
    fn shapes(&self, ui: &mut Ui, transform: &PlotTransform, shapes: &mut Vec<Shape>) {
        if let Some(handle) = &self.main {
            handle.add_shapes(transform, shapes, PlotPoint::new(0.0, 3.0), false)
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
        // TODO
        None
    }

    fn on_hover(
        &self,
        elem: ClosestElem,
        shapes: &mut Vec<Shape>,
        cursors: &mut Vec<egui::plot::Cursor>,
        plot: &PlotConfig<'_>,
        label_formatter: &LabelFormatter,
    ) {
        // TODO
    }

    fn bounds(&self) -> PlotBounds {
        let mut bounds = PlotBounds::NOTHING;
        if let Some(main) = &self.main {
            bounds.merge(&main.bounds());
        }
        bounds
    }
}

#[derive(Clone)]
struct HandleBox {
    inner: Handle,
    mesh: Mesh,
    mesh_bounds: Rect,
}

impl HandleBox {
    const TEXT_RENDER_SCALE: f32 = 30.0;
    const RECT_EXTENSION: f32 = 0.3;

    fn new(ui: &Ui, handle: Handle) -> Self {
        let text = handle.to_hex();
        let rich_text = RichText::new(&text)
            .size(Self::TEXT_RENDER_SCALE)
            .monospace()
            .color(Color32::WHITE);
        let galley = WidgetText::RichText(rich_text).into_galley(
            ui,
            Some(false),
            f32::INFINITY,
            TextStyle::Monospace,
        );
        if let Primitive::Mesh(mut mesh) = ui
            .ctx()
            .tessellate(vec![ClippedShape(
                Rect::EVERYTHING,
                TextShape::new(Pos2::ZERO, galley.galley).into(),
            )])
            .pop()
            .unwrap()
            .primitive
        {
            let mid_point =
                (mesh.calc_bounds().min.to_vec2() + mesh.calc_bounds().max.to_vec2()) / 2.0;
            mesh.translate(-mid_point);
            mesh.vertices.iter_mut().for_each(|v| {
                v.pos = Pos2::new(
                    v.pos.x / Self::TEXT_RENDER_SCALE,
                    v.pos.y / Self::TEXT_RENDER_SCALE,
                );
            });
            Self {
                inner: handle,
                mesh_bounds: mesh.calc_bounds().expand(Self::RECT_EXTENSION),
                mesh,
            }
        } else {
            panic!("Tessellated text should be a mesh")
        }
    }

    fn add_shapes(
        &self,
        transform: &PlotTransform,
        shapes: &mut Vec<Shape>,
        placement: PlotPoint,
        highlight: bool,
    ) {
        let scale_transform = |pos: Pos2| -> Pos2 {
            Pos2::new(
                pos.x * transform.dpos_dvalue_x() as f32,
                -pos.y * transform.dpos_dvalue_y() as f32,
            )
        };
        let translation = transform.position_from_point(&placement).to_vec2();

        let mut mesh = self.mesh.clone();
        mesh.vertices.iter_mut().for_each(|v| {
            v.pos = scale_transform(v.pos);
        });
        mesh.translate(transform.position_from_point(&placement).to_vec2());

        let mut mesh_bounds = self.mesh_bounds.clone();
        mesh_bounds.min = scale_transform(mesh_bounds.min);
        mesh_bounds.max = scale_transform(mesh_bounds.max);
        mesh_bounds = mesh_bounds.translate(translation);

        shapes.push(Shape::Mesh(mesh.clone()));
        shapes.push(Shape::rect_stroke(
            mesh_bounds,
            1.0,
            Stroke::new(2.0, Color32::WHITE),
        ));
        if highlight {
            shapes.push(Shape::rect_filled(mesh_bounds, 1.0, Color32::WHITE));
        }
    }

    fn bounds(&self) -> PlotBounds {
        let rect = self.mesh_bounds;
        PlotBounds::from_min_max(
            [rect.min.x.into(), rect.min.y.into()],
            [rect.max.x.into(), rect.max.y.into()],
        )
    }
}

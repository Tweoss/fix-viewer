use eframe::epaint::{ClippedShape, Primitive, RectShape, TextShape};
use egui::{
    plot::{PlotBounds, PlotPoint, PlotTransform},
    Color32, Mesh, Pos2, Rect, RichText, Shape, Stroke, TextStyle, Ui, WidgetText,
};

#[derive(Clone)]
pub(crate) struct Element {
    text: String,
    mesh: Mesh,
    mesh_bounds: Rect,
    point: PlotPoint,
    zoom: f32,
}

impl Element {
    const TEXT_RENDER_SCALE: f32 = 30.0;
    const RECT_EXTENSION: f32 = 0.02;
    /// The number of pixels just a full rendered handle takes.
    /// Used to scale the text.
    const TEXT_PIXEL_SCALE: f32 = 40.0;

    pub(crate) fn new(ui: &Ui, text: String, point: PlotPoint, zoom: f32) -> Self {
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
                    v.pos.x / Self::TEXT_RENDER_SCALE / Self::TEXT_PIXEL_SCALE,
                    v.pos.y / Self::TEXT_RENDER_SCALE / Self::TEXT_PIXEL_SCALE,
                );
            });
            Self {
                text,
                mesh_bounds: mesh.calc_bounds().expand(Self::RECT_EXTENSION),
                mesh,
                point,
                zoom,
            }
        } else {
            panic!("Tessellated text should be a mesh")
        }
    }

    fn graph_pos_to_screen_pos(
        position: Pos2,
        transform: &PlotTransform,
        zoom: f32,
        center: PlotPoint,
    ) -> Pos2 {
        let screen_center = transform.position_from_point(&center);
        Pos2::new(
            position.x * transform.dpos_dvalue_x() as f32 * zoom + screen_center.x,
            -position.y * transform.dpos_dvalue_y() as f32 * zoom + screen_center.y,
        )
    }

    pub(crate) fn add_shapes(
        &self,
        transform: &PlotTransform,
        shapes: &mut Vec<Shape>,
        highlight: bool,
    ) {
        let transform = |pos: Pos2| -> Pos2 {
            Self::graph_pos_to_screen_pos(pos, transform, self.zoom, self.point)
        };

        let mut mesh = self.mesh.clone();
        mesh.vertices.iter_mut().for_each(|v| {
            v.pos = transform(v.pos);
        });

        let mut mesh_bounds = self.mesh_bounds.clone();
        mesh_bounds.min = transform(mesh_bounds.min);
        mesh_bounds.max = transform(mesh_bounds.max);

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

    pub(crate) fn add_highlight(&self, transform: &PlotTransform, shapes: &mut Vec<Shape>) {
        let transform = transform;
        let scale_transform = |pos: Pos2| -> Pos2 {
            Pos2::new(
                pos.x * transform.dpos_dvalue_x() as f32 * self.zoom,
                -pos.y * transform.dpos_dvalue_y() as f32 * self.zoom,
            )
        };
        let translation = transform.position_from_point(&self.point).to_vec2();
        let mut mesh_bounds = self.mesh_bounds.clone();
        mesh_bounds.min = scale_transform(mesh_bounds.min);
        mesh_bounds.max = scale_transform(mesh_bounds.max);
        mesh_bounds = mesh_bounds.translate(translation);

        shapes.push(RectShape::filled(mesh_bounds, 1.0, Color32::BLUE.gamma_multiply(0.2)).into())
    }

    pub(crate) fn bounds(&self) -> PlotBounds {
        let mut rect = self.mesh_bounds;

        assert!(rect.center() == Pos2::ZERO);
        rect.min = (rect.min.to_vec2() * self.zoom + self.point.to_vec2()).to_pos2();
        rect.max = (rect.max.to_vec2() * self.zoom + self.point.to_vec2()).to_pos2();

        // Reverse the y axis because of rect vs plot coordinates.
        let bounds = PlotBounds::from_min_max(
            [rect.left().into(), rect.top().into()],
            [rect.right().into(), rect.bottom().into()],
        );
        bounds
    }
}

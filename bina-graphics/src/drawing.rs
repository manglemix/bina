use cgmath::Point2;

use crate::polygon::Polygon;

pub(crate) enum DrawInstruction {
    DrawPolygon {
        polygon: Polygon,
        origin: Point2<f32>
    },
}

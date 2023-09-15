use lyon::math::Point;

use crate::polygon::Polygon;

pub(crate) enum DrawInstruction {
    DrawPolygon {
        polygon: Polygon,
        origin: Point,
    },
}

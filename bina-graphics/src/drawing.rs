use crate::renderers::DrawPolygon;

pub(crate) enum DrawInstruction {
    DrawPolygon(DrawPolygon),
}

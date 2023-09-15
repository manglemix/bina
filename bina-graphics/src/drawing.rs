use bina_ecs::{rayon::slice::ParallelSliceMut, triomphe::Arc};
use wgpu::{CommandEncoder, RenderPipeline, TextureView};

use crate::polygon::{Material, PolygonInner, Vector};

pub(crate) struct DrawPolygon {
    pub(crate) polygon: Arc<PolygonInner>,
    pub(crate) origin: Vector,
    pub(crate) z: u32,
}

pub(crate) enum DrawInstruction {
    DrawPolygon(DrawPolygon),
}

pub(crate) struct Renderer {
    z_buffer: Vec<DrawPolygon>,
}

impl Renderer {
    pub(crate) fn new() -> Self {
        Self {
            z_buffer: Default::default(),
        }
    }
    pub(crate) fn draw_all(
        &mut self,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        render_pipeline: &RenderPipeline,
        instructions: &mut Vec<DrawInstruction>,
    ) {
        for instruction in instructions.drain(..) {
            match instruction {
                DrawInstruction::DrawPolygon(x) => self.z_buffer.push(x),
            }
        }

        self.z_buffer.par_sort_unstable_by_key(|x| x.z);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[
                    // This is what @location(0) in the fragment shader targets
                    Some(wgpu::RenderPassColorAttachment {
                        view: view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: true,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&render_pipeline);

            let mut last_bind_group = None;

            macro_rules! set_bind_grp {
                ($grp: expr) => {
                    if let Some(last_grp) = last_bind_group {
                        if !std::ptr::eq(last_grp, $grp) {
                            render_pass.set_bind_group(0, $grp, &[]);
                        }
                    } else {
                        last_bind_group = Some($grp);
                        render_pass.set_bind_group(0, $grp, &[]);
                    }
                };
            }

            for DrawPolygon {
                polygon,
                origin: _origin,
                z: _z,
            } in &self.z_buffer
            {
                if let Material::Texture(texture) = &polygon.material {
                    set_bind_grp!(&texture.texture.bind_group);
                }
                render_pass.set_vertex_buffer(0, polygon.vertices.slice(..));
                render_pass.set_index_buffer(polygon.indices.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..polygon.indices_count, 0, 0..1);
            }
        }
        self.z_buffer.clear();
    }
}

// impl PartialEq for DrawInstruction {
//     fn eq(&self, other: &Self) -> bool {
//         matches!(self.cmp(other), std::cmp::Ordering::Equal)
//     }
// }

// impl Eq for DrawInstruction { }

// impl PartialOrd for DrawInstruction {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         Some(self.cmp(other))
//     }
// }

// impl Ord for DrawInstruction {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         if let Self::DrawPolygon { z, .. } = self && let Self::DrawPolygon { z: other_z, .. } = other {
//             z.cmp(other_z)
//         } else {
//             std::cmp::Ordering::Equal
//         }
//     }
// }

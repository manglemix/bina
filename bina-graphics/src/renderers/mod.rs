use bina_ecs::{rayon::slice::ParallelSliceMut, triomphe::Arc};
use wgpu::{BindGroup, BindGroupLayout, Device, RenderPass, SurfaceConfiguration};

use crate::polygon::{Material, PolygonInner};

use self::textured::TexturedPolygonRenderer;

mod textured;

pub(crate) struct DrawPolygon {
    pub(crate) polygon: Arc<PolygonInner>,
    pub(crate) z: u32,
}

pub(super) struct PolygonRendererCreation {
    pub(super) poly_render: PolygonRenderer,
    pub(super) tex_grp_layout: BindGroupLayout,
}

pub(crate) struct PolygonRenderer {
    z_buffer: Vec<DrawPolygon>,
    pub(crate) tex_poly: TexturedPolygonRenderer,
}

impl PolygonRenderer {
    pub(super) fn new(device: &Device, config: &SurfaceConfiguration, transform_bind_group_layout: &BindGroupLayout, camera_bind_group_layout: &BindGroupLayout) -> PolygonRendererCreation {
        let (tex_poly, tex_grp_layout) = TexturedPolygonRenderer::new(device, config, transform_bind_group_layout, camera_bind_group_layout);
        PolygonRendererCreation {
            poly_render: Self {
                z_buffer: Default::default(),
                tex_poly,
            },
            tex_grp_layout,
        }
    }
    pub(super) fn push(&mut self, item: DrawPolygon) {
        self.z_buffer.push(item);
    }

    pub(super) fn draw_all<'a>(&'a mut self, render_pass: &mut RenderPass<'a>, camera_matrix_buffer_bind_group: &'a BindGroup) {
        self.z_buffer.par_sort_unstable_by_key(|x| x.z);

        for draw_polygon in self.z_buffer.drain(..) {
            unsafe {
                match &draw_polygon.polygon.material {
                    Material::FlatColor(_) => todo!(),
                    Material::Texture(_) => self.tex_poly.push(draw_polygon),
                }
            }
        }

        self.tex_poly.draw_all(render_pass, camera_matrix_buffer_bind_group);
    }

    pub(super) fn clear(&mut self) {
        self.tex_poly.clear();
    }
}

struct BindGroupTracker<'a> {
    index: u32,
    last: Option<&'a BindGroup>,
}

impl<'a> BindGroupTracker<'a> {
    fn new(index: u32) -> Self {
        Self { index, last: None }
    }
    fn set_bind_group(&mut self, render_pass: &mut RenderPass<'a>, bind_group: &'a BindGroup) {
        if let Some(last_grp) = self.last {
            if std::ptr::eq(last_grp, bind_group) {
                return;
            }
        }
        self.last = Some(bind_group);
        render_pass.set_bind_group(self.index, bind_group, &[]);
    }
}

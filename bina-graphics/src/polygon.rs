use bina_ecs::{
    component::{Component, Processable},
    triomphe::Arc,
};
use bytemuck::{Pod, Zeroable};
use image::Rgba;
use lyon::{math::point, lyon_tessellation::{FillTessellator, FillOptions, BuffersBuilder, VertexBuffers, FillVertex}, path::traits::PathBuilder};
use wgpu::util::DeviceExt;

use crate::{drawing::DrawInstruction, texture::Texture, Graphics};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct TextureVertex {
    pub x: f32,
    pub y: f32,
    // Texture coordinates
    pub tx: f32,
    pub ty: f32,
}

impl TextureVertex {
    pub(crate) const BUFFER_DESCRIPTOR: wgpu::VertexBufferLayout<'static> =
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<TextureVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<f32>() as wgpu::BufferAddress * 2,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    };
}

pub enum Material {
    FlatColor(Rgba<u8>),
    Texture(Texture),
}

#[derive(Clone)]
pub struct Polygon {
    pub(crate) inner: Arc<PolygonInner>,
}

pub(crate) struct PolygonInner {
    pub(crate) indices_count: u32,
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    pub(crate) material: Material,
}

impl Polygon {
    pub fn new(graphics: &Graphics, vertices: &[TextureVertex], material: Material) -> Self {
        let mut builder = lyon::path::Path::builder_with_attributes(2);
        let mut first = true;
        for v in vertices {
            if first {
                builder.begin(point(v.x, v.y), &[v.tx, v.ty]);
                first = false;
            } else {
                builder.line_to(point(v.x, v.y), &[v.tx, v.ty]);
            }
        }
        builder.close();
        let path = builder.build();

        let mut tessellator = FillTessellator::new();
        let mut geometry: VertexBuffers<TextureVertex, u32> = VertexBuffers::new();

        {
            // Compute the tessellation.
            tessellator.tessellate_path(
                &path,
                &FillOptions::default(),
                &mut BuffersBuilder::new(&mut geometry, |mut vertex: FillVertex| {
                    let attrs = vertex.interpolated_attributes();
                    TextureVertex {
                        tx: attrs[0],
                        ty: attrs[1],
                        x: vertex.position().x,
                        y: vertex.position().y,
                    }
                }),
            ).unwrap();
        }

        Self {
            inner: Arc::new(PolygonInner {
                vertices: graphics.inner.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Vertex Buffer"),
                        contents: bytemuck::cast_slice(&geometry.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ),
                indices: graphics.inner.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Index Buffer"),
                        contents: bytemuck::cast_slice(&geometry.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    }
                ),
                material,
                indices_count: geometry.indices.len() as u32,
            }),
        }
    }
}

impl Component for Polygon {
    fn get_ref<'a>(&'a self) -> Self::Reference<'a> {
        self
    }
}

impl Processable for Polygon {
    fn process<E: bina_ecs::entity::Entity>(
        component: Self::Reference<'_>,
        _my_entity: bina_ecs::entity::EntityReference<E>,
        universe: &bina_ecs::universe::Universe,
    ) {
        let graphics = unsafe { universe.try_get_singleton::<Graphics>().unwrap_unchecked() };
        graphics.queue_draw_instruction(DrawInstruction::DrawPolygon {
            polygon: component.clone(),
            origin: point(0.0, 0.0),
        });
    }
}

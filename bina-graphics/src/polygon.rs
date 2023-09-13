use bina_ecs::{triomphe::Arc, component::{Component, Processable}};
use bytemuck::{Pod, Zeroable};
use cgmath::Point2;
use image::Rgba;
use wgpu::util::DeviceExt;

use crate::{Graphics, texture::Texture, drawing::DrawInstruction};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct TextureVertex {
    pub x: f32,
    pub y: f32,
    // Texture coordinates
    pub tx: f32,
    pub ty: f32
}

pub(crate) const TEXTURE_VERTEX_BUFFER_DESCRIPTOR: wgpu::VertexBufferLayout<'static> =
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<TextureVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2, // NEW!
        }],
    };

pub enum Material {
    FlatColor(Rgba<u8>),
    Texture(Texture)
}

#[derive(Clone)]
pub struct Polygon {
    pub(crate) inner: Arc<PolygonInner>
}


pub(crate) struct PolygonInner {
    pub(crate) vertex_count: u32,
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) material: Material
}


impl Polygon {
    pub fn new(graphics: &Graphics, vertices: &[TextureVertex], material: Material) -> Self {
        Self{
            inner: Arc::new(PolygonInner {
                vertex_count: vertices.len() as u32,
                vertices: graphics.inner.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Vertex Buffer"),
                        contents: bytemuck::cast_slice(vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ),
                material
            })
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
        graphics.queue_draw_instruction(DrawInstruction::DrawPolygon { polygon: component.clone(), origin: Point2::new(0.0, 0.0) });
    }
}

use std::{
    ops::{Add, AddAssign, Deref, DerefMut, Sub, SubAssign},
    sync::atomic::Ordering,
};

use atomic_float::AtomicF32;
use bina_ecs::{
    component::{AtomicNumber, Component, NumberField, NumberFieldRef, Processable},
    triomphe::Arc,
};
use bytemuck::{Pod, Zeroable};
use image::Rgba;
use lyon::{
    lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers},
    math::point,
    path::traits::PathBuilder,
};
use wgpu::util::DeviceExt;

use crate::{
    drawing::{DrawInstruction, DrawPolygon},
    texture::Texture,
    Graphics,
};

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

pub struct Polygon {
    pub(crate) inner: Arc<PolygonInner>,
    origin: NumberField<Vector>,
    z: NumberField<u32>,
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
            tessellator
                .tessellate_path(
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
                )
                .unwrap();
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
                    },
                ),
                material,
                indices_count: geometry.indices.len() as u32,
            }),
            origin: NumberField::new(Vector::new(0.0, 0.0)),
            z: NumberField::new(0),
        }
    }
}

impl Component for Polygon {
    type Reference<'a> = PolygonRef<'a>;

    fn get_ref<'a>(&'a self) -> Self::Reference<'a> {
        PolygonRef {
            inner: &self.inner,
            origin: self.origin.get_ref(),
            z: self.z.get_ref(),
        }
    }
}

impl Processable for Polygon {
    fn process<E: bina_ecs::entity::Entity>(
        component: Self::Reference<'_>,
        _my_entity: bina_ecs::entity::EntityReference<E>,
        universe: &bina_ecs::universe::Universe,
    ) {
        let graphics = unsafe { universe.try_get_singleton::<Graphics>().unwrap_unchecked() };
        graphics.queue_draw_instruction(DrawInstruction::DrawPolygon(DrawPolygon {
            polygon: component.inner.clone(),
            origin: *component.origin,
            z: *component.z,
        }));
    }
}

#[derive(Clone, Copy)]
pub struct Vector(lyon::math::Vector);

impl Deref for Vector {
    type Target = lyon::math::Vector;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Vector {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Add for Vector {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Vector {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl AddAssign for Vector {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Vector {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl From<[f32; 2]> for Vector {
    fn from(value: [f32; 2]) -> Self {
        Self::new(value[0], value[1])
    }
}

impl AtomicNumber for Vector {
    type Atomic = [AtomicF32; 2];

    fn new_atomic(value: Self) -> Self::Atomic {
        [AtomicF32::new(value.x), AtomicF32::new(value.y)]
    }

    fn load(atomic: &mut Self::Atomic) -> Self {
        Self(lyon::math::Vector::new(
            *atomic[0].get_mut(),
            *atomic[1].get_mut(),
        ))
    }

    fn store(atomic: &Self::Atomic, other: Self) {
        atomic[0].store(other.x, Ordering::Relaxed);
        atomic[1].store(other.y, Ordering::Relaxed);
    }

    fn atomic_add_assign(atomic: &Self::Atomic, other: Self) {
        atomic[0].fetch_add(other.x, Ordering::Relaxed);
        atomic[1].fetch_add(other.y, Ordering::Relaxed);
    }

    fn atomic_sub_assign(atomic: &Self::Atomic, other: Self) {
        atomic[0].fetch_sub(other.x, Ordering::Relaxed);
        atomic[1].fetch_sub(other.y, Ordering::Relaxed);
    }

    fn atomic_mul_assign(_atomic: &Self::Atomic, _other: Self) {
        unimplemented!()
    }

    fn atomic_div_assign(_atomic: &Self::Atomic, _other: Self) {
        unimplemented!()
    }
}

impl Vector {
    pub fn new(x: f32, y: f32) -> Self {
        Self(lyon::math::Vector::new(x, y))
    }
}

pub struct PolygonRef<'a> {
    inner: &'a Arc<PolygonInner>,
    pub origin: NumberFieldRef<'a, Vector>,
    pub z: NumberFieldRef<'a, u32>,
}

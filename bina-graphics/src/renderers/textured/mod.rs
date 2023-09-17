use std::hint::unreachable_unchecked;

use wgpu::{BindGroupLayout, Device, RenderPass, RenderPipeline, SurfaceConfiguration, BindGroup};

use crate::polygon::{Material, TEXTURE_VERTEX_BUFFER_DESCRIPTOR};

use super::{BindGroupTracker, DrawPolygon};

pub(crate) struct TexturedPolygonRenderer {
    buffer: Vec<DrawPolygon>,
    render_pipeline: RenderPipeline,
}

impl TexturedPolygonRenderer {
    pub(crate) fn new(device: &Device, config: &SurfaceConfiguration, transform_bind_group_layout: &BindGroupLayout, camera_bind_group_layout: &BindGroupLayout) -> (Self, BindGroupLayout) {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    transform_bind_group_layout,
                    camera_bind_group_layout
                ],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",                       // 1.
                buffers: &[TEXTURE_VERTEX_BUFFER_DESCRIPTOR], // 2.
            },
            fragment: Some(wgpu::FragmentState {
                // 3.
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    // 4.
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw, // 2.
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None, // 1.
            multisample: wgpu::MultisampleState {
                count: 1,                         // 2.
                mask: !0,                         // 3.
                alpha_to_coverage_enabled: false, // 4.
            },
            multiview: None,
        });

        (
            Self {
                buffer: Default::default(),
                render_pipeline,
            },
            texture_bind_group_layout,
        )
    }

    pub(super) unsafe fn push(&mut self, polygon: DrawPolygon) {
        self.buffer.push(polygon);
    }

    pub(super) fn draw_all<'a>(&'a mut self, render_pass: &mut RenderPass<'a>, camera_matrix_buffer_bind_group: &'a BindGroup) {
        render_pass.set_pipeline(&self.render_pipeline);
        let mut bind_grp_tracker = BindGroupTracker::new(0);

        for DrawPolygon {
            polygon,
            ..
        } in &self.buffer
        {
            let Material::Texture(texture) = &polygon.material else {
                unsafe { unreachable_unchecked() }
            };

            bind_grp_tracker.set_bind_group(render_pass, &texture.texture.bind_group);
            render_pass.set_bind_group(1, &polygon.transform_bind_group, &[]);
            render_pass.set_bind_group(2, camera_matrix_buffer_bind_group, &[]);
            render_pass.set_vertex_buffer(0, polygon.vertices.slice(..));
            render_pass.set_index_buffer(polygon.indices.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..polygon.indices_count, 0, 0..1);
        }
    }

    pub(super) fn clear(&mut self) {
        self.buffer.clear();
    }
}

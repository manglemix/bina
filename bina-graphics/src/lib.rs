#![feature(associated_type_bounds)]
use std::{error::Error, sync::OnceLock};

use bina_ecs::{
    parking_lot::Mutex,
    rayon,
    triomphe::{self, Arc},
};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

pub mod raw_img;
pub use image;
pub mod texture;

struct Config {
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

struct GraphicsInner {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: Mutex<Config>,
    // The window must be declared after the surface so
    // it gets dropped after it as the surface contains
    // unsafe references to the window's resources.
    window: Window,
    // render_pipeline: wgpu::RenderPipeline,
}

pub struct Graphics(triomphe::Arc<GraphicsInner>);

impl Graphics {
    /// Creates a new GUI immediately
    pub async fn run(
        f: impl FnOnce(Self) -> Result<(), Box<dyn Error + Send + Sync>> + Send + 'static,
    ) -> ! {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().build(&event_loop).unwrap();

        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // let render_pipeline_layout =
        //     device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        //         label: Some("Render Pipeline Layout"),
        //         bind_group_layouts: &[],
        //         push_constant_ranges: &[],
        //     });
        // let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        //     label: Some("Render Pipeline"),
        //     layout: Some(&render_pipeline_layout),
        //     vertex: wgpu::VertexState {
        //         module: &shader,
        //         entry_point: "vs_main", // 1.
        //         buffers: &[],           // 2.
        //     },
        //     fragment: Some(wgpu::FragmentState {
        //         // 3.
        //         module: &shader,
        //         entry_point: "fs_main",
        //         targets: &[Some(wgpu::ColorTargetState {
        //             // 4.
        //             format: config.format,
        //             blend: Some(wgpu::BlendState::REPLACE),
        //             write_mask: wgpu::ColorWrites::ALL,
        //         })],
        //     }),
        //     primitive: wgpu::PrimitiveState {
        //         topology: wgpu::PrimitiveTopology::TriangleList, // 1.
        //         strip_index_format: None,
        //         front_face: wgpu::FrontFace::Ccw, // 2.
        //         cull_mode: Some(wgpu::Face::Back),
        //         // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
        //         polygon_mode: wgpu::PolygonMode::Fill,
        //         // Requires Features::DEPTH_CLIP_CONTROL
        //         unclipped_depth: false,
        //         // Requires Features::CONSERVATIVE_RASTERIZATION
        //         conservative: false,
        //     },
        //     depth_stencil: None, // 1.
        //     multisample: wgpu::MultisampleState {
        //         count: 1,                         // 2.
        //         mask: !0,                         // 3.
        //         alpha_to_coverage_enabled: false, // 4.
        //     },
        //     multiview: None,
        // });
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

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let graphics = Arc::new(GraphicsInner {
            surface,
            device,
            queue,
            config: Mutex::new(Config { config, size }),
            window,
            // render_pipeline,
        });

        let cloned = graphics.clone();
        let (exit_sender, mut exit_receiver) = bina_ecs::tokio::sync::oneshot::channel();

        rayon::spawn(move || {
            f(Graphics(cloned)).unwrap();
            let _ = exit_sender.send(0);
        });

        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::MainEventsCleared => {
                    if let Ok(n) = exit_receiver.try_recv() {
                        *control_flow = ControlFlow::ExitWithCode(n);
                        return;
                    }
                    let output = match graphics.surface.get_current_texture() {
                        Ok(x) => x,
                        Err(e) => match e {
                            wgpu::SurfaceError::Timeout => todo!(),
                            wgpu::SurfaceError::Outdated => todo!(),
                            wgpu::SurfaceError::Lost => {
                                let lock = graphics.config.lock();
                                graphics.surface.configure(&graphics.device, &lock.config);
                                return;
                            }
                            wgpu::SurfaceError::OutOfMemory => todo!(),
                        },
                    };
                    let view = output
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder =
                        graphics
                            .device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Render Encoder"),
                            });

                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Render Pass"),
                                color_attachments: &[
                                    // This is what @location(0) in the fragment shader targets
                                    Some(wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                                r: 0.1,
                                                g: 0.2,
                                                b: 0.3,
                                                a: 1.0,
                                            }),
                                            store: true,
                                        },
                                    }),
                                ],
                                depth_stencil_attachment: None,
                            });

                        // NEW!
                        // render_pass.set_pipeline(&graphics.render_pipeline); // 2.
                        // render_pass.draw(0..3, 0..1); // 3.
                    }

                    // submit will accept anything that implements IntoIter
                    graphics.queue.submit(std::iter::once(encoder.finish()));
                    output.present();
                }
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == graphics.window.id() => {
                    macro_rules! resize {
                        ($size: expr) => {
                            if $size.width > 0 && $size.height > 0 {
                                let mut lock = graphics.config.lock();
                                lock.size = $size;
                                lock.config.width = $size.width;
                                lock.config.height = $size.height;
                                graphics.surface.configure(&graphics.device, &lock.config);
                            }
                        };
                    }
                    match event {
                        WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            resize!(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            resize!(**new_inner_size);
                        }
                        WindowEvent::KeyboardInput { .. } => {}
                        _ => {}
                    }
                }
                _ => {}
            }
        });
    }
}

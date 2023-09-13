use std::{
    hint::unreachable_unchecked, marker::PhantomData, mem::MaybeUninit, ops::Deref, time::Duration,
};

use bina_ecs::{
    component::Component,
    crossbeam::atomic::AtomicCell,
    tokio::{
        self,
        fs::File,
        io::AsyncReadExt,
        sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
        time::Instant,
    },
    universe::Universe,
};
use image::{ImageBuffer, ImageFormat, Pixel, Rgba};
use wgpu::BindGroup;

use crate::Graphics;

pub(crate) struct TextureInner {
    // texture: wgpu::Texture,
    // view: wgpu::TextureView,
    // sampler: wgpu::Sampler,
    pub(crate) bind_group: BindGroup,
}

pub enum CacheOption {
    DontCache,
    UncacheAfter(Duration),
    CacheForever,
}

enum DataSource {
    Raw(&'static [u8]),
    File(
        &'static str,
        ImageFormat,
        CacheOption,
        AtomicCell<MaybeUninit<Instant>>,
    ),
}

struct SyncPhantom<T>(PhantomData<T>);

unsafe impl<T> Send for SyncPhantom<T> {}
unsafe impl<T> Sync for SyncPhantom<T> {}

enum MaybeTexture<P: Pixel> {
    Unloaded,
    Loaded(ImageBuffer<P, Box<[u8]>>),
    Processed(TextureInner),
}

pub struct TextureResource<P: Pixel + Send, const W: u32, const H: u32> {
    data_source: DataSource,
    texture: RwLock<MaybeTexture<P>>,
    _phantom: SyncPhantom<P>,
}

static_assertions::assert_impl_all!(TextureResource<Rgba<u8>, 0, 0>: Sync);

pub struct Texture {
    pub(crate) texture: RwLockReadGuard<'static, TextureInner>,
}

static_assertions::assert_impl_all!(Texture: Send, Sync);

fn load_img<const W: u32, const H: u32>(graphics: &Graphics, img: &[u8]) -> TextureInner {
    let texture_size = wgpu::Extent3d {
        width: W,
        height: H,
        depth_or_array_layers: 1,
    };

    let texture = graphics
        .inner
        .device
        .create_texture(&wgpu::TextureDescriptor {
            // All textures are stored as 3D, we represent our 2D texture
            // by setting depth to 1.
            size: texture_size,
            mip_level_count: 1, // We'll talk about this a little later
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Most images are stored using sRGB so we need to reflect that here.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
            // COPY_DST means that we want to copy data to this texture
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("diffuse_texture"),
            // This is the same as with the SurfaceConfig. It
            // specifies what texture formats can be used to
            // create TextureViews for this texture. The base
            // texture format (Rgba8UnormSrgb in this case) is
            // always supported. Note that using a different
            // texture format is not supported on the WebGL2
            // backend.
            view_formats: &[],
        });

    graphics.inner.queue.write_texture(
        // Tells wgpu where to copy the pixel data
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        // The actual pixel data
        &img,
        // The layout of the texture
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * W),
            rows_per_image: Some(H),
        },
        texture_size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = graphics
        .inner
        .device
        .create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

    let bind_group = graphics
        .inner
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &graphics.inner.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("texture_bind_group"),
        });

    TextureInner {
        // texture,
        // view,
        // sampler,
        bind_group,
    }
}

impl<const W: u32, const H: u32> TextureResource<Rgba<u8>, W, H> {
    pub const unsafe fn new_file(
        path: &'static str,
        img_format: ImageFormat,
        cache_option: CacheOption,
    ) -> Self {
        Self {
            data_source: DataSource::File(
                path,
                img_format,
                cache_option,
                AtomicCell::new(MaybeUninit::uninit()),
            ),
            texture: RwLock::const_new(MaybeTexture::Unloaded),
            _phantom: SyncPhantom(PhantomData),
        }
    }

    pub const unsafe fn new_raw(raw: &'static [u8]) -> Self {
        Self {
            data_source: DataSource::Raw(raw),
            texture: RwLock::const_new(MaybeTexture::Unloaded),
            _phantom: SyncPhantom(PhantomData),
        }
    }

    pub fn try_get(&'static self, universe: &Universe, graphics: &Graphics) -> Option<Texture> {
        // # Safety
        // The current texture must be processed
        let return_ref = |read: RwLockReadGuard<'static, MaybeTexture<Rgba<u8>>>| {
            let texture = RwLockReadGuard::map(read, |x| {
                let MaybeTexture::Processed(inner) = x else {
                    unsafe { unreachable_unchecked() }
                };
                inner
            });
            Some(Texture { texture })
        };

        let read = self.texture.try_read().ok()?;
        match read.deref() {
            MaybeTexture::Unloaded => {
                if let DataSource::Raw(data) = &self.data_source {
                    drop(read);
                    let mut write = self.texture.blocking_write();
                    let MaybeTexture::Unloaded = write.deref() else {
                        // If it is not unloaded, and data source is raw,
                        // the only other possibility is that the texture is processed
                        let read = RwLockWriteGuard::downgrade(write);
                        return return_ref(read);
                    };
                    let img = unsafe {
                        ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(W, H, *data).unwrap_unchecked()
                    };
                    let inner = load_img::<W, H>(graphics, &img);
                    *write = MaybeTexture::Processed(inner);
                    let read = RwLockWriteGuard::downgrade(write);
                    return return_ref(read);
                }

                let _guard = universe.enter_tokio();
                tokio::spawn(async {
                    let mut write = self.texture.write().await;
                    let MaybeTexture::Unloaded = write.deref() else {
                        return;
                    };
                    let DataSource::File(path, img_format, cache_option, last_access) =
                        &self.data_source
                    else {
                        unsafe { unreachable_unchecked() }
                    };

                    let Ok(mut file) = File::open(path).await else {
                        todo!("Unreadable")
                    };
                    let mut buf = Vec::with_capacity(W as usize * H as usize);
                    if file.read_to_end(&mut buf).await.is_err() {
                        todo!("Unreadable")
                    }
                    let img = unsafe {
                        image::load_from_memory_with_format(&buf, *img_format).unwrap_unchecked()
                    };
                    let img = img.to_rgba8();
                    let data = img.into_raw().into_boxed_slice();
                    let img = unsafe { ImageBuffer::from_raw(W, H, data).unwrap_unchecked() };
                    *write = MaybeTexture::Loaded(img);
                    last_access.store(MaybeUninit::new(Instant::now()));
                    drop(write);

                    if let CacheOption::UncacheAfter(duration) = cache_option {
                        let mut last_instant = unsafe { last_access.load().assume_init() };
                        let mut deadline = last_instant + *duration;
                        loop {
                            tokio::time::sleep_until(deadline).await;
                            let current_instant = unsafe { last_access.load().assume_init() };
                            if current_instant == last_instant {
                                break;
                            }
                            last_instant = current_instant;
                            deadline = last_instant + *duration;
                        }
                        write = self.texture.write().await;
                        *write = MaybeTexture::Unloaded;
                    }
                });
                return None;
            }
            MaybeTexture::Loaded(_) => {
                drop(read);
                let mut write = self.texture.blocking_write();
                let MaybeTexture::Loaded(img) = write.deref() else {
                    drop(write);
                    return self.try_get(universe, graphics);
                };
                let inner = load_img::<W, H>(graphics, &img);
                *write = MaybeTexture::Processed(inner);
                let read = RwLockWriteGuard::downgrade(write);

                let DataSource::File(_, _, cache_option, _) = &self.data_source else {
                    unsafe { unreachable_unchecked() }
                };

                if let CacheOption::DontCache = cache_option {
                    let _guard = universe.enter_tokio();
                    tokio::spawn(async {
                        *self.texture.write().await = MaybeTexture::Unloaded;
                    });
                }

                return return_ref(read);
            }
            MaybeTexture::Processed(_) => return return_ref(read),
        }
    }
}

impl Component for Texture {
    fn get_ref<'a>(&'a self) -> Self::Reference<'a> {
        self
    }
}

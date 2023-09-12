use std::{hint::unreachable_unchecked, marker::PhantomData, time::Duration};

use bina_ecs::{
    crossbeam::atomic::AtomicCell,
    tokio::{
        self,
        fs::File,
        io::AsyncReadExt,
        sync::{RwLock, RwLockReadGuard},
    },
    universe::Universe,
};
use image::{ImageBuffer, ImageFormat, Pixel, Rgba};

use crate::Graphics;

struct TextureInner {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

pub enum CacheOption {
    DontCache,
    UncacheAfter(Duration),
    CacheForever,
}

enum DataSource<P: Pixel> {
    Raw(&'static [u8]),
    File(
        &'static str,
        ImageFormat,
        CacheOption,
        AtomicCell<Option<ImageBuffer<P, Vec<u8>>>>,
    ),
}

struct SyncPhantom<T>(PhantomData<T>);

unsafe impl<T> Send for SyncPhantom<T> {}
unsafe impl<T> Sync for SyncPhantom<T> {}

pub struct Texture<P: Pixel + Send, const W: u32, const H: u32> {
    data_source: DataSource<P>,
    texture: RwLock<Option<TextureInner>>,
    _phantom: SyncPhantom<P>,
}

static_assertions::assert_impl_all!(Texture<Rgba<u8>, 0, 0>: Sync);

pub struct TextureReference {
    rw_lock: &'static RwLock<Option<TextureInner>>,
    lock: RwLockReadGuard<'static, TextureInner>,
}

static_assertions::assert_impl_all!(TextureReference: Send, Sync);

impl Clone for TextureReference {
    fn clone(&self) -> Self {
        Self {
            rw_lock: self.rw_lock,
            lock: RwLockReadGuard::map(self.rw_lock.blocking_read(), |x| unsafe {
                x.as_ref().unwrap_unchecked()
            }),
        }
    }
}

impl<const W: u32, const H: u32> Texture<Rgba<u8>, W, H> {
    pub const unsafe fn new_file(path: &'static str, img_format: ImageFormat, cache_option: CacheOption) -> Self {
        Self {
            data_source: DataSource::File(path, img_format, cache_option, AtomicCell::new(None)),
            texture: RwLock::new(None),
            _phantom: SyncPhantom(PhantomData),
        }
    }

    pub const unsafe fn new_raw(raw: &'static [u8]) -> Self {
        Self {
            data_source: DataSource::Raw(raw),
            texture: RwLock::new(None),
            _phantom: SyncPhantom(PhantomData),
        }
    }

    pub fn try_get(
        &'static self,
        universe: &Universe,
        graphics: &Graphics,
    ) -> Option<TextureReference> {
        let read = self.texture.try_read().ok()?;
        if read.is_none() {
            match &self.data_source {
                DataSource::Raw(data) => unsafe {
                    let img = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(W, H, *data).unwrap_unchecked();
                    
                }
                DataSource::File(x, _, _, _) => todo!(),
            }
            let _guard = universe.enter_tokio();
            tokio::spawn(async {
                let writer = self.texture.write().await;
                if writer.is_none() {
                    return;
                }
                let DataSource::File(path, img_format, cache_option, loaded) = &self.data_source else {
                    unsafe { unreachable_unchecked() }
                };
                let Ok(file) = File::open(path).await else {
                    todo!("Unreadable")
                };
                let mut buf = Vec::with_capacity(W as usize * H as usize);
                file.read_to_end(&mut buf).await;
                unsafe {
                    let img = image::load_from_memory_with_format(
                        &buf,
                        img_format,
                    )
                    .unwrap_unchecked();
                    loaded.store(Some(img.to_rgba8()));
                }
            });
            return None;
        }
        let lock = RwLockReadGuard::map(read, |x| unsafe { x.as_ref().unwrap_unchecked() });
        Some(TextureReference {
            rw_lock: &self.texture,
            lock,
        })
    }
}

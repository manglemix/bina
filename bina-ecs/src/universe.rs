use std::{
    any::TypeId,
    cell::SyncUnsafeCell,
    collections::hash_map::Entry,
    error::Error,
    time::{Duration, Instant},
};

use crossbeam::atomic::AtomicCell;
use fxhash::FxHashMap;
use parking_lot::Mutex;
use rayon::{
    join,
    prelude::{
        IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator,
        ParallelIterator,
    },
};
use spin_sleep::SpinSleeper;
use tokio::runtime::Handle;

use crate::{
    entity::{
        cast_entity_buffer, Entity, EntityBuffer, EntityBufferStruct, EntityReference, MaybeEntity,
    },
    singleton::Singleton,
};

#[derive(Default)]
struct BetterUnsafeCell<T>(SyncUnsafeCell<T>);

impl<T> BetterUnsafeCell<T> {
    unsafe fn get(&self) -> &T {
        &*self.0.get()
    }
    unsafe fn get_mut(&self) -> &mut T {
        &mut *self.0.get()
    }
    fn safe_get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }
}

pub struct Universe {
    entity_buffers: BetterUnsafeCell<FxHashMap<TypeId, Box<dyn EntityBuffer>>>,
    pending_new_entity_buffers: Mutex<FxHashMap<TypeId, Box<dyn EntityBuffer>>>,

    singletons: BetterUnsafeCell<FxHashMap<TypeId, Box<dyn Singleton>>>,
    pending_new_singletons: Mutex<FxHashMap<TypeId, Box<dyn Singleton>>>,

    exit_result: AtomicCell<Option<Result<(), Box<dyn Error + Send + Sync>>>>,
    async_handle: Option<Handle>,
    delta_accurate: f64,
    delta: f32,
}

impl Universe {
    /// Creates a new Universe that is ready for immediate use
    ///
    /// If called from within a tokio runtime, a handle to the runtime
    /// will be stored
    pub fn new() -> Self {
        Self {
            entity_buffers: Default::default(),
            pending_new_entity_buffers: Default::default(),
            singletons: Default::default(),
            pending_new_singletons: Default::default(),
            exit_result: Default::default(),
            async_handle: Handle::try_current().ok(),
            delta_accurate: Default::default(),
            delta: Default::default(),
        }
    }

    pub fn queue_add_entity<E: Entity>(&self, entity: E) {
        let type_id = TypeId::of::<EntityBufferStruct<E>>();
        let mut lock;
        let entry;

        let buffer = if let Some(buffer) = unsafe { self.entity_buffers.get() }.get(&type_id) {
            buffer
        } else {
            lock = self.pending_new_entity_buffers.lock();
            match lock.entry(type_id) {
                Entry::Occupied(x) => {
                    entry = x;
                    entry.get()
                }
                Entry::Vacant(x) => x.insert(Box::new(EntityBufferStruct::<E>::new())),
            }
        };

        let buffer: &EntityBufferStruct<E> = unsafe { cast_entity_buffer(&buffer) };
        buffer.queue_add_entity(entity);
    }

    pub fn iter_entities<E: Entity>(&self) -> Option<impl IndexedParallelIterator + '_> {
        unsafe {
            self.entity_buffers
                .get()
                .get(&TypeId::of::<EntityBufferStruct<E>>())
                .map(|buffer| {
                    let buffer: &EntityBufferStruct<E> = cast_entity_buffer(buffer);
                    buffer.par_iter()
                })
        }
    }

    pub fn queue_remove_entity<E: MaybeEntity>(&self, reference: EntityReference<E>) {
        unsafe {
            self.entity_buffers
                .get()
                .get(&E::get_buffer_type())
                .map(|buffer| buffer.queue_remove_entity(reference.index))
        };
    }

    /// Gets a singleton
    ///
    /// # Panics
    /// Panics if the singleton does not exist. Use `try_get_singleton` for a
    /// non-panicking version
    pub fn get_singleton<T: Singleton>(&self) -> &T {
        self.try_get_singleton()
            .expect("Singleton should be initialized")
    }

    /// Gets a singleton if it exists
    pub fn try_get_singleton<T: Singleton>(&self) -> Option<&T> {
        unsafe {
            self.singletons.get().get(&TypeId::of::<T>()).map(|x| {
                let ptr: *const T = x.get_void_ptr().cast();
                &*ptr
            })
        }
    }

    /// Adds a new singleton, or overwrites and existing singleton
    pub fn queue_set_singleton<T: Singleton>(&self, singleton: T) {
        self.pending_new_singletons
            .lock()
            .insert(TypeId::of::<T>(), Box::new(singleton));
    }

    /// If this universe was initialized without a tokio runtime,
    /// one can be added with this method
    ///
    /// # Panics
    /// This will panic if called outside of a tokio runtime
    pub fn init_tokio(&mut self) {
        self.async_handle = Some(Handle::current());
    }

    pub fn enter_tokio(&self) -> tokio::runtime::EnterGuard {
        self.async_handle.as_ref().unwrap().enter()
    }

    pub fn exit_ok(&self) {
        self.exit_result.store(Some(Ok(())));
    }

    pub fn exit_err(&self, e: impl Error + Send + Sync + 'static) {
        self.exit_result.store(Some(Err(Box::new(e))));
    }

    pub fn loop_once(&mut self) -> Option<Result<(), Box<dyn Error + Send + Sync>>> {
        join(
            // Process all entities
            || unsafe {
                self.entity_buffers
                    .get()
                    .par_iter()
                    .for_each(|(_, x)| x.process(self))
            },
            // Process all singletons
            || unsafe {
                self.singletons
                    .get()
                    .par_iter()
                    .for_each(|(_, x)| x.process(self))
            },
        );

        if let Some(result) = self.exit_result.take() {
            return Some(result);
        }

        join(
            // Flush entity buffers
            || unsafe {
                self.entity_buffers
                    .get_mut()
                    .par_iter_mut()
                    .for_each(|(_, x)| x.flush(self))
            },
            // Flush singletons
            || unsafe {
                self.singletons
                    .get_mut()
                    .par_iter_mut()
                    .for_each(|(_, x)| x.flush(self))
            },
        );

        join(
            // Add/replace singletons
            || {
                self.singletons
                    .safe_get_mut()
                    .extend(self.pending_new_singletons.get_mut().drain())
            },
            // Add new entity buffers
            || {
                self.entity_buffers
                    .safe_get_mut()
                    .extend(self.pending_new_entity_buffers.get_mut().drain())
            },
        );

        None
    }

    pub fn get_delta(&self) -> f32 {
        self.delta
    }

    pub fn get_delta_accurate(&self) -> f64 {
        self.delta_accurate
    }

    pub fn loop_many(
        &mut self,
        count: LoopCount,
        delta: DeltaStrategy,
    ) -> Option<Result<(), Box<dyn Error + Send + Sync>>> {
        macro_rules! loop_once {
            () => {
                if let Some(result) = self.loop_once() {
                    return Some(result);
                }
            };
        }
        let LoopCount::Count(n) = count else {
            match delta {
                DeltaStrategy::FakeDelta(delta) => loop {
                    loop_once!();
                    self.delta_accurate = delta.as_secs_f64();
                    self.delta = delta.as_secs_f32();
                },
                DeltaStrategy::RealDelta(delta) => {
                    let start = Instant::now();
                    let mut last_duration = Duration::ZERO;
                    if delta.is_zero() {
                        loop {
                            loop_once!();
                            let current_duration = start.elapsed();
                            self.delta_accurate = (current_duration - last_duration).as_secs_f64();
                            self.delta = self.delta_accurate as f32;
                            last_duration = current_duration
                        }
                    } else {
                        let sleeper = SpinSleeper::default();
                        loop {
                            loop_once!();
                            sleeper.sleep(delta.saturating_sub(start.elapsed() - last_duration));
                            let current_duration = start.elapsed();
                            self.delta_accurate = (current_duration - last_duration).as_secs_f64();
                            self.delta = self.delta_accurate as f32;
                            last_duration = current_duration
                        }
                    }
                }
            }
        };

        match delta {
            DeltaStrategy::FakeDelta(delta) => {
                for _i in 0..n {
                    loop_once!();
                    self.delta_accurate = delta.as_secs_f64();
                    self.delta = delta.as_secs_f32();
                }
            }
            DeltaStrategy::RealDelta(delta) => {
                let start = Instant::now();
                let mut last_duration = Duration::ZERO;
                if delta.is_zero() {
                    for _i in 0..n {
                        loop_once!();
                        let current_duration = start.elapsed();
                        self.delta_accurate = (current_duration - last_duration).as_secs_f64();
                        self.delta = self.delta_accurate as f32;
                        last_duration = current_duration
                    }
                } else {
                    let sleeper = SpinSleeper::default();
                    for _i in 0..n {
                        loop_once!();
                        sleeper.sleep(delta.saturating_sub(start.elapsed() - last_duration));
                        let current_duration = start.elapsed();
                        self.delta_accurate = (current_duration - last_duration).as_secs_f64();
                        self.delta = self.delta_accurate as f32;
                        last_duration = current_duration
                    }
                }
            }
        }

        None
    }
}

pub enum LoopCount {
    Forever,
    Count(usize),
}

pub enum DeltaStrategy {
    FakeDelta(Duration),
    RealDelta(Duration),
}

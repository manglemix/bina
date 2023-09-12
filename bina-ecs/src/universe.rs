use std::{
    any::{Any, TypeId},
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

use crate::entity::{
    cast_entity_buffer, Entity, EntityBuffer, EntityBufferStruct, EntityReference,
};

pub struct Universe {
    entity_buffers: FxHashMap<TypeId, Box<dyn EntityBuffer>>,
    pending_new_entity_buffers: Mutex<FxHashMap<TypeId, Box<dyn EntityBuffer>>>,

    singletons: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
    pending_new_singletons: Mutex<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>,

    exit_result: AtomicCell<Option<Result<(), Box<dyn Error + Send + Sync>>>>,
    async_handle: Handle,
    delta_accurate: f64,
    delta: f32,
}

impl Universe {
    /// Creates a new Universe that is ready for immediate use
    ///
    /// # Panics
    /// Panics if called from a thread that does not have a tokio runtime
    pub fn new() -> Self {
        Self {
            entity_buffers: Default::default(),
            pending_new_entity_buffers: Default::default(),
            singletons: Default::default(),
            pending_new_singletons: Default::default(),
            exit_result: Default::default(),
            async_handle: Handle::current(),
            delta_accurate: Default::default(),
            delta: Default::default(),
        }
    }

    pub fn queue_add_entity<E: Entity>(&self, entity: E) {
        let type_id = TypeId::of::<EntityBufferStruct<E>>();
        let mut lock;
        let entry;

        let buffer = if let Some(buffer) = self.entity_buffers.get(&type_id) {
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
        self.entity_buffers
            .get(&TypeId::of::<EntityBufferStruct<E>>())
            .map(|buffer| {
                let buffer: &EntityBufferStruct<E> = unsafe { cast_entity_buffer(buffer) };
                buffer.par_iter()
            })
    }

    pub fn queue_remove_entity<E: Entity>(&self, reference: EntityReference<E>) {
        self.entity_buffers
            .get(&TypeId::of::<EntityBufferStruct<E>>())
            .map(|buffer| buffer.queue_remove_entity(reference.index));
    }

    /// Gets a singleton
    ///
    /// # Panics
    /// Panics if the singleton does not exist. Use `try_get_singleton` for a
    /// non-panicking version
    pub fn get_singleton<T: Send + Sync + 'static>(&self) -> &T {
        self.try_get_singleton()
            .expect("Singleton should be initialized")
    }

    /// Gets a singleton if it exists
    pub fn try_get_singleton<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.singletons
            .get(&TypeId::of::<T>())
            .map(|x| unsafe { x.downcast_ref_unchecked() })
    }

    /// Adds a new singleton, or overwrites and existing singleton
    pub fn queue_set_singleton<T: Send + Sync + 'static>(&self, singleton: T) {
        self.pending_new_singletons
            .lock()
            .insert(TypeId::of::<T>(), Box::new(singleton));
    }

    #[must_use = "Enter guard is only useful while not dropped"]
    pub fn enter_tokio(&self) -> tokio::runtime::EnterGuard {
        self.async_handle.enter()
    }

    pub fn exit_ok(&self) {
        self.exit_result.store(Some(Ok(())));
    }

    pub fn exit_err(&self, e: impl Error + Send + Sync + 'static) {
        self.exit_result.store(Some(Err(Box::new(e))));
    }

    pub fn loop_once(&mut self) -> Option<Result<(), Box<dyn Error + Send + Sync>>> {
        // Process all entities
        self.entity_buffers
            .par_iter()
            .for_each(|(_, x)| x.process(self));

        if let Some(result) = self.exit_result.take() {
            return Some(result);
        }

        join(
            // Add/replace singletons
            || {
                self.singletons
                    .extend(self.pending_new_singletons.get_mut().drain())
            },
            // Add new entity buffers
            || {
                self.entity_buffers
                    .extend(self.pending_new_entity_buffers.get_mut().drain())
            },
        );

        // Flush entity buffers
        self.entity_buffers
            .par_iter_mut()
            .for_each(|(_, x)| x.flush());

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
                    let sleeper = SpinSleeper::default();
                    loop {
                        let start = Instant::now();
                        loop_once!();
                        sleeper.sleep(delta.saturating_sub(start.elapsed()));
                        self.delta_accurate = start.elapsed().as_secs_f64();
                        self.delta = self.delta_accurate as f32;
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
                let sleeper = SpinSleeper::default();
                for _i in 0..n {
                    let start = Instant::now();
                    loop_once!();
                    sleeper.sleep(delta.saturating_sub(start.elapsed()));
                    self.delta_accurate = start.elapsed().as_secs_f64();
                    self.delta = self.delta_accurate as f32;
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

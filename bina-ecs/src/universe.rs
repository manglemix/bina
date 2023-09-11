use std::{any::{TypeId, Any}, time::{Duration, Instant}, error::Error, marker::PhantomData, ops::Deref, collections::hash_map::Entry};

use crossbeam::atomic::AtomicCell;
use dashmap::{DashMap, mapref::{one::Ref, self}};
use fxhash::{FxHashMap, FxBuildHasher};
use parking_lot::Mutex;
use rayon::prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator, IndexedParallelIterator};
use spin_sleep::SpinSleeper;

use crate::entity::{cast_entity_buffer, Entity, EntityBuffer, EntityBufferStruct, EntityReference};

#[derive(Default)]
pub struct Universe {
    entity_buffers: FxHashMap<TypeId, Box<dyn EntityBuffer>>,
    pending_new_entity_buffers: Mutex<FxHashMap<TypeId, Box<dyn EntityBuffer>>>,
    singletons: DashMap<TypeId, Box<dyn Any + Send + Sync>, FxBuildHasher>,
    exit_result: AtomicCell<Option<Result<(), Box<dyn Error + Send + Sync>>>>,
    delta_accurate: f64,
    delta: f32
}

impl Universe {
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
        self.entity_buffers.get(&TypeId::of::<EntityBufferStruct<E>>())
            .map(|buffer| {
                let buffer: &EntityBufferStruct<E> = unsafe { cast_entity_buffer(buffer) };
                buffer.par_iter()
            })
    }

    pub fn queue_remove_entity<E: Entity>(&self, reference: EntityReference<E>) {
        self.entity_buffers.get(&TypeId::of::<EntityBufferStruct<E>>())
            .map(|buffer|
                buffer.queue_remove_entity(reference.index)
            );
    }

    pub fn get_singleton<T: Default + Send + Sync + 'static>(&self) -> Singleton<T> {
        let type_id = TypeId::of::<T>();
        let lock = self.singletons.get(&type_id).unwrap_or_else(|| {
            match self.singletons.entry(type_id) {
                mapref::entry::Entry::Occupied(x) => x.into_ref(),
                mapref::entry::Entry::Vacant(x) => x.insert(Box::new(T::default())),
            }.downgrade()
        });
        Singleton { lock, _phantom: PhantomData }
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
            return Some(result)
        }

        // Add new entity buffers
        self.entity_buffers
            .extend(self.pending_new_entity_buffers.get_mut().drain());

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

    pub fn loop_many(&mut self, count: LoopCount, delta: DeltaStrategy) -> Option<Result<(), Box<dyn Error + Send + Sync>>> {
        macro_rules! loop_once {
            () => {
                if let Some(result) = self.loop_once() {
                    return Some(result)
                }
            };
        }
        let LoopCount::Count(n) = count else {
            match delta {
                DeltaStrategy::FakeDelta(delta) =>
                    loop {
                        loop_once!();
                        self.delta_accurate = delta.as_secs_f64();
                        self.delta = delta.as_secs_f32();
                    }
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
            DeltaStrategy::FakeDelta(delta) =>
                for _i in 0..n {
                    loop_once!();
                    self.delta_accurate = delta.as_secs_f64();
                    self.delta = delta.as_secs_f32();
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
    RealDelta(Duration)
}

pub struct Singleton<'a, T> {
    lock: Ref<'a, TypeId, Box<dyn Any + Send + Sync + 'static>, FxBuildHasher>,
    _phantom: PhantomData<&'a T>
}


impl<'a, T: 'static> Deref for Singleton<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.lock.downcast_ref_unchecked() }
    }
}

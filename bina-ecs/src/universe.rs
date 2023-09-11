use std::{any::TypeId, collections::hash_map::Entry, time::{Duration, Instant}, error::Error};

use crossbeam::atomic::AtomicCell;
use fxhash::FxHashMap;
use parking_lot::Mutex;
use rayon::prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator, IndexedParallelIterator};
use spin_sleep::SpinSleeper;

use crate::entity::{cast_entity_buffer, Entity, EntityBuffer, EntityBufferStruct, EntityReference};

#[derive(Default)]
pub struct Universe {
    entity_buffers: FxHashMap<TypeId, Box<dyn EntityBuffer>>,
    pending_new_entity_buffers: Mutex<FxHashMap<TypeId, Box<dyn EntityBuffer>>>,
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

    pub fn loop_many(&mut self, count: LoopCount, min_delta: Duration) -> Option<Result<(), Box<dyn Error + Send + Sync>>> {
        macro_rules! loop_once {
            () => {
                if let Some(result) = self.loop_once() {
                    return Some(result)
                }
            };
        }
        let LoopCount::Count(n) = count else {
            if min_delta.is_zero() {
                loop {
                    let start = Instant::now();
                    loop_once!();
                    self.delta_accurate = start.elapsed().as_secs_f64();
                    self.delta = self.delta_accurate as f32;
                }
            } else {
                let sleeper = SpinSleeper::default();
                loop {
                    let start = Instant::now();
                    loop_once!();
                    sleeper.sleep(min_delta);
                    self.delta_accurate = start.elapsed().as_secs_f64();
                    self.delta = self.delta_accurate as f32;
                }
            }
        };

        if min_delta.is_zero() {
            for _i in 0..n {
                let start = Instant::now();
                loop_once!();
                self.delta_accurate = start.elapsed().as_secs_f64();
                self.delta = self.delta_accurate as f32;
            }
        } else {
            let sleeper = SpinSleeper::default();
            for _i in 0..n {
                let start = Instant::now();
                loop_once!();
                sleeper.sleep(min_delta);
                self.delta_accurate = start.elapsed().as_secs_f64();
                self.delta = self.delta_accurate as f32;
            }
        }

        None
    }
}

pub enum LoopCount {
    Forever,
    Count(usize),
}

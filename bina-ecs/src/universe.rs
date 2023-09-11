use std::{any::TypeId, collections::hash_map::Entry, time::Duration};

use fxhash::FxHashMap;
use parking_lot::Mutex;
use rayon::prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use spin_sleep::SpinSleeper;

use crate::entity::{cast_entity_buffer, Entity, EntityBuffer, EntityBufferStruct};

pub struct Universe {
    entity_buffers: FxHashMap<TypeId, Box<dyn EntityBuffer>>,
    pending_new_entity_buffers: Mutex<FxHashMap<TypeId, Box<dyn EntityBuffer>>>,
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

    pub fn loop_once(&mut self) {
        // Process all entities
        self.entity_buffers
            .par_iter()
            .for_each(|(_, x)| x.process(self));

        // Add new entity buffers
        self.entity_buffers
            .extend(self.pending_new_entity_buffers.get_mut().drain());

        // Flush entity buffers
        self.entity_buffers
            .par_iter_mut()
            .for_each(|(_, x)| x.flush());
    }

    pub fn loop_many(&mut self, count: LoopCount, min_delta: Duration) {
        let LoopCount::Count(n) = count else {
            if min_delta.is_zero() {
                loop {
                    self.loop_once();
                }
            } else {
                let sleeper = SpinSleeper::default();
                loop {
                    self.loop_once();
                    sleeper.sleep(min_delta);
                }
            }
        };

        if min_delta.is_zero() {
            for _i in 0..n {
                self.loop_once();
            }
        } else {
            let sleeper = SpinSleeper::default();
            for _i in 0..n {
                self.loop_once();
                sleeper.sleep(min_delta);
            }
        }
    }
}

pub enum LoopCount {
    Forever,
    Count(usize),
}

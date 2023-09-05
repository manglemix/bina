use std::{sync::atomic::{AtomicBool, Ordering, AtomicUsize}, marker::PhantomData, cell::SyncUnsafeCell, collections::BinaryHeap, ops::Deref, process::ExitCode};

use crossbeam::{queue::SegQueue, utils::Backoff, atomic::AtomicCell};
use parking_lot::Mutex;
use rayon::{slice::ParallelSlice, prelude::{ParallelIterator, IndexedParallelIterator}};
use triomphe::Arc;

use crate::{str::ToSharedString, utility::{Utility, ComponentsIter}, reference::StaticReference, locking::Guard, ConstFxHashMap, ConstFxHashSet, new_fx_hashmap, new_fx_hashset, bimap::BiMap};


#[derive(Clone)]
pub struct ComponentRef<T: Component> {
    pub(crate) component_index: usize,
    freed: Arc<AtomicBool>,
    _phantom: PhantomData<T>
}


impl<T: Component> ComponentRef<T> {
    pub fn is_alive(&self) -> bool {
        self.freed.load(Ordering::Acquire)
    }

    pub fn queue_free(self) {
        let lock = ComponentStore::<T>::get_process_lock();
        if self.is_alive() {
            lock.queue_free_component(self.component_index);
        }
    }
}


pub struct ComponentStore<T: Component> {
    components_vec: Vec<Mutex<T>>,
    component_ids: BiMap<Arc<str>, usize>,
    component_ids_freed: ConstFxHashMap<usize, Arc<AtomicBool>>,
    components_to_add: SegQueue<(T, Option<Arc<str>>)>,
    components_to_remove: SegQueue<usize>,
    ids_to_add: Mutex<ConstFxHashSet<Arc<str>>>,
    process_locks: AtomicUsize
}


const FLUSH_BIT: usize = 2usize.pow(usize::BITS - 1);

pub(crate) struct ProcessLock<T: Component> {
    _phantom: PhantomData<T>
}


impl<T: Component> Drop for ProcessLock<T> {
    fn drop(&mut self) {
        let process_locks_offset = std::mem::offset_of!(ComponentStore<T>, process_locks);
        unsafe {
            let ptr: *const AtomicUsize = *T::StoreRef::get().get().cast_const().cast::<u8>().add(process_locks_offset).cast();
            (*ptr).fetch_sub(1, Ordering::Release);
        }
    }
}


impl<T: Component> Deref for ProcessLock<T> {
    type Target = ComponentStore<T>;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &*T::StoreRef::get().get()
        }
    }
}


impl<T: Component> ComponentStore<T> {
    pub const fn new() -> Self {
        Self {
            components_vec: Vec::new(),
            component_ids: BiMap::new(),
            component_ids_freed: new_fx_hashmap(),
            components_to_add: SegQueue::new(),
            components_to_remove: SegQueue::new(),
            ids_to_add: Mutex::new(new_fx_hashset()),
            process_locks: AtomicUsize::new(0)
        }
    }

    pub(crate) fn get_process_lock() -> ProcessLock<T> {
        let store_ptr = T::StoreRef::get().get().cast_const();
        let process_locks_offset = std::mem::offset_of!(Self, process_locks);
        
        unsafe {
            let process_locks_ptr: *const AtomicUsize = store_ptr.cast::<u8>().add(process_locks_offset).cast();
            let backoff = Backoff::new();
            
            loop {
                // Try to lock
                let locks = (*process_locks_ptr).fetch_add(1, Ordering::AcqRel);
                // Presence of the flush bit means that we are already flushing
                if locks >= FLUSH_BIT {
                    (*process_locks_ptr).fetch_sub(1, Ordering::Release);
                    // Snooze until the flush bit is gone, then try to lock again
                    loop {
                        backoff.snooze();
                        if (*process_locks_ptr).load(Ordering::Acquire) < FLUSH_BIT {
                            break
                        }
                    }
                } else {
                    break
                }
            }
        }

        return ProcessLock { _phantom: Default::default() }
    }

    pub(crate) fn queue_add_unnamed_component(&self, component: T) {
        self.components_to_add.push((component, None));
    }

    pub(crate) fn queue_add_named_component(&self, component: T, id: impl ToSharedString) -> Result<(), T> {
        let id = id.to_shared_string();
        let mut set = self.ids_to_add.lock();
        if set.contains(&id) {
            return Err(component)
        }
        if self.component_ids.contains_left(&id) {
            return Err(component)
        }
        set.insert(id.clone());
        self.components_to_add.push((component, Some(id)));
        Ok(())
    }

    pub(crate) fn queue_free_component(&self, index: usize) {
        if index < self.components_vec.len() {
            self.components_to_remove.push(index);
        }
    }

    /// # Safety
    /// 
    /// Can only be called while not flushing
    pub(crate) unsafe fn get_components(&self) -> ComponentsIter<T> {
        todo!()
    }

    pub(crate) fn get_component(&self, name: &str) -> Option<ComponentRef<T>> {
        let component_index = *self.component_ids.get_by_left(name)?;
        let freed = unsafe {
            self.component_ids_freed.get(&component_index).unwrap_unchecked().clone()
        };
        Some(ComponentRef { component_index, freed, _phantom: Default::default() })
    }

    pub(crate) fn get_component_by_idx(&self, idx: usize) -> Option<&Mutex<T>> {
        self.components_vec.get(idx)
    }

    /// Processes all the pending queues of component adds or removes
    /// 
    /// This method blocks until `process` has finished, and then proceeds
    /// to start flushing. Flushing is a very critical section as it requires
    /// mutable access to the whole struct.
    /// 
    /// This method does not take mutable reference to the store as it is only
    /// safe to do so once `process` has finished. Therefore, it watches the status
    /// of the `process` method through a `const` pointer. Since all stores have a
    /// pointer to themselves, this method does not need to take in any references.
    /// 
    /// # Safety
    /// 
    /// Only one call to `flush` for a specific store at a time. This method
    /// can safely check if a `process` is running, but it cannot check if there
    /// is another call to `flush` happening.
    pub(crate) unsafe fn flush() {
        let store = {
            let ptr = T::StoreRef::get().get();
            let const_ptr = ptr.cast_const();
            let backoff = Backoff::new();
            (*const_ptr).process_locks.fetch_add(FLUSH_BIT, Ordering::Release);

            while (*const_ptr).process_locks.load(Ordering::Acquire) > FLUSH_BIT {
                backoff.snooze();
            }

            debug_assert_eq!((*const_ptr).process_locks.load(Ordering::Acquire), FLUSH_BIT);
            &mut *ptr
        };

        store.ids_to_add.get_mut().clear();

        let mut indices = BinaryHeap::with_capacity(store.components_to_remove.len());
        while let Some(i) = store.components_to_remove.pop() {
            indices.push(i);
        }

        let mut last = usize::MAX;
        for i in indices {
            if i == last {
                continue
            }
            last = i;
            store.component_ids_freed.remove(&i).map(|b| b.store(true, Ordering::Release));
            store.components_vec.swap_remove(i);
            store.component_ids.remove_by_right(&i);
        }

        let additional = store.components_to_add.len();
        store.components_vec.reserve(additional);
        store.component_ids.reserve(additional);

        while let Some((component, maybe_id)) = store.components_to_add.pop() {
            if let Some(id) = maybe_id {
                let i = store.components_vec.len();
                store.component_ids.insert(id, i);
                store.component_ids_freed.insert(i, Arc::new(AtomicBool::new(false)));
            }
            store.components_vec.push(component.into());
        }

        assert_eq!(store.process_locks.fetch_sub(FLUSH_BIT, Ordering::Release), FLUSH_BIT);
    }

    /// Calls `process` on all components stored inside this store
    pub(crate) fn process(&self, delta: f32, request_exit: &AtomicCell<Option<ExitCode>>) {
        const CHUNK_SIZE: usize = 100;

        self
            .components_vec
            .as_slice()
            .par_chunks(CHUNK_SIZE)
            .enumerate()
            .for_each(|(mut starting_i, chunk)| {
                starting_i *= CHUNK_SIZE;

                for (mut _i, component) in chunk.into_iter().enumerate() {
                    _i += starting_i;
                    T::process(Guard::from_mutex(component), delta, Utility::new(&request_exit));
                }
            });
        }
}


pub trait Component: Sized + Send + Sync + 'static {
    type StoreRef: StaticReference<Type=SyncUnsafeCell<ComponentStore<Self>>>;

    fn process(self: Guard<ComponentRef<Self>>, _delta: f32, _utility: Utility) { }
}


// struct ComponentItem<T: Component> {
//     component: T,
    
// }


#[cfg(test)]
mod tests {
}
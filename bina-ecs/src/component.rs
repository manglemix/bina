use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}, marker::PhantomData, cell::SyncUnsafeCell, collections::BinaryHeap};

use bimap::BiHashMap;
use crossbeam::queue::SegQueue;
use fxhash::{FxBuildHasher, FxHashSet, FxHashMap};
use parking_lot::Mutex;

use crate::{str::ToSharedString, utility::Utility, reference::MutStaticReference};


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
}


pub struct ComponentStore<T: Component> {
    components_vec: Vec<SyncUnsafeCell<T>>,
    component_ids: Option<BiHashMap<Arc<str>, usize, FxBuildHasher, FxBuildHasher>>,
    component_ids_freed: Option<FxHashMap<usize, Arc<AtomicBool>>>,
    components_to_add: SegQueue<(T, Option<Arc<str>>)>,
    components_to_remove: SegQueue<usize>,
    ids_to_add: Mutex<Option<FxHashSet<Arc<str>>>>
}


impl<T: Component> ComponentStore<T> {
    pub const fn new() -> Self {
        Self {
            components_vec: Vec::new(),
            component_ids: None,
            component_ids_freed: None,
            components_to_add: SegQueue::new(),
            components_to_remove: SegQueue::new(),
            ids_to_add: Mutex::new(None)
        }
    }

    pub fn add_unnamed_component(&self, component: T) {
        self.components_to_add.push((component, None));
    }

    pub fn add_named_component(&self, component: T, id: impl ToSharedString) -> Result<(), T> {
        let id = id.to_shared_string();
        let mut lock = self.ids_to_add.lock();
        let set = lock.get_or_insert_default();
        if set.contains(&id) {
            return Err(component)
        }
        if let Some(map) = self.component_ids.as_ref() {
            if map.contains_left(&id) {
                return Err(component)
            }
        }
        set.insert(id.clone());
        self.components_to_add.push((component, Some(id)));
        Ok(())
    }

    pub fn remove_component(&self, index: usize) {
        if index < self.components_vec.len() {
            self.components_to_remove.push(index);
        }
    }

    pub fn get_component(&self, name: &Arc<str>) -> Option<ComponentRef<T>> {
        let component_index = *self.component_ids.as_ref()?.get_by_left(name)?;
        let freed = unsafe {
            self.component_ids_freed.as_ref().unwrap_unchecked().get(&component_index).unwrap_unchecked().clone()
        };
        Some(ComponentRef { component_index, freed, _phantom: Default::default() })
    }

    pub fn get_component_ptr(&self, name: &Arc<str>) -> Option<*mut T> {
        let index = self.component_ids.as_ref()?.get_by_left(name)?;
        Some(unsafe { self.components_vec.get_unchecked(*index).get() })
    }

    pub fn flush(&mut self) {
        self.ids_to_add.get_mut().get_or_insert_default().clear();

        let mut indices = BinaryHeap::with_capacity(self.components_to_remove.len());
        while let Some(i) = self.components_to_remove.pop() {
            indices.push(i);
        }
        let component_ids = self.component_ids.get_or_insert_default();
        let component_ids_freed = self.component_ids_freed.get_or_insert_default();

        let mut last = usize::MAX;
        for i in indices {
            if i == last {
                continue
            }
            last = i;
            component_ids_freed.remove(&i).map(|b| b.store(true, Ordering::Release));
            self.components_vec.swap_remove(i);
            component_ids.remove_by_right(&i);
        }

        let additional = self.components_to_add.len();
        self.components_vec.reserve(additional);
        component_ids.reserve(additional);

        while let Some((component, maybe_id)) = self.components_to_add.pop() {
            if let Some(id) = maybe_id {
                let i = self.components_vec.len();
                component_ids.insert(id, i);
                component_ids_freed.insert(i, Arc::new(AtomicBool::new(false)));
            }
            self.components_vec.push(component.into());
        }
    }
}


pub trait Component: Sized {
    type StoreRef: MutStaticReference<Type=ComponentStore<Self>>;

    fn process(_delta: f32, _utility: Utility) { }
}



#[cfg(test)]
mod tests {
}
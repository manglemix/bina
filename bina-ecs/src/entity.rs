use std::{marker::Tuple, collections::BinaryHeap, ops::Deref, any::TypeId, mem::transmute};

use crossbeam::{queue::SegQueue, atomic::AtomicCell};
use rayon::prelude::{IntoParallelRefMutIterator, ParallelIterator, IndexedParallelIterator, IntoParallelRefIterator};
use triomphe::{Arc, UniqueArc};

use crate::{component::{ComponentCombination, MaybeComponent}, universe::Universe};


fn arr_to_arc<T: Copy, const N: usize>(arr: [T; N]) -> Arc<[T]> {
    let mut arc = UniqueArc::new_uninit_slice(N);
    for i in 0..N {
        arc[i].write(arr[i]);
    }
    unsafe {
        UniqueArc::assume_init_slice(arc).shareable()
    }
}


pub trait Entity: Tuple + Send + Sync + Sized + 'static {
    fn process(&self, my_index: usize, universe: &Universe);
    fn flush(&mut self);
}

impl<A: MaybeComponent + ComponentCombination<(A,)>> Entity for (A,) {
    fn flush(&mut self) {
        self.0.flush();
    }

    fn process(&self, my_index: usize, universe: &Universe) {
        self.0.process(EntityReference { index: my_index, entity: self, ignore_ptrs: arr_to_arc([ref_to_usize(&self.0)]) }, universe);
    }
}
impl<
        A: MaybeComponent + ComponentCombination<(A, B)>,
        B: MaybeComponent + ComponentCombination<(A, B)>,
    > Entity for (A, B)
{
    fn flush(&mut self) {
        rayon::join(|| self.0.flush(), || self.1.flush());
    }

    fn process(&self, my_index: usize, universe: &Universe) {
        macro_rules! make_ref {
            ($($index: tt) *) => {
                EntityReference { index: my_index, entity: self, ignore_ptrs: arr_to_arc([$(ref_to_usize(&self.$index)),*]) }
            };
        }
        rayon::join(|| self.0.process(make_ref!(0), universe), || self.1.process(make_ref!(1), universe));
    }
}

pub(crate) trait EntityBuffer: Send + Sync {
    /// Gets a void pointer to this buffer
    fn get_void_ptr(&self) -> *const ();

    /// Flushes all entities stored inside this buffer,
    /// then adds all entities from the last process frame
    ///
    /// Flushing an entity finalizes all the changes applied onto it
    fn flush(&mut self);

    /// Processes all the entities in this buffer
    ///
    /// Changes to components during this call are not applied immediately.
    /// They are applied when this buffer is flushed.
    fn process(&self, universe: &Universe);

    fn queue_remove_entity(&self, index: usize);
}

pub(crate) unsafe fn cast_entity_buffer<E: Entity>(
    boxed: &Box<dyn EntityBuffer>,
) -> &EntityBufferStruct<E> {
    let ptr: *const EntityBufferStruct<E> = boxed.get_void_ptr().cast();
    &*ptr
}


#[derive(Clone, Copy)]
enum EntityIndex {
    Moving,
    Alive(usize),
    Freed
}


struct EntityWrapper<E: Entity> {
    entity: E,
    index: Arc<AtomicCell<EntityIndex>>
}


impl<E: Entity> EntityWrapper<E> {
    fn new(entity: E, index: usize) -> Self {
        Self {
            entity,
            index: Arc::new(AtomicCell::new(EntityIndex::Alive(index)))
        }
    }
}


pub trait MaybeEntity {}


impl MaybeEntity for () {}
impl<E: Entity> MaybeEntity for E {}


#[derive(Clone)]
pub struct EntityReference<'a, E: MaybeEntity> {
    pub(crate) index: usize,
    entity: &'a E,
    ignore_ptrs: Arc<[usize]>
}


impl<'a, E: MaybeEntity> Deref for EntityReference<'a, E> {
    type Target = E;

    fn deref(&self) -> &Self::Target {
        self.entity
    }
}

fn ref_to_usize<T>(reference: &T) -> usize {
    std::ptr::from_ref(reference) as usize
}

impl<'a, E: Entity> EntityReference<'a, E> {
    fn is_ref_ignored<T>(&self, reference: &T) -> bool {
        self.ignore_ptrs.binary_search(&ref_to_usize(reference)).is_ok()
    }
}


impl<'a, A> EntityReference<'a, (A,)>
where
    (A,): Entity
{
    pub fn get_component<T: 'static>(&self) -> Option<&T> {
        if TypeId::of::<T>() == TypeId::of::<A>() {
            if self.is_ref_ignored(&self.entity.0) {
                None
            } else {
                Some(unsafe { transmute(&self.entity.0) })
            }
        } else {
            None
        }
    }

    pub fn get_components<T: 'static>(&self) -> Box<[&T]> {
        if TypeId::of::<T>() == TypeId::of::<A>() {
            if self.is_ref_ignored(&self.entity.0) {
                [].into()
            } else {
                [unsafe { transmute(&self.entity.0) }].into()
            }
        } else {
            [].into()
        }
    }
}

impl<'a, A, B> EntityReference<'a, (A, B)>
where
    (A, B): Entity
{
    pub fn get_component<T: 'static>(&self) -> Option<&T> {
        if TypeId::of::<T>() == TypeId::of::<A>() {
            if self.is_ref_ignored(&self.entity.0) {
                None
            } else {
                Some(unsafe { transmute(&self.entity.0) })
            }
        } else if TypeId::of::<T>() == TypeId::of::<B>() {
            if self.is_ref_ignored(&self.entity.1) {
                None
            } else {
                Some(unsafe { transmute(&self.entity.1) })
            }
        } else {
            None
        }
    }

    pub fn get_components<T: 'static>(&self) -> Box<[&T]> {
        unsafe {
            if TypeId::of::<T>() == TypeId::of::<A>() {
                if self.is_ref_ignored(&self.entity.0) {
                    if TypeId::of::<T>() == TypeId::of::<B>() {
                        if self.is_ref_ignored(&self.entity.1) {
                            [].into()
                        } else {
                            [transmute(&self.entity.1)].into()
                        }
                    } else {
                        [].into()
                    }
                } else {
                    if TypeId::of::<T>() == TypeId::of::<B>() {
                        if self.is_ref_ignored(&self.entity.1) {
                            [transmute(&self.entity.0)].into()
                        } else {
                            [transmute(&self.entity.0), transmute(&self.entity.1)].into()
                        }
                    } else {
                        [transmute(&self.entity.0)].into()
                    }
                }
                
            } else if TypeId::of::<T>() == TypeId::of::<B>() {
                if self.is_ref_ignored(&self.entity.1) {
                    [].into()
                } else {
                    [transmute(&self.entity.1)].into()
                }
            } else {
                [].into()
            }
        }
    }
}


pub(crate) struct EntityBufferStruct<E: Entity> {
    buffer: Vec<EntityWrapper<E>>,
    pending_adds: SegQueue<E>,
    pending_removes: SegQueue<usize>,
    remove_buffer: BinaryHeap<usize>
}


impl<E: Entity> EntityBufferStruct<E> {
    pub(crate) fn new() -> Self {
        Self {
            buffer: Default::default(),
            pending_adds: SegQueue::new(),
            pending_removes: SegQueue::new(),
            remove_buffer: Default::default(),
        }
    }

    pub(crate) fn queue_add_entity(&self, entity: E) {
        self.pending_adds.push(entity);
    }

    pub(crate) fn par_iter(&self) -> impl IndexedParallelIterator + '_ {
        self.buffer.par_iter()
    }
}

impl<E: Entity> EntityBuffer for EntityBufferStruct<E> {
    fn get_void_ptr(&self) -> *const () {
        std::ptr::from_ref(self).cast()
    }

    fn flush(&mut self) {
        self.buffer.par_iter_mut().for_each(|x| x.entity.flush());

        // Sort entity indices to remove from highest to lowest
        while let Some(index) = self.pending_removes.pop() {
            self.remove_buffer.push(index);
        }
        
        // Because we remove in reverse order, and we never remove the
        // same index twice, we can safely remove entities without double
        // frees or accidentally removing the wrong entity
        // There is also a guard in the queue_remote_entity that ignores
        // indices out of range
        let mut last = None;
        for index in self.remove_buffer.drain_sorted() {
            if Some(index) == last {
                continue
            }
            last = Some(index);

            unsafe {
                // We assume the entity exists here
                let removed = self.buffer.get_unchecked_mut(index);
                // Register the entity as removed by overwriting its index with Freed
                let old_index = removed.index.swap(EntityIndex::Freed);

                if index == self.buffer.len() - 1 {
                    // The entity we are removing just so happens to be at the end
                    // The pop is guaranteed to work
                    self.buffer.pop().unwrap_unchecked();
                } else {
                    // The entity is not at the end, so to perform a safe swap remove,
                    // we must set the index of the last element to Moving, so that threads
                    // wanting to access it right now see that it is currently moving
                    let last = self.buffer.last_mut().unwrap_unchecked();
                    last.index.store(EntityIndex::Moving);
                    // Now we can safely swap remove
                    self.buffer.swap_remove(index);
                    // We give the index of the removed entity to the entity that replaced it
                    self.buffer.get_unchecked(index).index.store(old_index);
                }
            };
        }

        self.buffer.reserve(self.pending_adds.len());
        while let Some(entity) = self.pending_adds.pop() {
            // It is safe to set the index before the entity is added
            // because there is no way that there are any references to it right now
            let entity = EntityWrapper::new(entity, self.buffer.len());
            self.buffer.push(entity);
        }
    }

    fn process(&self, universe: &Universe) {
        self.buffer.par_iter().enumerate().for_each(|(index, x)| x.entity.process(index, universe));
    }

    fn queue_remove_entity(&self, index: usize) {
        if index >= self.buffer.len() {
            return
        }
        self.pending_removes.push(index);
    }
}

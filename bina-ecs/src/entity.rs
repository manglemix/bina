use std::marker::Tuple;

use crossbeam::queue::SegQueue;

use crate::{component::{ComponentCombination, MaybeComponent}, universe::Universe};

pub trait Entity: Tuple + Send + Sync + 'static {
    fn process(&self, universe: &Universe);
    fn flush(&mut self);
}

impl<A: MaybeComponent + ComponentCombination<(A,)>> Entity for (A,) {
    fn flush(&mut self) {
        self.0.flush();
    }

    fn process(&self, universe: &Universe) {
        self.0.process((), universe);
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

    fn process(&self, universe: &Universe) {
        rayon::join(|| self.0.process((&self.1,), universe), || self.1.process((&self.0,), universe));
    }
}

pub(crate) trait EntityBuffer: Send + Sync {
    /// Gets a void pointer to this buffer
    ///
    /// This method is used internally and you should not need to use it
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
}

pub(crate) unsafe fn cast_entity_buffer<E: Entity>(
    boxed: &Box<dyn EntityBuffer>,
) -> &EntityBufferStruct<E> {
    let ptr: *const EntityBufferStruct<E> = boxed.get_void_ptr().cast();
    &*ptr
}

pub(crate) struct EntityBufferStruct<E: Entity> {
    buffer: Vec<E>,
    pending: SegQueue<E>,
}

impl<E: Entity> EntityBufferStruct<E> {
    pub(crate) fn new() -> Self {
        todo!()
    }
    pub(crate) fn queue_add_entity(&self, entity: E) {
        self.pending.push(entity);
    }
}

impl<E: Entity> EntityBuffer for EntityBufferStruct<E> {
    fn get_void_ptr(&self) -> *const () {
        std::ptr::from_ref(self).cast()
    }
    fn flush(&mut self) {
        self.buffer.iter_mut().for_each(|x| x.flush());
        self.buffer.reserve(self.pending.len());
        while let Some(entity) = self.pending.pop() {
            self.buffer.push(entity.into());
        }
    }
    fn process(&self, universe: &Universe) {
        self.buffer.iter().for_each(|x| x.process(universe));
    }
}

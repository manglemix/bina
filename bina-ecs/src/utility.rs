use std::cell::SyncUnsafeCell;
use std::marker::PhantomData;
use std::ops::Deref;
use std::process::ExitCode;

use crossbeam::atomic::AtomicCell;

use crate::component::{ComponentRef, Component};
use crate::str::ToSharedString;
use crate::universe::RegisteredComponent;
use crate::reference::StaticReference;

#[derive(Clone, Copy)]
pub struct Utility<'a> {
    request_exit: &'a AtomicCell<Option<ExitCode>>,
    _phantom: PhantomData<&'a ()>
}


impl<'a> Utility<'a> {
    pub(crate) fn new(request_exit: &'a AtomicCell<Option<ExitCode>>) -> Self {
        Self { request_exit, _phantom: Default::default() }
    }

    pub fn request_exit(self, code: ExitCode) {
        self.request_exit.store(Some(code));
    }

    pub fn queue_add_unnamed_component<T: RegisteredComponent>(self, component: T) {
        // SAFETY: Utility only exists in the process frame
        unsafe {
            (*T::StoreRef::get().get()).queue_add_unnamed_component(component);
        }
    }

    pub fn queue_add_named_component<T: RegisteredComponent>(self, component: T, id: impl ToSharedString) -> Result<(), T> {
        // SAFETY: Utility only exists in the process frame
        unsafe {
            (*T::StoreRef::get().get()).queue_add_named_component(component, id)
        }
    }

    pub fn get_component<T: RegisteredComponent>(&self, name: &str) -> Option<ComponentRef<T>> {
            // SAFETY: Utility only exists in the process frame
            unsafe {
                (*T::StoreRef::get().get()).get_component(name)
            }
    }

    pub fn get_components<T: RegisteredComponent>(self) -> ComponentsIter<'a, T> {
        // SAFETY: Utility only exists in the process frame
        unsafe {
            (*T::StoreRef::get().get()).get_components()
        }
    }
}


pub struct ComponentsIter<'a, T: Component> {
    local_slice: std::slice::Iter<'a, SyncUnsafeCell<T>>,
}


pub struct ComponentsIterItem<'a, T: Component> {
    item: &'a SyncUnsafeCell<T>
}


impl<'a, T: Component> Deref for ComponentsIterItem<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &*self.item.get()
        }
    }
}


impl<'a, T: Component> Iterator for ComponentsIter<'a, T> {
    type Item = ComponentsIterItem<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.local_slice.next() {
            return Some(ComponentsIterItem { item })
        }
        None
    }
}

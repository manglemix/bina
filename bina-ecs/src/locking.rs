use std::{marker::PhantomData, ops::{Deref, DerefMut}};

use crossbeam::utils::Backoff;
use parking_lot::{Mutex, MutexGuard};

use crate::component::{ComponentRef, Component};
use crate::reference::StaticReference;

/// A `LockToken` is required to lock any `bina` managed resources (such as `Component`s)
/// 
/// Since `bina` always attempts to run code across many threads, the possibility for deadlocks
/// is extremely high. `LockToken`s help to ensure that locks do not persist past some safe duration.
/// If you were able to lock a resource and keep it locked forever, you would be able to deadlock the
/// main threads.
pub struct LockToken<'a> {
    _phantom: PhantomData<&'a ()>
}


impl<'a> LockToken<'a> {
    pub(crate) fn new() -> Self {
        Self { _phantom: Default::default() }
    }

    pub fn lock<T: RefSet<'a>>(self, refs: T) -> Guard<'a, T> {
        refs.lock(self)
    }
}


pub trait RefSet<'a>: Sized {
    type Output;

    fn lock(self, token: LockToken) -> Guard<'a, Self>;
}


impl<'a, A: Component> RefSet<'a> for ComponentRef<A> {
    type Output = MutexGuard<'a, A>;

    fn lock(self, _token: LockToken) -> Guard<'a, Self> {
        unsafe {
            let mutex = (*A::StoreRef::get().get()).get_component_by_idx(self.component_index).unwrap_unchecked();
            Guard::from_mutex(mutex)
        }
    }
}


impl<'a, A: Component> RefSet<'a> for (ComponentRef<A>,) {
    type Output = (MutexGuard<'a, A>,);

    fn lock(self, _token: LockToken) -> Guard<'a, Self> {
        unsafe {
            let mutex = (*A::StoreRef::get().get()).get_component_by_idx(self.0.component_index).unwrap_unchecked();
            Guard {
                refs: (mutex.lock(),),
                _phantom: PhantomData::default(),
            }
        }
    }
}


impl<'a, A: Component, B: Component> RefSet<'a> for (ComponentRef<A>, ComponentRef<B>) {
    type Output = (MutexGuard<'a, A>, MutexGuard<'a, B>);

    fn lock(self, _token: LockToken) -> Guard<'a, Self> {
        unsafe {
            
            let mutex1 = (*A::StoreRef::get().get()).get_component_by_idx(self.0.component_index).unwrap_unchecked();
            let mutex2 = (*B::StoreRef::get().get()).get_component_by_idx(self.1.component_index).unwrap_unchecked();
            let backoff = Backoff::new();

            loop {
                let Some(guard1) = mutex1.try_lock() else {
                    backoff.snooze();
                    continue
                };
                let Some(guard2) = mutex2.try_lock() else {
                    backoff.snooze();
                    continue
                };

                break Guard {
                    refs: (guard1, guard2),
                    _phantom: PhantomData::default(),
                }
            }
        }
    }
}


pub struct Guard<'a, T: RefSet<'a>> {
    refs: T::Output,
    // token: Option<LockToken<'a>>
    _phantom: PhantomData<&'a T>
}


impl<'a, T: Component> Guard<'a, ComponentRef<T>> {
    pub(crate) fn from_mutex(mutex: &'a Mutex<T>) -> Self {
        Self {
            refs: mutex.lock(),
            _phantom: PhantomData::default(),
        }
    }
}


impl<'a, T: RefSet<'a>> Deref for Guard<'a, T> {
    type Target = T::Output;

    fn deref(&self) -> &Self::Target {
        &self.refs
    }
}


impl<'a, T: RefSet<'a>> DerefMut for Guard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.refs
    }
}


impl<'a, T: RefSet<'a>> Guard<'a, T> {
    /// Explicitly unlocks this `Guard`, allowing you to retrieve the `LockToken`
    /// 
    /// You do not need to call this for the `Guard` to be dropped safely, however,
    /// you may want to continue locking other resources, and for that you will
    /// need to retrieve the `LockToken` with which this `Guard` was locked with.
    pub fn unlock(_guard: Self) -> LockToken<'a> {
        LockToken::new()
    }
}
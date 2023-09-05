use std::{hash::Hash, ptr::{Unique, drop_in_place}, borrow::Borrow, ops::Deref};

use triomphe::Arc;

use crate::{ConstFxHashMap, new_fx_hashmap};


pub(crate) struct ManualRc<T>(Unique<T>);


impl<T> Clone for ManualRc<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}


impl<T> Copy for ManualRc<T> { }

impl<T: Hash> Hash for ManualRc<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl<T: PartialEq> PartialEq for ManualRc<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other)
    }
}

impl<T: Eq> Eq for ManualRc<T> { }

impl<T> ManualRc<T> {
    fn new(value: T) -> Self {
        unsafe {
            Self(Unique::new_unchecked(Box::into_raw(Box::new(value))))
        }
    }

    unsafe fn drop(self) {
        drop_in_place(self.0.as_ptr());
    }

    unsafe fn into_inner(self) -> T {
        std::ptr::read(self.0.as_ptr())
    }
}

impl<T> Borrow<T> for ManualRc<T> {
    fn borrow(&self) -> &T {
        self
    }
}

impl<T: ?Sized> Borrow<T> for ManualRc<Arc<T>> {
    fn borrow(&self) -> &T {
        self
    }
}


impl<T> Deref for ManualRc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            self.0.as_ref()
        }
    }
}


pub(crate) struct BiMap<L, R> {
    left: ConstFxHashMap<ManualRc<L>, ManualRc<R>>,
    right: ConstFxHashMap<ManualRc<R>, ManualRc<L>>,
}


impl<L: Hash + Eq, R: Hash + Eq> BiMap<L, R> {
    pub const fn new() -> Self {
        Self {
            left: new_fx_hashmap(),
            right: new_fx_hashmap()
        }
    }

    pub fn contains_left<Q>(&self, left: &Q) -> bool
    where
        Q: ?Sized,
        ManualRc<L>: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.left.contains_key(left)
    }

    pub fn get_by_left<Q>(&self, left: &Q) -> Option<&R>
    where
        Q: ?Sized,
        ManualRc<L>: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.left.get(left).map(Deref::deref)
    }

    pub fn remove_by_right<Q>(&mut self, right: &Q) -> Option<(L, R)>
    where
        Q: ?Sized,
        ManualRc<R>: Borrow<Q>,
        Q: Hash + Eq,
    {
        let left = self.right.remove(right)?;
        unsafe {
            let right = self.left.remove(&left).unwrap_unchecked();
            Some((left.into_inner(), right.into_inner()))
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        self.left.reserve(additional);
        self.right.reserve(additional);
    }

    pub fn insert(&mut self, left: L, right: R) {
        let left = ManualRc::new(left);
        let right = ManualRc::new(right);

        self.left.insert(left, right);
        self.right.insert(right, left);
    }
}

impl<L, R> Drop for BiMap<L, R> {
    fn drop(&mut self) {
        self.right.clear();
        self.left.drain().for_each(|(left, right)| unsafe {
            left.drop();
            right.drop();
        });
    }
}
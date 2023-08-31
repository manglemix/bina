use std::{
    any::{Any, TypeId},
    collections::hash_map::Entry,
    marker::Tuple, mem::replace, ops::DerefMut,
};

use delegate::delegate;
use fxhash::FxHashMap;

/// A Set of objects of unique types with accesses and pushes of constant time,
/// achieved with compile time evaluations and optimizations
///
/// The main limitation is that pushes modify the type of the set as a whole
#[derive(Default)]
pub struct UniqueTypeSet<T: Tuple = ()> {
    objects: T,
}

impl UniqueTypeSet<()> {
    pub const fn push<T: 'static>(self, item: T) -> Result<UniqueTypeSet<(T,)>, Self> {
        Ok(UniqueTypeSet { objects: (item,) })
    }
    pub const fn get<T>(&self) -> Option<&T> {
        None
    }
    pub const fn into_inner(self) -> () {
        self.objects
    }
}

impl<A: 'static> UniqueTypeSet<(A,)> {
    pub fn push<T: 'static>(self, item: T) -> Result<UniqueTypeSet<(A, T)>, Self> {
        if TypeId::of::<A>() == TypeId::of::<T>() {
            Err(self)
        } else {
            Ok(UniqueTypeSet {
                objects: (self.objects.0, item),
            })
        }
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        if TypeId::of::<A>() == TypeId::of::<T>() {
            Some(unsafe { std::mem::transmute(&self.objects.0) })
        } else {
            None
        }
    }
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if TypeId::of::<A>() == TypeId::of::<T>() {
            Some(unsafe { std::mem::transmute(&mut self.objects.0) })
        } else {
            None
        }
    }
    pub fn into_inner(self) -> (A,) {
        self.objects
    }
}

impl<A: 'static, B: 'static> UniqueTypeSet<(A, B)> {
    pub fn push<T: 'static>(self, item: T) -> Result<UniqueTypeSet<(A, B, T)>, Self> {
        if TypeId::of::<A>() == TypeId::of::<T>() || TypeId::of::<B>() == TypeId::of::<T>() {
            Err(self)
        } else {
            Ok(UniqueTypeSet {
                objects: (self.objects.0, self.objects.1, item),
            })
        }
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        if TypeId::of::<A>() == TypeId::of::<T>() {
            Some(unsafe { std::mem::transmute(&self.objects.0) })
        } else if TypeId::of::<B>() == TypeId::of::<T>() {
            Some(unsafe { std::mem::transmute(&self.objects.1) })
        } else {
            None
        }
    }
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        if TypeId::of::<A>() == TypeId::of::<T>() {
            Some(unsafe { std::mem::transmute(&mut self.objects.0) })
        } else if TypeId::of::<B>() == TypeId::of::<T>() {
            Some(unsafe { std::mem::transmute(&mut self.objects.1) })
        } else {
            None
        }
    }
    pub fn into_inner(self) -> (A, B) {
        self.objects
    }
}

/// A Set of objects of unique types with accesses and insertions of constant time,
/// achieved by using a fast hash map implementation
///
/// Compared to the UniqueTypeSet, there are less possible compiler optimizations with
/// this approach, and insertions will involve reallocations at some point, and the backing
/// memory will always hold some amount of unused space (as is the case with most hashmaps)
#[derive(Default)]
pub struct DynamicTypeSet {
    map: FxHashMap<TypeId, Box<dyn Any>>,
}

impl DynamicTypeSet {
    pub fn insert<T: 'static>(&mut self, item: T) -> Option<T> {
        match self.map.entry(TypeId::of::<T>()) {
            Entry::Occupied(mut entry) => unsafe {
                let entry: &mut Box<T> = entry.get_mut().downcast_mut_unchecked();
                Some(replace(entry.deref_mut(), item))
            }
            Entry::Vacant(entry) => {
                entry.insert(Box::new(item));
                None
            }
        }
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .map(|x| unsafe { x.downcast_ref_unchecked() })
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .map(|x| unsafe { x.downcast_mut_unchecked() })
    }

    pub fn get_mut_or_insert<T: 'static, F: FnOnce() -> T>(&mut self, f: F) -> &mut T {
        unsafe {
            match self.map.entry(TypeId::of::<T>()) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => entry.insert(Box::new((f)())),
            }
            .downcast_mut_unchecked()
        }
    }

    delegate! {
        to self.map {
            pub fn len(&self) -> usize;
            pub fn is_empty(&self) -> bool;
            pub fn clear(&mut self);
        }
    }
}

// pub trait TypeSet {
//     fn get<T: 'static>(&self) -> Option<&T>;
//     fn get_mut<T: 'static>(&mut self) -> Option<&mut T>;
// }

// impl TypeSet for DynamicTypeSet {
//     fn get<T: 'static>(&self) -> Option<&T> {
//         DynamicTypeSet::get(self)
//     }

//     fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
//         DynamicTypeSet::get_mut(self)
//     }
// }

// impl TypeSet for UniqueTypeSet<()> {
//     fn get<T: 'static>(&self) -> Option<&T> {
//         UniqueTypeSet::<()>::get(self)
//     }

//     fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
//         UniqueTypeSet::<()>::get_mut(self)
//     }
// }

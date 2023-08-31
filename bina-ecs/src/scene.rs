use std::{ops::{DerefMut, Deref}, marker::Tuple, sync::Arc};

use fxhash::FxHashMap;

use crate::{collections::{DynamicTypeSet, UniqueTypeSet}, str::ToSharedString};

pub trait StaticObjectName: 'static + DerefMut<Target = Self::ObjectType> {
    type ObjectType;
}

#[macro_export]
macro_rules! declare_static_object_name {
    ($vis: vis $name: ident $type: ty) => {
        $vis struct $name(pub $type);

        impl Deref for $name {
            type Target = $type;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $name {
            fn dere_mutf(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl StaticObjectName for $name {
            type ObjectType = $type;
        }
    };
}

struct DNameMap<T: 'static> {
    map: FxHashMap<Arc<str>, T>,
}

impl<T> Default for DNameMap<T> {
    fn default() -> Self {
        Self { map: Default::default() }
    }
}


pub struct Scene<S: Tuple> {
    static_types: UniqueTypeSet<S>,
    types: DynamicTypeSet,
}

impl<S: Tuple> Scene<S> {
    pub fn get_object_by_dname<T: 'static>(&self, dname: impl Deref<Target=str>) -> Option<&T> {
        self.types
            .get::<DNameMap<T>>()
            .map(|map| map.map.get(dname.deref()))
            .flatten()
    }
    pub fn insert_object_by_dname<T: 'static>(&mut self, dname: impl ToSharedString, item: T) -> Option<T> {
        self.types
            .get_mut_or_insert(DNameMap::default)
            .map
            .insert(dname.to_shared_string(), item)
    }
}

impl Scene<()> {
    pub fn get_object_by_sname<T: StaticObjectName>(&self) -> Option<&T::ObjectType> {
        self.types.get::<T>().map(T::deref)
    }
}

impl<A: 'static> Scene<(A,)> {
    pub fn get_object_by_sname<T: StaticObjectName>(&self) -> Option<&T::ObjectType> {
        self.static_types
            .get::<T>()
            .or_else(|| self.types.get::<T>())
            .map(T::deref)
    }
}

impl<A: 'static, B: 'static> Scene<(A, B)> {
    pub fn get_object_by_sname<T: StaticObjectName>(&self) -> Option<&T::ObjectType> {
        self.static_types
            .get::<T>()
            .or_else(|| self.types.get::<T>())
            .map(T::deref)
    }
}

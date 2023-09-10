#![feature(tuple_trait)]
// #![feature(downcast_unchecked)]
#![feature(hash_extract_if)]
#![feature(sync_unsafe_cell)]
// #![feature(option_get_or_insert_default)]
#![feature(offset_of)]
#![feature(const_collections_with_hasher)]
#![feature(ptr_internals)]
#![feature(arbitrary_self_types)]
// #![feature(vec_push_within_capacity)]
#![feature(associated_type_defaults)]

use std::{collections::{HashMap, HashSet}, hash::BuildHasher};

pub mod locking;
pub mod str;
pub mod singleton;
pub mod component;
pub mod utility;
pub mod reference;
pub mod universe;
mod bimap;
// mod priority_queue;

struct ConstFxHasher;

impl BuildHasher for ConstFxHasher {
    type Hasher = fxhash::FxHasher;

    fn build_hasher(&self) -> Self::Hasher {
        fxhash::FxHasher::default()
    }
}

type ConstFxHashMap<K, V> = HashMap<K, V, ConstFxHasher>;
type ConstFxHashSet<V> = HashSet<V, ConstFxHasher>;

const fn new_fx_hashmap<K, V>() -> ConstFxHashMap<K, V> {
    ConstFxHashMap::with_hasher(ConstFxHasher)
}

const fn new_fx_hashset<V>() -> ConstFxHashSet<V> {
    ConstFxHashSet::with_hasher(ConstFxHasher)
}

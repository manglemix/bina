use std::{
    fmt::{Debug, Display},
    ops::{AddAssign, Deref, SubAssign}, sync::atomic::{AtomicU8, Ordering, AtomicU16, AtomicU32, AtomicU64, AtomicUsize, AtomicI8, AtomicI16, AtomicI32, AtomicI64, AtomicIsize}, mem::MaybeUninit,
};

use atomic_float::{AtomicF32, AtomicF64};
use crossbeam::queue::SegQueue;

use crate::{
    entity::{Entity, EntityReference},
    universe::Universe,
};

pub trait Component: Send + Sync + 'static {
    type Reference<'a> = &'a Self;

    fn get_ref<'a>(&'a self) -> Self::Reference<'a>;
    fn flush(&mut self) {}
}

pub trait Processable: Component {
    fn process<E: Entity>(
        component: Self::Reference<'_>,
        my_entity: EntityReference<E>,
        universe: &Universe,
    );
}

pub trait ComponentField {
    fn process_modifiers(&mut self);
}

pub trait AtomicNumber:
    PartialOrd
    + Copy
    + Sized
{
    type Atomic;

    fn new_atomic() -> Self::Atomic;
    fn load(atomic: &mut Self::Atomic) -> Self;
    fn store(atomic: &Self::Atomic, other: Self);

    fn add_assign(&mut self, other: Self);
    fn sub_assign(&mut self, other: Self);
    fn mul_assign(&mut self, other: Self);
    fn div_assign(&mut self, other: Self);

    fn atomic_add_assign(atomic: &Self::Atomic, other: Self);
    fn atomic_sub_assign(atomic: &Self::Atomic, other: Self);
    fn atomic_mul_assign(atomic: &Self::Atomic, other: Self);
    fn atomic_div_assign(atomic: &Self::Atomic, other: Self);
}

macro_rules! impl_num {
    () => {
        fn new_atomic() -> Self::Atomic {
            Default::default()
        }

        fn load(atomic: &mut Self::Atomic) -> Self {
            *atomic.get_mut()
        }
    
        fn store(atomic: &Self::Atomic, other: Self) {
            atomic.store(other, Ordering::Relaxed);
        }
    
        fn atomic_add_assign(atomic: &Self::Atomic, other: Self) {
            atomic.fetch_add(other, Ordering::Relaxed);
        }
    
        fn atomic_sub_assign(atomic: &Self::Atomic, other: Self) {
            atomic.fetch_sub(other, Ordering::Relaxed);
        }
    
        fn atomic_mul_assign(atomic: &Self::Atomic, other: Self) {
            let _ = atomic.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| Some(x * other));
        }
    
        fn atomic_div_assign(atomic: &Self::Atomic, other: Self) {
            let _ = atomic.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| Some(x / other));
        }

        fn add_assign(&mut self, other: Self) {
            *self = *self + other;
        }

        fn sub_assign(&mut self, other: Self) {
            *self = *self - other;
        }

        fn mul_assign(&mut self, other: Self) {
            *self = *self * other;
        }

        fn div_assign(&mut self, other: Self) {
            *self = *self / other;
        }
    };
}

impl AtomicNumber for u8 {
    type Atomic = AtomicU8;
    impl_num!{}
}
impl AtomicNumber for u16 {
    type Atomic = AtomicU16;
    impl_num!{}
}
impl AtomicNumber for u32 {
    type Atomic = AtomicU32;
    impl_num!{}
}
impl AtomicNumber for u64 {
    type Atomic = AtomicU64;
    impl_num!{}
}
// impl AtomicNumber for u128 {
//     type Atomic = AtomicU128;
//     impl_num!{}
// }
impl AtomicNumber for usize {
    type Atomic = AtomicUsize;
    impl_num!{}
}

impl AtomicNumber for i8 {
    type Atomic = AtomicI8;
    impl_num!{}
}
impl AtomicNumber for i16 {
    type Atomic = AtomicI16;
    impl_num!{}
}
impl AtomicNumber for i32 {
    type Atomic = AtomicI32;
    impl_num!{}
}
impl AtomicNumber for i64 {
    type Atomic = AtomicI64;
    impl_num!{}
}
// impl AtomicNumber for i128 {
//     type Atomic = AtomicI128;
//     impl_num!{}
// }
impl AtomicNumber for isize {
    type Atomic = AtomicIsize;
    impl_num!{}
}

impl AtomicNumber for f32 {
    type Atomic = AtomicF32;
    impl_num!{}
}
impl AtomicNumber for f64 {
    type Atomic = AtomicF64;
    impl_num!{}
}
impl<T: AtomicNumber, const N: usize> AtomicNumber for [T; N] {
    type Atomic = [T::Atomic; N];

    fn new_atomic() -> Self::Atomic {
        let mut arr = MaybeUninit::uninit_array();
        for mutref in &mut arr {
            mutref.write(T::new_atomic());
        }
        unsafe { MaybeUninit::array_assume_init(arr) }
    }

    fn load(atomic: &mut Self::Atomic) -> Self {
        let mut arr = MaybeUninit::uninit_array();
        for (i, mutref) in arr.iter_mut().enumerate() {
            mutref.write(T::load(&mut atomic[i]));
        }
        unsafe { MaybeUninit::array_assume_init(arr) }
    }

    fn store(atomic: &Self::Atomic, other: Self) {
        for (i, atomic) in atomic.iter().enumerate() {
            T::store(atomic, other[i]);
        }
    }

    fn add_assign(&mut self, other: Self) {
        for (i, mutref) in self.iter_mut().enumerate() {
            T::add_assign(mutref, other[i]);
        }
    }

    fn sub_assign(&mut self, other: Self) {
        for (i, mutref) in self.iter_mut().enumerate() {
            T::sub_assign(mutref, other[i]);
        }
    }

    fn mul_assign(&mut self, other: Self) {
        for (i, mutref) in self.iter_mut().enumerate() {
            T::mul_assign(mutref, other[i]);
        }
    }

    fn div_assign(&mut self, other: Self) {
        for (i, mutref) in self.iter_mut().enumerate() {
            T::div_assign(mutref, other[i]);
        }
    }

    fn atomic_add_assign(atomic: &Self::Atomic, other: Self) {
        for (i, atomic) in atomic.iter().enumerate() {
            T::atomic_add_assign(atomic, other[i]);
        }
    }

    fn atomic_sub_assign(atomic: &Self::Atomic, other: Self) {
        for (i, atomic) in atomic.iter().enumerate() {
            T::atomic_sub_assign(atomic, other[i]);
        }
    }

    fn atomic_mul_assign(atomic: &Self::Atomic, other: Self) {
        for (i, atomic) in atomic.iter().enumerate() {
            T::atomic_mul_assign(atomic, other[i]);
        }
    }

    fn atomic_div_assign(atomic: &Self::Atomic, other: Self) {
        for (i, atomic) in atomic.iter().enumerate() {
            T::atomic_div_assign(atomic, other[i]);
        }
    }
}

pub struct NumberField<T: AtomicNumber> {
    number: T,
    new_number: T::Atomic,
}

impl<T: AtomicNumber> From<T> for NumberField<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: AtomicNumber + Debug> Debug for NumberField<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.number)
    }
}

impl<T: AtomicNumber + Display> Display for NumberField<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.number)
    }
}

impl<T: AtomicNumber> ComponentField for NumberField<T> {
    fn process_modifiers(&mut self) {
        self.number = T::load(&mut self.new_number);
    }
}

impl<T: AtomicNumber> NumberField<T> {
    pub fn new(number: T) -> Self {
        Self {
            number,
            new_number: T::new_atomic()
        }
    }

    pub fn get_ref(&self) -> NumberFieldRef<T> {
        NumberFieldRef {
            number: self.number,
            reference: self,
            // set_performed: true,
        }
    }
}

#[derive(Clone, Copy)]
pub struct NumberFieldRef<'a, T: AtomicNumber> {
    number: T,
    reference: &'a NumberField<T>,
    // set_performed: bool,
}

impl<'a, T: AtomicNumber> AddAssign<T> for NumberFieldRef<'a, T> {
    fn add_assign(&mut self, rhs: T) {
        T::add_assign(&mut self.number, rhs);
        <T as AtomicNumber>::atomic_add_assign(&self.reference.new_number, rhs);
        // if self.set_performed {
        //     self.reference
        //         .queue_modifier(NumberModifier::Set(self.number));
        // } else {
        //     self.reference.queue_modifier(NumberModifier::Add(rhs));
        // }
    }
}

impl<'a, T: AtomicNumber> SubAssign<T> for NumberFieldRef<'a, T> {
    fn sub_assign(&mut self, rhs: T) {
        T::sub_assign(&mut self.number, rhs);
        <T as AtomicNumber>::atomic_sub_assign(&self.reference.new_number, rhs);
    }
}

impl<'a, T: AtomicNumber + PartialOrd> PartialOrd for NumberFieldRef<'a, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.number.partial_cmp(&other.number)
    }
}

impl<'a, T: AtomicNumber + Ord> Ord for NumberFieldRef<'a, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.number.cmp(&other.number)
    }
}

impl<'a, T: AtomicNumber + PartialEq> PartialEq for NumberFieldRef<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number
    }
}

impl<'a, T: AtomicNumber + Eq> Eq for NumberFieldRef<'a, T> {}

impl<'a, T: AtomicNumber + PartialOrd> PartialOrd<T> for NumberFieldRef<'a, T> {
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.number.partial_cmp(other)
    }
}

impl<'a, T: AtomicNumber + PartialEq> PartialEq<T> for NumberFieldRef<'a, T> {
    fn eq(&self, other: &T) -> bool {
        &self.number == other
    }
}

impl<'a, T: AtomicNumber> Deref for NumberFieldRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.number
    }
}

impl<'a, T: AtomicNumber + Debug> Debug for NumberFieldRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.number)
    }
}

impl<'a, T: AtomicNumber + Display> Display for NumberFieldRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.number)
    }
}

impl<'a, T: AtomicNumber> NumberFieldRef<'a, T> {
    pub fn set(&mut self, value: T) {
        T::store(&self.reference.new_number, value);
        // self.reference.queue_modifier(NumberModifier::Set(value));
        // self.set_performed = true;
    }
    pub fn get(&self) -> T {
        self.number
    }
}

pub struct StagedMutField<T> {
    value: T,
    modifiers: SegQueue<Box<dyn FnOnce(&mut T)>>,
}

impl<T> ComponentField for StagedMutField<T> {
    fn process_modifiers(&mut self) {
        while let Some(modifier) = self.modifiers.pop() {
            modifier(&mut self.value);
        }
    }
}

#[derive(Clone, Copy)]
pub struct StagedMutFieldRef<'a, T> {
    reference: &'a StagedMutField<T>,
}

impl<'a, T> StagedMutFieldRef<'a, T> {
    pub fn queue_modifier(&self, modifier: impl FnOnce(&mut T) + 'static) {
        self.reference.modifiers.push(Box::new(modifier));
    }
}

impl<'a, T> Deref for StagedMutFieldRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.reference.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_u8() {
        let mut num = NumberField::new(43u8);
        {
            let mut num_ref = num.get_ref();
            num_ref += 2;
        }
        assert_eq!(num.number, 43);
        {
            let mut num_ref = num.get_ref();
            num_ref.set(2);
        }
        num.process_modifiers();
        assert_eq!(num.number, 2);
    }
}

use std::{
    fmt::{Debug, Display},
    ops::{AddAssign, Deref, DivAssign, MulAssign, SubAssign},
};

use crossbeam::{atomic::AtomicCell, queue::SegQueue};

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

// pub trait MaybeComponent: Send + Sync + 'static {
//     type Reference<'a>;

//     fn process<E: Entity>(&self, my_entity: EntityReference<E>, universe: &Universe);
//     fn flush(&mut self);
// }

// impl<T: Component + Processable> MaybeComponent for Option<T> {
//     type Reference<'a> = T::Reference<'a>;
//     fn flush(&mut self) {
//         self.as_mut().map(|x| x.flush());
//     }

//     fn process<'a, E: Entity>(&self, my_entity: EntityReference<E>, universe: &Universe) {
//         self.as_ref()
//             .map(|x| T::process(x.get_ref(), my_entity, universe));
//     }
// }
// impl<T: Component + Processable> MaybeComponent for T {
//     type Reference<'a> = T::Reference<'a>;
//     fn flush(&mut self) {
//         self.flush();
//     }

//     fn process<E: Entity>(&self, my_entity: EntityReference<E>, universe: &Universe) {
//         T::process(self.get_ref(), my_entity, universe);
//     }
// }

// pub trait ComponentCombination<CC: Tuple>: Send + 'static {}

pub trait ComponentField {
    fn process_modifiers(&mut self);
}

#[derive(Clone, Copy)]
pub enum NumberModifier<T> {
    Set(T),
    Add(T),
    Sub(T),
    Mul(T),
    Div(T),
}

pub trait Number:
    std::ops::Add<Output = Self>
    + AddAssign
    + std::ops::Sub<Output = Self>
    + SubAssign
    + std::ops::Mul<Output = Self>
    + MulAssign
    + std::ops::Div<Output = Self>
    + DivAssign
    + PartialOrd
    + Copy
    + Sized
{
    const IS_SIGNED: bool;
}

impl Number for u8 {
    const IS_SIGNED: bool = false;
}
impl Number for u16 {
    const IS_SIGNED: bool = false;
}
impl Number for u32 {
    const IS_SIGNED: bool = false;
}
impl Number for u64 {
    const IS_SIGNED: bool = false;
}
impl Number for u128 {
    const IS_SIGNED: bool = false;
}
impl Number for usize {
    const IS_SIGNED: bool = false;
}

impl Number for i8 {
    const IS_SIGNED: bool = true;
}
impl Number for i16 {
    const IS_SIGNED: bool = true;
}
impl Number for i32 {
    const IS_SIGNED: bool = true;
}
impl Number for i64 {
    const IS_SIGNED: bool = true;
}
impl Number for i128 {
    const IS_SIGNED: bool = true;
}
impl Number for isize {
    const IS_SIGNED: bool = true;
}

impl Number for f32 {
    const IS_SIGNED: bool = true;
}
impl Number for f64 {
    const IS_SIGNED: bool = true;
}

pub struct NumberField<T: Number> {
    number: T,
    modifier: AtomicCell<Option<NumberModifier<T>>>,
}

impl<T: Number> From<T> for NumberField<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Number + Debug> Debug for NumberField<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.number)
    }
}

impl<T: Number + Display> Display for NumberField<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.number)
    }
}

impl<T: Number> ComponentField for NumberField<T> {
    // type Modifier = NumberModifier<T>;

    fn process_modifiers(&mut self) {
        let Some(modifier) = std::mem::replace(&mut self.modifier, Default::default()).into_inner()
        else {
            return;
        };
        match modifier {
            NumberModifier::Set(x) => self.number = x,
            NumberModifier::Add(x) => self.number += x,
            NumberModifier::Sub(x) => self.number -= x,
            NumberModifier::Mul(x) => self.number *= x,
            NumberModifier::Div(x) => self.number /= x,
        }
    }
}

impl<T: Number> NumberField<T> {
    pub fn new(number: T) -> Self {
        Self {
            number,
            modifier: AtomicCell::new(None),
        }
    }
    pub fn get_ref(&self) -> NumberFieldRef<T> {
        NumberFieldRef {
            number: self.number,
            reference: self,
            set_performed: true,
        }
    }

    fn queue_modifier(&self, modifier: NumberModifier<T>) {
        let Some(self_modifier) = self.modifier.load() else {
            self.modifier.store(Some(modifier));
            return;
        };
        match modifier {
            NumberModifier::Set(_) => self.modifier.store(Some(modifier)),
            NumberModifier::Add(b) => match self_modifier {
                NumberModifier::Add(a) => self.modifier.store(Some(NumberModifier::Add(a + b))),
                NumberModifier::Sub(a) => {
                    if a > b {
                        self.modifier.store(Some(NumberModifier::Sub(a - b)));
                    } else {
                        self.modifier.store(Some(NumberModifier::Add(b - a)));
                    }
                }
                _ => {}
            },
            NumberModifier::Sub(b) => match self_modifier {
                NumberModifier::Add(a) => {
                    if T::IS_SIGNED {
                        self.modifier.store(Some(NumberModifier::Add(a - b)));
                    } else if a > b {
                        self.modifier.store(Some(NumberModifier::Add(a - b)));
                    } else {
                        self.modifier.store(Some(NumberModifier::Sub(b - a)));
                    }
                }
                NumberModifier::Sub(a) => self.modifier.store(Some(NumberModifier::Sub(a + b))),
                _ => {}
            },
            NumberModifier::Mul(_) => {
                if matches!(
                    self_modifier,
                    NumberModifier::Set(_) | NumberModifier::Mul(_) | NumberModifier::Div(_)
                ) {
                    return;
                }
                todo!()
            }
            NumberModifier::Div(_) => {
                if matches!(
                    self_modifier,
                    NumberModifier::Set(_) | NumberModifier::Mul(_) | NumberModifier::Div(_)
                ) {
                    return;
                }
                todo!()
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct NumberFieldRef<'a, T: Number> {
    number: T,
    reference: &'a NumberField<T>,
    set_performed: bool,
}

impl<'a, T: Number> AddAssign<T> for NumberFieldRef<'a, T> {
    fn add_assign(&mut self, rhs: T) {
        self.number += rhs;
        if self.set_performed {
            self.reference
                .queue_modifier(NumberModifier::Set(self.number));
        } else {
            self.reference.queue_modifier(NumberModifier::Add(rhs));
        }
    }
}

impl<'a, T: Number> SubAssign<T> for NumberFieldRef<'a, T> {
    fn sub_assign(&mut self, rhs: T) {
        self.number -= rhs;
        if self.set_performed {
            self.reference
                .queue_modifier(NumberModifier::Set(self.number));
        } else {
            self.reference.queue_modifier(NumberModifier::Sub(rhs));
        }
    }
}

impl<'a, T: Number + PartialOrd> PartialOrd for NumberFieldRef<'a, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.number.partial_cmp(&other.number)
    }
}

impl<'a, T: Number + Ord> Ord for NumberFieldRef<'a, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.number.cmp(&other.number)
    }
}

impl<'a, T: Number + PartialEq> PartialEq for NumberFieldRef<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number
    }
}

impl<'a, T: Number + Eq> Eq for NumberFieldRef<'a, T> {}

impl<'a, T: Number + PartialOrd> PartialOrd<T> for NumberFieldRef<'a, T> {
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.number.partial_cmp(other)
    }
}

impl<'a, T: Number + PartialEq> PartialEq<T> for NumberFieldRef<'a, T> {
    fn eq(&self, other: &T) -> bool {
        &self.number == other
    }
}

impl<'a, T: Number> Deref for NumberFieldRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.number
    }
}

impl<'a, T: Number + Debug> Debug for NumberFieldRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.number)
    }
}

impl<'a, T: Number + Display> Display for NumberFieldRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.number)
    }
}

impl<'a, T: Number> NumberFieldRef<'a, T> {
    pub fn set(&mut self, value: T) {
        self.reference.queue_modifier(NumberModifier::Set(value));
        self.set_performed = true;
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

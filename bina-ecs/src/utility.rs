use std::{marker::{PhantomData, Tuple}, ops::{Deref, DerefMut}};

use crate::component::{Component, ComponentRef};


pub trait RefSet: Tuple {
    type Output: Tuple;
}


impl<A: Component> RefSet for (ComponentRef<A>,) {
    type Output = (A,);
}


pub struct Guard<'a, T: RefSet> {
    refs: T::Output,
    utility: Option<Utility<'a>>
}


impl<'a, T: RefSet> Deref for Guard<'a, T> {
    type Target = T::Output;

    fn deref(&self) -> &Self::Target {
        &self.refs
    }
}


impl<'a, T: RefSet> DerefMut for Guard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.refs
    }
}


impl<'a, T: RefSet> Guard<'a, T> {
    pub fn unlock(mut self) -> Utility<'a> {
        unsafe { self.utility.take().unwrap_unchecked() }
    }
}


impl<'a, T: RefSet> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        
    }
}


pub struct Utility<'a> {
    _phantom: PhantomData<&'a ()>
}


impl<'a> Utility<'a> {
    // pub fn lock<T: RefSet>(refs: T) -> Guard<T> {

    // }
}

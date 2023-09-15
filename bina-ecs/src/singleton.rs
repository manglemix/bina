use crate::universe::Universe;

pub trait Singleton: Send + Sync + 'static {
    fn get_void_ptr(&self) -> *const () {
        std::ptr::from_ref(self).cast()
    }
    // fn get_void_mut_ptr(&mut self) -> *mut () {
    //     std::ptr::from_mut(self).cast()
    // }
    fn process(&self, _universe: &Universe) {}
    fn flush(&mut self, _universe: &Universe) {}
}

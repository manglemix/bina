pub trait StaticReference {
    type Type: Sync;

    fn get() -> &'static Self::Type;
}

pub trait MutStaticReference {
    type Type: Sync;

    unsafe fn get() -> &'static Self::Type;
    unsafe fn get_mut() -> &'static mut Self::Type;
}
pub trait StaticReference {
    type Type: Sync + 'static;

    fn get() -> &'static Self::Type;
}

pub trait MutStaticReference {
    type Type: Sync + 'static;

    unsafe fn get() -> &'static Self::Type;
    unsafe fn get_mut() -> &'static mut Self::Type;
}
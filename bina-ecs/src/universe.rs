use std::{time::Instant, process::ExitCode};

use crate::{component::{Component, ComponentStore}, utility::Utility};
use crossbeam::atomic::AtomicCell;
use rayon::prelude::*;
use crate::reference::StaticReference;


// struct SafePtr<T: ?Sized>(*mut T);


// unsafe impl Sync for SafePtr<dyn GenericComponentStore> { }


pub trait RegisteredComponent: Component { }


struct VTable {
    process: Box<dyn Fn(f32, &AtomicCell<Option<ExitCode>>) + Sync>,
    flush: Box<dyn Fn() + Sync>
}

trait Lmao {

}

pub struct Universe {
    vtables: Vec<VTable>,
    fixed_delta: Option<f32>
}


impl Universe {
    pub fn new() -> Self {
        Self {
            vtables: Vec::new(),
            fixed_delta: None
        }
    }

    pub fn with_fixed_delta(&mut self, delta: f32) -> &mut Self {
        self.fixed_delta = Some(delta);
        self
    }

    pub fn register_component<T: RegisteredComponent>(&mut self) {
        self.vtables.push(VTable {
            process: Box::new(|delta, request_exit| unsafe {
                (*T::StoreRef::get().get()).process(delta, request_exit)
            }),
            flush: Box::new(|| unsafe { ComponentStore::<T>::flush() })
        });
    }

    pub fn run_fn_once(&mut self, f: impl FnOnce(Utility)) -> Result<(), ExitCode> {
        let request_exit = AtomicCell::new(None);
        f(Utility::new(&request_exit));
            
        self
            .vtables
            .par_iter()
            .for_each(|vtable| (vtable.flush)());

        request_exit.into_inner().map(Err).unwrap_or(Ok(()))
    }

    pub fn run(self) -> ExitCode {
        let request_exit = AtomicCell::new(None);

        let iter = |delta| {
            self
                .vtables
                .par_iter()
                .for_each(|vtable| (vtable.process)(delta, &request_exit));
            
            self
                .vtables
                .par_iter()
                .for_each(|vtable| (vtable.flush)());
        };

        if let Some(delta) = self.fixed_delta {
            loop {
                iter(delta);
                unsafe {
                    if let Some(code) = *request_exit.as_ptr() {
                        return code
                    }
                }
            }
        } else {
            let mut last = Instant::now();
            loop {
                let delta = last.elapsed().as_secs_f32();
                last = Instant::now();
                iter(delta);
                unsafe {
                    if let Some(code) = *request_exit.as_ptr() {
                        return code
                    }
                }
            }
        }
    }
}

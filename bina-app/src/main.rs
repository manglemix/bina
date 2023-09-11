#![feature(sync_unsafe_cell)]
#![feature(arbitrary_self_types)]

use std::any::Any;
use std::fmt::Debug;
use std::process::ExitCode;
use std::time::Instant;

use bina::ecs::entity::Entity;

// use bina::ecs::component::{Component, ComponentRef};
// use bina::ecs::locking::Guard;
// use bina::ecs::universe::Universe;
// use bina::ecs::utility::Utility;
// use bina::macros::{component, register_component};

#[derive(Debug)]
struct Lmao {
    runtime: f32,
}

// #[component]
// impl Component for Lmao {
//     fn process(mut self: Guard<ComponentRef<Self>>, delta: f32, utility: Utility) {
//         self.runtime += delta;
//         if self.runtime > 3.0 {
//             utility.request_exit(ExitCode::SUCCESS);
//         }
//         Guard::unlock(self);
//         // std::thread::sleep(std::time::Duration::from_millis(10));
//     }
// }

fn main() {}

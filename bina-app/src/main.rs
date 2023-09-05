#![feature(sync_unsafe_cell)]
#![feature(arbitrary_self_types)]

use std::process::ExitCode;
use std::time::Instant;

use bina::ecs::component::{Component, ComponentRef};
use bina::ecs::locking::Guard;
use bina::ecs::universe::Universe;
use bina::ecs::utility::Utility;
use bina::macros::{component, register_component};


struct Lmao {
    runtime: f32
}


#[component]
impl Component for Lmao {
    fn process(mut self: Guard<ComponentRef<Self>>, delta: f32, utility: Utility) {
        self.runtime += delta;
        if self.runtime > 3.0 {
            utility.request_exit(ExitCode::SUCCESS);
        }
        Guard::unlock(self);
        // std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn main() -> ExitCode {
    let start = Instant::now();
    let mut universe = Universe::new();
    register_component!(universe, Lmao);
    universe.run_fn_once(|utility| {
        utility.queue_add_unnamed_component(Lmao { runtime: 0.0 });
    }).unwrap();
    universe.with_fixed_delta(1.0 / 120.0);
    let code = universe.run();
    println!("{}", start.elapsed().as_secs_f64());
    code
}
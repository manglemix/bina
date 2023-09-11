use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

use bina::ecs::component::{ComponentCombination, Processable, Siblings};
use bina::ecs::entity::EntityReference;
use bina::ecs::universe::{Universe, LoopCount};
use bina::macros::{derive_component};

derive_component!{
    #[derive(Debug)]
    struct Lmao {
        #[improve]
        runtime: f64,
    }
}


impl ComponentCombination<(Lmao,)> for Lmao {}

impl Processable for Lmao {
    fn process<'a, S: Siblings>(mut component: Self::Reference<'a>, _siblings: S, _my_entity: EntityReference<'a, ()>, universe: &Universe) {
        component.runtime += universe.get_delta_accurate();
        // println!("{}", component.runtime);
        if component.runtime > 3.0 {
            universe.exit_ok();
        }
    }
}


fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut universe = Universe::default();
    universe.queue_add_entity((Lmao { runtime: 0.0.into() },));
    let start = Instant::now();
    let result = universe.loop_many(LoopCount::Forever, Duration::ZERO).unwrap();
    println!("{}", start.elapsed().as_secs_f32());
    result
}

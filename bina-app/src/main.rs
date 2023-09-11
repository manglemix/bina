use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

use bina::ecs::component::{ComponentCombination, Processable};
use bina::ecs::entity::EntityReference;
use bina::ecs::universe::{Universe, LoopCount, DeltaStrategy};
use bina::macros::derive_component;

derive_component! {
    #[derive(Debug)]
    struct Lmao {
        #[improve]
        runtime: f64,
        #[improve]
        count: usize,
    }
}


impl ComponentCombination<(Lmao,)> for Lmao {}

impl Processable for Lmao {
    fn process<E: bina::ecs::entity::Entity>(mut component: Self::Reference<'_>, _my_entity: EntityReference<E>, universe: &Universe) {
        component.runtime += universe.get_delta_accurate();
        component.count += 1;
        if component.runtime > 15.0 {
            println!("{}", component.count);
            universe.exit_ok();
        }
    }
}


fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut universe = Universe::default();
    universe.queue_add_entity((Lmao { runtime: 0.0.into(), count: 0.into() },));
    let start = Instant::now();
    let result = universe.loop_many(LoopCount::Forever, DeltaStrategy::RealDelta(Duration::from_millis(1))).unwrap();
    println!("{}", start.elapsed().as_secs_f32());
    result
}

use std::fmt::Debug;
use std::time::{Duration, Instant};

use bina::ecs::component::Processable;
use bina::ecs::entity::EntityReference;
use bina::ecs::tokio;
use bina::ecs::universe::{DeltaStrategy, LoopCount, Universe};
use bina::graphics::Graphics;
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

// load_image! {
//     TEST = "test.jpg"
// }

impl Processable for Lmao {
    fn process<E: bina::ecs::entity::Entity>(
        mut component: Self::Reference<'_>,
        _my_entity: EntityReference<E>,
        universe: &Universe,
    ) {
        component.runtime += universe.get_delta_accurate();
        component.count += 1;
        if component.runtime > 5.0 {
            println!("{}", component.count);
            universe.exit_ok();
        }
    }
}

#[tokio::main]
async fn main() {
    let mut universe = Universe::new();
    universe.queue_add_entity((Lmao {
        runtime: 0.0.into(),
        count: 0.into(),
    },));
    let x = TEST.to_image();

    Graphics::run(move |graphics| {
        universe.queue_set_singleton(graphics);
        let start = Instant::now();
        let result = universe
            .loop_many(
                LoopCount::Forever,
                DeltaStrategy::RealDelta(Duration::from_millis(1)),
            )
            .unwrap();
        println!("{}", start.elapsed().as_secs_f32());
        result
    })
    .await;
}

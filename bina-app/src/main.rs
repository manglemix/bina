use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use bina::ecs::component::Processable;
use bina::ecs::crossbeam::atomic::AtomicCell;
use bina::ecs::entity::EntityReference;
use bina::ecs::tokio;
use bina::ecs::universe::{DeltaStrategy, LoopCount, Universe};
use bina::graphics::image::{ImageFormat, Rgba};
use bina::graphics::polygon::{Polygon, TextureVertex};
use bina::graphics::texture::{CacheOption, TextureResource};
use bina::graphics::Graphics;
use bina::macros::derive_component;

derive_component! {
    #[derive(Debug)]
    struct Lmao {
        start: AtomicCell<Instant>,
        #[improve]
        runtime: f64,
        #[improve]
        count: usize,
        constructed: AtomicBool
    }
}

impl Processable for Lmao {
    fn process<E: bina::ecs::entity::Entity>(
        mut component: Self::Reference<'_>,
        _my_entity: EntityReference<E>,
        universe: &Universe,
    ) {
        component.runtime += universe.get_delta_accurate();
        if component.count == 0 {
            component.start.store(Instant::now());
        }
        if !component
            .constructed
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let graphics = universe.get_singleton::<Graphics>();
            if let Some(texture) = TEST_JPG.try_get(universe, graphics) {
                universe.queue_add_entity((Polygon::new(
                    graphics,
                    &[
                        TextureVertex {
                            x: 0.0,
                            y: 0.0,
                            tx: 0.0,
                            ty: 0.0,
                        },
                        TextureVertex {
                            x: 1.0,
                            y: 0.0,
                            tx: 0.1,
                            ty: 0.0,
                        },
                        // TextureVertex {
                        //     x: 10.0,
                        //     y: 10.0,
                        //     tx: 10.0,
                        //     ty: 10.0,
                        // },
                        TextureVertex {
                            x: 0.0,
                            y: 1.0,
                            tx: 0.0,
                            ty: 0.1,
                        },
                    ],
                    bina::graphics::polygon::Material::Texture(texture),
                ),))
            }
        }
        component.count += 1;
        if component.runtime > 15.0 {
            universe.exit_ok();
        }
    }
}

impl Drop for Lmao {
    fn drop(&mut self) {
        println!("{}", self.count);
        println!("{}", self.start.load().elapsed().as_secs_f32());
    }
}

static TEST_JPG: TextureResource<Rgba<u8>, 3060, 4080> =
    unsafe { TextureResource::new_file("test.jpg", ImageFormat::Jpeg, CacheOption::CacheForever) };

#[tokio::main]
async fn main() {
    let universe = Universe::new();
    universe.queue_add_entity((Lmao {
        start: AtomicCell::new(Instant::now()),
        runtime: 0.0.into(),
        count: 0.into(),
        constructed: AtomicBool::new(false),
    },));

    Graphics::run(
        universe,
        LoopCount::Forever,
        DeltaStrategy::RealDelta(Duration::from_millis(8)),
    )
    .await;
}

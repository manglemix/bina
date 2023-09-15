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
                            x: -0.0868241,
                            y: 0.49240386,
                            tx: 0.4131759,
                            ty: 0.00759614,
                        },
                        TextureVertex {
                            x: -0.49513406,
                            y: 0.06958647,
                            tx: 0.0048659444,
                            ty: 0.43041354,
                        },
                        TextureVertex {
                            x: -0.21918549,
                            y: -0.44939706,
                            tx: 0.28081453,
                            ty: 0.949397,
                        },
                        TextureVertex {
                            x: 0.35966998,
                            y: -0.3473291,
                            tx: 0.85967,
                            ty: 0.84732914,
                        },
                        TextureVertex {
                            x: 0.44147372,
                            y: 0.2347359,
                            tx: 0.9414737,
                            ty: 0.2652641,
                        },
                    ],
                    bina::graphics::polygon::Material::Texture(texture),
                ),))
            }
        }
        component.count += 1;
        if component.runtime > 5.0 {
            universe.exit_ok();
        }
    }
}

impl Drop for Lmao {
    fn drop(&mut self) {
        println!(
            "{}\n{}",
            self.count,
            self.start.load().elapsed().as_secs_f32()
        );
    }
}

static TEST_JPG: TextureResource<Rgba<u8>, 256, 256> =
    unsafe { TextureResource::new_file("test.png", ImageFormat::Png, CacheOption::DontCache) };

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
        DeltaStrategy::RealDelta(Duration::from_millis(5)),
    )
    .await;
}

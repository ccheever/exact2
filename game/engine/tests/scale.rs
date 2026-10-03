//! Release measurements at Grow a Garden's scale (game/diaries/004-garden.md):
//! plants with about 3.3 fruit each, fruit optionally parented to their plant.
//! `cargo test --release -p exact-game --test scale -- --ignored --nocapture`
use exact_game::*;

#[derive(Args, Default)]
struct Garden {
    plants: u32,
    parented: bool,
    walker: bool,
}
#[derive(Default, Component)]
struct Plant {
    kind: u32,
    age: f32,
}
#[derive(Default, Component)]
struct Fruit {
    weight: f32,
    ripe: bool,
}
impl Game for Garden {
    const ID: &'static str = "scale-garden";
    type Args = Garden;
    fn setup(w: &mut World, a: &Garden) {
        if a.walker {
            w.spawn_named("player", Transform::at(0.0, 0.0, 0.0));
        }
        for i in 0..a.plants {
            let at = Vec3::new((i % 250) as f32, 0.0, (i / 250) as f32);
            let plant = w.spawn((
                Transform::at(at.x, 0.0, at.z),
                Mesh::Box { size: Vec3::ONE },
                Material::rgb(0.2, 0.6, 0.2),
                Plant {
                    kind: i % 14,
                    age: 1.0,
                },
            ));
            for k in 0..(3 + (i % 3 == 0) as u32) {
                let offset = Vec3::new(k as f32 * 0.2, 1.0, 0.0);
                let fruit = (
                    Mesh::Sphere { radius: 0.1 },
                    Material::rgb(0.9, 0.1, 0.1),
                    Fruit {
                        weight: 1.0 + k as f32,
                        ripe: true,
                    },
                );
                if a.parented {
                    w.spawn((
                        Transform::at(offset.x, offset.y, offset.z),
                        Parent(plant),
                        fruit,
                    ));
                } else {
                    let p = at + offset;
                    w.spawn((Transform::at(p.x, p.y, p.z), fruit));
                }
            }
        }
    }
    fn tick(w: &mut World, _: &Input, a: &Garden) {
        if a.walker {
            w.require_mut::<Transform>("player").position.x += w.dt();
        }
    }
}

fn sim(plants: u32, parented: bool, walker: bool) -> Sim<Garden> {
    Sim::<Garden>::new(Garden {
        plants,
        parented,
        walker,
    })
    .unwrap()
}
fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// Diary limit 1 (50,000 plants, parented): 4.472 ms per live tick and 483 s for
/// an hour's seek; unparented 0.0054 ms. Measured here at load ~100–500 before
/// incremental propagation: 8.74 ms per live tick, 636 s per 10 minutes' seek
/// (17.3 ms and 849 s with a walking player); after: 0.0003 ms and 0.87 s (with
/// the walker 0.0035 ms and 0.96 s), unparented 0.0001 ms and 1.33 s.
#[test]
#[ignore = "release hierarchy cost measurement"]
fn parented_fruit_cost_what_unparented_fruit_cost() {
    for plants in [2_000, 10_000, 50_000] {
        for (parented, walker) in [(false, false), (true, false), (true, true)] {
            let mut s = sim(plants, parented, walker);
            let entities = s.world().len();
            let hz = 60.0;
            let mut now = 0.0;
            s.advance(now, Clock::Live);
            let mut samples = Vec::new();
            for _ in 0..600 {
                now += 1000.0 / hz;
                let start = std::time::Instant::now();
                s.advance(now, Clock::Live);
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            let start = std::time::Instant::now();
            // Ten simulated minutes, seekable, observed once at the end.
            s.run(600_000.0);
            let seek = start.elapsed().as_secs_f64();
            println!(
                "plants {plants} entities {entities} parented {parented} walker {walker}: \
                 live tick median {:.4} ms, 10 min seek {seek:.3} s",
                median(samples)
            );
        }
    }
}

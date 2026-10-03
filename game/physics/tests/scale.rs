// Cost of the retained query scene and the step at game scale. Measurements, not
// checks: run with `cargo test -p exact-game-physics --release --test scale --
// --ignored --nocapture`. The shapes follow the RIVALS and Forest diaries.
use exact_game::{Transform, Vec3, World};
use exact_game_physics::{self as physics, CapsuleController, Collider, Shape};
use std::time::Instant;

fn mean_us(n: usize, mut f: impl FnMut()) -> f64 {
    f();
    let start = Instant::now();
    for _ in 0..n {
        f();
    }
    start.elapsed().as_secs_f64() * 1e6 / n as f64
}

// An arena of ~55 static colliders and `rockets` collider-free movers: each rocket
// casts one ray, then moves. Batched casts every ray before any write.
#[test]
#[ignore = "measurement"]
fn rays_interleaved_with_unrelated_writes() {
    for rockets in [10, 50, 200, 500] {
        let mut w = World::new(120, 0);
        physics::register(&mut w);
        for i in 0..55 {
            let (x, z) = ((i % 11) as f32 * 4. - 20., (i / 11) as f32 * 8. - 16.);
            w.spawn((
                Transform::at(x, 1., z),
                Collider {
                    shape: Shape::Box {
                        half: Vec3::new(0.5, 1., 0.5),
                    },
                    ..Collider::default()
                },
            ));
        }
        let movers: Vec<_> = (0..rockets)
            .map(|i| w.spawn(Transform::at(i as f32 * 0.01, 1., 18.)))
            .collect();
        let interleaved = mean_us(200, || {
            for &m in &movers {
                let at = w.get::<Transform>(m).unwrap().position;
                std::hint::black_box(physics::raycast(&w, at, -Vec3::Z, 40., u32::MAX));
                w.get_mut::<Transform>(m).unwrap().position.x += 1e-4;
            }
        });
        let batched = mean_us(200, || {
            let q = physics::queries(&w);
            let mut at = Vec::with_capacity(movers.len());
            for &m in &movers {
                let p = w.get::<Transform>(m).unwrap().position;
                std::hint::black_box(q.raycast(p, -Vec3::Z, 40., u32::MAX));
                at.push(p);
            }
            drop(q);
            for &m in &movers {
                w.get_mut::<Transform>(m).unwrap().position.x += 1e-4;
            }
        });
        println!(
            "SCALE rays rockets={rockets} interleaved_us={interleaved:.1} batched_us={batched:.1}"
        );
    }
}

// The Forest's trunks: static offset cylinders over a heightfield, one capsule
// walking and a few collider-free movers (camera, sun) per tick.
fn forest(trees: usize) -> World {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    let side = (trees as f32).sqrt().ceil() as usize;
    let half = side as f32 * 2.5;
    let n = side * 2 + 1;
    let (_, shape) = Shape::heightfield(
        n as u32,
        n as u32,
        vec![0.; n * n],
        Vec3::new(2. * half, 1., 2. * half),
    )
    .unwrap();
    w.spawn((
        Transform::default(),
        Collider {
            shape,
            ..Collider::default()
        },
    ));
    for i in 0..trees {
        let (x, z) = ((i % side) as f32 * 5. - half, (i / side) as f32 * 5. - half);
        w.spawn((
            Transform {
                scale: Vec3::splat(0.75 + (i % 7) as f32 * 0.08),
                ..Transform::at(x + 2.5, 0., z + 2.5)
            },
            Collider {
                shape: Shape::Cylinder {
                    radius: 0.35,
                    height: 6.,
                },
                offset: Vec3::Y * 3.,
                ..Collider::default()
            },
        ));
    }
    w.spawn_named(
        "player",
        (Transform::at(0., 1., 0.), CapsuleController::default()),
    );
    w.spawn_named("camera", Transform::default());
    w.spawn_named("sun", Transform::default());
    w
}
fn forest_tick(w: &mut World) {
    {
        let mut c = w.require_mut::<CapsuleController>("player");
        c.velocity.y -= 9.81 / 60.;
    }
    physics::capsule(w, "player").step(Vec3::new(1.5, 0., 0.4));
    physics::step(w);
    let at = w.require::<Transform>("player").position;
    w.require_mut::<Transform>("camera").position = at + Vec3::new(0., 6., 8.);
    w.require_mut::<Transform>("sun").position.y += 1e-3;
}

#[test]
#[ignore = "measurement"]
fn forest_trunks_per_tick_and_save() {
    for trees in [1000, 5000, 20000] {
        let mut w = forest(trees);
        let components = w.save().len();
        for _ in 0..30 {
            forest_tick(&mut w);
        }
        let tick = mean_us(300, || forest_tick(&mut w)) / 1000.;
        let save = w.save().len();
        println!(
            "SCALE forest trees={trees} tick_ms={tick:.3} save_bytes={save} physics_bytes_per_tree={}",
            (save - components) / trees
        );
    }
}

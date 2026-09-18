use exact_game::{Transform, Vec3, World};
use exact_game_physics::{self as physics, Body, Collider, Physics, Shape};
use std::{hint::black_box, time::Instant};

fn median(samples: &mut [f64]) -> f64 {
    samples.sort_by(f64::total_cmp);
    (samples[samples.len() / 2 - 1] + samples[samples.len() / 2]) * 0.5
}

/// Run with --release --test timing -- --ignored --nocapture --test-threads=1.
/// Query batches include the first read after motion (and any scene rebuild).
#[test]
#[ignore = "600-tick timing diagnostic; no wall-clock assertion"]
fn static_scenery() {
    for (statics, dynamics) in [(100, 10), (2_000, 10), (20_000, 100)] {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        w.resource_mut::<Physics>().gravity = Vec3::ZERO;
        for i in 0..statics {
            w.spawn((
                Transform::at((i % 100) as f32 * 3., 0., (i / 100) as f32 * 3.),
                Collider::default(),
            ));
        }
        for i in 0..dynamics {
            w.spawn((
                Transform::at(i as f32 * 3., 5., 0.),
                Collider::default(),
                Body {
                    velocity: Vec3::X,
                    ..Body::default()
                },
            ));
        }
        let mut steps = Vec::new();
        let mut rays = Vec::new();
        let mut overlaps = Vec::new();
        for tick in 0..620 {
            let start = Instant::now();
            physics::step(&mut w);
            let step = start.elapsed().as_secs_f64() * 1e6;
            let view = physics::queries(&w);
            let start = Instant::now();
            for i in 0..1_000 {
                let slot = (i * 97 + tick) % statics;
                black_box(view.raycast(
                    Vec3::new((slot % 100) as f32 * 3., 10., (slot / 100) as f32 * 3.),
                    -Vec3::Y,
                    20.,
                    u32::MAX,
                ));
            }
            let ray = start.elapsed().as_secs_f64() * 1e6;
            let start = Instant::now();
            for i in 0..1_000 {
                let slot = (i * 97 + tick) % statics;
                black_box(view.overlap(
                    &Shape::default(),
                    Transform::at((slot % 100) as f32 * 3., 0., (slot / 100) as f32 * 3.),
                    u32::MAX,
                ));
            }
            let overlap = start.elapsed().as_secs_f64() * 1e6;
            if tick >= 20 {
                steps.push(step);
                rays.push(ray);
                overlaps.push(overlap);
            }
        }
        eprintln!(
            "S={statics} D={dynamics}: median step={:.3} us, 1000 rays={:.3} us, 1000 overlaps={:.3} us (600 ticks, 20 warmup)",
            median(&mut steps), median(&mut rays), median(&mut overlaps),
        );
        assert!(w.query::<&Body>().iter().all(|(_, b)| !b.asleep));
    }
}

use exact_game::{Entity, Parent, Transform, Vec3, World, PAGE};
use exact_game_physics::{self as physics, Body, Character, Collider, Physics, Shape};
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
        let mut scenery = Vec::new();
        for i in 0..statics {
            scenery.push(w.spawn((
                Transform::at((i % 100) as f32 * 3., 0., (i / 100) as f32 * 3.),
                Collider::default(),
            )));
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
                let hit = black_box(view.raycast(
                    Vec3::new((slot % 100) as f32 * 3., 10., (slot / 100) as f32 * 3.),
                    -Vec3::Y,
                    20.,
                    u32::MAX,
                ))
                .expect("timing ray must hit scenery or a body above it");
                assert!(hit.distance <= 10.);
            }
            let ray = start.elapsed().as_secs_f64() * 1e6;
            let start = Instant::now();
            for i in 0..1_000 {
                let slot = (i * 97 + tick) % statics;
                let hits = black_box(view.overlap(
                    &Shape::default(),
                    Transform::at((slot % 100) as f32 * 3., 0., (slot / 100) as f32 * 3.),
                    u32::MAX,
                ));
                assert_eq!(hits, [scenery[slot]]);
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

fn scenery(interleaved: bool) -> (World, Vec<Entity>) {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    w.resource_mut::<Physics>().gravity = Vec3::ZERO;
    let root = if interleaved {
        moving_body(&mut w, 0);
        Some(w.spawn(Transform::default()))
    } else {
        None
    };
    let mut statics = Vec::new();
    for i in 0..20_000 {
        if interleaved && w.len().is_multiple_of(PAGE) {
            moving_body(&mut w, i);
        }
        let e = w.spawn((
            Transform::at((i % 100) as f32 * 3., 0., (i / 100) as f32 * 3.),
            Collider::default(),
        ));
        if let Some(root) = root {
            w.insert(e, Parent(root));
        }
        statics.push(e);
    }
    (w, statics)
}
fn moving_body(w: &mut World, i: usize) {
    w.spawn((
        Transform::at((i % 100) as f32 * 3., 5., (i / 100) as f32 * 3.),
        Collider::default(),
        Body {
            velocity: Vec3::X,
            ..Default::default()
        },
    ));
}
fn check(w: &World, e: Entity) {
    let view = physics::queries(w);
    let p = w.get::<Transform>(e).unwrap().position;
    assert_eq!(
        view.raycast(p + Vec3::Y, -Vec3::Y, 2., 1).unwrap().entity,
        e
    );
    assert_eq!(
        view.overlap(
            &Shape::default(),
            Transform {
                position: p,
                ..Default::default()
            },
            1
        ),
        [e]
    );
}
#[test]
#[ignore = "character and churn timing; no wall-clock assertion"]
fn characters_and_churn() {
    for (count, churn, interleaved) in [
        (1, false, false),
        (8, false, false),
        (1, true, false),
        (1, false, true),
    ] {
        let (mut w, statics) = scenery(interleaved);
        let characters: Vec<_> = (0..count)
            .map(|i| {
                w.spawn((
                    Transform::at(i as f32 * 3., 1.41, 0.),
                    Character::default(),
                    Collider {
                        layer: 2,
                        ..Default::default()
                    },
                ))
            })
            .collect();
        for &e in &characters {
            physics::move_character(&mut w, e, Vec3::ZERO);
        }
        let mut steps = Vec::new();
        let mut moves = Vec::new();
        let mut reads = Vec::new();
        for tick in 0..140 {
            if churn {
                let e = w.spawn(());
                w.despawn(e);
            }
            let start = Instant::now();
            for &e in &characters {
                physics::move_character(&mut w, e, Vec3::X);
            }
            let movement = start.elapsed().as_secs_f64() * 1e6;
            let start = Instant::now();
            physics::step(&mut w);
            let step = start.elapsed().as_secs_f64() * 1e6;
            let start = Instant::now();
            check(&w, statics[(tick * 97) % statics.len()]);
            let read = start.elapsed().as_secs_f64() * 1e6;
            if tick >= 20 {
                moves.push(movement);
                steps.push(step);
                reads.push(read);
            }
        }
        for &e in &characters {
            assert!(w.get::<Transform>(e).unwrap().position.x > 2.);
        }
        eprintln!("S=20000 characters={count} churn={churn} interleaved={interleaved}: median character batch={:.3} us, step={:.3} us, checked ray+overlap={:.3} us (120 ticks, 20 warmup)", median(&mut moves), median(&mut steps), median(&mut reads));
    }
}
#[test]
#[ignore = "1000 individual static teleports; no wall-clock assertion"]
fn static_edits() {
    let (w, statics) = scenery(false);
    check(&w, statics[0]);
    let mut samples = Vec::new();
    let total = Instant::now();
    for i in 0..1_000 {
        let e = statics[(i * 97) % statics.len()];
        let start = Instant::now();
        w.get_mut::<Transform>(e).unwrap().position.y += 0.1;
        check(&w, e);
        samples.push(start.elapsed().as_secs_f64() * 1e6);
    }
    eprintln!("S=20000, 1000 single-static teleports + checked ray/overlap: total={:.3} ms median={:.3} us", total.elapsed().as_secs_f64()*1e3, median(&mut samples));
}
fn rss_kib() -> usize {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
#[ignore = "Linux retained heightfield RSS/timing diagnostic; run alone"]
fn retained_terrain() {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    w.spawn((
        Transform::default(),
        Collider {
            shape: Shape::Heightfield {
                rows: 1024,
                cols: 1024,
                heights: vec![0.; 1024 * 1024],
                scale: Vec3::new(1000., 1., 1000.),
            },
            ..Default::default()
        },
    ));
    let player = w.spawn((Transform::at(0., 0.91, 0.), Character::default()));
    let before = rss_kib();
    let start = Instant::now();
    assert!(physics::raycast(&w, Vec3::Y, -Vec3::Y, 2., 1).is_some());
    let query_us = start.elapsed().as_secs_f64() * 1e6;
    let query = rss_kib();
    let start = Instant::now();
    physics::move_character(&mut w, player, Vec3::X);
    eprintln!("1024x1024 heightfield: first ray={query_us:.3} us first character={:.3} us, RSS world={before} query={query} character={} KiB", start.elapsed().as_secs_f64()*1e6, rss_kib());
}

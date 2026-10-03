//! Where the engine's limits are, measured. The plain tests assert behaviour
//! (hitscan on moving capsules, no tunneling); the ignored ones are timings:
//! `cargo test -p rivals-logic --release --test limits -- --ignored --nocapture`.
use exact_game::*;
use exact_game::{InputEvent, PointerPhase};
use exact_game_physics::{self as physics, Body, Collider, Shape};
use rivals_logic::weapons::{self, Rocket};
use rivals_logic::{Options, Rivals};
use std::time::Instant;

fn quantile(mut v: Vec<f64>, q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q) as usize]
}
fn ffa(bots: u32) -> Sim<Rivals> {
    let mut sim = Sim::<Rivals>::new(Options {
        seed: 11,
        bots,
        ..Options::default()
    })
    .unwrap();
    sim.viewport(1280.0, 720.0);
    sim
}
/// Per-tick wall time over `ticks` ticks, in microseconds: (mean, p50, p99, max).
fn time_ticks(sim: &mut Sim<Rivals>, ticks: u32) -> (f64, f64, f64, f64) {
    let ms = 1000.0 / 120.0;
    let mut samples = Vec::new();
    for _ in 0..ticks {
        let t = Instant::now();
        sim.run(ms);
        samples.push(t.elapsed().as_secs_f64() * 1e6);
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let max = samples.iter().cloned().fold(0.0, f64::max);
    (mean, quantile(samples.clone(), 0.5), quantile(samples, 0.99), max)
}

#[test]
#[ignore]
fn bench_bots() {
    for bots in [1, 3, 7, 11, 15, 23] {
        let mut sim = ffa(bots);
        sim.key_down("KeyF");
        time_ticks(&mut sim, 120);
        let (mean, p50, p99, max) = time_ticks(&mut sim, 1200);
        let hits = sim.world().resource::<rivals_logic::round::Round>().log.len();
        println!(
            "bots {bots:>2}: tick mean {mean:>7.1} µs  p50 {p50:>7.1}  p99 {p99:>7.1}  max {max:>8.1}  (hits {hits}, budget 8333 µs)"
        );
    }
}

/// Rockets fired from the arena's middle in a fan, refilled to `n` in flight.
fn keep_rockets(sim: &mut Sim<Rivals>, n: usize, k: &mut u32) {
    let w = sim.world_mut();
    let now = w.seconds() as f32;
    let flying = w.query::<&Rocket>().iter().count();
    for _ in flying..n {
        *k += 1;
        let a = *k as f32 * 2.399;
        let dir = Vec3::new(math::cos(a), 0.35 + 0.3 * math::sin(*k as f32), math::sin(a)).normalize();
        // Owner 31 is nobody: rockets damage every fighter they find.
        weapons::spawn_rocket(w, 31, Vec3::new(0.0, 3.5, 0.0) + dir * 3.2, dir * 12.0, now);
    }
}

#[test]
#[ignore]
fn bench_rockets() {
    for n in [0usize, 50, 200, 500, 1000] {
        let mut sim = ffa(7);
        let mut k = 0;
        let ms = 1000.0 / 120.0;
        let mut samples = Vec::new();
        for i in 0..600 {
            keep_rockets(&mut sim, n, &mut k);
            let t = Instant::now();
            sim.run(ms);
            if i >= 60 {
                samples.push(t.elapsed().as_secs_f64() * 1e6);
            }
        }
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        println!(
            "7 bots + {n:>4} rockets: tick mean {mean:>8.1} µs  p99 {:>8.1}  (spawned {k})",
            quantile(samples, 0.99)
        );
    }
}

/// The batched rocket step (all rays, then all writes) against the obvious loop
/// that writes each rocket's pose after its ray. Any Transform write invalidates
/// the shared query scene, so the obvious loop rebuilds it once per rocket.
#[test]
#[ignore]
fn bench_query_scene_invalidation() {
    for n in [10usize, 50, 200, 500] {
        let mut sim = ffa(7);
        let mut k = 0;
        keep_rockets(&mut sim, n, &mut k);
        sim.run(10.0);
        let w = sim.world_mut();
        let rockets: Vec<(Entity, Vec3, Vec3)> = w
            .query::<(&Rocket, &Transform)>()
            .iter()
            .map(|(e, (r, t))| (e, t.position, r.velocity / 120.0))
            .collect();
        let t = Instant::now();
        {
            let q = physics::queries(w);
            for (_, at, step) in &rockets {
                std::hint::black_box(q.raycast(*at, *step, step.length(), u32::MAX));
            }
        }
        for (e, at, step) in &rockets {
            w.require_mut::<Transform>(*e).position = *at + *step;
        }
        let batched = t.elapsed().as_secs_f64() * 1e6;
        let t = Instant::now();
        for (e, at, step) in &rockets {
            std::hint::black_box(physics::raycast(w, *at, *step, step.length(), u32::MAX));
            w.require_mut::<Transform>(*e).position = *at + *step * 2.0;
        }
        let naive = t.elapsed().as_secs_f64() * 1e6;
        std::hint::black_box(physics::raycast(w, Vec3::ZERO, Vec3::X, 10.0, u32::MAX));
        let t = Instant::now();
        std::hint::black_box(physics::raycast(w, Vec3::ZERO, Vec3::X, 10.0, u32::MAX));
        let cached = t.elapsed().as_secs_f64() * 1e6;
        w.require_mut::<Transform>("camera").position.y += 0.001;
        let t = Instant::now();
        std::hint::black_box(physics::raycast(w, Vec3::ZERO, Vec3::X, 10.0, u32::MAX));
        let rebuilt = t.elapsed().as_secs_f64() * 1e6;
        println!(
            "{n:>4} rockets: batched {batched:>8.1} µs, ray-then-write {naive:>9.1} µs; one ray: cached {cached:.1} µs, after moving the camera {rebuilt:.1} µs"
        );
    }
}

/// Hitscan against a capsule strafing at twice sprint speed: a ray per tick for
/// two seconds, aimed at the capsule, must hit it every time, and the hit point
/// must tell head from body.
#[test]
fn hitscan_tracks_a_strafing_capsule() {
    let mut sim = ffa(1);
    sim.run(100.0);
    let (mut hits, mut heads) = (0, 0);
    for i in 0..240 {
        let w = sim.world_mut();
        // ±2 m at up to 24 m/s.
        let x = 2.0 * math::sin(i as f32 * 0.1);
        w.teleport(w.named("bot-1").unwrap(), Transform::at(x, 0.92, 13.0));
        let eye = Vec3::new(0.0, 1.62, 18.0);
        let head = i % 2 == 0;
        let target = Vec3::new(x, if head { 1.62 } else { 0.92 }, 13.0);
        let shot = weapons::hitscan(w, eye, target - eye, 40.0, 1 << 1).unwrap();
        assert_eq!(shot.fighter, w.named("bot-1"), "tick {i}");
        hits += 1;
        heads += usize::from(shot.point.y - 0.92 >= rivals_logic::fighter::HEAD_FROM);
        sim.run(1000.0 / 120.0);
    }
    assert_eq!((hits, heads), (240, 120));
}

/// The engine's raycast against a capsule collider misses rays aimed straight at
/// it: Parry 0.30.2 casts rays on capsules with GJK on the support map, which
/// gives up on a few configurations. The analytic test in `fighter.rs` agrees
/// with geometry everywhere. Recorded, not fixed (the game works around it).
#[test]
fn parry_capsule_rays_miss() {
    let mut w = World::new(120, 1);
    physics::register(&mut w);
    let c = w.spawn((
        Transform::at(0.0, 0.92, 13.0),
        Collider {
            shape: Shape::Capsule { radius: 0.35, height: 1.8 },
            layer: 4,
            ..Collider::default()
        },
    ));
    let eye = Vec3::new(0.0, 1.62, 18.0);
    let (mut misses, mut analytic, mut first) = (0, 0, None);
    let n = 4000;
    for i in 0..n {
        let x = -2.0 + 4.0 * i as f32 / n as f32;
        w.require_mut::<Transform>(c).position.x = x;
        for dy in [0.0f32, 0.3, -0.3, 0.6] {
            let target = Vec3::new(x, 0.92 + dy, 13.0);
            if physics::raycast(&w, eye, target - eye, 40.0, 4).is_none() {
                misses += 1;
                first.get_or_insert((x, dy));
            }
            let dir = (target - eye).normalize();
            analytic += usize::from(rivals_logic::fighter::ray_capsule(eye, dir, Vec3::new(x, 0.92, 13.0)).is_none());
        }
    }
    println!("engine raycast missed {misses} of {} rays aimed inside a capsule (first at {first:?}); analytic missed {analytic}", n * 4);
    assert_eq!(analytic, 0);
    assert!(misses > 0, "Parry's capsule raycast no longer misses: delete the workaround");
}

/// A rocket at 2 km/s (17 m per tick) still stops at a 0.2 m wall: it sweeps.
#[test]
fn swept_rockets_do_not_tunnel() {
    let mut sim = ffa(1);
    sim.run(100.0);
    let w = sim.world_mut();
    let wall = Mesh::cuboid(Vec3::new(4.0, 4.0, 0.2));
    w.spawn((Transform::at(-19.0, 2.0, -15.5), Collider::of(&wall), wall));
    let now = w.seconds() as f32;
    weapons::spawn_rocket(w, 31, Vec3::new(-19.0, 2.0, 15.0), Vec3::new(0.0, 0.0, -2000.0), now);
    sim.run(100.0);
    let w = sim.world();
    assert_eq!(w.query::<&Rocket>().iter().count(), 0, "rocket exploded");
    let blast = w
        .query::<(&weapons::Effect, &Transform)>()
        .iter()
        .find(|(_, (fx, _))| fx.grow > 0.0)
        .map(|(_, (_, t))| t.position)
        .expect("an explosion");
    assert!((blast.z + 15.4).abs() < 0.1, "exploded at the wall's face: {blast}");
}

/// The engine's Body has no CCD switch, yet Rapier's dynamic spheres stop at a
/// 5 cm static wall even at 5 km/s (42 m per tick): no tunneling to record.
#[test]
fn dynamic_bodies_do_not_tunnel_through_thin_walls() {
    struct Shot;
    impl Game for Shot {
        const ID: &'static str = "rivals-shot";
        const HZ: u32 = 120;
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            physics::register(w);
            let wall = Mesh::cuboid(Vec3::new(4.0, 4.0, 0.05));
            w.spawn((Transform::at(0.0, 0.0, -5.0), Collider::of(&wall), wall));
            for (i, speed) in [20.0f32, 200.0, 1000.0, 5000.0].into_iter().enumerate() {
                w.spawn_named(
                    format!("ball-{i}"),
                    (
                        Transform::at(i as f32 * 0.5 - 0.75, 0.0, 0.0),
                        Collider {
                            shape: Shape::Sphere { radius: 0.05 },
                            ..Collider::default()
                        },
                        Body {
                            velocity: Vec3::new(0.0, 0.0, -speed),
                            gravity: 0.0,
                            ..Body::default()
                        },
                    ),
                );
            }
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            physics::step(w);
        }
    }
    let mut sim = Sim::<Shot>::new(()).unwrap();
    sim.run(2000.0);
    let ends: Vec<f32> = (0..4)
        .map(|i| sim.local_position(format!("ball-{i}").as_str()).unwrap().z)
        .collect();
    println!("ball z after 2 s at 20/200/1000/5000 m/s: {ends:?}");
    // The wall's face is at z = -4.975; a resting ball's centre is 5 cm before it.
    assert!(ends.iter().all(|z| (*z + 4.925).abs() < 0.01), "{ends:?}");
}



/// Input to displayed pose, through the engine's own live clock: a display
/// callback every frame (`Clock::Live` with the frame period), mouse events
/// arriving between frames, and the pose the renderer would draw (the last two
/// ticks blended by `Sim::alpha`). Returns per trial the delay to the first
/// visible change and to the whole turn, in ms; and for a steady 2,000 pt/s
/// mouse, the coefficient of variation of the per-frame turn (judder).
fn live_latency<G: Game<Args = Options>>(display_hz: f64) -> (Vec<f64>, Vec<f64>, f64) {
    let mut sim = Sim::<G>::new(Options { seed: 3, bots: 3, range: true, ..Options::default() }).unwrap();
    sim.viewport(1280.0, 720.0);
    let period = 1000.0 / display_hz;
    sim.frame_period(period);
    let mut t = 0.0;
    let mut yaws: Vec<f32> = Vec::new();
    let yaw_of = |w: &World| w.require::<rivals_logic::fighter::Fighter>("player").yaw;
    let mut frame = |sim: &mut Sim<G>, t: f64, yaws: &mut Vec<f32>| {
        sim.advance_with(t, exact_game::Clock::Live, |w, _| yaws.push(yaw_of(w)));
        let n = yaws.len();
        let (a, b) = (yaws[n.saturating_sub(2)], yaws[n - 1]);
        a + math::wrap_angle(b - a) * sim.alpha()
    };
    yaws.push(yaw_of(sim.world()));
    let (mut x, mut seed) = (640.0f32, 0x2545F491u32);
    sim.input(InputEvent::Pointer { id: 1, phase: PointerPhase::Move, x, y: 360.0, at_ms: 0.0 });
    for _ in 0..120 {
        frame(&mut sim, t, &mut yaws);
        t += period;
    }
    let (mut first, mut whole) = (Vec::new(), Vec::new());
    for _ in 0..200 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let at = t - period + period * (seed >> 8) as f64 / (1u32 << 24) as f64;
        let shown = frame(&mut sim, t - period + 1e-6, &mut yaws);
        x += 40.0;
        sim.input(InputEvent::Pointer { id: 1, phase: PointerPhase::Move, x, y: 360.0, at_ms: at });
        let goal = shown - 40.0 * rivals_logic::DEFAULT_SENSITIVITY;
        let (mut f, mut w) = (None, None);
        let mut ft = t;
        for _ in 0..40 {
            let now = frame(&mut sim, ft, &mut yaws);
            if f.is_none() && math::wrap_angle(now - shown).abs() > 1e-6 {
                f = Some(ft - at);
            }
            if math::wrap_angle(now - goal).abs() < 1e-5 {
                w = Some(ft - at);
                break;
            }
            ft += period;
        }
        let end = frame(&mut sim, ft, &mut yaws);
        first.push(f.expect("the turn shows"));
        whole.push(w.unwrap_or_else(|| panic!("the turn completes: shown {shown} goal {goal} end {end}")));
        t = ft + 10.0 * period;
        frame(&mut sim, t - period, &mut yaws);
    }
    // Steady motion: a 1 kHz mouse at 2,000 points per second for two seconds.
    let mut deltas = Vec::new();
    let mut last = frame(&mut sim, t, &mut yaws);
    let start = t;
    let mut event = start;
    while t < start + 2000.0 {
        t += period;
        while event < t {
            event += 1.0;
            x += 2.0;
            sim.input(InputEvent::Pointer { id: 1, phase: PointerPhase::Move, x, y: 360.0, at_ms: event });
        }
        let now = frame(&mut sim, t, &mut yaws);
        if t > start + 200.0 {
            deltas.push(math::wrap_angle(now - last) as f64);
        }
        last = now;
    }
    let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
    let sd = (deltas.iter().map(|d| (d - mean) * (d - mean)).sum::<f64>() / deltas.len() as f64).sqrt();
    (first, whole, (sd / mean).abs())
}

#[test]
fn mouse_latency_by_tick_and_display_rate() {
    use rivals_logic::rates::{Rivals240, Rivals30, Rivals60};
    let mut rows = Vec::new();
    for display in [60.0, 120.0, 144.0] {
        for (hz, (first, whole, judder)) in [
            (30, live_latency::<Rivals30>(display)),
            (60, live_latency::<Rivals60>(display)),
            (120, live_latency::<Rivals>(display)),
            (240, live_latency::<Rivals240>(display)),
        ] {
            let (f50, f95) = (quantile(first.clone(), 0.5), quantile(first, 0.95));
            let (w50, w95) = (quantile(whole.clone(), 0.5), quantile(whole, 0.95));
            println!(
                "display {display:>3} Hz, tick {hz:>3} Hz: first change p50 {f50:5.1} ms p95 {f95:5.1}; whole turn p50 {w50:5.1} p95 {w95:5.1}; steady-turn judder {judder:.3}"
            );
            rows.push((display, hz, w95, judder));
        }
    }
    // A completed turn never takes more than a display frame plus two ticks.
    for (display, hz, w95, _) in rows {
        assert!(w95 <= 1000.0 / display + 2000.0 / hz as f64 + 0.5, "{display} {hz} {w95}");
    }
}

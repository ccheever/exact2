//! Where the engine's limits are, measured. The plain tests assert behaviour
//! (hitscan on moving capsules, no tunneling); the ignored ones are timings:
//! `cargo test -p rivals-logic --release --test limits -- --ignored --nocapture`.
use exact_game::*;
use exact_game::{InputEvent, PointerPhase};
use exact_game_physics::{self as physics, Body, Collider, Shape};
use rivals_logic::weapons::{self, Rocket};
use rivals_logic::{Options, Rivals};
// Wall time belongs only to these ignored measurements, never to game state.
#[allow(clippy::disallowed_types)]
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
/// One live display frame at 120 Hz: exactly one tick, without the seekable
/// clock's settle observation (a whole-world comparison per advance).
fn frame(sim: &mut Sim<Rivals>) {
    let next = sim.world().seconds() * 1000.0 + 1000.0 / 120.0;
    sim.advance(next, exact_game::Clock::Live);
}
fn live(mut sim: Sim<Rivals>) -> Sim<Rivals> {
    sim.frame_period(1000.0 / 120.0);
    sim.advance(0.0, exact_game::Clock::Live);
    sim
}
/// Per-tick wall time over `ticks` ticks, in microseconds: (mean, p50, p99, max).
#[allow(clippy::disallowed_types)]
fn time_ticks(sim: &mut Sim<Rivals>, ticks: u32) -> (f64, f64, f64, f64) {
    let mut samples = Vec::new();
    let mut max = 0.0;
    let mut worst = (0, 0, 0);
    for _ in 0..ticks {
        let before = sim.world().resource::<rivals_logic::round::Round>().number;
        let t = Instant::now();
        frame(sim);
        let elapsed = t.elapsed().as_secs_f64() * 1e6;
        samples.push(elapsed);
        if elapsed > max {
            max = elapsed;
            worst = (
                sim.world().tick(),
                before,
                sim.world().resource::<rivals_logic::round::Round>().number,
            );
        }
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    println!(
        "slowest of {ticks} ticks: tick {}, round {} -> {}, {max:.1} µs",
        worst.0, worst.1, worst.2
    );
    (
        mean,
        quantile(samples.clone(), 0.5),
        quantile(samples, 0.99),
        max,
    )
}

#[test]
#[ignore]
fn bench_bots() {
    for bots in [1, 3, 7, 11, 15, 24] {
        let mut sim = live(ffa(bots));
        sim.key_down("KeyF");
        time_ticks(&mut sim, 120);
        let (mean, p50, p99, max) = time_ticks(&mut sim, 1200);
        let hits = sim
            .world()
            .resource::<rivals_logic::round::Round>()
            .log
            .len();
        println!(
            "bots {bots:>2}: tick mean {mean:>7.1} µs  p50 {p50:>7.1}  p99 {p99:>7.1}  max {max:>8.1}  (hits {hits}, budget 8333 µs)"
        );
        // Exercise the actual round transition even when this fight's winner
        // arrives too late for the measured ten seconds.
        sim.world_mut()
            .resource_mut::<rivals_logic::round::Round>()
            .over_until = sim.world().seconds() as f32;
        let (mean, _, p99, max) = time_ticks(&mut sim, 120);
        println!("bots {bots:>2}, round reset and first second: mean {mean:.1} µs  p99 {p99:.1}  max {max:.1}");
    }
}

/// The seekable clock (agents, proofs, `Sim::run`) observes the final tick pair
/// of every advance to answer `settle`; the live clock does not. Same fight, both.
#[test]
#[ignore]
#[allow(clippy::disallowed_types)]
fn bench_seekable_observation() {
    for bots in [7, 23] {
        let mut seek = ffa(bots);
        seek.key_down("KeyF");
        let t = Instant::now();
        for _ in 0..600 {
            seek.run(1000.0 / 120.0);
        }
        let seekable = t.elapsed().as_secs_f64() * 1e6 / 600.0;
        let mut sim = live(ffa(bots));
        sim.key_down("KeyF");
        let t = Instant::now();
        for _ in 0..600 {
            frame(&mut sim);
        }
        let live = t.elapsed().as_secs_f64() * 1e6 / 600.0;
        println!(
            "{bots:>2} bots, one tick per advance: seekable {seekable:.1} µs, live {live:.1} µs"
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
        let dir = Vec3::new(
            math::cos(a),
            0.35 + 0.3 * math::sin(*k as f32),
            math::sin(a),
        )
        .normalize();
        // Owner 31 is nobody: rockets damage every fighter they find.
        weapons::spawn_rocket(w, 31, Vec3::new(0.0, 3.5, 0.0) + dir * 3.2, dir * 12.0, now);
    }
}

#[test]
#[ignore]
#[allow(clippy::disallowed_types)]
fn bench_rockets() {
    for n in [0usize, 50, 200, 500, 1000] {
        let mut sim = live(ffa(7));
        let mut k = 0;
        let mut samples = Vec::new();
        for i in 0..600 {
            keep_rockets(&mut sim, n, &mut k);
            let t = Instant::now();
            frame(&mut sim);
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
#[allow(clippy::disallowed_types)]
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

/// A rocket at 2 km/s (17 m per tick) still stops at a 0.2 m wall: it sweeps.
#[test]
fn swept_rockets_do_not_tunnel() {
    let mut sim = ffa(1);
    sim.run(100.0);
    let w = sim.world_mut();
    let wall = Mesh::cuboid(Vec3::new(4.0, 4.0, 0.2));
    w.spawn((Transform::at(-19.0, 2.0, -15.5), Collider::of(&wall), wall));
    let now = w.seconds() as f32;
    weapons::spawn_rocket(
        w,
        31,
        Vec3::new(-19.0, 2.0, 15.0),
        Vec3::new(0.0, 0.0, -2000.0),
        now,
    );
    sim.run(100.0);
    let w = sim.world();
    assert_eq!(w.query::<&Rocket>().iter().count(), 0, "rocket exploded");
    let blast = w
        .query::<(&weapons::Effect, &Transform)>()
        .iter()
        .find(|(_, (fx, _))| fx.grow > 0.0)
        .map(|(_, (_, t))| t.position)
        .expect("an explosion");
    assert!(
        (blast.z + 15.4).abs() < 0.1,
        "exploded at the wall's face: {blast}"
    );
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

/// A locked mouse's motion: `dx` points, the position unchanged.
fn motion<G: Game>(sim: &mut Sim<G>, at_ms: f64, dx: f32) {
    sim.input(InputEvent::Pointer {
        id: 1,
        phase: PointerPhase::Move,
        x: 640.0,
        y: 360.0,
        dx,
        dy: 0.0,
        buttons: 0,
        at_ms,
    });
}

/// Input to displayed pose, through the engine's own live clock: a display
/// callback every frame (`Clock::Live` with the frame period), mouse events
/// arriving between frames, and the pose the renderer would draw (the last two
/// ticks blended by `Sim::alpha`). Returns per trial the delay to the first
/// visible change and to the whole turn, in ms; and for a steady 2,000 pt/s
/// mouse, the coefficient of variation of the per-frame turn (judder).
fn live_latency<G: Game<Args = Options>>(display_hz: f64, look: bool) -> (Vec<f64>, Vec<f64>, f64) {
    let mut sim = Sim::<G>::new(Options {
        seed: 3,
        bots: 3,
        ..Options::default()
    })
    .unwrap();
    // Measure an unlimited camera session, not the range's thirty-second drill.
    // Stationary bots keep combat from killing the observer during the sweep.
    for (_, brain) in sim
        .world_mut()
        .query::<&mut rivals_logic::bots::Brain>()
        .iter()
    {
        brain.dummy = true;
    }
    sim.viewport(1280.0, 720.0);
    let period = 1000.0 / display_hz;
    sim.frame_period(period);
    let mut t = 0.0;
    let mut yaws: Vec<f32> = Vec::new();
    let yaw_of = |w: &World| w.require::<rivals_logic::fighter::Fighter>("player").yaw;
    let frame = |sim: &mut Sim<G>, t: f64, yaws: &mut Vec<f32>| {
        sim.advance_with(t, exact_game::Clock::Live, |w, _| yaws.push(yaw_of(w)));
        let n = yaws.len();
        let (a, b) = (yaws[n.saturating_sub(2)], yaws[n - 1]);
        // The renderer's MouseLook adds the motion no drawn tick shows yet.
        let unshown = if look { sim.unshown_motion().x } else { 0.0 };
        a + math::wrap_angle(b - a) * sim.alpha() - unshown * rivals_logic::DEFAULT_SENSITIVITY
    };
    yaws.push(yaw_of(sim.world()));
    let mut seed = 0x2545F491u32;
    for _ in 0..120 {
        frame(&mut sim, t, &mut yaws);
        t += period;
    }
    let (mut first, mut whole) = (Vec::new(), Vec::new());
    for _ in 0..200 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let at = t - period + period * (seed >> 8) as f64 / (1u32 << 24) as f64;
        let shown = frame(&mut sim, t - period + 1e-6, &mut yaws);
        motion(&mut sim, at, 40.0);
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
        whole.push(
            w.unwrap_or_else(|| panic!("the turn completes: shown {shown} goal {goal} end {end}")),
        );
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
            motion(&mut sim, event, 2.0);
        }
        let now = frame(&mut sim, t, &mut yaws);
        if t > start + 200.0 {
            deltas.push(math::wrap_angle(now - last) as f64);
        }
        last = now;
    }
    let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
    let sd =
        (deltas.iter().map(|d| (d - mean) * (d - mean)).sum::<f64>() / deltas.len() as f64).sqrt();
    (first, whole, (sd / mean).abs())
}

#[test]
fn mouse_latency_by_tick_and_display_rate() {
    use rivals_logic::rates::{Rivals240, Rivals30, Rivals60};
    let mut rows = Vec::new();
    for look in [false, true] {
        for display in [60.0, 120.0, 144.0] {
            for (hz, (first, whole, judder)) in [
                (30, live_latency::<Rivals30>(display, look)),
                (60, live_latency::<Rivals60>(display, look)),
                (120, live_latency::<Rivals>(display, look)),
                (240, live_latency::<Rivals240>(display, look)),
            ] {
                let (f50, f95) = (quantile(first.clone(), 0.5), quantile(first, 0.95));
                let (w50, w95) = (quantile(whole.clone(), 0.5), quantile(whole, 0.95));
                println!(
                    "{} display {display:>3} Hz, tick {hz:>3} Hz: first change p50 {f50:5.1} ms p95 {f95:5.1}; whole turn p50 {w50:5.1} p95 {w95:5.1}; steady-turn judder {judder:.3}",
                    if look { "MouseLook" } else { "ticks only" }
                );
                rows.push((look, display, hz, w95, judder));
            }
        }
    }
    for (look, display, hz, w95, _) in rows {
        // Ticks alone: a turn completes within a display frame plus two ticks.
        // With MouseLook: within the next frame, whatever the rates.
        let bound = 1000.0 / display + if look { 0.0 } else { 2000.0 / hz as f64 } + 0.5;
        assert!(
            w95 <= bound,
            "look {look} display {display} tick {hz}: {w95} ms"
        );
    }
}

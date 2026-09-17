//! Release measurements: clocks never enter simulation state.
use exact_game::{Transform, Vec3, World};
use exact_game_physics::{self as physics, Body, Collider, Physics, Shape};
use std::time::Instant;
#[path = "../tests/common/mod.rs"]
mod common;
fn box_at(w: &mut World, p: Vec3, half: Vec3, dynamic: bool) {
    let e = w.spawn((
        Transform {
            position: p,
            ..Transform::default()
        },
        Collider {
            shape: Shape::Box { half },
            ..Collider::default()
        },
    ));
    if dynamic {
        w.insert(e, Body::default());
    }
}
fn percentiles(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n == 0 {
        (f64::NAN, f64::NAN)
    } else {
        (v[n / 2], v[(n * 95 / 100).min(n - 1)])
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--verify") {
        let mut s = common::scene("pile");
        for t in 1..=600 {
            common::tick(&mut s, t);
        }
        println!("PILE_HASH_600=0x{:016x}", s.world().hash());
        let mut a = common::scene("bounce");
        for t in 1..=45 {
            common::tick(&mut a, t);
        }
        let mut b = common::scene("bounce");
        b.restore(&a.save()).unwrap();
        common::tick(&mut b, 45);
        for t in 46..=240 {
            common::tick(&mut a, t);
            common::tick(&mut b, t);
            assert_eq!(a.world().hash(), b.world().hash(), "resume tick {t}");
        }
        println!("MID_BOUNCE_RESUME=exact ticks=45..240");
        return;
    }
    let counts: Vec<usize> = args.iter().filter_map(|s| s.parse().ok()).collect();
    let limit = std::env::var("PILE_TICKS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2400);
    for count in if counts.is_empty() {
        vec![1000, 2000, 5000]
    } else {
        counts
    } {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        // Same finite 24 m bin and 100-box/12-tick pour as the first attempt.
        for (p, h) in [
            (Vec3::new(0.0, -0.5, 0.0), Vec3::new(6.0, 0.5, 6.0)),
            (Vec3::new(-6.0, 12.0, 0.0), Vec3::new(0.5, 12.0, 6.5)),
            (Vec3::new(6.0, 12.0, 0.0), Vec3::new(0.5, 12.0, 6.5)),
            (Vec3::new(0.0, 12.0, -6.0), Vec3::new(6.0, 12.0, 0.5)),
            (Vec3::new(0.0, 12.0, 6.0), Vec3::new(6.0, 12.0, 0.5)),
        ] {
            box_at(&mut w, p, h, false);
        }
        let (mut poured, mut sleep_tick) = (0, 0);
        let (mut previous, mut observed_tick) = (None, None);
        let mut active = Vec::new();
        let mut asleep = Vec::new();
        let mut snapshots = Vec::new();
        let mut snapshot_bytes = 0;
        for tick in 0..limit {
            if tick % 300 == 0 {
                eprintln!("boxes={count} tick={tick} poured={poured}");
            }
            if poured < count && tick % 12 == 0 {
                for _ in 0..100.min(count - poured) {
                    let i = poured;
                    box_at(
                        &mut w,
                        Vec3::new(
                            ((i % 10) as f32 - 4.5) * 1.02,
                            4.0 + (i / 100) as f32 * 1.02,
                            (((i / 10) % 10) as f32 - 4.5) * 1.02,
                        ),
                        Vec3::splat(0.5),
                        true,
                    );
                    poured += 1;
                }
            }
            let sleeping = poured == count && physics::quiescent(&w);
            let start = Instant::now();
            physics::step(&mut w);
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            // Resources are deliberately excluded, exactly as in Sim's observation.
            // Transform and Body are the only changing components in this fixture.
            let components: Vec<_> = w
                .query::<(&Transform, &Body)>()
                .iter()
                .map(|(e, (t, b))| (e, (*t, b.clone())))
                .collect();
            let hash = exact_game::hash::of(&components);
            if poured == count && previous == Some(hash) && observed_tick.is_none() {
                observed_tick = Some(tick);
            }
            previous = Some(hash);
            if sleeping {
                if sleep_tick == 0 {
                    sleep_tick = tick;
                }
                asleep.push(ms);
            } else if poured == count {
                active.push(ms);
            }
            if poured == count && snapshots.len() < 10 {
                let start = Instant::now();
                let bytes = w.resource::<Physics>().refresh_snapshot();
                snapshots.push(start.elapsed().as_secs_f64() * 1000.0);
                snapshot_bytes = bytes;
            }
            if asleep.len() >= 180 {
                break;
            }
        }
        println!("observed_still_tick={observed_tick:?}");
        let a = percentiles(active);
        let s = percentiles(asleep);
        let sleep_seconds = if sleep_tick == 0 {
            f64::NAN
        } else {
            (sleep_tick as f64 - ((count - 1) / 100 * 12) as f64) / 60.0
        };
        println!("boxes={count} active_ms={:.3}/{:.3} asleep_ms={:.3}/{:.3} sleep_tick={sleep_tick} sleep_after_last_pour_s={sleep_seconds:.3}",a.0,a.1,s.0,s.1);
        if !snapshots.is_empty() {
            let first = snapshots[0];
            let p = percentiles(snapshots);
            println!(
                "snapshot_active_ms={:.3}/{:.3} first_ms={first:.3} rapier_bytes={snapshot_bytes}",
                p.0, p.1
            );
        }
    }
}

//! Release throughput diagnostic; time measures the call, never enters the world.
use exact_game::{Transform, Vec3, World};
use exact_game_physics::{self as physics, Body, Collider, Shape};
use std::time::Instant;
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
fn percentiles(mut times: Vec<f64>) -> (f64, f64) {
    times.sort_by(f64::total_cmp);
    let n = times.len();
    (times[n / 2], times[(n * 95 / 100).min(n - 1)])
}
fn main() {
    for count in [1000, 2000] {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        box_at(
            &mut w,
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(6.0, 0.5, 6.0),
            false,
        );
        for (p, h) in [
            (Vec3::new(-6.0, 12.0, 0.0), Vec3::new(0.5, 12.0, 6.5)),
            (Vec3::new(6.0, 12.0, 0.0), Vec3::new(0.5, 12.0, 6.5)),
            (Vec3::new(0.0, 12.0, -6.0), Vec3::new(6.0, 12.0, 0.5)),
            (Vec3::new(0.0, 12.0, 6.0), Vec3::new(6.0, 12.0, 0.5)),
        ] {
            box_at(&mut w, p, h, false);
        }
        let mut active = Vec::new();
        let mut sleeping = Vec::new();
        let mut poured = 0;
        let mut sleep_tick = 0;
        for tick in 0..2400 {
            if tick % 300 == 0 {
                eprintln!("pile boxes={count} tick={tick} poured={poured}");
            }
            if poured < count && tick % 12 == 0 {
                for _ in 0..100.min(count - poured) {
                    let x = (poured % 10) as f32 - 4.5;
                    let z = ((poured / 10) % 10) as f32 - 4.5;
                    box_at(
                        &mut w,
                        Vec3::new(x * 1.02, 4.0 + (poured / 100) as f32 * 1.02, z * 1.02),
                        Vec3::splat(0.5),
                        true,
                    );
                    poured += 1;
                }
            }
            let asleep = physics::quiescent(&w) && poured == count;
            let start = Instant::now();
            physics::step(&mut w);
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            if asleep {
                if sleep_tick == 0 {
                    sleep_tick = tick;
                }
                sleeping.push(ms);
            } else if poured == count {
                active.push(ms);
            }
            if sleeping.len() >= 180 {
                break;
            }
        }
        let a = percentiles(active);
        if sleeping.is_empty() {
            println!(
                "boxes={count} active p50={:.3}ms p95={:.3}ms NEVER_SLEPT",
                a.0, a.1
            );
        } else {
            let s = percentiles(sleeping);
            println!("boxes={count} active p50={:.3}ms p95={:.3}ms asleep p50={:.3}ms p95={:.3}ms sleep_tick={sleep_tick}",a.0,a.1,s.0,s.1);
        }
    }
}

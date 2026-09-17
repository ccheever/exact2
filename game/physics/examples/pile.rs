//! Release throughput diagnostic; clocks and allocation counters never enter the world.
use exact_game::{Transform, Vec3, World};
use exact_game_physics::{self as physics, Body, Collider, Shape};
use rapier3d::{pipeline::PhysicsWorld, prelude::*};
#[path = "../tests/common/mod.rs"]
mod common;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
// The example alone wraps the system allocator; no allocation policy changes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;
const PHASES: [&str; 13] = [
    "gather",
    "broadphase",
    "narrowphase",
    "matching",
    "islands",
    "warm",
    "solve",
    "integrate",
    "relax",
    "restitution",
    "sleep",
    "writeback",
    "other",
];
#[derive(Default)]
struct Profile {
    ms: [f64; 13],
    counts: [usize; 4],
    sub: [[f64; 4]; 32],
    samples: usize,
}
impl Profile {
    fn step(&mut self, w: &mut World) {
        let mut last = Instant::now();
        physics::step_observed(w, |phase, count| {
            let now = Instant::now();
            let ms = now.duration_since(last).as_secs_f64() * 1000.0;
            last = now;
            let i = PHASES.iter().position(|p| *p == phase).unwrap_or(12);
            self.ms[i] += ms;
            if (5..=8).contains(&i) {
                self.sub[count][i - 5] += ms;
            }
            match phase {
                "broadphase" => self.counts[0] += count,
                "narrowphase" => self.counts[1] += count,
                "touching" => self.counts[2] += count,
                "points" => self.counts[3] += count,
                _ => {}
            }
        });
        self.samples += 1;
    }
    fn print(&self) {
        let n = self.samples.max(1) as f64;
        for (name, ms) in PHASES.iter().zip(self.ms) {
            println!("phase {name:12} {:.4} ms", ms / n);
        }
        for (i, ms) in self
            .sub
            .iter()
            .enumerate()
            .filter(|(_, ms)| ms.iter().any(|v| *v > 0.0))
        {
            println!(
                "substep {i} warm={:.4} solve={:.4} integrate={:.4} relax={:.4}",
                ms[0] / n,
                ms[1] / n,
                ms[2] / n,
                ms[3] / n
            );
        }
        println!("counts candidates={:.0} narrow_tests={:.0} touching={:.0} points={:.0} profile_samples={}", self.counts[0] as f64/n, self.counts[1] as f64/n, self.counts[2] as f64/n, self.counts[3] as f64/n, self.samples);
    }
}
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
fn bin() -> [(Vec3, Vec3); 5] {
    [
        (Vec3::new(0.0, -0.5, 0.0), Vec3::new(6.0, 0.5, 6.0)),
        (Vec3::new(-6.0, 12.0, 0.0), Vec3::new(0.5, 12.0, 6.5)),
        (Vec3::new(6.0, 12.0, 0.0), Vec3::new(0.5, 12.0, 6.5)),
        (Vec3::new(0.0, 12.0, -6.0), Vec3::new(6.0, 12.0, 0.5)),
        (Vec3::new(0.0, 12.0, 6.0), Vec3::new(6.0, 12.0, 0.5)),
    ]
}
fn position(i: usize) -> Vec3 {
    Vec3::new(
        ((i % 10) as f32 - 4.5) * 1.02,
        4.0 + (i / 100) as f32 * 1.02,
        (((i / 10) % 10) as f32 - 4.5) * 1.02,
    )
}
fn percentiles(mut times: Vec<f64>) -> (f64, f64) {
    times.sort_by(f64::total_cmp);
    let n = times.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    (times[n / 2], times[(n * 95 / 100).min(n - 1)])
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--verify") {
        let mut scene = common::scene("pile");
        for tick in 1..=600 {
            common::tick(&mut scene, tick);
        }
        println!("PILE_HASH_600=0x{:016x}", scene.world().hash());
        let mut original = common::scene("bounce");
        for tick in 1..=45 {
            common::tick(&mut original, tick);
        }
        let ball = original.world().named("ball").unwrap();
        assert!(original.world().get::<Body>(ball).unwrap().velocity.y > 0.0);
        let mut restored = common::scene("bounce");
        restored.restore(&original.save()).unwrap();
        common::tick(&mut restored, 45);
        for tick in 46..=240 {
            common::tick(&mut original, tick);
            common::tick(&mut restored, tick);
            assert_eq!(
                original.world().hash(),
                restored.world().hash(),
                "mid-bounce resume tick {tick}"
            );
        }
        println!("MID_BOUNCE_RESUME=exact ticks=45..240");
        return;
    }
    let rapier = args.iter().any(|a| a == "--rapier");
    let limit = std::env::var("PILE_TICKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2400);
    let counts: Vec<usize> = args.iter().filter_map(|v| v.parse().ok()).collect();
    for count in if counts.is_empty() {
        vec![1000, 2000, 5000]
    } else {
        counts
    } {
        let mut w = World::new(60, 0);
        physics::register(&mut w);
        let mut oracle = PhysicsWorld::new();
        for (p, h) in bin() {
            box_at(&mut w, p, h, false);
            oracle.insert_collider(
                ColliderBuilder::cuboid(h.x, h.y, h.z)
                    .translation(Vector::from_array(p.to_array()))
                    .friction(0.6),
                None,
            );
        }
        let mut handles = Vec::new();
        let mut active = Vec::new();
        let mut sleeping = Vec::new();
        let mut allocations = 0;
        let mut profile = Profile::default();
        let mut poured = 0;
        let mut sleep_tick = 0;
        for tick in 0..limit {
            if tick % 300 == 0 {
                eprintln!(
                    "{} boxes={count} tick={tick} poured={poured}",
                    if rapier { "rapier" } else { "exact" }
                );
            }
            if poured < count && tick % 12 == 0 {
                for _ in 0..100.min(count - poured) {
                    let p = position(poured);
                    if rapier {
                        let (h, _) = oracle.insert(
                            RigidBodyBuilder::dynamic()
                                .translation(Vector::from_array(p.to_array())),
                            ColliderBuilder::cuboid(0.5, 0.5, 0.5)
                                .density(1000.0)
                                .friction(0.6),
                        );
                        handles.push(h);
                    } else {
                        box_at(&mut w, p, Vec3::splat(0.5), true);
                    }
                    poured += 1;
                }
            }
            let asleep = poured == count
                && if rapier {
                    handles.iter().all(|h| oracle.bodies[*h].is_sleeping())
                } else {
                    physics::quiescent(&w)
                };
            let measured = !rapier && !asleep && poured == count && tick % 30 == 0;
            let before = ALLOCS.load(Ordering::Relaxed);
            let start = Instant::now();
            if rapier {
                oracle.step();
            } else if measured {
                profile.step(&mut w);
            } else {
                physics::step(&mut w);
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            let allocs = ALLOCS.load(Ordering::Relaxed) - before;
            if asleep {
                if sleep_tick == 0 {
                    sleep_tick = tick;
                }
                sleeping.push(ms);
            } else if poured == count && !measured {
                active.push(ms);
                allocations += allocs;
            }
            if sleeping.len() >= 180 {
                break;
            }
        }
        let n = active.len().max(1) as f64;
        let a = percentiles(active);
        let s = percentiles(sleeping);
        println!("{} boxes={count} active p50={:.3} p95={:.3} asleep p50={:.3} p95={:.3} sleep_tick={sleep_tick} allocations/tick={:.0}",if rapier {"rapier"} else {"exact"},a.0,a.1,s.0,s.1,allocations as f64/n);
        if !rapier {
            profile.print();
            let mut speeds: Vec<_> = w
                .query::<&Body>()
                .iter()
                .filter(|(_, b)| !b.asleep)
                .map(|(_, b)| (b.velocity.length(), b.spin.length(), b.calm))
                .collect();
            speeds.sort_by(|a, b| a.0.total_cmp(&b.0));
            if !speeds.is_empty() {
                println!(
                    "awake={} median_v={:.5} max_v={:.5} max_spin={:.5} min_calm={}",
                    speeds.len(),
                    speeds[speeds.len() / 2].0,
                    speeds.last().unwrap().0,
                    speeds.iter().map(|s| s.1).fold(0.0f32, f32::max),
                    speeds.iter().map(|s| s.2).min().unwrap()
                );
            }
        }
    }
}

//! Where the forest stops scaling, hostless: setup, tick, save and restore cost
//! against tree and wolf counts. Diagnostic, never a check.
//!
//!   cargo run --release --manifest-path game/games/forest/.shells/Cargo.toml \
//!     -p forest-logic --example scale -- trees 1000,5000,20000,100000 [lite] [primitives]
//!   ... --example scale -- wolves 8,64,512,4096 [lite]
use exact_game::{Clock, Sim};
use forest_logic::camp::DAY;
use forest_logic::forest::Grove;
use forest_logic::{Forest, Options};
use std::time::Instant;

const TICK: f64 = 1000.0 / 60.0;

fn baked(name: &str) -> Result<Vec<u8>, String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
    std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))
}

fn rss_mib() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0)
        / 1024.0
}

fn quantile(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

/// Time `n` live-clock frames of one tick each while the player walks east.
/// The live clock skips the seekable path's per-advance world observation.
fn ticks(sim: &mut Sim<Forest>, now: &mut f64, n: usize) -> (f64, f64, f64) {
    let mut ms = Vec::with_capacity(n);
    sim.key_down("KeyD");
    for _ in 0..n {
        *now += TICK;
        let t = Instant::now();
        sim.advance(*now, Clock::Live);
        ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    sim.key_up("KeyD");
    let mean = ms.iter().sum::<f64>() / n as f64;
    (mean, quantile(&mut ms, 0.5), quantile(&mut ms, 0.95))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let axis = args.first().map(String::as_str).unwrap_or("trees");
    let counts: Vec<u32> = args
        .get(1)
        .map(|s| s.split(',').map(|n| n.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![1000, 5000, 20000]);
    let lite = args.iter().any(|a| a == "lite");
    let primitives = args.iter().any(|a| a == "primitives");
    for n in counts {
        let (trees, wolves) = if axis == "wolves" { (5000, n) } else { (n, 8) };
        let base = rss_mib();
        let t = Instant::now();
        let mut sim = Sim::<Forest>::with_assets(
            Options {
                trees,
                wolves,
                lite,
                primitives,
                ..Options::default()
            },
            baked,
        )
        .unwrap();
        let setup = t.elapsed().as_secs_f64() * 1000.0;
        let rss = rss_mib() - base;
        let entities = sim.world().len();
        let half = sim.world().resource::<Grove>().half;
        let mut now = 0.0;
        sim.advance(now, Clock::Live);
        let day = ticks(&mut sim, &mut now, 300);
        // Jump to night: the Deer and every wolf awake.
        sim.world_mut()
            .resource_mut::<forest_logic::camp::Cycle>()
            .t = DAY + 1.0;
        let night = ticks(&mut sim, &mut now, 300);
        // The agent and test path: a seekable one-tick run observes the world.
        let t = Instant::now();
        for _ in 0..10 {
            sim.run(TICK);
        }
        let seek_ms = t.elapsed().as_secs_f64() * 100.0;
        let t = Instant::now();
        let hash = sim.world().hash();
        let hash_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let saved = sim.save().unwrap();
        let save_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let mut fresh = Sim::<Forest>::with_assets(
            Options {
                trees,
                wolves,
                lite,
                primitives,
                ..Options::default()
            },
            baked,
        )
        .unwrap();
        fresh.restore(&saved).unwrap();
        let restore_ms = t.elapsed().as_secs_f64() * 1000.0 - setup;
        assert_eq!(fresh.world().hash(), hash, "restore changed the hash");
        // One felled tree: a despawn and, with Rapier, a collider removal.
        let fell = {
            let g = sim.world().resource::<Grove>();
            (0..g.hp.len()).find(|&c| g.hp[c] > 0).unwrap() as u32
        };
        sim.world_mut().resource_mut::<Grove>().hp[fell as usize] = 1;
        let t = Instant::now();
        forest_logic::forest::fell(sim.world_mut(), fell);
        sim.run(TICK);
        let fell_ms = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{{\"trees\":{trees},\"wolves\":{wolves},\"lite\":{lite},\"primitives\":{primitives},\"side_m\":{:.0},\"entities\":{entities},\"setup_ms\":{setup:.1},\"rss_mib\":{rss:.1},\"day_tick_ms\":[{:.3},{:.3},{:.3}],\"night_tick_ms\":[{:.3},{:.3},{:.3}],\"seek_tick_ms\":{seek_ms:.2},\"hash_ms\":{hash_ms:.2},\"save_bytes\":{},\"save_ms\":{save_ms:.1},\"restore_ms\":{restore_ms:.1},\"fell_tick_ms\":{fell_ms:.2}}}",
            half * 2.0,
            day.0,
            day.1,
            day.2,
            night.0,
            night.1,
            night.2,
            saved.len()
        );
    }
}

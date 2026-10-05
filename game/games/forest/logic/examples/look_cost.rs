//! What the declared look (`motion.look`) costs per present against the Rust it
//! replaced (`look.rs`), hostless, on the same world states. Diagnostic, never
//! a check.
//!
//!   cargo run --release --manifest-path game/games/forest/.shells/Cargo.toml \
//!     -p forest-logic --example look_cost -- 2000,20000
use exact_game::{Clock, Sim, Transform, Vec3};
use forest_logic::forest::Grove;
use forest_logic::{motion, Forest, Options};

const TICK: f64 = 1000.0 / 60.0;

fn quantile(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

fn game(trees: u32) -> Sim<Forest> {
    let mut sim = Sim::<Forest>::baked(Options {
        trees,
        art: "pass".into(),
        ..Options::default()
    });
    sim.advance(0.0, Clock::Live);
    let tree = {
        let g = sim.world().resource::<Grove>();
        let c = (0..g.hp.len())
            .filter(|&c| g.hp[c] > 0)
            .nth(trees as usize / 3)
            .unwrap();
        g.at(c as u32)
    };
    let w = sim.world_mut();
    let e = w.resolve("player").unwrap();
    let t = *w.require::<Transform>(e);
    w.teleport(
        e,
        Transform {
            position: Vec3::new(tree.x, tree.y + 0.95, tree.z + 1.5),
            ..t
        },
    );
    sim
}

fn main() {
    let counts: Vec<u32> = std::env::args()
        .nth(1)
        .map(|s| s.split(',').map(|n| n.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![2000, 20000]);
    motion::spent();
    for trees in counts {
        for (scene, key) in [("walking", Some("KeyD")), ("standing", None)] {
            let mut rust = game(trees);
            let mut declared = game(trees);
            let mut now = 0.0;
            if let Some(key) = key {
                rust.key_down(key);
                declared.key_down(key);
            }
            let (mut a, mut b) = (Vec::new(), Vec::new());
            let (mut run, mut kept, mut reads, mut rows) = (0, 0, 0, 0);
            let mut per_rule: Vec<(String, f64)> = Vec::new();
            for frame in 0..400 {
                now += TICK;
                motion::use_rust(true);
                rust.advance(now, Clock::Live);
                let ra = motion::spent();
                motion::use_rust(false);
                declared.advance(now, Clock::Live);
                let rb = motion::spent();
                let (stats, _) = motion::stats();
                if frame >= 100 {
                    a.extend(ra);
                    b.extend(rb);
                    run += stats.run;
                    kept += stats.kept;
                    reads += stats.reads;
                    rows += stats.rows;
                    for (k, (name, us)) in motion::rule_us().into_iter().enumerate() {
                        if per_rule.len() <= k {
                            per_rule.push((name, 0.0));
                        }
                        per_rule[k].1 += us / 300.0;
                    }
                }
            }
            assert!(motion::errors().is_empty(), "{:?}", motion::errors());
            let n = a.len().max(1) as f64;
            let m = b.len().max(1) as f64;
            println!(
                "{{\"trees\":{trees},\"scene\":\"{scene}\",\"rust_us\":[{:.1},{:.1},{:.1}],\"declared_us\":[{:.1},{:.1},{:.1}],\"rules_run\":{:.2},\"rules_kept\":{:.2},\"row_reads\":{:.0},\"rows\":{:.0}}}",
                a.iter().sum::<f64>() / n,
                quantile(&mut a, 0.5),
                quantile(&mut a, 0.95),
                b.iter().sum::<f64>() / m,
                quantile(&mut b, 0.5),
                quantile(&mut b, 0.95),
                run as f64 / m,
                kept as f64 / m,
                reads as f64 / m,
                rows as f64 / m,
            );
            let parts: Vec<String> = per_rule
                .iter()
                .map(|(n, us)| format!("{n} {us:.0}µs"))
                .collect();
            println!("  per rule: {}", parts.join(", "));
        }
    }
}

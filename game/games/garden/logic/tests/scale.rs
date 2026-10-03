//! The measurements in the diary. Ignored by default; run in release:
//! `cargo test --release --manifest-path game/games/garden/.shells/Cargo.toml
//!  -p garden-logic --test scale -- --ignored --nocapture --test-threads 1`
use exact_game::{Args, Clock, Sim};
use garden_logic::farm::Farm;
use garden_logic::garden::{Census, Schedule};
use garden_logic::{Garden, Options};
use std::time::Instant;

fn new(smooth: bool) -> Sim<Garden> {
    Sim::<Garden>::new(Options {
        seed: 1,
        smooth,
        ..Options::default()
    })
    .unwrap()
}

fn send(game: &mut Sim<Garden>, cmd: &str) -> f64 {
    let o = Options {
        seed: 1,
        cmd: cmd.into(),
        cmd_id: game.args().cmd_id + 1,
        smooth: game.args().smooth,
        ..Options::default()
    };
    game.bind(&o.values(), None).unwrap();
    let t = Instant::now();
    game.run(1000.0 / 30.0);
    t.elapsed().as_secs_f64() * 1000.0
}

/// Mean and worst milliseconds per display frame over `frames` live 60 Hz
/// frames: what a host pays per frame for the simulation (no rendering, and
/// no observation, which only seekable advances do). Two frames in one
/// carry a 30 Hz tick.
fn frames(game: &mut Sim<Garden>, frames: u32) -> (f64, f64) {
    let mut sum = 0.0f64;
    let mut worst = 0.0f64;
    let start = game.world().tick() as f64 * 1000.0 / 30.0 + 1.0;
    let ticked = game.world().tick();
    for i in 1..=frames {
        let t = Instant::now();
        game.advance(start + i as f64 * 1000.0 / 60.0, Clock::Live);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        sum += ms;
        worst = worst.max(ms);
    }
    let ticked = game.world().tick() - ticked;
    assert!(
        ticked + 2 >= frames as u64 / 2,
        "{frames} live frames ran {ticked} ticks"
    );
    (sum / frames as f64, worst)
}

/// One seekable single-tick run: a tick plus the agent's rest observation.
fn observed_tick(game: &mut Sim<Garden>) -> f64 {
    let t = Instant::now();
    game.run(1000.0 / 30.0);
    t.elapsed().as_secs_f64() * 1000.0
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn sizes() -> Vec<u32> {
    std::env::var("GARDEN_SIZES")
        .ok()
        .map(|s| s.split(',').map(|n| n.parse().unwrap()).collect())
        .unwrap_or(vec![100, 500, 2_000, 10_000, 50_000])
}

#[test]
#[ignore]
fn entity_ramp() {
    println!("| plants | entities | fill ms | frame ms mean/max (stepped) | frame ms mean/max (smooth) | observed tick ms | frame ms mean/max (mature) | +1 h seek ms | ms/tick in seek | events | save B | save ms | restore ms | hash ms |");
    for n in sizes() {
        let mut game = new(false);
        let fill = send(&mut game, &format!("fill {n}"));
        // Into the growing phase: every plant is between stages.
        game.run(5_000.0);
        let stepped = frames(&mut game, 600);
        let observed = observed_tick(&mut game);
        let mut smooth = new(true);
        send(&mut smooth, &format!("fill {n}"));
        smooth.run(5_000.0);
        let smoothed = frames(&mut smooth, 600);
        drop(smooth);
        let before = game.world().resource::<Schedule>().processed;
        let t = Instant::now();
        game.run(3_600_000.0);
        let hour = ms(t);
        let events = game.world().resource::<Schedule>().processed - before;
        let entities = game.world().len();
        // A mature garden: every fruit on the vine, nothing due.
        let mature = frames(&mut game, 600);
        let t = Instant::now();
        let saved = game.save().unwrap();
        let save = ms(t);
        let t = Instant::now();
        let mut back = new(false);
        back.restore(&saved).unwrap();
        let restore = ms(t);
        let t = Instant::now();
        let h = game.world().hash();
        let hash = ms(t);
        assert_eq!(h, back.world().hash());
        println!(
            "| {n} | {entities} | {fill:.1} | {:.3} / {:.2} | {:.3} / {:.2} | {observed:.2} | {:.3} / {:.2} | {hour:.0} | {:.4} | {events} | {} | {save:.1} | {restore:.1} | {hash:.1} |",
            stepped.0,
            stepped.1,
            smoothed.0,
            smoothed.1,
            mature.0,
            mature.1,
            hour / 108_000.0,
            saved.len()
        );
    }
}

#[test]
#[ignore]
fn economy_at_scale() {
    println!("| plants | ripe | harvest all ms | backpack publish (in harvest) | sell all ms | away 8 h ms | away events |");
    for n in sizes() {
        let mut game = new(false);
        send(&mut game, &format!("fill {n}"));
        game.run(20.0 * 60_000.0);
        let ripe = game.world().resource::<Census>().ripe;
        let harvest = send(&mut game, "harvest all");
        let bag = game.world().resource::<Farm>().bag.len();
        let sell = send(&mut game, "sell all");
        let t = Instant::now();
        let before = game.world().resource::<Schedule>().processed;
        send(&mut game, "away 28800");
        let away = ms(t);
        let events = game.world().resource::<Schedule>().processed - before;
        println!("| {n} | {ripe} | {harvest:.1} | bag {bag} | {sell:.1} | {away:.1} | {events} |");
    }
}

/// A tick that publishes the backpack: the cost of one more fruit in a bag
/// of N, which republishes the whole list.
#[test]
#[ignore]
fn backpack_republish() {
    println!(
        "| bag | harvest one ms (republishes bag) | frame ms idle (live) | observed tick ms idle |"
    );
    for n in sizes() {
        let mut game = new(false);
        send(&mut game, &format!("fill {n}"));
        game.run(20.0 * 60_000.0);
        send(&mut game, "harvest all");
        // Regrown fruit ripen; harvesting one fruit at the player's tile
        // republishes the whole backpack.
        game.run(5.0 * 60_000.0);
        let bag = game.world().resource::<Farm>().bag.len();
        game.tap("KeyE");
        let t = Instant::now();
        game.run(1000.0 / 30.0);
        let one = ms(t);
        let idle = frames(&mut game, 120);
        let observed = observed_tick(&mut game);
        game.world_mut().resource_mut::<Farm>().bag_dirty = true;
        game.run(1000.0 / 30.0);
        let record = game.take_published().map_or(0, |r| r.len());
        println!(
            "| {bag} | {one:.2} | {:.3} | {observed:.2} | {record} |",
            idle.0
        );
    }
}

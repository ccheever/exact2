//! The character controller under a shooter's demands: ramps, jumping onto
//! cover, sliding along walls without sticking, slides and rocket jumps.
use exact_game::{Sim, Transform, Vec3};
use rivals_logic::fighter::{self, Fighter};
use rivals_logic::{Options, Rivals};

fn range() -> Sim<Rivals> {
    let mut sim = Sim::<Rivals>::new(Options {
        seed: 5,
        bots: 1,
        range: true,
        ..Options::default()
    })
    .unwrap();
    sim.run(100.0);
    sim
}
/// Put the player at `at` facing `yaw`, standing.
fn stand(sim: &mut Sim<Rivals>, at: [f32; 2], yaw: f32) {
    let w = sim.world_mut();
    let e = w.named("player").unwrap();
    fighter::place(w, e, at);
    w.require_mut::<Fighter>(e).yaw = yaw;
    sim.run(300.0);
}
fn pos(sim: &Sim<Rivals>) -> Vec3 {
    sim.world().require::<Transform>("player").position
}
fn speed(sim: &Sim<Rivals>) -> f32 {
    let f = sim.world().require::<Fighter>("player");
    Vec3::new(f.planar.x, 0.0, f.planar.z).length()
}

#[test]
fn walks_up_the_ramp_onto_the_deck() {
    let mut sim = range();
    stand(&mut sim, [0.0, 8.5], 0.0);
    sim.key_down("KeyW");
    sim.run(1600.0);
    let p = pos(&sim);
    // The deck's top is 2.6 m; the capsule's centre rides 0.9 m above it.
    assert!(p.z < 2.4 && (p.y - 3.5).abs() < 0.1, "{p}");
}

#[test]
fn jumps_onto_a_one_metre_crate() {
    let mut sim = range();
    // The crate at (-14, 3) is 1.6 m wide and 1 m tall; run at it from 4 m out.
    stand(&mut sim, [-14.0, 7.0], 0.0);
    sim.key_down("KeyW");
    sim.run(250.0);
    sim.tap("Space");
    sim.run(220.0);
    sim.key_up("KeyW");
    sim.run(600.0);
    let p = pos(&sim);
    assert!((p.y - 1.92).abs() < 0.05 && (p.z - 3.0).abs() < 0.8, "{p}");
}

#[test]
fn without_a_jump_the_crate_stops_you() {
    let mut sim = range();
    stand(&mut sim, [-14.0, 7.0], 0.0);
    sim.key_down("KeyW");
    sim.run(1000.0);
    let p = pos(&sim);
    // 1 m is above the 0.35 m autostep: the capsule stops at the crate's face.
    assert!(p.y < 1.0 && (p.z - (3.8 + 0.35)).abs() < 0.05, "{p}");
}

#[test]
fn slides_along_a_wall_instead_of_sticking() {
    let mut sim = range();
    // Run at the west wall (inner face x = -22) at 45 degrees.
    stand(&mut sim, [-20.0, 6.0], std::f32::consts::FRAC_PI_4);
    sim.key_down("KeyW");
    sim.run(600.0);
    let a = pos(&sim);
    sim.run(500.0);
    let b = pos(&sim);
    let along = (b.z - a.z).abs() / 0.5;
    println!("against the wall: x {:.3}, sliding {along:.2} m/s", b.x);
    assert!((b.x + 22.0 - fighter::RADIUS).abs() < 0.05, "{b}");
    // The full walk speed's component along the wall is 7 cos 45° = 4.95 m/s.
    assert!(along > 4.5, "stuck: {along} m/s");
}

#[test]
fn a_slide_is_faster_than_a_sprint_and_ends() {
    let mut sim = range();
    stand(&mut sim, [-18.0, 18.0], std::f32::consts::FRAC_PI_2 * 3.0);
    sim.key_down("KeyW");
    sim.key_down("ShiftLeft");
    sim.run(500.0);
    let sprint = speed(&sim);
    sim.tap("KeyC");
    sim.run(100.0);
    let slide = speed(&sim);
    let eye = sim.world().require::<Fighter>("player").eye;
    sim.run(1200.0);
    let after = speed(&sim);
    println!("sprint {sprint:.2} m/s, slide {slide:.2}, after {after:.2}; eye {eye:.2}");
    assert!((sprint - fighter::SPRINT).abs() < 0.01);
    assert!(slide > sprint + 2.0 && eye < 0.5);
    assert!((after - fighter::SPRINT).abs() < 0.2);
}

#[test]
fn a_rocket_at_your_feet_launches_you() {
    let mut sim = range();
    stand(&mut sim, [-18.0, -18.0], 0.0);
    sim.tap("Digit2");
    sim.run(400.0);
    // Look straight down and fire as you jump.
    sim.hold("ArrowDown", 1200.0);
    sim.tap("Space");
    sim.run(30.0);
    sim.tap("KeyF");
    let mut top: f32 = 0.0;
    for _ in 0..80 {
        sim.run(1000.0 / 120.0);
        top = top.max(pos(&sim).y);
    }
    let f = sim.world().require::<Fighter>("player").clone();
    println!("rocket jump: apex {top:.2} m, hp {}", f.hp);
    // A plain jump peaks at 0.92 + 7.6²/44 = 2.23 m.
    // A rocket jump clears the 2.6 m deck: its centre passes 3.5 m.
    assert!(top > 3.5 && top < 6.0, "{top}");
    assert!(f.hp < 100.0 && f.hp > 50.0, "self damage {}", f.hp);
}

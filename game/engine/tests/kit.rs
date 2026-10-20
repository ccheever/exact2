//! The gameplay kit's oracles (LLP 1139.001 §4), each run against the kit and
//! against a broken stand-in that the oracle must refuse. The recipes
//! (`game/recipes/{steering,schooling,pickup,spawner}.md`) pin the same numbers
//! as doc-tests; these keep the negative controls.
use exact_game::collect::{drift, Drift};
use exact_game::spawn::Spawner;
use exact_game::steer::{self, Steer};
use exact_game::*;

const HZ: u32 = 120;
const DT: f32 = 1.0 / HZ as f32;

fn facing_angle(a: Quat, b: Quat) -> f32 {
    math::acos((a * Vec3::NEG_Z).dot(b * Vec3::NEG_Z).clamp(-1.0, 1.0))
}

fn advance(pose: &mut Transform, s: Steer) {
    pose.position += s.velocity * DT;
    pose.rotation = s.facing;
}

type Seek = fn(&Transform, Vec3, f32, f32, f32) -> Steer;

/// Ticks a mover facing −Z at the origin takes to reach a target behind it,
/// 5 m away, at 2 m/s turning 2 rad/s; Err when a tick turns faster than the
/// rate allows or the target is not reached in time.
fn seek_oracle(seek: Seek) -> Result<u32, String> {
    let (speed, rate, target) = (2.0, 2.0, Vec3::new(0.0, 0.0, 5.0));
    // Half a turn, then the straight run, then half a second.
    let bound = ((std::f32::consts::PI / rate + 5.0 / speed + 0.5) * HZ as f32) as u32;
    let mut pose = Transform::default();
    for tick in 1..=bound {
        let s = seek(&pose, target, speed, rate, DT);
        let turned = facing_angle(pose.rotation, s.facing);
        if turned > rate * DT + 1e-5 {
            return Err(format!(
                "tick {tick} turned {turned} rad, over {}",
                rate * DT
            ));
        }
        advance(&mut pose, s);
        if pose.position.distance(target) < 0.01 {
            return Ok(tick);
        }
    }
    Err(format!(
        "not reached in {bound} ticks: {}",
        pose.position.distance(target)
    ))
}

#[test]
fn seek_reaches_its_target_within_the_turn_rate_bound() {
    assert_eq!(seek_oracle(steer::seek), Ok(444));
    // Broken: faces the target at once, ignoring the turn rate.
    let instant: Seek = |from, to, speed, _, dt| steer::seek(from, to, speed, 1e6, dt);
    let refused = seek_oracle(instant).unwrap_err();
    assert!(refused.contains("turned"), "{refused}");
    // Broken: never moves.
    let stuck: Seek = |from, _, _, _, _| Steer {
        velocity: Vec3::ZERO,
        facing: from.rotation,
    };
    assert!(seek_oracle(stuck).unwrap_err().contains("not reached"));
}

type Arrive = fn(&Transform, Vec3, f32, f32, f32, f32) -> Steer;

/// A mover 6 m from its target, facing it, arriving with a 2 m radius at
/// 3 m/s: its speed 1 m out, and the tick it stops; Err on an overshoot.
fn arrive_oracle(arrive: Arrive) -> Result<(f32, u32), String> {
    let target = Vec3::new(0.0, 0.0, -6.0);
    let mut pose = Transform::default();
    let mut at_one = None;
    for tick in 1..=1200 {
        let s = arrive(&pose, target, 2.0, 3.0, 4.0, DT);
        advance(&mut pose, s);
        if pose.position.z < target.z - 1e-4 {
            return Err(format!("overshot to {}", pose.position.z));
        }
        if at_one.is_none() && pose.position.distance(target) <= 1.0 {
            at_one = Some(s.velocity.length());
        }
        if s.velocity == Vec3::ZERO && tick > 1 {
            return Ok((at_one.unwrap_or(f32::NAN), tick));
        }
    }
    Err("never stopped".into())
}

#[test]
fn arrive_slows_inside_its_radius_and_stops_on_the_target() {
    let (speed, stopped) = arrive_oracle(steer::arrive).unwrap();
    assert!(
        (speed - 1.5).abs() < 0.05,
        "half the radius left, half the speed: {speed}"
    );
    // The last two metres close exponentially, a time constant of radius / speed.
    assert_eq!(stopped, 766);
    // Broken: seek, which runs in at full speed.
    let (rushed, _) =
        arrive_oracle(|from, to, _, v, r, dt| steer::seek(from, to, v, r, dt)).unwrap();
    assert!(rushed > 2.9, "{rushed}");
}

type Wander = fn(&Transform, &mut Rng, &mut f32, f32, f32, f32) -> Steer;

/// Ten seconds of wandering at 1.5 m/s turning up to 2 rad/s, from seed 5:
/// the heading's net change, or Err when a tick turns faster than the rate.
fn wander_oracle(wander: Wander) -> Result<f32, String> {
    let (mut pose, mut rng, mut turn) = (Transform::default(), Rng::new(5), 0.0);
    let mut heading = 0.0f32;
    let mut farthest = 0.0f32;
    for tick in 1..=10 * HZ {
        let s = wander(&pose, &mut rng, &mut turn, 1.5, 2.0, DT);
        let turned = facing_angle(pose.rotation, s.facing);
        if turned > 2.0 * DT + 1e-5 {
            return Err(format!("tick {tick} turned {turned}"));
        }
        if (s.facing * Vec3::NEG_Z).y.abs() > 1e-6 {
            return Err(format!("tick {tick} pitched"));
        }
        heading += turned * turn.signum();
        farthest = farthest.max(heading.abs());
        advance(&mut pose, s);
    }
    Ok(farthest)
}

#[test]
fn wander_meanders_within_its_turn_rate_on_the_world_stream() {
    let farthest = wander_oracle(steer::wander).unwrap();
    assert!(farthest > 1.0, "the heading wandered {farthest} rad");
    // The same stream gives the same walk, bit for bit.
    assert_eq!(wander_oracle(steer::wander), Ok(farthest));
    // Broken: a wander that never turns.
    let straight: Wander = |from, _, _, speed, _, _| Steer {
        velocity: from.rotation * Vec3::NEG_Z * speed,
        facing: from.rotation,
    };
    assert!(wander_oracle(straight).unwrap() < 1.0);
}

type Flee = fn(&Transform, Vec3, f32, f32, f32, f32) -> Option<Steer>;

/// The tick a mover that has been inside a threat's 4 m radius leaves it, or
/// Err after `limit` ticks.
/// The mover, at the origin, faces the threat; it runs at 2 m/s, turning 4 rad/s.
fn flee_oracle(flee: Flee, mut threat: Vec3, approach: f32, limit: u32) -> Result<u32, String> {
    let (radius, speed, rate) = (4.0, 2.0, 4.0);
    let mut pose = Transform::default().looking_at(threat, Vec3::Y);
    let park = Vec3::ZERO;
    let mut inside = false;
    for tick in 1..=limit {
        // The threat heads for where the mover started and stops there.
        let to_park = park - threat;
        threat += to_park.clamp_length_max(approach * DT);
        if let Some(s) = flee(&pose, threat, radius, speed, rate, DT) {
            advance(&mut pose, s);
        }
        let distance = pose.position.distance(threat);
        if inside && distance > radius {
            return Ok(tick);
        }
        inside |= distance <= radius;
    }
    Err(format!(
        "still {} m from the threat after {limit} ticks",
        pose.position.distance(threat)
    ))
}

#[test]
fn flee_leaves_the_radius_of_a_faster_threat_and_of_one_it_starts_inside() {
    // A threat 6 m off at 3 m/s, faster than the mover, parking where it began.
    assert_eq!(
        flee_oracle(steer::flee, Vec3::new(0., 0., -6.), 3.0, 600),
        Ok(386)
    );
    // A threat 1 m off that does not move.
    assert_eq!(
        flee_oracle(steer::flee, Vec3::new(0., 0., -1.), 0.0, 600),
        Ok(248)
    );
    // Broken: a flee that stays put fails both.
    let stationary: Flee = |from, _, _, _, _, _| {
        Some(Steer {
            velocity: Vec3::ZERO,
            facing: from.rotation,
        })
    };
    assert!(flee_oracle(stationary, Vec3::new(0., 0., -6.), 3.0, 600).is_err());
    assert!(flee_oracle(stationary, Vec3::new(0., 0., -1.), 0.0, 600).is_err());
    // Outside the radius flee says so, and the game falls back.
    assert_eq!(
        steer::flee(
            &Transform::default(),
            Vec3::new(0., 0., -5.),
            4.0,
            2.0,
            4.0,
            DT
        ),
        None
    );
}

#[derive(Default, Component)]
struct Fish;

type School = fn(&Transform, Vec<(Entity, Transform)>, f32, f32, f32, f32, f32, f32) -> Steer;

/// Twenty fish start 0.6 m apart in a grid with random headings and school for
/// 10 s with 1 m separation; the closest pair's distance and the farthest
/// fish's distance from the school's centre at the end.
fn school_oracle(school: School) -> (f32, f32) {
    let mut w = World::new(HZ, 11);
    for i in 0..20 {
        let yaw = w.rand(-3.0f32..3.0);
        w.spawn((
            Fish,
            Transform {
                position: Vec3::new((i % 5) as f32 * 0.6, 0.0, (i / 5) as f32 * 0.6),
                rotation: Quat::from_rotation_y(yaw),
                ..Transform::default()
            },
        ));
    }
    let fish: Vec<Entity> = w.query::<&Fish>().iter().map(|(e, _)| e).collect();
    for _ in 0..10 * HZ {
        // Every decision reads the poses as the tick found them.
        let steers: Vec<Steer> = fish
            .iter()
            .map(|&e| {
                let pose = *w.require::<Transform>(e);
                let mates = w.near_xz::<Fish>(e, 3.0).limit(7).collect();
                school(&pose, mates, 1.0, 0.5, 0.3, 1.5, 3.0, DT)
            })
            .collect();
        for (&e, s) in fish.iter().zip(steers) {
            advance(&mut w.require_mut::<Transform>(e), s);
        }
    }
    let at: Vec<Vec3> = fish
        .iter()
        .map(|&e| w.require::<Transform>(e).position)
        .collect();
    let centre = at.iter().copied().sum::<Vec3>() / at.len() as f32;
    let mut closest = f32::INFINITY;
    for (i, a) in at.iter().enumerate() {
        for b in &at[i + 1..] {
            closest = closest.min(a.distance(*b));
        }
    }
    let spread = at.iter().map(|p| p.distance(centre)).fold(0.0, f32::max);
    (closest, spread)
}

#[test]
fn a_school_of_twenty_keeps_its_separation_after_ten_seconds() {
    let (closest, spread) = school_oracle(|from, mates, s, a, c, v, r, dt| {
        steer::school(from, mates, s, a, c, v, r, dt)
    });
    println!("school: closest {closest}, spread {spread}");
    assert!(
        closest > 0.8 && spread < 4.0,
        "closest {closest}, spread {spread}"
    );
    // Broken: no separation term.
    let (crowded, _) = school_oracle(|from, mates, _, a, c, v, r, dt| {
        steer::school(from, mates, 0.0, a, c, v, r, dt)
    });
    println!("school without separation: closest {crowded}");
    assert!(crowded < 0.8, "closest {crowded}");
}

type DriftFn = fn(Vec3, Vec3, f32, f32, f32) -> Drift;

/// The tick a coin 2 m from its collector, drifting at 4 m/s inside a 3 m
/// radius, is taken; Err if it is taken before it arrives or moves outside the radius.
fn drift_oracle(drift: DriftFn) -> Result<u32, String> {
    let still = drift(Vec3::new(3.5, 0., 0.), Vec3::ZERO, 3.0, 4.0, DT);
    if still.position != Vec3::new(3.5, 0., 0.) || still.taken {
        return Err(format!("moved outside the radius: {still:?}"));
    }
    let mut coin = Vec3::new(2.0, 0.0, 0.0);
    for tick in 1..=600 {
        let d = drift(coin, Vec3::ZERO, 3.0, 4.0, DT);
        if d.taken {
            return if d.position == Vec3::ZERO {
                Ok(tick)
            } else {
                Err(format!(
                    "taken at {} m, before contact",
                    d.position.length()
                ))
            };
        }
        coin = d.position;
    }
    Err("never taken".into())
}

#[test]
fn a_drift_takes_on_contact_and_not_before() {
    // 2 m at 1/30 m a tick: sixty steps, and a sixty-first for the float residue.
    assert_eq!(drift_oracle(drift), Ok(61));
    // Broken: taken as soon as it is in the radius.
    let early: DriftFn = |item, collector, radius, speed, dt| Drift {
        taken: item.distance(collector) <= radius,
        ..drift(item, collector, radius, speed, dt)
    };
    assert!(drift_oracle(early).unwrap_err().contains("before contact"));
    // Broken: drifts from anywhere.
    let everywhere: DriftFn =
        |item, collector, _, speed, dt| drift(item, collector, 1e9, speed, dt);
    assert!(drift_oracle(everywhere)
        .unwrap_err()
        .contains("outside the radius"));
}

/// A spawner whose serial a save leaves out: a restore starts it again.
#[derive(Clone, Copy, Debug, Default, Data)]
struct Forgetful {
    next: u64,
    #[data(skip)]
    serial: u32,
    every: u64,
}

/// A game with two spawners of type `$ty` in one resource, logging serials.
macro_rules! two_spawners {
    ($game:ident, $res:ident, $ty:ty, $every:expr) => {
        #[derive(Default, Resource)]
        struct $res {
            fish: $ty,
            coins: $ty,
            fish_log: Vec<u32>,
            coin_log: Vec<u32>,
            /// The tick of each spawn, fish and coins in the order they came.
            ticks: Vec<u64>,
        }
        struct $game;
        impl Game for $game {
            const ID: &'static str = "kit-spawner";
            const HZ: u32 = HZ;
            type Args = ();
            fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
                w.register_resource::<$res>();
            }
            fn setup(w: &mut World, _: &()) {
                let every = $every;
                w.insert_resource($res {
                    fish: every(7),
                    coins: every(11),
                    ..$res::default()
                });
            }
            fn tick(w: &mut World, _: &Input, _: &()) {
                let tick = w.tick();
                let mut d = w.resource_mut::<$res>();
                while let Some(serial) = d.fish.due(tick) {
                    d.fish_log.push(serial);
                    d.ticks.push(tick);
                }
                while let Some(serial) = d.coins.due(tick) {
                    d.coin_log.push(serial);
                    d.ticks.push(tick);
                }
            }
        }
        impl Logged for $game {
            fn log(w: &World) -> (Vec<u32>, Vec<u32>, Vec<u64>) {
                let d = w.resource::<$res>();
                (d.fish_log.clone(), d.coin_log.clone(), d.ticks.clone())
            }
        }
    };
}
trait Logged: Game<Args = ()> {
    fn log(w: &World) -> (Vec<u32>, Vec<u32>, Vec<u64>);
}
impl Forgetful {
    fn due(&mut self, tick: u64) -> Option<u32> {
        (tick >= self.next).then(|| {
            self.next += self.every;
            self.serial += 1;
            self.serial - 1
        })
    }
}
two_spawners!(Kit, KitDrops, Spawner, Spawner::every);
two_spawners!(Broken, BrokenDrops, Forgetful, |every| Forgetful {
    next: every,
    serial: 0,
    every
});

/// Two spawners, every 7 and every 11 ticks, run for 1000 ticks with a save
/// after 500 restored into a fresh `Sim`; Err when the count by the tick
/// numbered `t` is not `t / every`, or the restored run's serials differ from
/// the uninterrupted run's.
fn spawner_oracle<G: Logged>() -> Result<(usize, usize), String> {
    let ms = |ticks: u64| ticks as f64 * 1000.0 / HZ as f64;
    let mut whole = Sim::<G>::new(()).unwrap();
    whole.run(ms(500));
    let saved = whole.save().unwrap();
    whole.run(ms(500));
    // The last tick that ran is numbered one less than the count of ticks.
    let tick = whole.world().tick() - 1;
    let (fish, coins, ticks) = G::log(whole.world());
    if fish.len() as u64 != tick / 7 || coins.len() as u64 != tick / 11 {
        return Err(format!(
            "by tick {tick}: {} and {} spawns",
            fish.len(),
            coins.len()
        ));
    }
    // Each on its grid, in order: every multiple of 7, then of 11, up to `tick`.
    let grid: Vec<u64> = (1..=tick)
        .flat_map(|t| {
            [7, 11]
                .into_iter()
                .filter(move |e| t % e == 0)
                .map(move |_| t)
        })
        .collect();
    if ticks != grid {
        return Err(format!(
            "spawned at ticks {:?}…, not on the grid",
            &ticks[..3]
        ));
    }
    if fish != (0..fish.len() as u32).collect::<Vec<_>>() {
        return Err(format!("serials {fish:?}"));
    }
    let mut fresh = Sim::<G>::new(()).unwrap();
    fresh.restore(&saved).unwrap();
    fresh.run(ms(500));
    if G::log(fresh.world()) != (fish.clone(), coins.clone(), ticks) {
        return Err("a restored run spawned other serials".into());
    }
    Ok((fish.len(), coins.len()))
}

#[test]
fn two_spawners_owe_t_over_every_and_keep_their_serials_across_a_restore() {
    assert_eq!(spawner_oracle::<Kit>(), Ok((142, 90)));
    let refused = spawner_oracle::<Broken>().unwrap_err();
    assert!(refused.contains("other serials"), "{refused}");
}

#[test]
fn a_late_call_owes_one_spawn_per_interval_up_to_the_cap() {
    let mut s = Spawner::every(10).cap(3);
    assert_eq!(s.due(9), None);
    assert_eq!(s.due(10), Some(0));
    assert_eq!(s.due(10), None);
    // Ten intervals late: three owed, the schedule still on multiples of ten.
    let late: Vec<u32> = std::iter::from_fn(|| s.due(115)).collect();
    assert_eq!(late, [1, 2, 3]);
    assert_eq!(s.next, 120);
    assert_eq!(
        Spawner::every(10).due(1000),
        Some(0),
        "the default cap is one"
    );
    assert_eq!(
        Spawner::default().due(u64::MAX),
        None,
        "every 0 never spawns"
    );
}

#[test]
fn near_xz_limit_keeps_the_nearest_in_entity_order() {
    let mut w = World::new(HZ, 0);
    let origin = w.spawn((Fish, Transform::default()));
    let at = |x: f32| (Fish, Transform::at(x, 0.0, 0.0));
    let far = w.spawn(at(3.0));
    let near = w.spawn(at(-1.0));
    let tie_low = w.spawn(at(2.0));
    let tie_high = w.spawn(at(-2.0));
    w.spawn(at(9.0));
    let ids = |rows: Vec<(Entity, Transform)>| rows.into_iter().map(|(e, _)| e).collect::<Vec<_>>();
    assert_eq!(
        ids(w.near_xz::<Fish>(origin, 5.0).collect()),
        [far, near, tie_low, tie_high]
    );
    assert_eq!(
        ids(w.near_xz::<Fish>(origin, 5.0).limit(2).collect()),
        [near, tie_low]
    );
    assert_eq!(
        ids(w.near_xz::<Fish>(origin, 5.0).limit(9).collect()),
        [far, near, tie_low, tie_high]
    );
    assert_eq!(w.near_xz::<Fish>(origin, 5.0).limit(0).count(), 0);
    // The nearest two of three, where the nearer has the higher index: still entity order.
    let mut v = World::new(HZ, 0);
    let o = v.spawn((Fish, Transform::default()));
    let (p, q) = (v.spawn(at(1.5)), v.spawn(at(0.5)));
    v.spawn(at(3.0));
    assert_eq!(ids(v.near_xz::<Fish>(o, 5.0).limit(2).collect()), [p, q]);
    // A limited set is read when limit is called; an unlimited one as the loop
    // reaches each row, so a row moved away before its turn drops out.
    let read = |limit: Option<usize>| {
        let mut seen = Vec::new();
        let rows: Box<dyn Iterator<Item = (Entity, Transform)>> = match limit {
            Some(n) => Box::new(w.near_xz::<Fish>(origin, 5.0).limit(n)),
            None => Box::new(w.near_xz::<Fish>(origin, 5.0)),
        };
        for (e, _) in rows {
            if seen.is_empty() {
                w.require_mut::<Transform>(tie_low).position.x = 100.0;
            }
            seen.push(e);
        }
        w.require_mut::<Transform>(tie_low).position.x = 2.0;
        seen
    };
    assert_eq!(read(Some(2)), [near, tie_low]);
    assert_eq!(read(None), [far, near, tie_high]);
}

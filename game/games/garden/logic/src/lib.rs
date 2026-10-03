//! Grow a Garden, greybox: buy seeds, plant them, let them grow (in real
//! time, and while you are away), harvest mutated fruit, sell, expand.

pub mod crops;
pub mod farm;
pub mod garden;
pub mod hud;
pub mod shop;

use exact_game::character::Character;
use exact_game::*;
use farm::Farm;
use garden::{Census, Due, Fruit, GardenClock, Plant, Schedule, Weather};
use shop::Shop;

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    #[restart]
    pub restart: bool,
    /// The HUD's latest command (`buy carrot`, `sell all`, …) and its
    /// number: a command applies once, when its number changes.
    #[live]
    pub cmd: String,
    #[live]
    pub cmd_id: u32,
    /// The host's Unix time (ms) at its clock zero (`exactTime().epochAtZero`).
    /// A later session whose epoch is ahead of this world's clock grows the
    /// garden by the difference.
    #[live]
    pub epoch: f64,
    /// Rescale every growing plant and fruit every tick (smooth growth),
    /// instead of only at stage events. The O(entities) path, for measuring.
    #[live]
    pub smooth: bool,
}

/// Status is republished at most this often unless something happened.
const STATUS_MS: u64 = 1000;

/// What the status line last showed, so a tick republishes only on change.
#[derive(Default, Resource)]
pub struct Shown {
    pub second: u64,
    pub prompt: String,
    pub events: u64,
}

pub struct Garden;
impl Game for Garden {
    const ID: &'static str = "garden";
    /// A garden does not need 120 Hz: walking interpolates, and every tick
    /// of an hour-long `clock +N` seek is paid for.
    const HZ: u32 = 30;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows())
            .button("act", &["KeyE"])
            .button("jump", &["Space"])
    }
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        // Plants and fruit first appear mid-game; a fresh process restoring a
        // save must know them.
        w.register::<Plant>()
            .register::<Fruit>()
            .register::<Parent>();
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.insert_resource(Environment {
            background: Some([0.55, 0.75, 0.9]),
            fog: Some(Fog {
                color: Some([0.55, 0.75, 0.9]),
                ..Fog::new(0.004, 0.02)
            }),
            ..Environment::default()
        });
        w.insert_resource(Schedule::default());
        w.insert_resource(GardenClock::default());
        w.insert_resource(Weather::default());
        w.insert_resource(Census::default());
        w.insert_resource(Shop::default());
        w.insert_resource(Farm::new());
        w.insert_resource(Shown::default());
        w.spawn_named(
            "ground",
            (
                Transform::default(),
                Mesh::plane(1.0, 1.0),
                Material::grid([0.2, 0.11, 0.05], garden::TILE),
            ),
        );
        let player = w.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::capsule(0.4, 1.8),
                garden::paint([0.95, 0.75, 0.3]),
                Character::new().ground(0.9),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::default(),
                Follow::new(player).offset(0.0, 9.0, 11.0).lag(0.15),
            ),
        );
        w.spawn_named(
            "sun",
            (
                Transform::at(10.0, 20.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight::default(),
            ),
        );
        farm::lay_ground(w);
        shop::restock(w, 0);
        w.resource_mut::<Schedule>()
            .push(garden::change_after(w), Due::Weather);
        hud::publish(w, String::new(), true);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        farm::observe_epoch(w, args.epoch);
        let now = garden::now_ms(w);
        garden::run_due(w, now);
        if args.smooth {
            garden::animate(w, now);
        }
        let mut acted = false;
        // The HUD resets its command to id 0 when the world acknowledges it
        // (`done <id>`), so a resting HUD binds the same arguments in every
        // session and a restore with fresh HUD state saves byte-identically.
        if args.cmd_id == 0 {
            if w.resource::<Farm>().cmd_seen != 0 {
                w.resource_mut::<Farm>().cmd_seen = 0;
            }
        } else if w.resource::<Farm>().cmd_seen != args.cmd_id {
            w.resource_mut::<Farm>().cmd_seen = args.cmd_id;
            farm::command(w, &args.cmd);
            w.emit(format!("done {}", args.cmd_id));
            acted = true;
        }
        w.character("player")
            .step(input.stick_xz("move"), input.pressed("jump"));
        let (tile, prompt) = farm::prompt(w);
        if input.pressed("act") {
            if let Some(tile) = tile {
                farm::act(w, tile);
                acted = true;
            }
        }
        let now = garden::now_ms(w);
        let processed = w.resource::<Schedule>().processed;
        let changed = {
            let shown = w.resource::<Shown>();
            acted
                || shown.second != now / STATUS_MS
                || shown.prompt != prompt
                || shown.events != processed
        };
        if changed {
            let prompt = if acted { farm::prompt(w).1 } else { prompt };
            {
                let mut shown = w.resource_mut::<Shown>();
                shown.second = now / STATUS_MS;
                shown.prompt = prompt.clone();
                shown.events = processed;
            }
            hud::publish(w, prompt, false);
        }
    }
}

/// Every plant's and fruit's saved state in entity order, for tests that
/// compare two gardens.
pub fn census_of(w: &World) -> Vec<String> {
    let mut out = Vec::new();
    for (e, p) in w.query::<&Plant>().iter() {
        out.push(format!(
            "plant {} {} {:?} {}",
            e.index(),
            p.kind,
            p.tile,
            p.stage
        ));
    }
    for (e, f) in w.query::<&Fruit>().iter() {
        out.push(format!(
            "fruit {} {} {} {} {:.4}",
            e.index(),
            f.kind,
            f.ripe,
            f.muts,
            f.weight
        ));
    }
    out
}

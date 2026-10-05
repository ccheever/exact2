//! Grow a Garden: buy seeds, plant them, let them grow (in real
//! time, and while you are away), harvest mutated fruit, sell, expand.

mod art;
pub mod crops;
pub mod farm;
mod feedback;
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
    pub tile: Option<[u16; 2]>,
    pub events: u64,
    pub at_barrel: bool,
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
            .button("water", &["KeyQ"])
            .button("refill", &["KeyR"])
            .button("feed", &["KeyF"])
            .button("jump", &["Space"])
    }
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        // Plants and fruit first appear mid-game; a fresh process restoring a
        // save must know them.
        w.register::<Plant>().register::<Fruit>();
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.insert_resource(Environment {
            zenith: [0.18, 0.38, 0.60],
            horizon: [0.62, 0.76, 0.67],
            ground: [0.12, 0.16, 0.07],
            ambient: 0.45,
            exposure: 0.9,
            fog: Some(Fog {
                color: Some([0.62, 0.76, 0.67]),
                ..Fog::new(0.0025, 0.04)
            }),
            ..Environment::default()
        });
        w.insert_resource(AmbientOcclusion {
            radius: 0.6,
            intensity: 0.8,
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
                Material::grid([0.12, 0.065, 0.025], garden::TILE),
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
                DirectionalLight {
                    color: [1.0, 0.90, 0.72],
                    ..DirectionalLight::default()
                },
            ),
        );
        farm::create_plot_outline(w);
        w.spawn_named(
            "water-barrel",
            (
                Transform::at(farm::BARREL.x, 0.5, farm::BARREL.z),
                Mesh::cylinder(0.65, 1.0),
                garden::paint([0.2, 0.35, 0.65]),
            ),
        );
        w.spawn_named(
            "barrel-water",
            (
                Transform::at(farm::BARREL.x, 1.01, farm::BARREL.z),
                Mesh::cylinder(0.53, 0.04),
                garden::paint([0.2, 0.7, 1.0]),
            ),
        );
        art::setup(w);
        feedback::setup(w);
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
        // HUD commands (`buy carrot`, `sell all`, …) arrive as messages,
        // each once, in order.
        for cmd in input.messages() {
            farm::command(w, cmd);
            acted = true;
        }
        let contact = w
            .character("player")
            .step(input.stick_xz("move"), input.pressed("jump"));
        let (tile, prompt) = farm::prompt(w);
        if input.pressed("act") {
            if let Some(tile) = tile {
                farm::act(w, tile);
                acted = true;
            }
        }
        for (action, f) in [
            (
                "water",
                farm::water_here as fn(&World) -> Result<String, String>,
            ),
            ("refill", farm::refill),
            ("feed", farm::feed_here),
        ] {
            if input.pressed(action) {
                let message = f(w).unwrap_or_else(|why| why);
                w.resource_mut::<Farm>().last = message;
                acted = true;
            }
        }
        feedback::step(w, contact);
        audio::step(w);
        let now = garden::now_ms(w);
        let processed = w.resource::<Schedule>().processed;
        let at_barrel = farm::at_barrel(w);
        let changed = {
            let shown = w.resource::<Shown>();
            acted
                || shown.second != now / STATUS_MS
                || shown.prompt != prompt
                || shown.tile != tile
                || shown.events != processed
                || shown.at_barrel != at_barrel
        };
        if changed {
            let prompt = if acted { farm::prompt(w).1 } else { prompt };
            farm::show_plot(w, tile);
            {
                let mut shown = w.resource_mut::<Shown>();
                shown.second = now / STATUS_MS;
                shown.prompt = prompt.clone();
                shown.tile = tile;
                shown.events = processed;
                shown.at_barrel = at_barrel;
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

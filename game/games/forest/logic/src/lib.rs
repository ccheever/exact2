//! 99 Nights in the Forest, greybox: keep the campfire burning, survive the Deer.
pub mod camp;
pub mod creatures;
pub mod forest;
pub mod player;

use camp::{Cycle, Fire};
use creatures::{Deer, Scene};
use exact_game::*;
use exact_game_physics as physics;
use forest::Grove;
use player::{Action, Child, Item, Kind, Player};

#[derive(Args)]
pub struct Options {
    pub seed: u64,
    /// Trees in the forest; the world grows to hold them at constant density.
    pub trees: u32,
    pub wolves: u32,
    pub children: u32,
    /// Extra point lights on poles in a spiral out from camp: the renderer keeps 16.
    pub torches: u32,
    /// Trunk and crown as two primitive entities instead of one generated pine.
    pub primitives: bool,
    /// Collide in game code against the tree grid instead of Rapier colliders.
    pub lite: bool,
    #[live]
    pub paused: bool,
    #[restart]
    pub restart: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            seed: 7,
            trees: 2000,
            wolves: 8,
            children: 2,
            torches: 0,
            primitives: false,
            lite: false,
            paused: false,
            restart: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Hud {
    pub day: u32,
    pub phase: String,
    pub left: u32,
    pub survived: u32,
    pub health: u32,
    pub hunger: u32,
    pub battery: u32,
    pub flashlight: bool,
    pub fuel: u32,
    pub radius: u32,
    pub logs: u32,
    pub scrap: u32,
    pub food: u32,
    pub prompt: String,
    pub rescued: u32,
    pub children: u32,
    pub dead: bool,
    pub objective: String,
    pub tracking: String,
    pub camp_bearing: String,
    pub night_plan: String,
    pub night_supplies: String,
    pub windbreak: bool,
    pub build_ready: bool,
    pub build_hint: String,
    pub deer: String,
    pub chasing: u32,
    pub trees: u32,
    pub wolves: u32,
}

pub struct Forest;
impl Game for Forest {
    const ID: &'static str = "forest";
    const HZ: u32 = 60;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows())
            .button("use", &["KeyE"])
            .button("eat", &["KeyQ"])
            .button("light", &["KeyF"])
            .button("build", &["KeyR"])
    }
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        physics::register(w);
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        physics::register(w);
        forest::grow(w, args.trees, args.primitives, !args.lite);
        camp::build(w);
        camp::torches(w, args.torches);
        let half = w.resource::<Grove>().half;
        player::spawn(w, !args.lite, args.children);
        w.insert_resource(player::Trail::default());
        creatures::spawn_deer(w, half);
        creatures::spawn_wolves(w, args.wolves, half);
        player::scatter(w, Kind::Scrap, args.trees / 60 + 6, 18.0, half * 0.9);
        player::scatter(w, Kind::Food, args.trees / 50 + 10, 18.0, half * 0.7);
        player::scatter(w, Kind::Log, 3, 6.0, 12.0);
        hud(w, Action::None, 0, 0);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        let mut build = input.pressed("build");
        for command in input.messages() {
            if command == "build windbreak" {
                build = true;
            } else {
                player::track(w, command);
            }
        }
        if build {
            player::build_windbreak(w);
        }
        let colliders = !args.lite;
        let was_dead = w.require::<Player>("player").dead;
        let at = w.require::<Transform>("player").position;
        let act = player::action(w, at);
        let ready = w.require::<Player>("player").cooldown <= 0.0;
        // Holding E repeats axe swings at their normal cadence. Picking up,
        // rescuing and feeding still require a fresh press.
        let use_action =
            ready && (input.pressed("use") || input.held("use") && matches!(act, Action::Chop(_)));
        if use_action || input.pressed("eat") {
            let act = if use_action { act } else { Action::None };
            player::interact(w, act, input.pressed("eat"));
        }
        let safe = w.resource::<Fire>().radius();
        player::walk(w, input.stick_xz("move"), colliders, safe);
        if colliders {
            physics::step(w);
        }
        player::flashlight(w, input.pressed("light"));
        let at = w.require::<Transform>("player").position;
        let dawned = camp::step(w, at);
        if dawned {
            let half = w.resource::<Grove>().half;
            player::scatter(w, Kind::Food, 4, 18.0, half * 0.6);
            w.emit("dawn");
        }
        let scene = {
            let p = w.require::<Player>("player");
            Scene {
                player: at,
                facing: p.facing,
                flashlight: p.flashlight,
                night: w.resource::<Cycle>().night(),
                safe: w.resource::<Fire>().radius(),
                dead: p.dead,
            }
        };
        let outcome = creatures::step(w, &scene);
        if outcome.damage > 0.0 {
            let mut p = w.require_mut::<Player>("player");
            if !p.dead {
                p.health -= outcome.damage;
                if p.health <= 0.0 {
                    p.health = 0.0;
                    p.dead = true;
                }
            }
        }
        if w.require::<Player>("player").dead && !was_dead {
            w.emit("died");
        }
        let rescued = player::children(w, at, scene.safe);
        player::update_trail(
            w,
            dawned || build || input.pressed("use") || input.pressed("eat"),
        );
        emitter::step(w);
        let act = player::action(w, w.require::<Transform>("player").position);
        hud(w, act, rescued, outcome.chasing);
    }
}

fn hud(w: &World, act: Action, rescued: u32, chasing: u32) {
    let c = *w.resource::<Cycle>();
    let f = *w.resource::<Fire>();
    let p = w.require::<Player>("player");
    let count = |kind: Kind| {
        p.pack
            .iter()
            .filter(|&&e| w.get::<Item>(e).is_some_and(|i| i.kind == kind))
            .count() as u32
    };
    let deer = w.require::<Deer>("deer").mind;
    let children = w.count::<Child>(|_| true);
    let (night_plan, night_supplies) = player::preparation(w);
    w.publish_record(&Hud {
        day: c.day,
        phase: c.phase().into(),
        left: math::ceil(c.left()) as u32,
        survived: c.survived,
        health: math::ceil(p.health) as u32,
        hunger: math::ceil(p.hunger) as u32,
        battery: math::ceil(p.battery) as u32,
        flashlight: p.flashlight,
        fuel: math::ceil(f.fuel) as u32,
        radius: math::round(f.radius()) as u32,
        logs: count(Kind::Log),
        scrap: count(Kind::Scrap),
        food: count(Kind::Food),
        prompt: act.prompt(w),
        rescued,
        children,
        dead: p.dead,
        objective: player::guidance(w),
        tracking: w.resource::<player::Trail>().kind.label().into(),
        camp_bearing: player::camp_bearing(w),
        night_plan,
        night_supplies,
        windbreak: f.windbreak,
        build_ready: player::can_build(w),
        build_hint: player::build_hint(w).into(),
        deer: format!("{deer:?}"),
        chasing,
        trees: w.resource::<Grove>().standing,
        wolves: w.count::<creatures::Wolf>(|_| true),
    });
}

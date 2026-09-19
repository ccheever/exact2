//! Fixed I3 consumer; compiled edits share one portable save identity.
#![forbid(unsafe_code)]
use exact_game::{Game, Input, World};
/// Canvas inputs. Action counters let Contract overlay buttons feed distinct
/// touch presses while the left thumb remains on the raw movement stick.
#[derive(Default, exact_game::Args)]
pub struct Options {
    /// Compiled implementation used by this fixed evidence consumer.
    pub variant: u32,
    /// Procedural scenery seed.
    pub seed: u64,
    /// Whether the title has started this round.
    #[live]
    pub started: bool,
    /// Pause simulation without destroying the world.
    #[live]
    pub paused: bool,
    /// Setup counter; changing it starts a fresh game.
    pub round: u32,
    /// Monotonic Contract Jump-button press counter.
    #[live]
    pub jump_press: u32,
    /// Monotonic Contract Light-button press counter.
    #[live]
    pub light_press: u32,
    /// Whether new synthesized sounds may start.
    #[live]
    pub sound: bool,
    /// Baked typed initial conditions; a construction argument, never a live edit.
    pub scene: String,
}

#[path = "../../game.rs"]
pub mod baseline;
/// Independently compiled gravity/jump edit.
pub mod physics_edit {
    include!(concat!(env!("OUT_DIR"), "/i3_physics.rs"));
}
/// Independently compiled animation-selection edit.
pub mod clip_edit {
    include!(concat!(env!("OUT_DIR"), "/i3_clip.rs"));
}
/// Independently compiled light-intensity edit.
pub mod appearance_edit {
    include!(concat!(env!("OUT_DIR"), "/i3_appearance.rs"));
}
pub use baseline::{scene_assets, scene_types};
/// Select a compiled implementation through a construction argument.
pub struct Evidence;
impl Game for Evidence {
    const ID: &'static str = "lanterns-evidence";
    const ASSETS: &'static [&'static str] = baseline::Lanterns::ASSETS;
    const CAPTURE_SUPPORTED: bool = true;
    type Args = Options;
    fn actions() -> exact_game::Actions {
        baseline::Lanterns::actions()
    }
    fn validate(args: &Options) -> Result<(), String> {
        if args.variant > 4 {
            return Err("I3 variant must be in 0..=4".into());
        }
        baseline::Lanterns::validate(args)
    }
    fn paused(args: &Options) -> bool {
        baseline::Lanterns::paused(args)
    }
    fn release_input(args: &mut Options) {
        baseline::Lanterns::release_input(args);
    }
    fn register(w: &mut World, args: &std::collections::BTreeMap<&str, exact_game::Value>) {
        match args["variant"].as_f64().unwrap() as u32 {
            2 => physics_edit::Lanterns::register(w, args),
            3 => clip_edit::Lanterns::register(w, args),
            4 => appearance_edit::Lanterns::register(w, args),
            _ => baseline::Lanterns::register(w, args),
        }
    }
    fn setup(w: &mut World, args: &Options) {
        match args.variant {
            2 => physics_edit::Lanterns::setup(w, args),
            3 => clip_edit::Lanterns::setup(w, args),
            4 => appearance_edit::Lanterns::setup(w, args),
            _ => baseline::Lanterns::setup(w, args),
        }
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        match args.variant {
            2 => physics_edit::Lanterns::tick(w, input, args),
            3 => clip_edit::Lanterns::tick(w, input, args),
            4 => appearance_edit::Lanterns::tick(w, input, args),
            _ => baseline::Lanterns::tick(w, input, args),
        }
    }
}
/// Load the declared baked model through the production asset store.
pub fn simulation<G: Game>(args: G::Args) -> exact_game::Sim<G> {
    exact_game::Sim::with_assets(args, |name| {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../assets")
                .join(name),
        )
        .map_err(|e| e.to_string())
    })
    .unwrap()
}

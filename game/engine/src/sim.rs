use crate::{bin, Actions, Data, DataError, Event, Input, InputEvent, Value, Vec2, World};
use crate::{Args, ArgumentKind, PointerPhase};
use std::{collections::VecDeque, marker::PhantomData};

mod snapshot;

/// One immutable simulation instant; Copy keeps world borrows short in game code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub struct Now {
    /// Completed fixed steps.
    pub tick: u64,
    /// Fixed steps per second.
    pub hz: u32,
}
impl Now {
    /// Simulation seconds, using the fixed f32 timestep used by game motion.
    pub fn seconds(self) -> f32 {
        self.tick as f32 * (1.0 / self.hz as f32)
    }
}
/// A stateless game; components and resources hold every bit of simulation state.
pub trait Game: 'static {
    /// Surface name in Contract.
    const NAME: &'static str = "world";
    /// Stable save identity, independent of the shared surface name.
    const ID: &'static str;
    /// Models and textures required before setup. Later mesh references load on sight.
    const ASSETS: &'static [&'static str] = &[];
    /// Models and textures fetched from the start but not awaited: setup and the
    /// first frames run without them and they draw as they land. Like any
    /// undeclared asset, simulation cannot read them (`World::model` is None), so
    /// load order never reaches the hash; once loaded they stay resident.
    const STREAMED: &'static [&'static str] = &[];
    /// One typed JSON value required before setup; declares its own asset name.
    const LEVEL: Option<crate::asset::Level> = None;
    /// Canvas argument declarations in positional order; also declares exact arity.
    type Args: Args;
    /// Discoverable controls.
    fn actions() -> Actions {
        Actions::default()
    }
    /// Refuse domain-invalid arguments before any simulation mutation.
    fn validate(_args: &Self::Args) -> Result<(), String> {
        Ok(())
    }
    /// Register types that vary with setup arguments, without gameplay side effects.
    /// Receives only named setup/restart arguments; live values cannot shape the schema.
    fn register(_world: &mut World, _args: &std::collections::BTreeMap<&str, Value>) {}
    /// Construct the world after argument decoding succeeds.
    fn setup(world: &mut World, args: &Self::Args);
    /// Stop world time while continuing to serve reads.
    fn paused(_args: &Self::Args) -> bool {
        false
    }
    /// One fixed step, called after inputs and before transform propagation.
    fn tick(world: &mut World, input: &Input, args: &Self::Args);
    /// Fixed steps per second.
    const HZ: u32 = 60;
}
/// How host elapsed time becomes simulation time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// Honor every microsecond, including a large agent seek.
    Seekable,
    /// Limit one display gap to 250 milliseconds, dropping its excess.
    /// A hitch collapses input inside it onto the first step after the gap.
    /// Look ahead by the display period, aligning the tick origin to its lattice.
    Live,
}
#[derive(Clone, Default, Data)]
struct Queued {
    host_us: i64,
    world_us: Option<i64>,
    event: InputEvent,
}
#[derive(Default, Data)]
struct Saved {
    game: String,
    world: Vec<u8>,
    args: String,
    input: Input,
    queue: Vec<Queued>,
    world_us: i64,
    journal: Vec<Event>,
    journal_next: u64,
    overflow_logged: bool,
    published: std::collections::BTreeMap<String, crate::values::Stored>,
}
// Host-only phase: one tick is 1_000_000 units. Only the bounded remainder is
// floating point; neither elapsed world time nor the slew grows in an f64.
#[derive(Clone, Copy)]
struct LiveTime {
    phase: i128,
    remainder: f64,
    period_ms: f64,
    slew_left: Option<f64>,
    lookahead: f64,
}
/// Opt-in save reconstruction at the last tick of every advance, every tick that
/// received input, and every 16th tick inside an advance. Never enabled by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Paranoid {
    /// Normal execution.
    #[default]
    Off,
    /// Rebuild through the production restore path at each sampled tick.
    Save,
    /// Also discard and decode immutable assets before reconstructing the world.
    FreshGame,
}
impl Paranoid {
    #[inline]
    fn environment() -> Self {
        #[cfg(target_arch = "wasm32")]
        let mode = option_env!("EXACT_GAME_PARANOID");
        #[cfg(not(target_arch = "wasm32"))]
        let owned = std::env::var("EXACT_GAME_PARANOID").ok();
        #[cfg(not(target_arch = "wasm32"))]
        let mode = owned.as_deref();
        match mode {
            Some("1") => Self::Save,
            Some("fresh-game") => Self::FreshGame,
            _ => Self::Off,
        }
    }
}
/// The clock, bounded device queue, and a game's world, without a host or GPU.
pub struct Sim<G: Game> {
    pub(crate) world: World,
    setup_pending: bool,
    asset_mesh_revision: u64,
    asset_sprite_names: Vec<(crate::Entity, String)>,
    // Every asset a mesh or sprite has shown: once loaded it stays resident.
    pub(crate) shown: std::collections::BTreeSet<String>,
    defer_assets: bool,
    textures: std::collections::BTreeMap<String, crate::asset::TextureData>,
    pub(crate) args: G::Args,
    pub(crate) restarted: u64,
    pub(crate) restored_from: Option<String>,
    settle_delay: std::cell::Cell<u32>,
    last_epoch: std::cell::Cell<u64>,
    pub(crate) args_json: String,
    pub(crate) input: Input,
    queue: VecDeque<Queued>,
    overflow_logged: bool,
    // Posts a full input queue refused; reported in agent state, never saved.
    pub(crate) refused_posts: u64,
    // Whether this overflow episode's refused post is journaled; never saved.
    posts_logged: bool,
    rebase_queue: bool,
    pub(crate) restored: bool,
    pub(crate) last_us: Option<i64>,
    world_us: i64,
    observations: [crate::world::Observation; 2],
    // Live frame precision only; never part of seekable time, saves or hashes.
    live_time: Option<LiveTime>,
    last_ms: Option<f64>,
    period_ms: f64,
    paused_clock: bool,
    // Last scheduled lookahead, in microseconds × HZ (one tick = 1_000_000).
    lookahead_us_hz: i128,
    // The last tick's pointer motion, for presentation between ticks only.
    last_motion: crate::Vec2,
    paranoid: Option<fn(&mut Self)>,
    game: PhantomData<G>,
}
const QUEUE_LIMIT: usize = 1024;
const PARANOID_EVERY: u64 = 16;
pub(crate) fn micros(ms: f64) -> i64 {
    (ms * 1000.0).round() as i64
}
impl<G: Game> Sim<G> {
    /// Construct and load assets with the same loader as `load_assets`.
    pub fn with_assets<E: std::fmt::Display>(
        args: G::Args,
        loader: impl FnMut(&str) -> Result<Vec<u8>, E>,
    ) -> Result<Self, String> {
        let mut sim = Self::new(args)?;
        sim.load_assets(loader).map_err(|e| e.to_string())?;
        Ok(sim)
    }
    /// Assert a test hash against the game's single pins.json (included by its test).
    /// Unknown metadata fields are skipped by Data; no generated Rust is needed.
    pub fn assert_pin(&self, pins: &str) {
        let (game, tick, got) = (G::ID, self.world.tick(), self.world.hash());
        #[derive(Default, crate::Data)]
        struct Pins {
            game: String,
            ticks: std::collections::BTreeMap<String, String>,
        }
        let pins: Pins = crate::json::from_str(pins).unwrap_or_else(|error| {
            panic!("pins.json game <invalid>, expected `{game}`: {error}; run bun game/prove.mjs {game}")
        });
        assert!(
            pins.game == game,
            "pins.json game `{}`, expected `{game}`; run bun game/prove.mjs {game}",
            if pins.game.is_empty() {
                "<missing>"
            } else {
                &pins.game
            }
        );
        let expected = pins
            .ticks
            .get(&tick.to_string())
            .unwrap_or_else(|| panic!("pin {tick} missing; run bun game/prove.mjs {game}"));
        let got = format!("0x{got:016x}");
        assert!(expected == &got,
            "pin {tick} differs (expected {expected}, got {got}); if the change is intended: bun game/prove.mjs {game} --repin");
    }

    /// Override EXACT_GAME_PARANOID for this simulation.
    pub fn paranoid(mut self, mode: Paranoid) -> Self {
        self.paranoid = Self::reconstruction(mode);
        self
    }

    // The ordinary artifact stores no reconstruction executor. A function pointer
    // keeps FreshGame's model decoder out of a build whose environment selects Off.
    #[inline]
    fn reconstruction(mode: Paranoid) -> Option<fn(&mut Self)> {
        match mode {
            Paranoid::Off => None,
            Paranoid::Save => Some(|sim| sim.paranoid_rebuild(Paranoid::Save)),
            Paranoid::FreshGame => Some(|sim| sim.paranoid_rebuild(Paranoid::FreshGame)),
        }
    }

    fn register(world: &mut World, args: &G::Args) {
        let fields = G::Args::FIELDS
            .iter()
            .zip(args.values())
            .filter(|((_, kind), _)| *kind != ArgumentKind::Live)
            .map(|((name, _), value)| (*name, value))
            .collect();
        G::register(world, &fields);
    }
    fn build(args: &G::Args, assets: crate::asset::AssetStore) -> World {
        let mut world = World::new(G::HZ, 0);
        world.assets = assets;
        world.assets.restart_generated();
        if !G::ASSETS.is_empty() || G::LEVEL.is_some() {
            for name in crate::asset::level::names::<G>() {
                if !world.assets.declared.contains(name) {
                    world.assets.declared.insert(name.into());
                }
                if !world.assets.required.contains(name) {
                    world.assets.required.insert(name.into());
                }
                if !world.assets.models.contains_key(name) {
                    world.assets.request(name);
                }
            }
        }
        if !world.assets.ready() {
            return world;
        }
        Self::register(&mut world, args);
        G::setup(&mut world, args);
        crate::scene::place_followers(&world);
        world.published_pending.set(true);
        world.propagate();
        world
    }
    /// Whether setup is waiting for declared model bytes.
    pub fn is_loading(&self) -> bool {
        self.setup_pending
    }
    /// Drain first-sight model requests. Nondeclared meshes may pop in after tick zero.
    pub fn take_assets(&mut self) -> Vec<String> {
        if self.setup_pending && !self.assets_pending() {
            return Vec::new();
        }
        for name in G::STREAMED {
            if self.shown.insert((*name).to_owned()) {
                self.world.assets.request(name);
            }
        }
        let revision = self.world.revision::<crate::Mesh>();
        let sprites_changed =
            crate::sprite::texture_names_changed(&self.world, &mut self.asset_sprite_names);
        if revision != self.asset_mesh_revision || sprites_changed {
            let names: Vec<_> = self
                .world
                .query::<&crate::Mesh>()
                .iter()
                .filter_map(|(_, mesh)| {
                    if let crate::Mesh::Asset(name) = mesh {
                        Some((name.clone(), ".model"))
                    } else {
                        None
                    }
                })
                .chain(
                    self.world
                        .query::<&crate::Sprite>()
                        .iter()
                        .map(|(_, s)| (s.texture.clone(), ".tex")),
                )
                .collect();
            let mut roots: std::collections::BTreeSet<_> =
                names.iter().map(|(n, _)| n.clone()).collect();
            // Declared and once-loaded assets stay resident: an asset that leaves
            // the screen and returns is still Loaded, so a save never refuses for
            // it. Only unreferenced requests still in flight (or failed) retire.
            roots.extend(G::ASSETS.iter().map(|n| (*n).to_owned()));
            roots.extend(G::STREAMED.iter().map(|n| (*n).to_owned()));
            let assets = &self.world.assets;
            roots.extend(assets.declared.iter().cloned());
            self.shown.extend(roots.iter().cloned());
            roots.extend(
                assets
                    .states
                    .iter()
                    .filter(|(n, s)| {
                        **s == crate::asset::AssetState::Loaded && self.shown.contains(*n)
                    })
                    .map(|(n, _)| n.clone()),
            );
            if let Some(level) = G::LEVEL {
                roots.insert(level.name.into());
            }
            self.world.assets.retire(&roots);
            self.textures
                .retain(|n, _| self.world.assets.states.contains_key(n));
            for (name, suffix) in names {
                self.world.assets.request(&name);
                if !name.ends_with(suffix) {
                    self.asset_failed(&name, &format!("component requires a {suffix} name"));
                }
            }
            self.asset_mesh_revision = revision;
        }
        let assets = &mut *self.world.assets;
        let mut names: Vec<_> = assets
            .states
            .iter()
            .filter(|(n, s)| {
                (**s == crate::asset::AssetState::Pending || assets.redelivery.contains(*n))
                    && !assets.requested.contains(*n)
            })
            .map(|(n, _)| n.clone())
            .collect();
        // What setup and the first frame wait for goes first; streamed names last.
        names.sort_by_key(|n| {
            (
                G::STREAMED.contains(&n.as_str()),
                !assets.required.contains(n),
            )
        });
        assets.requested.extend(names.iter().cloned());
        names
    }
    /// Drain names whose last cosmetic mesh reference disappeared.
    pub fn take_retired_assets(&mut self) -> Vec<String> {
        std::mem::take(&mut self.world.assets.retired)
    }
    /// Device-backed surfaces retain arriving texture payloads until upload.
    pub fn defer_assets(&mut self, defer: bool) {
        self.defer_assets = defer;
    }
    /// Renderer-only model feed; games receive World, whose reads enforce declarations.
    pub fn presentation_models(&self) -> impl Iterator<Item = (&str, &crate::asset::Model)> {
        self.world
            .assets
            .models
            .iter()
            .filter(|(n, _)| self.world.assets.states.contains_key(n))
            .map(|(n, m)| (n.as_str(), m.model.as_ref()))
    }
    /// Content remains Loaded after loss; only device preparation is invalidated.
    pub fn invalidate_device_assets(&mut self) -> Vec<String> {
        let assets = &mut *self.world.assets;
        assets.prepared.clear();
        let mut retry = Vec::new();
        for (name, state) in &assets.states {
            if *state == crate::asset::AssetState::Pending {
                assets.requested.remove(name);
            }
            if name.ends_with(".tex") && *state == crate::asset::AssetState::Loaded {
                assets.requested.remove(name);
                assets.redelivery.insert(name.clone());
                retry.push(name.clone());
                // The module must reopen this answered name even if the replacement
                // attached and prepared assets before failing again.
                assets.retired.push(name.clone());
            }
        }
        retry
    }
    /// Names still awaiting bytes, including recovery uploads.
    pub fn assets_pending(&self) -> bool {
        if self.setup_pending
            && self.world.assets.required.iter().any(|n| {
                matches!(
                    self.world.assets.states.get(n),
                    Some(crate::asset::AssetState::Failed(_))
                )
            })
        {
            return false;
        }
        self.world
            .assets
            .states
            .values()
            .any(|s| *s == crate::asset::AssetState::Pending)
            || !self.world.assets.redelivery.is_empty()
    }
    /// Every retained model and dependency prepared for the current device.
    pub fn device_assets_ready(&self) -> bool {
        self.world.assets.states.iter().all(|(n, s)| {
            *s != crate::asset::AssetState::Loaded
                || n.ends_with(".level.json")
                || n.ends_with(".sound")
                || self.world.assets.prepared.contains(n)
        })
    }
    /// Move texture payloads to the renderer; no CPU mip copy survives upload.
    pub fn take_textures(
        &mut self,
    ) -> std::collections::BTreeMap<String, crate::asset::TextureData> {
        std::mem::take(&mut self.textures)
    }
    /// A named preparation/upload completed, or failed without poisoning the surface.
    pub fn asset_prepared(&mut self, name: &str, result: Result<(), String>) {
        use crate::asset::AssetState;
        match result {
            Ok(()) => {
                self.world.assets.prepared.insert(name.into());
            }
            Err(reason) => {
                self.world.assets.states.insert(
                    name.into(),
                    AssetState::Failed(format!("asset `{name}`: {reason}")),
                );
            }
        }
        self.finish_assets();
    }
    fn finish_assets(&mut self) {
        use crate::asset::AssetState;
        let assets = &mut *self.world.assets;
        for (name, textures) in &assets.dependencies {
            if !assets.states.contains_key(name) {
                continue;
            }
            if matches!(assets.states.get(name), Some(AssetState::Failed(_))) {
                continue;
            }
            let failed = textures.iter().find_map(|n| match assets.states.get(n) {
                Some(AssetState::Failed(e)) => Some(e.clone()),
                _ => None,
            });
            if let Some(reason) = failed {
                assets
                    .states
                    .insert(name.clone(), AssetState::Failed(reason));
            } else if textures
                .iter()
                .all(|n| assets.states.get(n) == Some(&AssetState::Loaded))
            {
                assets.states.insert(name.clone(), AssetState::Loaded);
            }
        }
        if self.setup_pending && assets.ready() {
            self.world = Self::build(&self.args, self.world.assets.clone());
            self.setup_pending = false;
            self.asset_mesh_revision = u64::MAX;
        }
    }
    /// Transport failure after the host's bounded retries.
    pub fn asset_failed(&mut self, name: &str, reason: &str) {
        self.world.assets.request(name);
        self.world.assets.redelivery.remove(name);
        self.world.assets.requested.insert(name.into());
        self.asset_prepared(name, Err(reason.into()));
    }
    /// Install a validated content result. Decoding belongs to the model adapter.
    pub fn deliver_asset(
        &mut self,
        name: &str,
        result: Result<crate::asset::Content, String>,
    ) -> Result<(), String> {
        use crate::asset::{AssetState, Content};
        self.world.assets.request(name);
        self.world.assets.requested.insert(name.into());
        self.world.assets.redelivery.remove(name);
        let result = result.and_then(|content| match content {
            Content::Level(text) => self.deliver_level(name, text).map(|()| None),
            Content::Sound(sound) => self.deliver_sound(name, sound).map(|()| None),
            other => Ok(Some(other)),
        });
        match result {
            Ok(None) => {}
            Ok(Some(Content::Level(_) | Content::Sound(_))) => unreachable!(),
            Ok(Some(Content::Texture(texture))) => {
                self.world
                    .assets
                    .states
                    .insert(name.into(), AssetState::Loaded);
                if self.defer_assets {
                    self.textures.insert(name.into(), texture);
                }
            }
            Ok(Some(Content::Model(model))) => {
                for texture in &model.textures {
                    self.world.assets.request(texture);
                    if self.world.assets.declared.contains(name) {
                        self.world.assets.required.insert(texture.clone());
                    }
                }
                self.world
                    .assets
                    .dependencies
                    .insert(name.into(), model.textures.clone());
                self.world.assets.models.insert(name.into(), model.into());
            }
            Err(reason) => {
                self.asset_failed(name, &reason);
                return Err(format!("asset `{name}`: {reason}"));
            }
        }
        self.finish_assets();
        Ok(())
    }
    fn decode_args(values: &[Value]) -> Result<G::Args, String> {
        crate::args::arity(values, G::Args::FIELDS)?;
        if values.len() == G::Args::FIELDS.len() {
            return G::Args::decode(values);
        }
        let mut complete = G::Args::default().values();
        complete[..values.len()].clone_from_slice(values);
        G::Args::decode(&complete)
    }
    /// Build at tick zero with seed zero; setup may reseed from a named argument.
    pub fn from_values(values: &[Value]) -> Result<Self, String> {
        Self::new(Self::decode_args(values)?)
    }
    /// Construct a simulation from typed game arguments.
    pub fn new(args: G::Args) -> Result<Self, String> {
        if G::HZ == 0 {
            return Err("game HZ must be positive".into());
        }
        crate::asset::level::validate_declaration::<G>()?;
        args.check_scalars()?;
        G::validate(&args)?;
        let world = Self::build(&args, Default::default());
        Self::from_world(
            args,
            world,
            Input::new(G::actions()),
            VecDeque::with_capacity(QUEUE_LIMIT),
        )
    }
    // Both callers prepare and validate their state before installing it.
    fn from_world(
        args: G::Args,
        world: World,
        input: Input,
        queue: VecDeque<Queued>,
    ) -> Result<Self, String> {
        Ok(Self {
            setup_pending: !world.assets.ready(),
            world,
            asset_mesh_revision: u64::MAX,
            asset_sprite_names: Vec::new(),
            shown: Default::default(),
            defer_assets: false,
            textures: Default::default(),
            args_json: crate::json::to_string(&args).map_err(|e| e.to_string())?,
            args,
            restarted: 0,
            last_epoch: std::cell::Cell::new(0),
            restored_from: None,
            settle_delay: std::cell::Cell::new(100),
            input,
            queue,
            overflow_logged: false,
            refused_posts: 0,
            posts_logged: false,
            rebase_queue: false,
            restored: false,
            last_us: None,
            world_us: 0,
            observations: Default::default(),
            live_time: None,
            last_ms: None,
            period_ms: 0.0,
            paused_clock: false,
            lookahead_us_hz: 0,
            last_motion: crate::Vec2::ZERO,
            paranoid: Self::reconstruction(Paranoid::environment()),
            game: PhantomData,
        })
    }
    /// Validate first, then seek under the old arguments to the host's stamp.
    /// Setup changes restart at tick zero; live changes affect subsequent ticks.
    pub fn bind(&mut self, values: &[Value], at_ms: Option<f64>) -> Result<(), String> {
        self.bind_with(values, at_ms, |_, _| {})
    }
    /// Timed bind with the same post-tick observer as advance_with.
    pub fn bind_with(
        &mut self,
        values: &[Value],
        at_ms: Option<f64>,
        after: impl FnMut(&World, u32),
    ) -> Result<(), String> {
        if at_ms.is_some_and(|at| !at.is_finite()) {
            return Err("bind clock must be finite".into());
        }
        let args = Self::decode_args(values)?;
        args.check_scalars()?;
        G::validate(&args)?;
        let args_json = crate::json::to_string(&args).map_err(|e| e.to_string())?;
        let changed = self.args_json != args_json;
        // Decoding is complete before seeking under the old arguments.
        let restart = if !self.args.setup_changed(&args) {
            None
        } else {
            Some(Self::build(&args, self.world.assets.clone()))
        };
        if let Some(at) = at_ms {
            self.advance_with(at, Clock::Seekable, after);
        }
        if let Some(mut world) = restart {
            let old_values = self.args.values();
            let new_values = args.values();
            let changes: Vec<_> = G::Args::FIELDS
                .iter()
                .zip(&old_values)
                .zip(&new_values)
                .filter(|((arg, old), new)| {
                    arg.1 != ArgumentKind::Live
                        && match (old, new) {
                            (Value::Number(a), Value::Number(b)) => a.to_bits() != b.to_bits(),
                            _ => old != new,
                        }
                })
                .map(|((arg, old), new)| {
                    format!(
                        "{} {} → {}",
                        arg.0,
                        crate::values::value_json(old, false),
                        crate::values::value_json(new, false)
                    )
                })
                .collect();
            self.restarted += u64::from(
                G::Args::FIELDS
                    .iter()
                    .zip(&old_values)
                    .zip(&new_values)
                    .any(|((field, old), new)| field.1 == ArgumentKind::Restart && old != new),
            );
            world.presentation_generation = self
                .world
                .presentation_generation
                .checked_add(1)
                .expect("presentation generation exhausted");
            self.setup_pending = !world.assets.ready();
            self.asset_mesh_revision = u64::MAX;
            self.world = world;
            self.world_us = 0;
            self.live_time = None;
            self.lookahead_us_hz = 0;
            self.queue.clear();
            let viewport = self.input.viewport;
            self.input = Input::new(G::actions());
            self.input.viewport = viewport;
            self.overflow_logged = false;
            self.posts_logged = false;
            self.restored = false;
            self.world
                .log(format_args!("world restarted: {}", changes.join(", ")));
        }
        self.args_json = args_json;
        self.args = args;
        if changed {
            self.invalidate();
        }
        if G::paused(&self.args) {
            self.flush_paused(self.last_us.unwrap_or(0));
            self.world_us = self.exact_world_us();
            self.live_time = None;
            self.lookahead_us_hz = 0;
            self.paused_clock = true;
        }
        Ok(())
    }
    /// Set canvas dimensions in points, for touch regions and agent projection.
    pub fn viewport(&mut self, width: f32, height: f32) {
        assert!(
            width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0,
            "viewport dimensions must be positive finite points"
        );
        self.input.viewport = Vec2::new(width, height);
    }
    fn projected(&self, e: &Queued) -> i64 {
        e.world_us.unwrap_or_else(|| {
            self.world_us
                .saturating_add(e.host_us.saturating_sub(self.last_us.unwrap_or(0)).max(0))
        })
    }
    fn boundary(&self, e: &Queued) -> u128 {
        self.projected(e) as u128 * G::HZ as u128 / 1_000_000
    }
    /// Validate host input without mutating the simulation or panicking.
    pub fn validate_input(&self, event: &InputEvent) -> Result<(), String> {
        self.input.validate(event)
    }
    /// Queue a raw event in stamp order, preserving arrival order at equal stamps.
    /// Paused input updates held state directly, without edges or queued wheel deltas.
    pub fn input(&mut self, event: InputEvent) {
        if let Err(error) = self.validate_input(&event) {
            self.world.log(error);
            return;
        }
        self.invalidate();
        // A posted message waits for the next tick, paused or not.
        if G::paused(&self.args) && !matches!(event, InputEvent::Message { .. }) {
            self.input.apply_paused(event);
            return;
        }
        let host_us = micros(event.at_ms());
        let e = Queued {
            host_us,
            world_us: self
                .last_us
                .filter(|last| host_us <= *last)
                .map(|_| self.world_us),
            event,
        };
        let position = self
            .queue
            .partition_point(|old| self.projected(old) <= self.projected(&e));
        if let InputEvent::Key { code, down, .. } = &e.event {
            let previous = self
                .queue
                .range(..position)
                .rev()
                .find_map(|old| match &old.event {
                    InputEvent::Key {
                        code: key, down, ..
                    } if key == code => Some(*down),
                    InputEvent::Blur { .. } => Some(false),
                    _ => None,
                })
                .unwrap_or_else(|| self.input.keys.contains(code));
            if previous == *down {
                return;
            }
        }
        // Only coalesce across other moves/wheels: contact and key edges retain
        // the positions and device state that were current when they arrived.
        let boundary = self.boundary(&e);
        for i in (0..position).rev() {
            if self.boundary(&self.queue[i]) != boundary {
                break;
            }
            match (&mut self.queue[i].event, &e.event) {
                (old, new) if old.same_motion(new) => {
                    let mut e = e;
                    e.event.add_motion(old);
                    self.queue.remove(i);
                    self.queue.insert(position - 1, e);
                    return;
                }
                (InputEvent::Wheel { dx, dy, .. }, InputEvent::Wheel { dx: x, dy: y, .. }) => {
                    *dx += x;
                    *dy += y;
                    return;
                }
                (
                    InputEvent::Pointer {
                        phase: PointerPhase::Move,
                        ..
                    }
                    | InputEvent::Control {
                        phase: PointerPhase::Move,
                        ..
                    }
                    | InputEvent::Wheel { .. },
                    _,
                ) => {}
                _ => break,
            }
        }
        let (mut position, mut e) = (position, e);
        if self.queue.len() < QUEUE_LIMIT {
            // Below the limit an overflow episode is over: the next one journals again.
            self.overflow_logged = false;
            self.posts_logged = false;
        }
        if self.queue.len() == QUEUE_LIMIT {
            // A posted message is never dropped silently: a full queue refuses
            // the new post, by name, and a device event never displaces one.
            let message = |e: &Queued| matches!(e.event, InputEvent::Message { .. });
            let drop = (!message(&e))
                .then(|| {
                    let moves = self.queue.iter().position(|e| e.event.is_move());
                    moves.or_else(|| self.queue.iter().position(|e| !message(e)))
                })
                .flatten();
            let Some(drop) = drop else {
                self.refused_posts += u64::from(message(&e));
                // Once per overflow episode for each kind; state counts each post.
                let logged = if message(&e) {
                    &mut self.posts_logged
                } else {
                    &mut self.overflow_logged
                };
                if !*logged {
                    *logged = true;
                    self.world.log(if message(&e) {
                        "postMessage refused: the input queue holds 1024 events; post less often (state input.refusedPosts counts them)"
                    } else {
                        "input queue overflow: every queued event is a posted message; dropped the new event"
                    });
                }
                return;
            };
            let was_move = self.queue[drop].event.is_move();
            let dropped = self.queue.remove(drop).expect("a queued event");
            position -= usize::from(drop < position);
            // A dropped move's motion joins that pointer's next event, so a
            // flood of moves loses positions, never the turn they add up to.
            if let InputEvent::Pointer { id, .. } = dropped.event {
                let same =
                    |ev: &InputEvent| matches!(ev, InputEvent::Pointer { id: n, .. } if *n == id);
                let (mut before, mut after) = (drop..position, position..self.queue.len());
                if let Some(i) = before.find(|&i| same(&self.queue[i].event)) {
                    self.queue[i].event.add_motion(&dropped.event);
                } else if same(&e.event) {
                    e.event.add_motion(&dropped.event);
                } else if let Some(i) = after.find(|&i| same(&self.queue[i].event)) {
                    self.queue[i].event.add_motion(&dropped.event);
                }
            }
            if !self.overflow_logged {
                self.world.log(if was_move {
                    "input queue overflow: dropped oldest move"
                } else {
                    "input queue overflow: dropped oldest event"
                });
                self.overflow_logged = true;
            }
        }
        self.queue.insert(position, e);
    }
    fn flush_paused(&mut self, now: i64) {
        let mut held = VecDeque::new();
        while self
            .queue
            .front()
            .is_some_and(|e| e.world_us.is_some() || e.host_us <= now)
        {
            let e = self.queue.pop_front().unwrap();
            if let InputEvent::Message { .. } = e.event {
                // Restamped at the pause's end, so it reaches the first tick after.
                held.push_back(Queued {
                    world_us: None,
                    host_us: now,
                    ..e
                });
            } else {
                self.input.apply_paused(e.event);
            }
        }
        held.append(&mut self.queue);
        self.queue = held;
        self.input.clear_edges();
    }
    /// Host display period in milliseconds; zero means not yet known. Applied
    /// only by the next accepted live advance, never by a backwards sample.
    pub fn frame_period(&mut self, period_ms: f64) {
        let period = if period_ms.is_finite() && period_ms > 0.0 {
            period_ms
        } else {
            0.0
        };
        if (period - self.period_ms).abs() >= self.period_ms * 0.005 {
            self.period_ms = period;
        }
    }
    fn phase_us(ms: f64) -> i128 {
        (ms * G::HZ as f64 * 1000.0).round() as i128
    }
    fn live_time(&self, now_ms: f64) -> LiveTime {
        let mut live = self.live_time.unwrap_or(LiveTime {
            phase: self.world_us as i128 * G::HZ as i128,
            remainder: 0.0,
            period_ms: self.period_ms,
            slew_left: None,
            // A known period at the first epoch has no preceding pose to jump.
            lookahead: Self::phase_us(self.period_ms.min(1000.0 / G::HZ as f64)) as f64,
        });
        let delta = if self.paused_clock {
            0.0
        } else {
            (now_ms - self.last_ms.unwrap_or(now_ms)).clamp(0.0, 250.0)
        };
        if live.period_ms != self.period_ms {
            live.period_ms = self.period_ms;
            live.slew_left = None;
        }
        // In particular, do not round a carried -0.5 again on a duplicate stamp.
        if delta == 0.0 {
            return live;
        }
        let units = delta * G::HZ as f64 * 1000.0;
        let increment = units + live.remainder;
        live.phase += increment.round() as i128;
        live.remainder = increment - increment.round();
        // L and grid alignment share one budget: |Δ(T + L) - delta| <= .0025*delta.
        // Schedule against this same horizon so the retained tick pair covers R.
        let wanted = Self::phase_us(live.period_ms.min(1000.0 / G::HZ as f64)) as f64;
        let budget = units * 0.0025;
        let horizon = crate::math::clamp(wanted - live.lookahead, -budget, budget);
        live.lookahead += horizon;
        let budget = (budget - horizon.abs()).max(0.0);
        if live.period_ms > 0.0 && budget > 0.0 {
            let period = live.period_ms * G::HZ as f64 * 1000.0;
            let left = live.slew_left.get_or_insert_with(|| {
                // Move the origin to the nearest frame, not every successive
                // deadline: noncommensurate grids must not chase a cyclic phase.
                let until = -(live.phase.rem_euclid(1_000_000) as f64) - live.remainder;
                let phase = until.rem_euclid(period);
                if phase > period / 2.0 {
                    phase - period
                } else {
                    phase
                }
            });
            let correction = crate::math::clamp(*left, -budget, budget);
            *left -= correction;
            let increment = correction + live.remainder;
            live.phase += increment.round() as i128;
            live.remainder = increment - increment.round();
        }
        live
    }
    fn lookahead(live: LiveTime) -> i128 {
        live.lookahead.round() as i128
    }
    fn target(phase: i128, lookahead: i128) -> u64 {
        // ceil(horizon / step) - 1 with L; equality waits for the next frame.
        ((phase + lookahead - i128::from(lookahead > 0)).max(0) / 1_000_000) as u64
    }
    fn exact_world_us(&self) -> i64 {
        let deadline = (self.world.tick() as u128 * 1_000_000).div_ceil(G::HZ as u128);
        self.world_us
            .max(i64::try_from(deadline).expect("simulation clock exhausted"))
    }
    fn backwards(&self, now_ms: f64) -> bool {
        self.last_ms.is_some_and(|last| now_ms < last)
    }
    fn seek_time(&self, now: i64) -> i64 {
        let base = if self.live_time.is_some() {
            self.exact_world_us()
        } else {
            self.world_us
        };
        base.checked_add(now.saturating_sub(self.last_us.unwrap_or(now)))
            .expect("simulation clock exhausted")
    }
    /// Number of steps the next advance would complete. Presentation can skip
    /// timing samples that a long advance would immediately evict from its ring.
    pub fn ticks_due(&self, now_ms: f64, clock: Clock) -> u32 {
        if self.setup_pending || !now_ms.is_finite() || G::paused(&self.args) {
            return 0;
        }
        if self.backwards(now_ms) {
            return 0;
        }
        let target = if clock == Clock::Live {
            let live = self.live_time(now_ms);
            Self::target(live.phase, Self::lookahead(live))
        } else {
            Self::target(self.seek_time(micros(now_ms)) as i128 * G::HZ as i128, 0)
        };
        u32::try_from(target.saturating_sub(self.world.tick())).unwrap_or(u32::MAX)
    }
    /// Advance integer world time; the first call establishes the host epoch only.
    pub fn advance(&mut self, now_ms: f64, clock: Clock) -> u32 {
        self.advance_with(now_ms, clock, |_, _| {})
    }
    /// Call back after each completed tick and propagation, with ticks still to run.
    pub fn advance_with(
        &mut self,
        now_ms: f64,
        clock: Clock,
        mut after: impl FnMut(&World, u32),
    ) -> u32 {
        assert!(now_ms.is_finite(), "host clock must be finite");
        if self.setup_pending {
            return 0;
        }
        if self.backwards(now_ms) {
            return 0;
        }
        self.check_epoch();
        if clock == Clock::Live {
            self.world.unobserve();
        }
        let now = micros(now_ms);
        let last = self.last_us.unwrap_or(now);
        // Restored pending host stamps are offsets until the first host sample.
        if self.last_us.is_none() && self.rebase_queue {
            for e in &mut self.queue {
                if e.world_us.is_none() {
                    e.host_us = now.saturating_add(e.host_us);
                }
            }
        }
        self.rebase_queue = false;
        if clock == Clock::Live && self.last_us.is_some() {
            // The epoch sample preserves rebased future stamps. On later frames,
            // every queued device event arrived before this callback. Pacing can
            // name the frame earlier than event.timeStamp; it cannot defer delivery.
            // min preserves stamp order and the spread of earlier catch-up input.
            for e in &mut self.queue {
                e.host_us = e.host_us.min(now);
            }
        }
        if G::paused(&self.args) {
            self.flush_paused(now);
            self.world_us = self.exact_world_us();
            self.live_time = None;
            self.lookahead_us_hz = 0;
            self.paused_clock = true;
            self.last_us = Some(now);
            self.last_ms = Some(now_ms);
            return 0;
        }
        let gap = now.saturating_sub(last);
        let live = (clock == Clock::Live).then(|| self.live_time(now_ms));
        let before = if clock == Clock::Seekable && self.live_time.is_some() {
            self.exact_world_us()
        } else {
            self.world_us
        };
        let world_us = live.map_or_else(
            || self.seek_time(now),
            |live| i64::try_from(live.phase / G::HZ as i128).expect("simulation clock exhausted"),
        );
        let lookahead = live.map_or(0, Self::lookahead);
        let phase = live.map_or(world_us as i128 * G::HZ as i128, |live| live.phase);
        let target = Self::target(phase, lookahead);
        self.lookahead_us_hz = lookahead;
        self.live_time = live;
        for e in &mut self.queue {
            if e.world_us.is_none() && e.host_us <= now {
                let offset = if clock == Clock::Live && (gap > 250_000 || self.paused_clock) {
                    0
                } else if clock == Clock::Live && gap > 0 {
                    // Stamp on the same dilated clock as the frame. Integer
                    // interpolation retains order and puts frame-time input at T.
                    (e.host_us.saturating_sub(last).clamp(0, gap) as i128
                        * (world_us - before) as i128
                        / gap as i128) as i64
                } else {
                    e.host_us.saturating_sub(last).clamp(0, gap)
                };
                e.world_us = Some(before.saturating_add(offset));
            }
        }
        self.world_us = world_us;
        self.last_us = Some(now);
        self.last_ms = Some(now_ms);
        self.paused_clock = false;
        let start = self.world.tick();
        // Exactly two samples per seek with work: before the last tick and after it.
        // A single-tick seek samples its starting state. Live never enters this path.
        if clock == Clock::Seekable && target == start + 1 {
            self.world.observe(&mut self.observations[0]);
        }
        while self.world.tick() < target {
            self.world.begin_tick();
            self.input.clear_edges();
            let end = (self.world.tick() as u128 + 1) * 1_000_000;
            let mut delivered = false;
            while self.queue.front().is_some_and(|e| {
                e.world_us.is_some_and(|us| {
                    let stamp = us as u128 * G::HZ as u128;
                    stamp < end || (clock == Clock::Live && stamp == end)
                })
            }) {
                self.input.apply(self.queue.pop_front().unwrap().event);
                delivered = true;
            }
            self.restored = false;
            self.restored_from = None;
            G::tick(&mut self.world, &self.input, &self.args);
            self.last_motion = self.input.pointer().map_or(crate::Vec2::ZERO, |p| p.delta);
            crate::scene::follow(&self.world);
            self.world.reap_orphans();
            self.world.propagate();
            self.world.step_clock();
            // Paranoid modes round-trip at every point an advance can be observed
            // (its last tick), at every tick that received input, and every
            // PARANOID_EVERY-th tick inside an advance.
            if let Some(rebuild) = self.paranoid {
                let tick = self.world.tick();
                if tick == target || delivered || tick.is_multiple_of(PARANOID_EVERY) {
                    rebuild(self);
                }
            }
            if clock == Clock::Seekable {
                let left = target - self.world.tick();
                if left == 1 {
                    self.world.observe(&mut self.observations[0]);
                }
                if left == 0 {
                    self.world.observe(&mut self.observations[1]);
                    self.world
                        .compare(&self.observations[0], &self.observations[1]);
                    self.last_epoch.set(self.world.mutation_epoch());
                }
            }
            after(
                &self.world,
                u32::try_from(target - self.world.tick()).unwrap_or(u32::MAX),
            );
        }
        u32::try_from(self.world.tick() - start).unwrap_or(u32::MAX)
    }
    fn paranoid_rebuild(&mut self, mode: Paranoid) {
        let tick = self.world.tick();
        let hash = self.world.hash();
        // advance_with owns the seek horizon, but EXSIM checkpoints describe a
        // completed boundary. Retain the horizon outside the reconstructed Sim.
        let horizon = self.world_us;
        self.world_us = ((tick as u128 * 1_000_000).div_ceil(G::HZ as u128)) as i64;
        let bytes = match self.save() {
            Ok(bytes) => bytes,
            // An asset first shown this tick is still in flight: no save can be
            // taken until it lands, so this sample waits for the next one rather
            // than failing the proof (a model first requested mid-game).
            Err(error) if error.message.starts_with("save refused: assets are not ready") => {
                self.world_us = horizon;
                return;
            }
            Err(error) => panic!("paranoid {:?} {} tick {tick}: {error}; rerun bun game/games/{}/proof.mjs linux --paranoid", mode, G::ID, G::ID),
        };
        let host = self.last_us;
        let mut queue = std::mem::take(&mut self.queue);
        queue.shrink_to_fit();
        let last_ms = self.last_ms;
        let live_time = self.live_time;
        let period_ms = self.period_ms;
        let lookahead = self.lookahead_us_hz;
        let last_motion = self.last_motion;
        let paused_clock = self.paused_clock;
        let rebase_queue = self.rebase_queue;
        let posts_logged = self.posts_logged;
        let observations = std::mem::take(&mut self.observations);
        // Resetting sprite names must still discover removal of the last texture.
        let assets_current = self.asset_mesh_revision == self.world.revision::<crate::Mesh>()
            && self.asset_sprite_names.is_empty();
        let delay = self.settle_delay.get();
        let pending = self.world.published_pending.get();
        let messages = self.take_messages();
        // These are driver outputs/ownership, not dependencies of Game::tick.
        // Keep them outside the rebuild just like advance_with's callback.
        if mode == Paranoid::FreshGame {
            let mut assets = std::mem::take(&mut self.world.assets);
            let models = assets
                .models
                .iter()
                .map(|(name, model)| (name.clone(), bin::to_vec(model.model.as_ref())))
                .collect::<Vec<_>>();
            assets.models = Default::default();
            // Drop all old component/resource values (including skipped fields
            // and physics executors) before decoding the replacement.
            let generation = self.world.presentation_generation;
            self.world = self.world.registered_scratch();
            self.world.presentation_generation = generation;
            for (name, bytes) in models {
                assets.models.insert(
                    name,
                    bin::from_slice::<crate::asset::Model>(&bytes)
                        .expect("paranoid asset decode")
                        .into(),
                );
            }
            self.world.assets = assets;
        }
        self.restore(&bytes)
            .unwrap_or_else(|error| panic!("paranoid {:?} {} tick {tick}: {error}; rerun bun game/games/{}/proof.mjs linux --paranoid", mode, G::ID, G::ID));
        assert_eq!(
            hash,
            self.world.hash(),
            "paranoid {:?} tick {tick}: world hash; rerun bun game/games/{}/proof.mjs linux --paranoid",
            mode, G::ID
        );
        self.world_us = horizon;
        self.last_us = host;
        self.last_ms = last_ms;
        self.live_time = live_time;
        self.period_ms = period_ms;
        self.lookahead_us_hz = lookahead;
        self.last_motion = last_motion;
        self.paused_clock = paused_clock;
        self.rebase_queue = rebase_queue;
        self.posts_logged = posts_logged;
        self.queue = queue;
        self.observations = observations;
        if assets_current {
            self.asset_mesh_revision = self.world.revision::<crate::Mesh>();
        }
        self.settle_delay.set(delay);
        self.last_epoch.set(self.world.mutation_epoch());
        self.world.published_pending.set(pending);
        *self.world.messages.borrow_mut() = messages;
        self.restored = false;
        self.restored_from = None;
    }
    fn alpha_numerator(&self) -> i128 {
        let world = self
            .live_time
            .map_or(self.world_us as i128 * G::HZ as i128, |live| live.phase);
        // R = T + L - step; alpha = (R - (tick - 1) * step) / step.
        world + self.lookahead_us_hz - self.world.tick() as i128 * 1_000_000
    }
    /// Render one tick behind the scheduled horizon, between the last two ticks.
    pub fn alpha(&self) -> f32 {
        if self.world.tick() == 0 {
            return 0.0;
        }
        // Startup or restore can lack the required history. Period changes slew
        // the shared horizon and therefore stay within the retained tick pair.
        self.alpha_numerator().clamp(0, 1_000_000) as f32 / 1_000_000.0
    }
    /// Pointer motion received but not yet shown by the pose `alpha` draws:
    /// the undrawn share of the last tick's and every queued event's, in
    /// points. A camera's [`crate::MouseLook`] turns by it at presentation, so
    /// a turn shows at the next frame whatever the tick and display rates.
    /// Never read by a tick, saved or hashed.
    pub fn unshown_motion(&self) -> crate::Vec2 {
        if G::paused(&self.args) || self.world.tick() == 0 {
            return crate::Vec2::ZERO;
        }
        let id = self.input.pointer().map(|p| p.id);
        let queued = self
            .queue
            .iter()
            .fold(crate::Vec2::ZERO, |sum, e| match &e.event {
                InputEvent::Pointer { id: at, dx, dy, .. } if id.is_none_or(|id| id == *at) => {
                    sum + crate::Vec2::new(*dx, *dy)
                }
                _ => sum,
            });
        self.last_motion * (1.0 - self.alpha()) + queued
    }
    /// Replacement generation for presentation caches; not saved or hashed.
    pub fn generation(&self) -> u64 {
        self.world.presentation_generation()
    }
    /// Read simulation state.
    pub fn world(&self) -> &World {
        &self.world
    }
    /// Current decoded canvas arguments.
    pub fn args(&self) -> &G::Args {
        &self.args
    }
    /// Edit simulation state, for setup tools and tests.
    pub fn world_mut(&mut self) -> &mut World {
        self.invalidate();
        &mut self.world
    }
    /// Take the current public record once after a change, rebuild or load.
    pub fn take_published(&mut self) -> Option<String> {
        self.world
            .published_pending
            .replace(false)
            .then(|| self.world.published_json(false))
    }
    /// Drain only explicit string events, in emission order.
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut *self.world.messages.borrow_mut())
    }
    fn invalidate(&mut self) {
        self.world.unobserve();
        self.settle_delay.set(100);
    }
    /// Read a component by entity handle or name.
    #[track_caller]
    pub fn get<C: crate::Component>(
        &self,
        entity: impl crate::Target,
    ) -> Option<crate::Ref<'_, C>> {
        self.world.get::<C>(entity)
    }
    /// Read the entity's local Transform position.
    #[track_caller]
    pub fn local_position(&self, entity: impl crate::Target) -> Option<crate::Vec3> {
        self.world.local_position(entity)
    }
    /// Current global position including parents.
    #[track_caller]
    pub fn global_position(&self, entity: impl crate::Target) -> Option<crate::Vec3> {
        self.world.global_position(entity)
    }
    /// Advance by milliseconds on the seekable clock, establishing an epoch if needed.
    pub fn run(&mut self, ms: f64) -> u32 {
        assert!(
            ms.is_finite() && ms >= 0.0,
            "run duration must be finite and nonnegative"
        );
        if self.last_us.is_none() {
            self.advance(0.0, Clock::Seekable);
        }
        self.advance(
            self.last_us.unwrap_or(0) as f64 / 1000.0 + ms,
            Clock::Seekable,
        )
    }
    /// Press a physical key at the current clock.
    pub fn key_down(&mut self, code: &str) {
        self.key(code, true);
    }
    /// Release a physical key at the current clock.
    pub fn key_up(&mut self, code: &str) {
        self.key(code, false);
    }
    fn key(&mut self, code: &str, down: bool) {
        self.input(InputEvent::Key {
            code: code.into(),
            down,
            at_ms: self.last_us.unwrap_or(0) as f64 / 1000.0,
        });
    }
    /// Post a message into the world at the current clock, as Contract's
    /// `postMessage(text, surface)` does; the next tick reads it in `Input::messages`.
    pub fn post(&mut self, text: impl Into<String>) {
        self.input(InputEvent::Message {
            text: text.into(),
            at_ms: self.last_us.unwrap_or(0) as f64 / 1000.0,
        });
    }
    /// Queue both key edges at the current clock.
    pub fn tap(&mut self, code: &str) {
        self.key_down(code);
        self.key_up(code);
    }
    /// Press, run for milliseconds, then queue a release at that clock.
    pub fn hold(&mut self, code: &str, ms: f64) {
        assert!(
            ms.is_finite() && ms >= 0.0,
            "hold duration must be finite and nonnegative"
        );
        self.key_down(code);
        self.run(ms);
        self.key_up(code);
    }
    /// Observe sixteen host rounds (initial read plus fifteen advances), with 100ms–2s back-off.
    pub fn settle(&mut self) -> bool {
        if self.last_us.is_none() {
            self.advance(0.0, Clock::Seekable);
        }
        // Hosts first advance to their current clock, read, then make at most
        // fifteen follow-up advances (sixteen rounds total).
        for round in 0..16 {
            if self.quiescent() {
                return true;
            }
            let at = self.settle_at(true);
            if round == 15 {
                break;
            }
            self.advance(at, Clock::Seekable);
        }
        let settled = self.quiescent();
        if !settled {
            self.world.log(format!(
                "settle exhausted: {}",
                self.changing(false).join(", ")
            ));
        }
        settled
    }
    // External leases restart back-off; ticks record their last observation epoch.
    fn check_epoch(&self) {
        let epoch = self.world.mutation_epoch();
        if self.last_epoch.replace(epoch) != epoch {
            self.settle_delay.set(100);
        }
    }
    /// Pending input and unobserved mutations prevent rest unless time is paused.
    pub fn quiescent(&self) -> bool {
        self.check_epoch();
        let settled = !self.setup_pending
            && self.queue.is_empty()
            && (G::paused(&self.args) || self.world.quiescent());
        if settled {
            self.settle_delay.set(100);
        }
        settled
    }
    pub(crate) fn changing(&self, quiescent: bool) -> Vec<String> {
        if self.setup_pending {
            return vec!["loading".into()];
        }
        if G::paused(&self.args) {
            return if self.queue.is_empty() {
                vec!["paused".into()]
            } else {
                vec!["paused".into(), "input queued".into()]
            };
        }
        if quiescent {
            return vec![];
        }
        let mut reasons = self.world.changing();
        if !self.queue.is_empty() && reasons.len() < 8 {
            reasons.push("input queued".into());
        }
        reasons
    }
    pub(crate) fn settle_at(&self, settle: bool) -> f64 {
        self.check_epoch();
        let now = self.last_us.unwrap_or(0);
        let tick_us = |tick: u64| {
            i64::try_from((tick as u128 * 1_000_000).div_ceil(G::HZ as u128)).unwrap_or(i64::MAX)
        };
        let deadline = self
            .world
            .settle_tick()
            .filter(|tick| *tick > self.world.tick());
        let delay = self.settle_delay.get();
        let mut next = deadline.map_or(self.world_us.saturating_add(delay as i64 * 1000), tick_us);
        if let Some(event) = self.queue.front() {
            // Events on an exact boundary belong to the following tick.
            let tick =
                (self.projected(event).max(0) as u128 * G::HZ as u128 / 1_000_000) as u64 + 1;
            next = next.min(tick_us(tick));
        }
        next = next.max(tick_us(self.world.tick() + 1));
        if settle {
            self.settle_delay.set((delay * 2).min(2000));
        }
        now.saturating_add(next.saturating_sub(self.world_us)) as f64 / 1000.0
    }
    /// Device state at the host boundary, including events waiting for a tick.
    pub(crate) fn host_input(&self) -> std::borrow::Cow<'_, Input> {
        if self.queue.is_empty() {
            return std::borrow::Cow::Borrowed(&self.input);
        }
        let mut input = self.input.clone();
        for queued in &self.queue {
            input.apply_paused(queued.event.clone());
        }
        std::borrow::Cow::Owned(input)
    }
    /// Named contacts, including queued presses and releases at the host boundary.
    pub fn held_controls(&self) -> Vec<String> {
        self.host_input().held_controls()
    }
}

#[cfg(test)]
#[path = "sim_tests.rs"]
mod render_time_tests;

mod paranoid;
use crate::data::limits::LoadBudget;
use crate::{bin, Actions, Data, DataError, Event, Input, InputEvent, Value, Vec2, World};
use crate::{Args, ArgumentKind, PointerPhase};
use std::{collections::VecDeque, marker::PhantomData};

/// One immutable simulation instant; Copy keeps world borrows short in game code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub struct Now {
    /// Completed fixed steps.
    pub tick: u64,
    /// Fixed steps per second.
    pub hz: u32,
}
/// A stateless game; components and resources hold every bit of simulation state.
pub trait Game: 'static {
    /// Surface name in Contract.
    const NAME: &'static str = "world";
    /// Stable save identity, independent of the shared surface name.
    const ID: &'static str;
    /// Named models required before setup and tick zero. Later mesh references load on sight.
    const ASSETS: &'static [&'static str] = &[];
    /// Game save schema version; older versions pass through migrate.
    const SAVE_VERSION: u32 = 1;
    /// Opt in only when every tick dependency is world state, bindings, input, or explicit time.
    /// Hidden network/storage/random results must remain unsupported by world-only replay.
    const CAPTURE_SUPPORTED: bool = false;
    /// Canvas argument declarations in positional order; also declares exact arity.
    type Args: Args;
    /// Immutable model and texture bytes embedded in the lazily loaded game module.
    /// Names are the values used by [`crate::Mesh::asset`].
    fn assets() -> &'static [crate::Asset] {
        &[]
    }
    /// Discoverable controls.
    fn actions() -> Actions {
        Actions::default()
    }
    /// Refuse domain-invalid arguments before any simulation mutation.
    fn validate(_args: &Self::Args) -> Result<(), String> {
        Ok(())
    }
    /// Construct the world after argument decoding succeeds.
    fn setup(world: &mut World, args: &Self::Args);
    /// Upgrade a loaded older world before it becomes observable.
    fn migrate(_world: &mut World, _from: u32) {}
    /// Stop world time while continuing to serve reads.
    fn paused(_args: &Self::Args) -> bool {
        false
    }
    /// One fixed step, called after inputs and before transform propagation.
    fn tick(world: &mut World, input: &Input, args: &Self::Args);
    /// Fixed steps per second.
    const HZ: u32 = 60;
    /// Clear any live bindings that represent physically held input on handoff.
    fn release_input(_args: &mut Self::Args) {}
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
    version: u32,
    world: Vec<u8>,
    base: Vec<u8>,
    base_args: String,
    args: String,
    input: Input,
    queue: Vec<Queued>,
    world_us: i64,
    journal: Vec<Event>,
    journal_next: u64,
    overflow_logged: bool,
    published: std::collections::BTreeMap<String, Value>,
}
// Host-only phase: one tick is 1_000_000 units. Only the bounded remainder is
// floating point; neither elapsed world time nor the slew grows in an f64.
#[derive(Clone, Copy)]
struct LiveTime {
    phase: i128,
    remainder: f64,
    period_ms: f64,
    slew_left: Option<f64>,
}
/// Opt-in save reconstruction after every completed tick. Never enabled by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Paranoid {
    /// Normal execution.
    #[default]
    Off,
    /// Rebuild through the production restore path after every tick.
    Save,
    /// Also discard and decode immutable assets before reconstructing the world.
    FreshGame,
}
impl Paranoid {
    fn environment() -> Self {
        match std::env::var("EXACT_GAME_PARANOID").as_deref() {
            Ok("1") => Self::Save,
            Ok("fresh-game") => Self::FreshGame,
            _ => Self::Off,
        }
    }
}
/// The clock, bounded device queue, and a game's world, without a host or GPU.
pub struct Sim<G: Game> {
    pub(crate) world: World,
    base: Vec<u8>,
    base_args: String,
    pub(crate) reload: crate::world::reload::Report,
    setup_pending: bool,
    asset_mesh_revision: u64,
    defer_assets: bool,
    textures: std::collections::BTreeMap<String, crate::asset::TextureData>,
    pub(crate) args: G::Args,
    pub(crate) restored_from: Option<String>,
    settle_delay: std::cell::Cell<u32>,
    last_epoch: std::cell::Cell<u64>,
    args_json: String,
    pub(crate) input: Input,
    queue: VecDeque<Queued>,
    overflow_logged: bool,
    rebase_queue: bool,
    pub(crate) restored: bool,
    pub(crate) last_us: Option<i64>,
    pub(crate) world_us: i64,
    observations: [crate::world::Observation; 2],
    pub(crate) recorder: Option<crate::capture::Recorder>,
    pub(crate) agent_owned: bool,
    pub(crate) contamination: u64,
    pub(crate) source_tagged: bool,
    // Live frame precision only; never part of seekable time, saves or hashes.
    live_time: Option<LiveTime>,
    last_ms: Option<f64>,
    period_ms: f64,
    paused_clock: bool,
    // Last scheduled lookahead, in microseconds × HZ (one tick = 1_000_000).
    lookahead_us_hz: i128,
    paranoid: Paranoid,
    game: PhantomData<G>,
}
const QUEUE_LIMIT: usize = 1024;
pub(crate) fn micros(ms: f64) -> i64 {
    (ms * 1000.0).round() as i64
}
impl<G: Game> Sim<G> {
    /// Override the test driver's EXACT_GAME_PARANOID setting for this simulation.
    pub fn paranoid(mut self, mode: Paranoid) -> Self {
        self.paranoid = mode;
        self
    }
    fn build(args: &G::Args, assets: crate::asset::Assets) -> World {
        let mut world = World::new(G::HZ, 0);
        world.assets = assets;
        world.register_scene();
        for &name in G::ASSETS {
            world.assets.declared.insert(name.into());
            world.assets.required.insert(name.into());
            world.assets.request(name);
        }
        if !world.assets.ready() {
            return world;
        }
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
        let revision = self.world.revision::<crate::Mesh>();
        if revision != self.asset_mesh_revision {
            let names: Vec<_> = self
                .world
                .query::<&crate::Mesh>()
                .iter()
                .filter_map(|(_, mesh)| {
                    if let crate::Mesh::Asset(name) = mesh {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect();
            for name in names {
                if !G::assets().iter().any(|asset| asset.name == name) {
                    self.world.assets.request(&name);
                }
            }
            self.asset_mesh_revision = revision;
        }
        let assets = &mut self.world.assets;
        let names: Vec<_> = assets
            .states
            .iter()
            .filter(|(n, s)| {
                **s == crate::asset::AssetState::Pending && !assets.requested.contains(*n)
            })
            .map(|(n, _)| n.clone())
            .collect();
        assets.requested.extend(names.iter().cloned());
        names
    }
    /// Device-backed surfaces defer readiness until pipeline preparation and upload finish.
    pub fn defer_assets(&mut self, defer: bool) {
        self.defer_assets = defer;
    }
    /// Renderer-only model feed; games receive World, whose reads enforce declarations.
    pub fn presentation_models(&self) -> impl Iterator<Item = (&str, &crate::asset::Model)> {
        self.world
            .assets
            .models
            .iter()
            .map(|(n, m)| (n.as_str(), m.as_ref()))
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
                if name.ends_with(".tex") {
                    self.world
                        .assets
                        .states
                        .insert(name.into(), AssetState::Loaded);
                }
            }
            Err(reason) => {
                self.world.assets.states.insert(
                    name.into(),
                    AssetState::Failed(format!("asset `{name}`: {reason}")),
                );
            }
        }
        if let Err(reason) = self.finish_assets() {
            self.world.log(format_args!("setup refused: {reason}"));
            self.world
                .assets
                .states
                .insert(name.into(), AssetState::Failed(reason));
        }
    }
    fn finish_assets(&mut self) -> Result<(), String> {
        use crate::asset::AssetState;
        let assets = &mut self.world.assets;
        for (name, model) in &assets.models {
            if matches!(assets.states.get(name), Some(AssetState::Failed(_))) {
                continue;
            }
            let failed = model
                .textures
                .iter()
                .find_map(|n| match assets.states.get(n) {
                    Some(AssetState::Failed(e)) => Some(e.clone()),
                    _ => None,
                });
            if let Some(reason) = failed {
                assets
                    .states
                    .insert(name.clone(), AssetState::Failed(reason));
            } else if assets.prepared.contains(name)
                && model
                    .textures
                    .iter()
                    .all(|n| assets.states.get(n) == Some(&AssetState::Loaded))
            {
                assets.states.insert(name.clone(), AssetState::Loaded);
            }
        }
        if self.setup_pending && assets.ready() {
            let world = Self::build(&self.args, self.world.assets.clone());
            let base = world.initializer().map_err(|e| e.to_string())?;
            self.base_args = crate::json::to_string(&self.args).map_err(|e| e.to_string())?;
            self.world = world;
            self.base = base;
            self.setup_pending = false;
            self.asset_mesh_revision = u64::MAX;
        }
        Ok(())
    }
    /// Transport failure after the host's bounded retries.
    pub fn asset_failed(&mut self, name: &str, reason: &str) {
        self.world.assets.requested.insert(name.into());
        self.asset_prepared(name, Err(reason.into()));
    }
    /// Install and validate one named model/texture. A declaration includes its textures.
    pub fn asset(&mut self, name: &str, bytes: Option<&[u8]>) -> Result<(), String> {
        use crate::asset::{AssetState, Model, TextureData};
        self.world.assets.request(name);
        self.world.assets.requested.insert(name.into());
        let result: Result<(), String> = (|| {
            if !crate::asset::asset_name(name) {
                return Err("invalid asset name".into());
            }
            let bytes = bytes.ok_or_else(|| "missing file".to_string())?;
            if name.ends_with(".tex") {
                let texture: TextureData = bin::from_slice(bytes).map_err(|e| e.to_string())?;
                texture.validate()?;
                if self.defer_assets {
                    self.textures.insert(name.into(), texture);
                } else {
                    self.world
                        .assets
                        .states
                        .insert(name.into(), AssetState::Loaded);
                }
            } else {
                let model: Model = bin::from_slice(bytes).map_err(|e| e.to_string())?;
                model.validate()?;
                for texture in &model.textures {
                    self.world.assets.request(texture);
                    if self.world.assets.declared.contains(name) {
                        self.world.assets.required.insert(texture.clone());
                    }
                }
                self.world
                    .assets
                    .models
                    .insert(name.into(), std::sync::Arc::new(model));
                if !self.defer_assets {
                    self.world.assets.prepared.insert(name.into());
                }
            }
            Ok(())
        })();
        let result = result.and(self.finish_assets());
        if let Err(reason) = &result {
            self.world.assets.states.insert(
                name.into(),
                AssetState::Failed(format!("asset `{name}`: {reason}")),
            );
        }
        result.map_err(|e| format!("asset `{name}`: {e}"))
    }
    /// Build at tick zero with seed zero; setup may reseed from a named argument.
    pub fn from_values(values: &[Value]) -> Result<Self, String> {
        Self::new(G::Args::decode(values)?)
    }
    /// Construct a simulation from typed game arguments.
    pub fn new(args: G::Args) -> Result<Self, String> {
        if G::HZ == 0 {
            return Err("game HZ must be positive".into());
        }
        for name in G::ASSETS {
            if !crate::asset::asset_name(name) {
                return Err(format!("asset `{name}`: invalid declaration name"));
            }
        }
        args.check_scalars()?;
        G::validate(&args)?;
        let world = Self::build(&args, Default::default());
        let base = world.initializer().map_err(|e| e.to_string())?;
        Ok(Self {
            world,
            base,
            base_args: crate::json::to_string(&args).map_err(|e| e.to_string())?,
            reload: Default::default(),
            setup_pending: !G::ASSETS.is_empty(),
            asset_mesh_revision: u64::MAX,
            defer_assets: false,
            textures: Default::default(),
            args_json: crate::json::to_string(&args).map_err(|e| e.to_string())?,
            args,
            last_epoch: std::cell::Cell::new(0),
            restored_from: None,
            settle_delay: std::cell::Cell::new(100),
            input: Input::new(G::actions()),
            queue: VecDeque::with_capacity(QUEUE_LIMIT),
            overflow_logged: false,
            rebase_queue: false,
            restored: false,
            last_us: None,
            world_us: 0,
            observations: Default::default(),
            recorder: None,
            agent_owned: false,
            contamination: 0,
            source_tagged: false,
            live_time: None,
            last_ms: None,
            period_ms: 0.0,
            paused_clock: false,
            lookahead_us_hz: 0,
            paranoid: Paranoid::environment(),
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
        let args = G::Args::decode(values)?;
        args.check_scalars()?;
        G::validate(&args)?;
        let old_values = self.args.values();
        let new_values = args.values();
        let changed = crate::bin::to_vec(&self.args) != crate::bin::to_vec(&args);
        let changes: Vec<_> = G::Args::FIELDS
            .iter()
            .zip(&old_values)
            .zip(&new_values)
            .filter(|((arg, old), new)| {
                arg.1 == ArgumentKind::Setup
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
        // Decoding is complete before seeking under the old arguments.
        let restart = if !self.args.setup_changed(&args) {
            None
        } else {
            let world = Self::build(&args, self.world.assets.clone());
            let base = world.initializer().map_err(|e| e.to_string())?;
            Some((world, base))
        };
        if let Some(at) = at_ms {
            self.advance_with(at, Clock::Seekable, after);
        }
        if let Some((mut world, base)) = restart {
            self.base_args = crate::json::to_string(&args).map_err(|e| e.to_string())?;
            self.base = base;
            self.reload = Default::default();
            self.capture_fail("construction binding restarted the world; start a new capture");
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
            self.restored = false;
            self.world
                .log(format_args!("world restarted: {}", changes.join(", ")));
        }
        let before = self.world.tick();
        self.args_json = crate::json::to_string(&args).map_err(|e| e.to_string())?;
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
        if changed {
            self.record(before, crate::capture::Operation::Bind(values.to_vec()));
        }
        Ok(())
    }
    /// Set canvas dimensions in points, for touch regions and agent projection.
    pub fn viewport(&mut self, width: f32, height: f32) {
        assert!(
            width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0,
            "viewport dimensions must be positive finite points"
        );
        if self.input.viewport != Vec2::new(width, height) {
            self.input.viewport = Vec2::new(width, height);
            self.record(
                self.world.tick(),
                crate::capture::Operation::Viewport { width, height },
            );
        }
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
    /// Queue a raw event in stamp order, preserving arrival order at equal stamps.
    /// Paused input updates held state directly, without edges or queued wheel deltas.
    pub fn input(&mut self, event: InputEvent) {
        let before = self.world.tick();
        let saved = self
            .recorder
            .as_ref()
            .filter(|r| r.active())
            .map(|_| event.clone());
        let accepted = self.accept_input(event);
        if let Some(event) = saved.filter(|_| accepted) {
            self.record(before, crate::capture::Operation::Input(event));
        }
    }
    fn accept_input(&mut self, event: InputEvent) -> bool {
        assert!(event.at_ms().is_finite(), "input stamp must be finite");
        self.invalidate();
        if G::paused(&self.args) {
            self.input.apply_paused(event);
            return true;
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
                return false;
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
                (
                    InputEvent::Pointer {
                        id: old,
                        phase: PointerPhase::Move,
                        ..
                    },
                    InputEvent::Pointer {
                        id,
                        phase: PointerPhase::Move,
                        ..
                    },
                ) if old == id => {
                    self.queue.remove(i);
                    self.queue.insert(position - 1, e);
                    return true;
                }
                (InputEvent::Wheel { dx, dy, .. }, InputEvent::Wheel { dx: x, dy: y, .. }) => {
                    *dx += x;
                    *dy += y;
                    return true;
                }
                (
                    InputEvent::Pointer {
                        phase: PointerPhase::Move,
                        ..
                    }
                    | InputEvent::Wheel { .. },
                    _,
                ) => {}
                _ => break,
            }
        }
        let mut position = position;
        if self.queue.len() == QUEUE_LIMIT {
            self.capture_fail("simulation input queue overflow; accepted event was dropped");
            let drop = self
                .queue
                .iter()
                .position(|e| {
                    matches!(
                        e.event,
                        InputEvent::Pointer {
                            phase: PointerPhase::Move,
                            ..
                        }
                    )
                })
                .unwrap_or(0);
            let was_move = matches!(
                self.queue[drop].event,
                InputEvent::Pointer {
                    phase: PointerPhase::Move,
                    ..
                }
            );
            if drop == 0 {
                self.queue.pop_front();
            } else {
                self.queue.remove(drop);
            }
            position -= usize::from(drop < position);
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
        true
    }
    fn flush_paused(&mut self, now: i64) {
        while self
            .queue
            .front()
            .is_some_and(|e| e.world_us.is_some() || e.host_us <= now)
        {
            self.input
                .apply_paused(self.queue.pop_front().unwrap().event);
        }
        self.input.clear_edges();
    }
    /// Explicit owner handoff clears physical input and rebases at the next host sample.
    /// It does not alter any persistent save or simulate time spent detached.
    pub fn handoff(&mut self, agent: bool) {
        self.capture_fail("input/clock owner changed; start a new capture window");
        self.queue.clear();
        self.input.apply_paused(InputEvent::Blur { at_ms: 0.0 });
        self.input.clear_edges();
        G::release_input(&mut self.args);
        self.args_json = crate::json::to_string(&self.args).expect("valid input bindings");
        self.agent_owned = agent;
        self.world_us = self.exact_world_us();
        self.last_us = None;
        self.last_ms = None;
        self.live_time = None;
        self.lookahead_us_hz = 0;
        self.world.log(if agent {
            "control: agent attached; controlled clock"
        } else {
            "control: agent detached; human input and live clock"
        });
    }
    /// Input with a host-attested source; unexpected human input contaminates a controlled run.
    pub fn input_from(&mut self, event: InputEvent, agent: bool) {
        self.source_tagged = true;
        if self.agent_owned && !agent {
            self.contamination = self.contamination.saturating_add(1);
            self.capture_fail("external human input contaminated the controlled capture");
            self.world
                .log("control: external human input during agent ownership");
        }
        self.input(event);
    }
    /// Rebase after a staged replacement without executing a tick or consuming an action.
    pub fn rebase(&mut self, now_ms: f64, release_input: bool) -> Result<(), String> {
        if !now_ms.is_finite() {
            return Err("rebase clock must be finite".into());
        }
        self.capture_fail("host clock rebased during recording; start a new capture window");
        let owner = self.agent_owned;
        if release_input {
            self.handoff(owner);
        }
        self.world_us = self.exact_world_us();
        self.last_us = Some(micros(now_ms));
        self.last_ms = Some(now_ms);
        self.live_time = None;
        self.lookahead_us_hz = 0;
        if self.rebase_queue {
            for e in &mut self.queue {
                if e.world_us.is_none() {
                    e.host_us = e.host_us.saturating_add(micros(now_ms));
                }
            }
            self.rebase_queue = false;
        }
        Ok(())
    }
    /// Host display period in milliseconds; zero means not yet known. Applied
    /// only by the next accepted live advance, never by a backwards sample.
    pub fn frame_period(&mut self, period_ms: f64) {
        self.period_ms = if period_ms.is_finite() && period_ms > 0.0 {
            period_ms
        } else {
            0.0
        };
    }
    fn phase_us(ms: f64) -> i128 {
        (ms * G::HZ as f64 * 1000.0).round() as i128
    }
    fn live_time(&self, now_ms: f64) -> LiveTime {
        let mut live = self.live_time.unwrap_or(LiveTime {
            phase: self.world_us as i128 * G::HZ as i128,
            remainder: 0.0,
            period_ms: 0.0,
            slew_left: None,
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
        let units = delta * G::HZ as f64 * 1000.0;
        let increment = units + live.remainder;
        live.phase += increment.round() as i128;
        live.remainder = increment - increment.round();
        if live.period_ms > 0.0 && delta > 0.0 {
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
            let correction = left.clamp(-units * 0.0025, units * 0.0025);
            *left -= correction;
            let increment = correction + live.remainder;
            live.phase += increment.round() as i128;
            live.remainder = increment - increment.round();
        }
        live
    }
    fn lookahead(live: LiveTime) -> i128 {
        Self::phase_us(live.period_ms.min(1000.0 / G::HZ as f64))
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
        let before_tick = self.world.tick();
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
        if G::paused(&self.args) {
            self.flush_paused(now);
            if now != last {
                self.record(
                    before_tick,
                    crate::capture::Operation::Advance {
                        at_us: now,
                        live: clock == Clock::Live,
                    },
                );
            }
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
            while self.queue.front().is_some_and(|e| {
                e.world_us.is_some_and(|us| {
                    let stamp = us as u128 * G::HZ as u128;
                    stamp < end || (clock == Clock::Live && stamp == end)
                })
            }) {
                self.input.apply(self.queue.pop_front().unwrap().event);
            }
            self.restored = false;
            self.restored_from = None;
            G::tick(&mut self.world, &self.input, &self.args);
            self.world.reap_orphans();
            self.world.propagate();
            self.world.step_clock();
            self.paranoid_rebuild();
            if clock == Clock::Seekable {
                let left = target - self.world.tick();
                if left == 1 {
                    self.world.observe(&mut self.observations[0]);
                }
                if left == 0 {
                    self.world
                        .observe_with_hash(&mut self.observations[1], true);
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
        if now != last {
            self.record(
                before_tick,
                crate::capture::Operation::Advance {
                    at_us: now,
                    live: clock == Clock::Live,
                },
            );
        }
        u32::try_from(self.world.tick() - start).unwrap_or(u32::MAX)
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
        // Startup, restore or a display-rate change can lack the required history.
        // A stall with an unchanged period never changes L or hits this guard.
        self.alpha_numerator().clamp(0, 1_000_000) as f32 / 1_000_000.0
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
        self.capture_fail("direct world mutation is outside the input/binding capture");
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
    pub fn get<C: crate::Component>(
        &self,
        entity: impl crate::Target,
    ) -> Option<crate::Ref<'_, C>> {
        self.world.get::<C>(entity)
    }
    /// Read the entity's authored position, or None when it has no Transform.
    pub fn position(&self, entity: impl crate::Target) -> Option<crate::Vec3> {
        self.get::<crate::Transform>(entity)
            .map(|pose| pose.position)
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
        self.quiescent()
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
    pub(crate) fn held_keys(&self) -> Vec<String> {
        let mut keys: std::collections::BTreeSet<_> = self.input.keys.iter().cloned().collect();
        for queued in &self.queue {
            match &queued.event {
                InputEvent::Key {
                    code, down: true, ..
                } => {
                    keys.insert(code.clone());
                }
                InputEvent::Key {
                    code, down: false, ..
                } => {
                    keys.remove(code);
                }
                InputEvent::Blur { .. } => keys.clear(),
                _ => {}
            }
        }
        keys.into_iter().collect()
    }
    pub(crate) fn capture_queue(&self) -> Vec<u8> {
        bin::to_vec(&self.relative_queue())
    }
    fn relative_queue(&self) -> Vec<Queued> {
        let world_us = self.exact_world_us();
        let mut queue: Vec<_> = self.queue.iter().cloned().collect();
        for e in &mut queue {
            if let Some(us) = &mut e.world_us {
                *us -= world_us;
                e.host_us = 0;
            } else {
                e.host_us = e.host_us.saturating_sub(self.last_us.unwrap_or(0));
            }
            e.event.set_at_ms(0.0); // the queue owns the stamp; no absolute host time in a save
        }
        queue
    }
    /// Save world time and relative pending input, independent of the host epoch.
    pub fn save(&self) -> Vec<u8> {
        assert!(
            !self.is_loading(),
            "save refused: declared assets are not ready: {}",
            self.world.assets.state_json()
        );
        let saved = Saved {
            game: G::ID.into(),
            version: G::SAVE_VERSION,
            world: self.world.save(),
            base: self.base.clone(),
            // Usually identical: only retain a separate construction after live
            // bindings or an authored carry made the two argument sets differ.
            base_args: if self.base_args == self.args_json {
                String::new()
            } else {
                self.base_args.clone()
            },
            args: self.args_json.clone(),
            input: self.input.clone(),
            queue: self.relative_queue(),
            world_us: self.exact_world_us(),
            published: self.world.publications(),
            journal: self.world.journal(),
            journal_next: self.world.journal_next(),
            overflow_logged: self.overflow_logged,
        };
        let mut bytes = b"EXSIM\0\x06".to_vec();
        bytes.extend(bin::to_vec(&saved));
        bytes
    }
    /// Build a fresh simulation using the checkpoint construction arguments.
    /// Defaults need not be valid construction input for this game.
    pub fn from_save(bytes: &[u8]) -> Result<Self, DataError> {
        Self::from_save_in(bytes, None)
    }
    pub(crate) fn from_save_in(
        bytes: &[u8],
        budget: Option<&LoadBudget>,
    ) -> Result<Self, DataError> {
        let payload = bytes
            .strip_prefix(b"EXSIM\0\x06")
            .ok_or_else(|| DataError::new("unsupported simulation save format (expected EXSIM v6; older saves lack the tick-zero base, restart required)"))?;
        let saved: Saved = bin::from_slice_in(payload, budget)?;
        if saved.game != G::ID {
            return Err(DataError::new("save game ID differs"));
        }
        let args = crate::json::from_str_in(
            if saved.base_args.is_empty() {
                &saved.args
            } else {
                &saved.base_args
            },
            budget,
        )?;
        let mut sim = Self::new(args).map_err(DataError::new)?;
        sim.restore_into(bytes, None, budget)?;
        Ok(sim)
    }
    /// Atomically restore dynamic state onto this binary's actions.
    /// An agent-owned clock retains its established host boundary; a live clock
    /// rebases on its next sample, excluding time spent paused from simulation.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        self.restore_into(bytes, None, None)
    }
    /// Continue retains saved construction arguments and takes current live bindings.
    pub fn restore_bound(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        let args = crate::json::to_string(&self.args)?;
        let sounds = self
            .world
            .has_audio()
            .then(|| self.world.resource::<crate::audio::Sounds>().clone());
        self.restore_into(bytes, Some(&args), None)?;
        if let Some(sounds) = sounds {
            *self.world.resource_mut::<crate::audio::Sounds>() = sounds;
        }
        Ok(())
    }
    fn restore_into(
        &mut self,
        bytes: &[u8],
        args: Option<&str>,
        budget: Option<&LoadBudget>,
    ) -> Result<(), DataError> {
        let payload = bytes.strip_prefix(b"EXSIM\0\x06").ok_or_else(|| {
            DataError::new(format!(
                "unsupported simulation save format (expected EXSIM v6; older saves lack the tick-zero base, restart required; saw {:02x?})",
                &bytes[..bytes.len().min(8)]
            ))
        })?;
        let s: Saved = bin::from_slice_in(payload, budget)?;
        if s.game != G::ID {
            return Err(DataError::new(format!(
                "save belongs to `{}`, expected `{}`",
                s.game,
                G::ID
            )));
        }
        if s.version > G::SAVE_VERSION {
            return Err(DataError::new(format!(
                "save for `{}` has newer version {} (supported {})",
                G::ID,
                s.version,
                G::SAVE_VERSION
            )));
        }
        if s.world_us < 0 || s.queue.len() > QUEUE_LIMIT {
            return Err(DataError::new("invalid saved clock or input queue"));
        }
        let saved_args: G::Args = crate::json::from_str_in(&s.args, budget)?;
        let bound = if let Some(args) = args {
            let current: G::Args = crate::json::from_str_in(args, budget)?;
            let saved_values = saved_args.values();
            let values: Vec<_> = G::Args::FIELDS
                .iter()
                .zip(current.values())
                .zip(saved_values)
                .map(|(((_, kind), current), saved)| {
                    if *kind == ArgumentKind::Setup {
                        saved
                    } else {
                        current
                    }
                })
                .collect();
            G::Args::decode(&values).map_err(DataError::new)?
        } else {
            saved_args
        };
        // The saved gameplay construction may predate an applied scene edit.
        // Reconstruct tick zero from the arguments that produced its base, so a
        // second same-build restore cannot undo that edit.
        let initial_args = if args.is_some() {
            &self.base_args
        } else if s.base_args.is_empty() {
            &s.args
        } else {
            &s.base_args
        };
        let initial_args = crate::json::from_str_in(initial_args, budget)?;
        let mut next = Self::new(initial_args).map_err(DataError::new)?;
        next.world = Self::build(&next.args, self.world.assets.clone());
        next.args_json = crate::json::to_string(&bound)?;
        next.args = bound;
        next.setup_pending = !next.world.assets.ready();
        next.defer_assets = self.defer_assets;
        if next.setup_pending {
            return Err(DataError::new("restore awaits declared assets"));
        }
        // A bound continuation compares against this destination's construction,
        // including its freshly baked scene, before retaining saved setup args.
        let theirs = if args.is_some() {
            self.base.clone()
        } else {
            next.world.initializer()?
        };
        if args.is_some() && self.setup_pending {
            return Err(DataError::new("restore awaits declared assets"));
        }
        next.base = theirs;
        if args.is_some() {
            next.world.inherit_registry(&self.world);
        }
        next.world.load_in(&s.world, budget)?;
        let due = s.world_us as u128 * G::HZ as u128 / 1_000_000;
        if next.world.hz() != G::HZ || next.world.tick() as u128 != due {
            return Err(DataError::new("saved world and clock disagree"));
        }
        if s.version < G::SAVE_VERSION {
            G::migrate(&mut next.world, s.version);
        }
        next.reload = next.world.merge_initializer(&s.base, &next.base, budget)?;
        crate::scene::place_followers(&next.world);
        next.world.propagate();
        next.world.restore_journal(s.journal, s.journal_next);
        next.reload.log(&next.world);
        next.world.restore_publications(s.published);
        next.world.published_pending.set(true);
        next.input.restore_dynamic(s.input);
        next.queue = s.queue.into();
        for e in &mut next.queue {
            if let Some(us) = &mut e.world_us {
                *us = us
                    .checked_add(s.world_us)
                    .ok_or_else(|| DataError::new("saved input stamp overflow"))?;
            }
        }
        next.world_us = s.world_us;
        next.world.unobserve();
        next.restored_from = Some(s.args);
        next.overflow_logged = s.overflow_logged;
        next.rebase_queue = true;
        next.restored = true;
        next.world.presentation_generation = self
            .world
            .presentation_generation
            .checked_add(1)
            .expect("presentation generation exhausted");
        // The controlled host cannot move during restore. Anchor its current
        // boundary explicitly, including saved pending input offsets, so the
        // next seek executes its full duration. Live time instead rebases on
        // the next sample; loading a checkpoint must not simulate the pause.
        if let Some(now) = self.last_us.filter(|_| self.agent_owned) {
            next.rebase(now as f64 / 1000.0, false)
                .map_err(DataError::new)?;
        }
        self.capture_fail("world restored during recording; start a new capture window");
        next.recorder = self.recorder.take();
        next.paranoid = self.paranoid;
        next.agent_owned = self.agent_owned;
        next.contamination = self.contamination;
        next.source_tagged = self.source_tagged;
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod render_time_tests {
    use super::*;
    struct Ticker<const HZ: u32>;
    impl<const HZ: u32> Game for Ticker<HZ> {
        const ID: &'static str = "alpha-guard";
        const HZ: u32 = HZ;
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    fn steady<const HZ: u32>() {
        for frame_hz in [60.0, 59.94, 120.0, 144.0, 240.0] {
            for epoch in [0.0, 1234.567, 1_000_000.123] {
                for phase in [0.0, 0.1, 0.25, 0.4] {
                    let mut s = Sim::<Ticker<HZ>>::new(()).unwrap();
                    s.frame_period(1000.0 / frame_hz);
                    s.advance(epoch, Clock::Live);
                    for frame in 1..=600 {
                        let now =
                            epoch + frame as f64 * 1000.0 / frame_hz + phase * 1000.0 / HZ as f64;
                        let due = s.ticks_due(now, Clock::Live);
                        assert_eq!(s.advance(now, Clock::Live), due);
                        if HZ == 120 && frame_hz == 120.0 && phase == 0.0 {
                            assert_eq!(due, 1, "120/120 frame {frame}");
                        }
                        // Inspect the signed numerator BEFORE alpha's guard.
                        let raw = s.alpha_numerator();
                        assert!((0..=1_000_000).contains(&raw),
                            "world {HZ}, display {frame_hz}, epoch {epoch}, phase {phase}, frame {frame}: {raw}");
                        if s.world.tick() > 0 {
                            assert_eq!(s.alpha(), raw as f32 / 1_000_000.0);
                        }
                    }
                    // Seekable must never reuse the preceding live lookahead.
                    s.advance(epoch + 602.0 * 1000.0 / frame_hz, Clock::Seekable);
                    assert_eq!(s.lookahead_us_hz, 0);
                    assert!(s.live_time.is_none());
                }
            }
        }
    }
    #[test]
    fn alpha_guard_never_fires_on_600_steady_frames_at_both_world_rates() {
        steady::<60>();
        steady::<120>();
    }
    #[test]
    fn integer_phase_survives_the_old_eighteen_hour_precision_limit() {
        let mut s = Sim::<Ticker<120>>::new(()).unwrap();
        // Just beyond 2^26 milliseconds, without running eight million ticks.
        let tick = 8_100_000;
        for _ in 0..tick {
            s.world.step_clock();
        }
        assert_eq!(s.world.tick(), tick);
        s.world_us = tick as i64 * 1_000_000 / 120;
        s.frame_period(1000.0 / 120.0);
        s.advance(0.0, Clock::Live);
        for frame in 1..=3600 {
            let now = frame as f64 * 1000.0 / 120.0;
            assert_eq!(s.ticks_due(now, Clock::Live), 1);
            assert_eq!(s.advance(now, Clock::Live), 1);
            let live = s.live_time.unwrap();
            assert!(live.remainder.abs() <= 0.5);
            assert_eq!(live.phase, (tick as i128 + frame as i128) * 1_000_000);
            assert_eq!(s.alpha(), 1.0);
        }
    }
    #[test]
    fn restore_refuses_a_one_tick_ahead_clock() {
        let mut s = Sim::<Ticker<60>>::new(()).unwrap();
        s.advance(0.0, Clock::Seekable);
        s.advance(17.0, Clock::Seekable);
        let good = s.save();
        let mut saved: Saved = bin::from_slice(&good[7..]).unwrap();
        saved.world_us = 10_000;
        let mut bad = b"EXSIM\0\x06".to_vec();
        bad.extend(bin::to_vec(&saved));
        assert!(s
            .restore(&bad)
            .unwrap_err()
            .to_string()
            .contains("world and clock disagree"));
        assert_eq!(s.save(), good);
    }
}

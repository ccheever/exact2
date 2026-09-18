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
        // wasm has no process environment; proof builds select the same mode.
        let mode = if cfg!(target_arch = "wasm32") {
            option_env!("EXACT_GAME_PARANOID").map(str::to_owned)
        } else {
            std::env::var("EXACT_GAME_PARANOID").ok()
        };
        match mode.as_deref() {
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
    defer_assets: bool,
    textures: std::collections::BTreeMap<String, crate::asset::TextureData>,
    pub(crate) args: G::Args,
    pub(crate) restarted: u64,
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
    world_us: i64,
    observations: [crate::world::Observation; 2],
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
    /// Override EXACT_GAME_PARANOID for this simulation.
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
            if !world.assets.models.contains_key(name) {
                world.assets.request(name);
            }
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
        if self.setup_pending && !self.assets_pending() {
            return Vec::new();
        }
        let revision = self
            .world
            .revision::<crate::Mesh>()
            .wrapping_add(self.world.revision::<crate::Sprite>());
        if revision != self.asset_mesh_revision {
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
            if self.setup_pending {
                roots.extend(G::ASSETS.iter().map(|n| (*n).to_owned()));
            }
            self.world.assets.retire(&roots);
            self.textures
                .retain(|n, _| self.world.assets.states.contains_key(n));
            for (name, suffix) in names {
                if !self.world.assets.request(&name) {
                    continue;
                }
                if !name.ends_with(suffix) {
                    self.asset_failed(&name, &format!("component requires a {suffix} name"));
                }
            }
            self.asset_mesh_revision = revision;
        }
        let assets = &mut self.world.assets;
        let names: Vec<_> = assets
            .states
            .iter()
            .filter(|(n, s)| {
                (**s == crate::asset::AssetState::Pending || assets.redelivery.contains(*n))
                    && !assets.requested.contains(*n)
            })
            .map(|(n, _)| n.clone())
            .collect();
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
            .map(|(n, m)| (n.as_str(), m.as_ref()))
    }
    /// Content remains Loaded after loss; only device preparation is invalidated.
    pub fn invalidate_device_assets(&mut self) -> Vec<String> {
        let assets = &mut self.world.assets;
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
            *s != crate::asset::AssetState::Loaded || self.world.assets.prepared.contains(n)
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
        let assets = &mut self.world.assets;
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
        if !self.world.assets.request(name) {
            return;
        }
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
        if !self.world.assets.request(name) {
            return Err(format!("asset `{name}`: surface limit is 256 names"));
        }
        self.world.assets.requested.insert(name.into());
        self.world.assets.redelivery.remove(name);
        match result {
            Ok(Content::Texture(texture)) => {
                self.world
                    .assets
                    .states
                    .insert(name.into(), AssetState::Loaded);
                if self.defer_assets {
                    self.textures.insert(name.into(), texture);
                }
            }
            Ok(Content::Model(model)) => {
                let extra = model
                    .textures
                    .iter()
                    .filter(|n| !self.world.assets.states.contains_key(n))
                    .count();
                if self.world.assets.states.len() + extra > 256 {
                    self.asset_failed(name, "dependencies exceed surface limit of 256 names");
                    return Err(format!(
                        "asset `{name}`: dependencies exceed surface limit of 256 names"
                    ));
                }
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
                self.world
                    .assets
                    .models
                    .insert(name.into(), std::sync::Arc::new(model));
            }
            Err(reason) => {
                self.asset_failed(name, &reason);
                return Err(format!("asset `{name}`: {reason}"));
            }
        }
        self.finish_assets();
        Ok(())
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
        if G::ASSETS.len() > 256 {
            return Err(format!(
                "asset `{}`: surface limit is 256 names",
                G::ASSETS[256]
            ));
        }
        for name in G::ASSETS {
            if !crate::asset::asset_name(name)
                || !(name.ends_with(".model") || name.ends_with(".tex"))
            {
                return Err(format!(
                    "asset `{name}`: declaration requires a .model or .tex name"
                ));
            }
        }
        args.check_scalars()?;
        G::validate(&args)?;
        Ok(Self {
            world: Self::build(&args, Default::default()),
            setup_pending: !G::ASSETS.is_empty(),
            asset_mesh_revision: u64::MAX,
            defer_assets: false,
            textures: Default::default(),
            args_json: crate::json::to_string(&args).map_err(|e| e.to_string())?,
            args,
            restarted: 0,
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
            self.restored = false;
            self.world
                .log(format_args!("world restarted: {}", changes.join(", ")));
        }
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
    /// Queue a raw event in stamp order, preserving arrival order at equal stamps.
    /// Paused input updates held state directly, without edges or queued wheel deltas.
    pub fn input(&mut self, event: InputEvent) {
        assert!(event.at_ms().is_finite(), "input stamp must be finite");
        self.invalidate();
        if G::paused(&self.args) {
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
                    | InputEvent::Wheel { .. },
                    _,
                ) => {}
                _ => break,
            }
        }
        let mut position = position;
        if self.queue.len() == QUEUE_LIMIT {
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
        let horizon = (wanted - live.lookahead).clamp(-budget, budget);
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
            let correction = left.clamp(-budget, budget);
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
        u32::try_from(self.world.tick() - start).unwrap_or(u32::MAX)
    }
    fn paranoid_rebuild(&mut self) {
        if self.paranoid == Paranoid::Off {
            return;
        }
        let tick = self.world.tick();
        let hash = self.world.hash();
        // advance_with owns the seek horizon, but EXSIM checkpoints describe a
        // completed boundary. Retain the horizon outside the reconstructed Sim.
        let horizon = self.world_us;
        self.world_us = ((tick as u128 * 1_000_000).div_ceil(G::HZ as u128)) as i64;
        let bytes = self.save().expect("paranoid save");
        let host = self.last_us;
        let queue = self.queue.clone();
        let last_ms = self.last_ms;
        let live_time = self.live_time;
        let period_ms = self.period_ms;
        let lookahead = self.lookahead_us_hz;
        let paused_clock = self.paused_clock;
        let rebase_queue = self.rebase_queue;
        let observations = std::mem::take(&mut self.observations);
        let delay = self.settle_delay.get();
        let pending = self.world.published_pending.get();
        let messages = self.take_messages();
        // These are driver outputs/ownership, not dependencies of Game::tick.
        // Keep them outside the rebuild just like advance_with's callback.
        if self.paranoid == Paranoid::FreshGame {
            let mut assets = std::mem::take(&mut self.world.assets);
            let models = assets
                .models
                .iter()
                .map(|(name, model)| (name.clone(), bin::to_vec(model.as_ref())))
                .collect::<Vec<_>>();
            assets.models = Default::default();
            // Drop all old component/resource values (including skipped fields
            // and physics executors) before invoking setup for the replacement.
            let generation = self.world.presentation_generation;
            self.world = World::new(G::HZ, 0);
            self.world.presentation_generation = generation;
            for (name, bytes) in models {
                assets.models.insert(
                    name,
                    std::sync::Arc::new(bin::from_slice(&bytes).expect("paranoid asset decode")),
                );
            }
            self.world.assets = assets;
        }
        self.restore(&bytes)
            .unwrap_or_else(|error| panic!("paranoid {} tick {tick}: {error}", G::ID));
        assert_eq!(
            hash,
            self.world.hash(),
            "paranoid {} tick {tick}: world hash",
            G::ID
        );
        self.world_us = horizon;
        self.last_us = host;
        self.last_ms = last_ms;
        self.live_time = live_time;
        self.period_ms = period_ms;
        self.lookahead_us_hz = lookahead;
        self.paused_clock = paused_clock;
        self.rebase_queue = rebase_queue;
        self.queue = queue;
        self.observations = observations;
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
    pub fn get<C: crate::Component>(
        &self,
        entity: impl crate::Target,
    ) -> Option<crate::Ref<'_, C>> {
        self.world.get::<C>(entity)
    }
    /// Read the entity's current global position, including ancestor transforms.
    pub fn position(&self, entity: impl crate::Target) -> Option<crate::Vec3> {
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
    /// Save world time and relative pending input, independent of the host epoch.
    pub fn save(&self) -> Result<Vec<u8>, DataError> {
        let assets = &self.world.assets;
        let mut needed = std::collections::BTreeSet::new();
        for (_, mesh) in self.world.query::<&crate::Mesh>().iter() {
            if let crate::Mesh::Asset(name) = mesh {
                needed.insert(name.clone());
            }
        }
        needed.extend(
            self.world
                .query::<&crate::Sprite>()
                .iter()
                .map(|(_, s)| s.texture.clone()),
        );
        for name in needed.clone() {
            if matches!(
                assets.states.get(&name),
                Some(crate::asset::AssetState::Failed(_))
            ) {
                continue;
            }
            if let Some(deps) = assets.dependencies.get(&name) {
                needed.extend(deps.iter().cloned());
            }
        }
        let pending: Vec<_> = needed
            .iter()
            .filter(|name| {
                !matches!(
                    assets.states.get(name),
                    Some(crate::asset::AssetState::Loaded | crate::asset::AssetState::Failed(_))
                )
            })
            .collect();
        if self.is_loading() || !pending.is_empty() {
            return Err(DataError::new(format!(
                "save refused: assets are not ready: {:?}; {}",
                pending,
                assets.state_json()
            )));
        }
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
        let saved = Saved {
            game: G::ID.into(),
            world: self.world.save(),
            args: self.args_json.clone(),
            input: self.input.clone(),
            queue,
            world_us,
            published: self.world.publications(),
            journal: self.world.journal(),
            journal_next: self.world.journal_next(),
            overflow_logged: self.overflow_logged,
        };
        let mut bytes = b"EXSIM\0\x05".to_vec();
        bytes.extend(bin::to_vec(&saved));
        Ok(bytes)
    }
    /// Atomically restore dynamic state onto this binary's actions and a new epoch.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        self.restore_into(bytes, None)
    }
    /// A surface retains the current app bindings, including setup arguments.
    pub fn restore_bound(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        let args = crate::json::to_string(&self.args)?;
        self.restore_into(bytes, Some(&args))
    }
    fn restore_into(&mut self, bytes: &[u8], args: Option<&str>) -> Result<(), DataError> {
        let payload = bytes.strip_prefix(b"EXSIM\0\x05").ok_or_else(|| {
            DataError::new(format!(
                "unsupported simulation save format (expected EXSIM v5; saw {:02x?})",
                &bytes[..bytes.len().min(8)]
            ))
        })?;
        let s: Saved = bin::from_slice(payload)?;
        if s.game != G::ID {
            return Err(DataError::new(format!(
                "save belongs to `{}`, expected `{}`",
                s.game,
                G::ID
            )));
        }
        if s.world_us < 0 || s.queue.len() > QUEUE_LIMIT {
            return Err(DataError::new("invalid saved clock or input queue"));
        }
        let bound: G::Args = crate::json::from_str(args.unwrap_or(&s.args))?;
        let mut next = Self::new(bound).map_err(DataError::new)?;
        next.world = Self::build(&next.args, self.world.assets.clone());
        next.setup_pending = !next.world.assets.ready();
        next.defer_assets = self.defer_assets;
        if next.setup_pending {
            return Err(DataError::new("restore awaits declared assets"));
        }
        next.world.load(&s.world)?;
        let due = s.world_us as u128 * G::HZ as u128 / 1_000_000;
        if next.world.hz() != G::HZ || next.world.tick() as u128 != due {
            return Err(DataError::new("saved world and clock disagree"));
        }
        crate::scene::place_followers(&next.world);
        next.world.propagate();
        next.world.restore_journal(s.journal, s.journal_next);
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
        // A deferred restore may commit on the last texture's content delivery,
        // before the presenter has drained that payload into the current device.
        next.textures = std::mem::take(&mut self.textures);
        next.paranoid = self.paranoid;
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
    fn live_restore_preserves_a_pending_future_stamp_on_the_epoch_sample() {
        let mut s = Sim::<Ticker<60>>::new(()).unwrap();
        s.advance(10.0, Clock::Live);
        s.queue.push_back(Queued {
            host_us: 12_000,
            ..Default::default()
        });
        let save = s.save().unwrap();
        s.restore(&save).unwrap();
        s.advance(100.0, Clock::Live);
        assert_eq!(s.queue.len(), 1);
        assert_eq!(s.queue[0].host_us, 102_000);
        assert!(s.queue[0].world_us.is_none());
    }
    #[test]
    fn restore_refuses_a_one_tick_ahead_clock() {
        let mut s = Sim::<Ticker<60>>::new(()).unwrap();
        s.advance(0.0, Clock::Seekable);
        s.advance(17.0, Clock::Seekable);
        let good = s.save().unwrap();
        let mut saved: Saved = bin::from_slice(&good[7..]).unwrap();
        saved.world_us = 10_000;
        let mut bad = b"EXSIM\0\x05".to_vec();
        bad.extend(bin::to_vec(&saved));
        assert!(s
            .restore(&bad)
            .unwrap_err()
            .to_string()
            .contains("world and clock disagree"));
        assert_eq!(s.save().unwrap(), good);
    }
}

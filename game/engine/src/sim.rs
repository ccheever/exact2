mod assets;
mod capture_clock;
mod checkpoint;
use crate::data::limits::LoadBudget;
use crate::{bin, Actions, Data, DataError, Event, Input, InputEvent, Value, Vec2, World};
use crate::{Args, ArgumentKind, PointerPhase};
pub(crate) use capture_clock::CaptureClock;
use std::{collections::VecDeque, marker::PhantomData};

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
    /// Named models required before setup and tick zero. Later mesh references load on sight.
    const ASSETS: &'static [&'static str] = &[];
    /// Opt in only when every tick dependency is world state, bindings, input, or explicit time.
    /// Hidden network/storage/random results must remain unsupported by world-only replay.
    const CAPTURE_SUPPORTED: bool = false;
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
    delivered: bool,
}
#[derive(Default, Data)]
struct Saved {
    game: String,
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
    base: Vec<u8>,
    base_args: String,
    pub(crate) reload: crate::world::reload::Report,
    setup_pending: bool,
    asset_mesh_revision: u64,
    asset_sprite_names: Vec<(crate::Entity, String)>,
    defer_assets: bool,
    textures: std::collections::BTreeMap<String, crate::asset::TextureData>,
    pub(crate) args: G::Args,
    pub(crate) restarted: u64,
    pub(crate) restored_from: Option<String>,
    settle_delay: std::cell::Cell<u32>,
    last_epoch: std::cell::Cell<u64>,
    pub(crate) args_json: String,
    pub(crate) input: Input,
    pub(crate) inspection_viewport: Option<Vec2>,
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
    paranoid: Option<fn(&mut Self)>,
    game: PhantomData<G>,
}
const QUEUE_LIMIT: usize = 1024;
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
            ticks: std::collections::BTreeMap<String, String>,
        }
        let pins: Pins = crate::json::from_str(pins).expect("pins.json must contain ticks");
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

    fn build(args: &G::Args, assets: crate::asset::AssetStore) -> World {
        let mut world = World::new(G::HZ, 0);
        world.assets = assets;
        for &name in G::ASSETS {
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
        if !world.assets.ready() {
            return world;
        }
        G::setup(&mut world, args);
        crate::scene::place_followers(&world);
        world.published_pending.set(true);
        world.propagate();
        world
    }
    /// Build at tick zero with seed zero; setup may reseed from a named argument.
    pub fn from_values(values: &[Value]) -> Result<Self, String> {
        Self::new(Self::decode_args(values)?)
    }
    /// Construct a simulation from typed game arguments.
    pub fn new(args: G::Args) -> Result<Self, String> {
        Self::with_store(args, Default::default())
    }
    fn with_store(args: G::Args, assets: crate::asset::AssetStore) -> Result<Self, String> {
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
        let world = Self::build(&args, assets);
        let base = world.initializer().map_err(|e| e.to_string())?;
        Ok(Self {
            base,
            base_args: crate::json::to_string(&args).map_err(|e| e.to_string())?,
            reload: Default::default(),
            setup_pending: !world.assets.ready(),
            world,
            asset_mesh_revision: u64::MAX,
            asset_sprite_names: Vec::new(),
            defer_assets: false,
            textures: Default::default(),
            args_json: crate::json::to_string(&args).map_err(|e| e.to_string())?,
            args,
            restarted: 0,
            last_epoch: std::cell::Cell::new(0),
            restored_from: None,
            settle_delay: std::cell::Cell::new(100),
            input: Input::new(G::actions()),
            inspection_viewport: None,
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
            let world = Self::build(&args, self.world.assets.clone());
            let base = world.initializer().map_err(|e| e.to_string())?;
            Some((world, base))
        };
        if let Some(at) = at_ms {
            self.advance_with(at, Clock::Seekable, after);
        }
        if let Some((mut world, base)) = restart {
            self.base = base;
            self.base_args = crate::json::to_string(&args).map_err(|e| e.to_string())?;
            self.reload = Default::default();
            self.capture_fail("construction binding restarted the world; start a new capture");
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
        self.inspection_viewport = None;
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
    /// Validate host input without mutating the simulation or panicking.
    pub fn validate_input(&self, event: &InputEvent) -> Result<(), String> {
        self.input.validate(event)
    }
    /// Queue a raw event in stamp order, preserving arrival order at equal stamps.
    /// Paused input updates held state directly, without edges or queued wheel deltas.
    pub fn input(&mut self, event: InputEvent) {
        self.queue_input(event, true);
    }
    /// Schedule an explicit future stamp. Live frames do not clamp it to delivery time.
    pub fn scheduled_input(&mut self, event: InputEvent) {
        self.queue_input(event, false);
    }
    /// Queue an event already delivered by a device. Live pacing may clamp its
    /// stamp to the next frame; explicitly scheduled input keeps its future stamp.
    pub fn device_input(&mut self, event: InputEvent) {
        self.queue_input(event, true);
    }
    fn queue_input(&mut self, event: InputEvent, delivered: bool) {
        let before = self.world.tick();
        let saved = self
            .recorder
            .as_ref()
            .filter(|r| r.active())
            .map(|_| event.clone());
        let accepted = self.accept_input(event, delivered);
        if let Some(event) = saved.filter(|_| accepted) {
            self.record(
                before,
                if delivered {
                    crate::capture::Operation::DeviceInput(event)
                } else {
                    crate::capture::Operation::Input(event)
                },
            );
        }
    }
    fn accept_input(&mut self, event: InputEvent, delivered: bool) -> bool {
        if let Err(error) = self.validate_input(&event) {
            self.world.log(error);
            return false;
        }

        self.invalidate();
        if G::paused(&self.args) {
            self.input.apply_paused(event);
            return true;
        }
        let host_us = micros(event.at_ms());
        let e = Queued {
            delivered,
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
                (old, new) if old.same_motion(new) => {
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
        let mut position = position;
        if self.queue.len() == QUEUE_LIMIT {
            self.capture_fail("simulation input queue overflow; accepted event was dropped");
            let drop = self
                .queue
                .iter()
                .position(|e| e.event.is_move())
                .unwrap_or(0);
            let was_move = self.queue[drop].event.is_move();
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
        if let Err(error) = self.validate_input(&event) {
            self.world.log(error);
            return;
        }
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
    pub(crate) fn exact_world_us(&self) -> i64 {
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
        if clock == Clock::Live && self.last_us.is_some() {
            // The epoch sample preserves rebased future stamps. On later frames,
            // every queued device event arrived before this callback. Pacing can
            // name the frame earlier than event.timeStamp; it cannot defer delivery.
            // min preserves stamp order and the spread of earlier catch-up input.
            for e in &mut self.queue {
                if e.delivered {
                    e.host_us = e.host_us.min(now);
                }
            }
        }
        if G::paused(&self.args) {
            self.flush_paused(now);
            if now != last {
                self.record(
                    before_tick,
                    crate::capture::Operation::Frame {
                        at_ms: now_ms,
                        period_ms: self.period_ms,
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
            if let Some(rebuild) = self.paranoid {
                rebuild(self);
            }
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
                crate::capture::Operation::Frame {
                    at_ms: now_ms,
                    period_ms: self.period_ms,
                    live: clock == Clock::Live,
                },
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
        let bytes = self.save().unwrap_or_else(|error| panic!("paranoid {:?} {} tick {tick}: {error}; rerun bun game/games/{}/proof.mjs linux --paranoid", mode, G::ID, G::ID));
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
        if mode == Paranoid::FreshGame {
            let mut assets = std::mem::take(&mut self.world.assets);
            let models = assets
                .models
                .iter()
                .map(|(name, model)| (name.clone(), bin::to_vec(model.model.as_ref())))
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
                    bin::from_slice::<crate::asset::Model>(&bytes)
                        .expect("paranoid asset decode")
                        .into(),
                );
            }
            self.world.assets = assets;
        }
        let recorder = self.recorder.take();
        let reload = std::mem::take(&mut self.reload);
        self.restore_into(&bytes, None, None, false)
            .unwrap_or_else(|error| panic!("paranoid {:?} {} tick {tick}: {error}; rerun bun game/games/{}/proof.mjs linux --paranoid", mode, G::ID, G::ID));
        assert_eq!(
            hash,
            self.world.hash(),
            "paranoid {:?} tick {tick}: world hash; rerun bun game/games/{}/proof.mjs linux --paranoid",
            mode, G::ID
        );
        self.recorder = recorder;
        self.reload = reload;
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
    /// Read the entity's local Transform position.
    pub fn local_position(&self, entity: impl crate::Target) -> Option<crate::Vec3> {
        self.world.local_position(entity)
    }
    /// Current global position including parents.
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
}

#[cfg(test)]
#[path = "sim_tests.rs"]
mod render_time_tests;

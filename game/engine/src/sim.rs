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
}
/// How host elapsed time becomes simulation time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// Honor every microsecond, including a large agent seek.
    Seekable,
    /// Limit one display gap to 250 milliseconds, dropping its excess.
    /// A hitch collapses input inside it onto the first step after the gap.
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
    args: String,
    input: Input,
    queue: Vec<Queued>,
    world_us: i64,
    journal: Vec<Event>,
    journal_next: u64,
    overflow_logged: bool,
    published: std::collections::BTreeMap<String, Value>,
}
/// The clock, bounded device queue, and a game's world, without a host or GPU.
pub struct Sim<G: Game> {
    pub(crate) world: World,
    setup_pending: bool,
    asset_mesh_revision: u64,
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
    world_us: i64,
    observations: [crate::world::Observation; 2],
    game: PhantomData<G>,
}
const QUEUE_LIMIT: usize = 1024;
pub(crate) fn micros(ms: f64) -> i64 {
    (ms * 1000.0).round() as i64
}
impl<G: Game> Sim<G> {
    fn build(args: &G::Args, assets: crate::asset::Assets) -> World {
        let mut world = World::new(G::HZ, 0);
        world.assets = assets;
        world.register_scene();
        for &name in G::ASSETS {
            if !world.assets.models.contains_key(name) {
                world.assets.pending.insert(name.into());
            }
        }
        if G::ASSETS
            .iter()
            .any(|name| !world.assets.models.contains_key(*name))
        {
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
                if !self.world.assets.models.contains_key(&name)
                    && !self.world.assets.failed.contains_key(&name)
                {
                    self.world.assets.pending.insert(name);
                }
            }
            self.asset_mesh_revision = revision;
        }
        let assets = &mut self.world.assets;
        let names: Vec<_> = assets
            .pending
            .difference(&assets.requested)
            .cloned()
            .collect();
        assets.requested.extend(names.iter().cloned());
        names
    }
    /// Install baked model bytes without a device. Missing or malformed models refuse by name.
    pub fn asset(&mut self, name: &str, bytes: Option<&[u8]>) -> Result<(), String> {
        let result = bytes
            .ok_or_else(|| format!("asset `{name}`: missing file"))
            .and_then(|bytes| {
                bin::from_slice::<crate::asset::Model>(bytes)
                    .map_err(|e| format!("asset `{name}`: {e}"))
            })
            .and_then(|model| {
                model
                    .validate()
                    .map_err(|e| format!("asset `{name}`: {e}"))?;
                Ok(model)
            });
        self.world.assets.pending.remove(name);
        match result {
            Ok(model) => {
                self.world
                    .assets
                    .models
                    .insert(name.into(), std::sync::Arc::new(model));
            }
            Err(error) => {
                self.world.assets.failed.insert(name.into(), error.clone());
                return Err(error);
            }
        }
        self.world.assets.revision += 1;
        if self.setup_pending
            && G::ASSETS
                .iter()
                .all(|name| self.world.assets.models.contains_key(*name))
        {
            self.world = Self::build(&self.args, self.world.assets.clone());
            self.setup_pending = false;
            self.asset_mesh_revision = u64::MAX;
        }
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
        args.check_scalars()?;
        G::validate(&args)?;
        Ok(Self {
            world: Self::build(&args, Default::default()),
            setup_pending: !G::ASSETS.is_empty(),
            asset_mesh_revision: u64::MAX,
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
            Some(Self::build(&args, self.world.assets.clone()))
        };
        if let Some(at) = at_ms {
            self.advance_with(at, Clock::Seekable, after);
        }
        if let Some(mut world) = restart {
            world.presentation_generation = self
                .world
                .presentation_generation
                .checked_add(1)
                .expect("presentation generation exhausted");
            self.setup_pending = G::ASSETS
                .iter()
                .any(|name| !world.assets.models.contains_key(*name));
            self.asset_mesh_revision = u64::MAX;
            self.world = world;
            self.world_us = 0;
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
    /// Number of steps the next advance would complete. Presentation can skip
    /// timing samples that a long advance would immediately evict from its ring.
    pub fn ticks_due(&self, now_ms: f64, clock: Clock) -> u32 {
        if self.setup_pending || !now_ms.is_finite() || G::paused(&self.args) {
            return 0;
        }
        let now = micros(now_ms);
        let gap = now.saturating_sub(self.last_us.unwrap_or(now)).max(0);
        let elapsed = if clock == Clock::Live {
            gap.min(250_000)
        } else {
            gap
        };
        let us = self.world_us.saturating_add(elapsed);
        let target = us as u128 * G::HZ as u128 / 1_000_000;
        u32::try_from(target.saturating_sub(self.world.tick() as u128)).unwrap_or(u32::MAX)
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
            self.last_us = Some(micros(now_ms));
            return 0;
        }
        self.check_epoch();
        if clock == Clock::Live {
            self.world.unobserve();
        }
        let now = micros(now_ms);
        let last = self.last_us.unwrap_or(now);
        if now < last {
            return 0;
        }
        // Restored pending host stamps are offsets until the first host sample.
        if self.last_us.is_none() && self.rebase_queue {
            for e in &mut self.queue {
                if e.world_us.is_none() {
                    e.host_us = now.saturating_add(e.host_us);
                }
            }
        }
        self.rebase_queue = false;
        self.last_us = Some(now);
        if G::paused(&self.args) {
            self.flush_paused(now);
            return 0;
        }
        let gap = now.saturating_sub(last);
        let elapsed = if clock == Clock::Live {
            gap.min(250_000)
        } else {
            gap
        };
        for e in &mut self.queue {
            if e.world_us.is_none() && e.host_us <= now {
                let offset = if gap > elapsed {
                    0
                } else {
                    e.host_us.saturating_sub(last).clamp(0, elapsed)
                };
                e.world_us = Some(self.world_us.saturating_add(offset));
            }
        }
        self.world_us = self
            .world_us
            .checked_add(elapsed)
            .expect("simulation clock exhausted");
        let target = (self.world_us as u128 * G::HZ as u128 / 1_000_000) as u64;
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
                e.world_us
                    .is_some_and(|us| us as u128 * (G::HZ as u128) < end)
            }) {
                self.input.apply(self.queue.pop_front().unwrap().event);
            }
            self.restored = false;
            self.restored_from = None;
            G::tick(&mut self.world, &self.input, &self.args);
            self.world.reap_orphans();
            self.world.propagate();
            self.world.step_clock();
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
    /// Fraction of a tick remaining after the last completed boundary, in [0,1).
    pub fn alpha(&self) -> f32 {
        (self.world_us as u128 * G::HZ as u128 % 1_000_000) as f32 / 1_000_000.0
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
        self.advance(self.last_us.unwrap() as f64 / 1000.0 + ms, Clock::Seekable)
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
    pub fn save(&self) -> Vec<u8> {
        let mut queue: Vec<_> = self.queue.iter().cloned().collect();
        for e in &mut queue {
            if let Some(us) = &mut e.world_us {
                *us -= self.world_us;
                e.host_us = 0;
            } else {
                e.host_us = e.host_us.saturating_sub(self.last_us.unwrap_or(0));
            }
            e.event.set_at_ms(0.0); // the queue owns the stamp; no absolute host time in a save
        }
        let saved = Saved {
            game: G::ID.into(),
            version: G::SAVE_VERSION,
            world: self.world.save(),
            args: self.args_json.clone(),
            input: self.input.clone(),
            queue,
            world_us: self.world_us,
            published: self.world.publications(),
            journal: self.world.journal(),
            journal_next: self.world.journal_next(),
            overflow_logged: self.overflow_logged,
        };
        let mut bytes = b"EXSIM\0\x05".to_vec();
        bytes.extend(bin::to_vec(&saved));
        bytes
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
        let bound: G::Args = crate::json::from_str(args.unwrap_or(&s.args))?;
        let mut next = Self::new(bound).map_err(DataError::new)?;
        next.world = Self::build(&next.args, self.world.assets.clone());
        next.setup_pending = G::ASSETS
            .iter()
            .any(|name| !next.world.assets.models.contains_key(*name));
        if next.setup_pending {
            return Err(DataError::new("restore awaits declared assets"));
        }
        next.world.load(&s.world)?;
        if next.world.hz() != G::HZ
            || next.world.tick() as u128 != s.world_us as u128 * G::HZ as u128 / 1_000_000
        {
            return Err(DataError::new("saved world and clock disagree"));
        }
        if s.version < G::SAVE_VERSION {
            G::migrate(&mut next.world, s.version);
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
        *self = next;
        Ok(())
    }
}

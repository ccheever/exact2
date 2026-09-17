use crate::{bin, Actions, Data, DataError, Event, Input, InputEvent, Value, Vec2, World};
use crate::{Arg, Args, PointerPhase};
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
    /// Game save schema version; older versions pass through migrate.
    const SAVE_VERSION: u32 = 1;
    /// Canvas argument declarations in positional order; also declares exact arity.
    const ARGS: &'static [Arg] = &[];
    /// Discoverable controls.
    fn actions() -> Actions {
        Actions::default()
    }
    /// Construct the world. Refusals name the invalid argument.
    fn setup(world: &mut World, args: &Args) -> Result<(), String>;
    /// Validate every argument before setup or a timed bind can change anything.
    fn check(_args: &Args) -> Result<(), String> {
        Ok(())
    }
    /// Upgrade a loaded older world before it becomes observable.
    fn migrate(_world: &mut World, _from: u32) {}
    /// Stop world time while continuing to serve reads.
    fn paused(_args: &Args) -> bool {
        false
    }
    /// One fixed step, called after inputs and before transform propagation.
    fn tick(world: &mut World, input: &Input);
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
    args: Vec<Value>,
    input: Input,
    queue: Vec<Queued>,
    world_us: i64,
    journal: Vec<Event>,
    journal_next: u64,
    messages_pending: bool,
    published: std::collections::BTreeMap<String, Value>,
}
/// The clock, bounded device queue, and a game's world, without a host or GPU.
pub struct Sim<G: Game> {
    pub(crate) world: World,
    pub(crate) input: Input,
    queue: VecDeque<Queued>,
    overflow_logged: bool,
    rebase_queue: bool,
    pub(crate) last_us: Option<i64>,
    world_us: i64,
    game: PhantomData<G>,
    generation: u64,
}
const QUEUE_LIMIT: usize = 1024;
pub(crate) fn micros(ms: f64) -> i64 {
    (ms * 1000.0).round() as i64
}
impl<G: Game> Sim<G> {
    fn args(values: &[Value]) -> Result<Args, String> {
        let args = Args {
            values: values.to_vec(),
            declarations: G::ARGS,
        };
        args.arity(G::NAME, G::ARGS)?;
        G::check(&args)?;
        Ok(args)
    }
    fn build(args: Args) -> Result<World, String> {
        let mut world = World::new(G::HZ, 0);
        world.register_scene();
        G::setup(&mut world, &args)?;
        world.args = args;
        world.propagate();
        Ok(world)
    }
    /// Build at tick zero with seed zero; setup may reseed from a named argument.
    pub fn new(values: &[Value]) -> Result<Self, String> {
        if G::HZ == 0 {
            return Err("game HZ must be positive".into());
        }
        Ok(Self {
            world: Self::build(Self::args(values)?)?,
            input: Input::new(G::actions()),
            queue: VecDeque::with_capacity(QUEUE_LIMIT),
            overflow_logged: false,
            rebase_queue: false,
            last_us: None,
            world_us: 0,
            game: PhantomData,
            generation: 0,
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
        let args = Self::args(values)?;
        let changes: Vec<_> = G::ARGS
            .iter()
            .zip(&self.world.args.values)
            .zip(values)
            .filter(|((arg, old), new)| arg.setup && old != new)
            .map(|((arg, old), new)| {
                format!(
                    "{} {} → {}",
                    arg.name,
                    crate::values::value_json(old, false),
                    crate::values::value_json(new, false)
                )
            })
            .collect();
        // Even a setup refusal is atomic: build off to the side before seeking.
        let restart = if changes.is_empty() {
            None
        } else {
            Some(Self::build(args.clone())?)
        };
        if let Some(at) = at_ms {
            self.advance_with(at, Clock::Seekable, after);
        }
        if let Some(world) = restart {
            self.generation = self.generation.wrapping_add(1);
            self.world = world;
            self.world_us = 0;
            self.queue.clear();
            let viewport = self.input.viewport;
            self.input = Input::new(G::actions());
            self.input.viewport = viewport;
            self.overflow_logged = false;
            self.world
                .log(format_args!("world restarted: {}", changes.join(", ")));
        } else {
            self.world.args = args;
        }
        if G::paused(self.world.args()) {
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
        if G::paused(self.world.args()) {
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
        if G::paused(self.world.args()) {
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
            G::tick(&mut self.world, &self.input);
            self.world.reap_orphans();
            self.world.propagate();
            self.world.step_clock();
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
        self.generation
    }
    /// Read simulation state.
    pub fn world(&self) -> &World {
        &self.world
    }
    /// Edit simulation state, for setup tools and tests.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
    /// Drain at most one complete publication record whenever any key changed.
    pub fn take_messages(&mut self) -> Vec<String> {
        if self.world.messages_pending.replace(false) {
            vec![self.world.published_json(false)]
        } else {
            vec![]
        }
    }
    /// Paused worlds are settled; otherwise inspect springs and game-declared work.
    pub fn quiescent(&self) -> bool {
        G::paused(self.world.args()) || self.world.quiescent()
    }
    pub(crate) fn settle_at(&self) -> f64 {
        let now = self.last_us.unwrap_or(0);
        self.world
            .settle_tick()
            .filter(|tick| *tick > self.world.tick())
            .map_or(now as f64 / 1000.0 + 250.0, |tick| {
                let deadline = (tick as u128 * 1_000_000).div_ceil(G::HZ as u128) as i64;
                now.saturating_add(deadline.saturating_sub(self.world_us)) as f64 / 1000.0
            })
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
            args: self.world.args.values.clone(),
            input: self.input.clone(),
            queue,
            world_us: self.world_us,
            published: self.world.publications(),
            journal: self.world.journal(),
            journal_next: self.world.journal_next(),
            messages_pending: self.world.messages_pending.get(),
        };
        let mut bytes = b"EXSIM\0\x02".to_vec();
        bytes.extend(bin::to_vec(&saved));
        bytes
    }
    /// Atomically restore dynamic state onto this binary's actions and a new epoch.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        let payload = bytes
            .strip_prefix(b"EXSIM\0\x02")
            .ok_or_else(|| DataError::new("invalid simulation save magic or version"))?;
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
        let mut next = Self::new(&s.args).map_err(DataError::new)?;
        let args = next.world.args.clone();
        next.world.load(&s.world)?;
        next.world.args = args;
        if next.world.hz() != G::HZ
            || next.world.tick() as u128 != s.world_us as u128 * G::HZ as u128 / 1_000_000
        {
            return Err(DataError::new("saved world and clock disagree"));
        }
        if s.version < G::SAVE_VERSION {
            G::migrate(&mut next.world, s.version);
        }
        next.world.propagate();
        next.world.restore_journal(s.journal, s.journal_next);
        next.world.restore_publications(s.published);
        next.world.messages_pending.set(s.messages_pending);
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
        next.rebase_queue = true;
        next.generation = self.generation.wrapping_add(1);
        *self = next;
        Ok(())
    }
}

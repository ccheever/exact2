use crate::{bin, Actions, Data, DataError, Event, Input, InputEvent, Value, Vec2, World};
use std::marker::PhantomData;

/// One immutable simulation instant; Copy keeps world borrows short in game code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub struct Now {
    /// Completed fixed steps.
    pub tick: u64,
    /// Fixed steps per second.
    pub hz: u32,
}
/// Positional canvas arguments with refusals that name the author-facing argument.
#[derive(Clone, Default, Data)]
pub struct Args(Vec<Value>);
impl Args {
    /// Number of supplied arguments.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Whether no arguments were supplied.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn expected(&self, i: usize, name: &str, kind: &str) -> String {
        format!(
            "argument `{name}` at position {i}: expected {kind}{}",
            if i >= self.len() {
                ", got no value"
            } else {
                ""
            }
        )
    }
    /// Read text, refusing missing values or a different type.
    pub fn text(&self, i: usize, name: &str) -> Result<&str, String> {
        self.0
            .get(i)
            .and_then(Value::as_str)
            .ok_or_else(|| self.expected(i, name, "text"))
    }
    /// Read a finite number, refusing missing values or a different type.
    pub fn number(&self, i: usize, name: &str) -> Result<f64, String> {
        self.0
            .get(i)
            .and_then(Value::as_number)
            .filter(|n| n.is_finite())
            .ok_or_else(|| self.expected(i, name, "a finite number"))
    }
    /// Read a non-negative safe integer (at most 2^53 - 1), refusing by name.
    pub fn integer(&self, i: usize, name: &str) -> Result<u64, String> {
        let n = self.number(i, name)?;
        if !(0.0..=9_007_199_254_740_991.0).contains(&n) || n.fract() != 0.0 {
            return Err(self.expected(i, name, "a non-negative safe integer"));
        }
        Ok(n as u64)
    }
    fn arity(&self, game: &str, names: &[&str]) -> Result<(), String> {
        if self.len() < names.len() {
            Err(format!(
                "missing argument `{}` at position {}",
                names[self.len()],
                self.len()
            ))
        } else if self.len() > names.len() {
            Err(format!(
                "{} expects {} arguments ({}), got {}",
                game,
                names.len(),
                names.join(", "),
                self.len()
            ))
        } else {
            Ok(())
        }
    }
    /// Read a boolean, refusing missing values or a different type.
    pub fn flag(&self, i: usize, name: &str) -> Result<bool, String> {
        self.0
            .get(i)
            .and_then(Value::as_bool)
            .ok_or_else(|| self.expected(i, name, "a boolean"))
    }
    pub(crate) fn json(&self, names: &[&str]) -> String {
        format!(
            "{{{}}}",
            names
                .iter()
                .zip(&self.0)
                .map(|(n, v)| format!(
                    "{}:{}",
                    crate::values::quote(n),
                    crate::values::value_json(v, true)
                ))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}
/// A stateless game; components and resources hold every bit of simulation state.
pub trait Game: 'static {
    /// Surface name in Contract.
    const NAME: &'static str = "world";
    /// Canvas argument names in positional order; also declares exact arity.
    const ARGS: &'static [&'static str] = &[];
    /// Discoverable controls.
    fn actions() -> Actions {
        Actions::default()
    }
    /// Construct the world. Refusals name the invalid argument.
    fn setup(world: &mut World, args: &Args) -> Result<(), String>;
    /// Respond to changed canvas arguments.
    fn bind(_world: &mut World, args: &Args) -> Result<(), String> {
        args.arity(Self::NAME, Self::ARGS)
    }
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
    args: Args,
    input: Input,
    queue: Vec<Queued>,
    last_us: Option<i64>,
    world_us: i64,
    journal: Vec<Event>,
    journal_next: u64,
    messages_pending: bool,
    published: std::collections::BTreeMap<String, Value>,
}
/// The clock, queued device events, and a game's world, without a host or GPU.
pub struct Sim<G: Game> {
    pub(crate) world: World,
    pub(crate) args: Args,
    pub(crate) input: Input,
    queue: Vec<Queued>,
    pub(crate) last_us: Option<i64>,
    world_us: i64,
    game: PhantomData<G>,
}
pub(crate) fn micros(ms: f64) -> i64 {
    (ms * 1000.0).round() as i64
}
impl<G: Game> Sim<G> {
    fn args(values: &[Value]) -> Result<Args, String> {
        let args = Args(values.to_vec());
        args.arity(G::NAME, G::ARGS)?;
        Ok(args)
    }
    /// Build at tick zero with seed zero; setup may reseed from a named argument.
    pub fn new(args: &[Value]) -> Result<Self, String> {
        if G::HZ == 0 {
            return Err("game HZ must be positive".into());
        }
        let args = Self::args(args)?;
        let mut world = World::new(G::HZ, 0);
        world.register_scene();
        G::setup(&mut world, &args)?;
        world.propagate();
        Ok(Self {
            world,
            args,
            input: Input::new(G::actions()),
            queue: vec![],
            last_us: None,
            world_us: 0,
            game: PhantomData,
        })
    }
    /// Apply new canvas arguments. A refused bind leaves the old arguments active.
    pub fn bind(&mut self, args: &[Value]) -> Result<(), String> {
        let args = Self::args(args)?;
        G::bind(&mut self.world, &args)?;
        self.args = args;
        self.world.propagate();
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
    /// Queue a raw device event; equal stamps preserve arrival order.
    pub fn input(&mut self, event: InputEvent) {
        assert!(event.at_ms().is_finite(), "input stamp must be finite");
        let host_us = micros(event.at_ms());
        self.queue.push(Queued {
            host_us,
            world_us: None,
            event,
        });
    }
    /// Advance integer world time; the first call establishes the host epoch only.
    pub fn advance(&mut self, now_ms: f64, clock: Clock) -> u32 {
        assert!(now_ms.is_finite(), "host clock must be finite");
        self.advance_us(micros(now_ms), clock)
    }
    fn advance_us(&mut self, now: i64, clock: Clock) -> u32 {
        let last = self.last_us.unwrap_or(now);
        if now < last {
            return 0;
        }
        self.last_us = Some(now);
        let gap = now.saturating_sub(last);
        let elapsed = if G::paused(&self.args) {
            0
        } else if clock == Clock::Live {
            gap.min(250_000)
        } else {
            gap
        };
        // Map events through this exact host interval, including pauses and dropped
        // live time. Already mapped events keep their boundary across short seeks.
        for e in &mut self.queue {
            if e.world_us.is_none() && e.host_us <= now {
                e.world_us = Some(
                    self.world_us
                        .saturating_add(e.host_us.saturating_sub(last).clamp(0, elapsed)),
                );
            }
        }
        self.queue
            .sort_by_key(|e| (e.world_us.unwrap_or(i64::MAX), e.host_us));
        self.world_us = self
            .world_us
            .checked_add(elapsed)
            .expect("simulation clock exhausted");
        let target = (self.world_us as u128 * G::HZ as u128 / 1_000_000) as u64;
        let start = self.world.tick();
        while self.world.tick() < target {
            self.world.begin_tick();
            self.input.clear_edges();
            let boundary = self.world.tick() as u128 * 1_000_000;
            while self.queue.first().is_some_and(|e| {
                e.world_us
                    .is_some_and(|us| us as u128 * G::HZ as u128 <= boundary)
            }) {
                self.input.apply(self.queue.remove(0).event);
            }
            G::tick(&mut self.world, &self.input);
            self.world.reap_orphans();
            self.world.propagate();
            self.world.step_clock();
        }
        u32::try_from(self.world.tick() - start).unwrap_or(u32::MAX)
    }
    /// Fraction of a tick remaining after the last completed boundary, in [0,1).
    pub fn alpha(&self) -> f32 {
        let rem = self.world_us as u128 * G::HZ as u128 % 1_000_000;
        rem as f32 / 1_000_000.0
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
    /// Whether no springs or game-declared work are in flight.
    pub fn quiescent(&self) -> bool {
        self.world.quiescent()
    }
    pub(crate) fn settle(&mut self) {
        self.last_us.get_or_insert(0);
        for _ in 0..600 {
            if self.quiescent() || G::paused(&self.args) {
                break;
            }
            let next = ((self.world.tick() as u128 + 1) * 1_000_000).div_ceil(G::HZ as u128) as i64;
            self.advance_us(
                self.last_us
                    .unwrap_or(0)
                    .saturating_add(next - self.world_us),
                Clock::Seekable,
            );
        }
    }
    /// Save world state plus held input, queued events, arguments and clock remainder.
    pub fn save(&self) -> Vec<u8> {
        let saved = Saved {
            game: G::NAME.into(),
            world: self.world.save(),
            args: self.args.clone(),
            input: self.input.clone(),
            queue: self.queue.clone(),
            last_us: self.last_us,
            world_us: self.world_us,
            published: self.world.publications(),
            journal: self.world.journal(),
            journal_next: self.world.journal_next(),
            messages_pending: self.world.messages_pending.get(),
        };
        let mut bytes = b"EXSIM\0\x01".to_vec();
        bytes.extend(bin::to_vec(&saved));
        bytes
    }
    /// Atomically restore a simulation, refusing incompatible games or clocks.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        let payload = bytes
            .strip_prefix(b"EXSIM\0\x01")
            .ok_or_else(|| DataError::new("invalid simulation save magic or version"))?;
        let s: Saved = bin::from_slice(payload)?;
        if s.game != G::NAME || s.world_us < 0 {
            return Err(DataError::new("incompatible simulation save"));
        }
        let mut next = Self::new(&s.args.0).map_err(DataError::new)?;
        next.world.load(&s.world)?;
        if next.world.hz() != G::HZ
            || next.world.tick() as u128 != s.world_us as u128 * G::HZ as u128 / 1_000_000
        {
            return Err(DataError::new("saved world and clock disagree"));
        }
        next.world.propagate();
        next.world.restore_journal(s.journal, s.journal_next);
        next.world.restore_publications(s.published);
        next.world.messages_pending.set(s.messages_pending);
        next.input = s.input;
        next.queue = s.queue;
        next.last_us = s.last_us;
        next.world_us = s.world_us;
        *self = next;
        Ok(())
    }
}

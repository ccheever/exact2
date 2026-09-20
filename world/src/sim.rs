use crate::{
    args::SetupArgs, bin, Action, Args, Data, DataError, Input, InputEvent, Reader, World, Writer,
};
use std::{collections::VecDeque, marker::PhantomData};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub struct Now {
    pub tick: u64,
    pub hz: u32,
}
/// A stateless game. All continuation state belongs in saved kernel Data.
pub trait Game: 'static {
    const ID: &'static str;
    const HZ: u32 = 60;
    const ACTIONS: &'static [Action] = &[];
    type Args: Args;
    fn validate(_args: &Self::Args) -> Result<(), String> {
        Ok(())
    }
    fn register(_world: &mut World, _args: SetupArgs<'_, Self::Args>) -> Result<(), DataError> {
        Ok(())
    }
    fn setup(world: &mut World, args: &Self::Args);
    fn tick(world: &mut World, input: &Input, args: &Self::Args);
    fn paused(_args: &Self::Args) -> bool {
        false
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Paranoid {
    #[default]
    Off,
    Save,
    FreshGame,
}
/// Passive fixed-step driver. Hosts map their clocks to this simulation clock.
pub struct Sim<G: Game> {
    world: World,
    args: G::Args,
    input: Input,
    queue: VecDeque<InputEvent>,
    world_us: i64,
    caller_us: i64,
    paranoid: Paranoid,
    tick_failed: bool,
    game: PhantomData<G>,
}
const MAGIC: &[u8] = b"EXSIM\0\x09";
const MAX_TICKS: u64 = 216_000;
fn micros(ms: f64) -> Result<i64, DataError> {
    if !ms.is_finite() || ms < 0. || ms > (i64::MAX / 1000) as f64 {
        return Err(DataError::new("invalid clock"));
    }
    Ok((ms * 1000.).round() as i64)
}
impl<G: Game> Sim<G> {
    fn check(args: &G::Args) -> Result<(), DataError> {
        if G::HZ == 0 || G::HZ > 1000 || G::ID.len() > 256 {
            return Err(DataError::new("invalid game identity or tick rate"));
        }
        args.check_scalars().map_err(DataError::new)?;
        G::validate(args).map_err(DataError::new)
    }
    pub fn new(args: G::Args) -> Result<Self, DataError> {
        Self::check(&args)?;
        let input = Input::new(G::ACTIONS)?;
        let mut world = World::new(G::HZ, 0);
        G::register(&mut world, SetupArgs(&args))?;
        G::setup(&mut world, &args);
        world.validate()?;
        Ok(Self {
            world,
            args,
            input,
            queue: VecDeque::new(),
            world_us: 0,
            caller_us: 0,
            paranoid: Paranoid::Off,
            tick_failed: false,
            game: PhantomData,
        })
    }
    pub fn world(&self) -> &World {
        &self.world
    }
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
    pub fn args(&self) -> &G::Args {
        &self.args
    }
    pub fn input_state(&self) -> &Input {
        &self.input
    }
    pub fn paranoid(mut self, mode: Paranoid) -> Self {
        self.paranoid = mode;
        self
    }
    /// Validate before swapping. Setup/restart edges construct a new world at tick zero.
    pub fn bind(&mut self, args: G::Args) -> Result<(), DataError> {
        Self::check(&args)?;
        if self.args.setup_changed(&args) {
            let mut next = Self::new(args)?;
            next.paranoid = self.paranoid;
            self.world.adopt(next.world)?;
            self.args = next.args;
            self.input = next.input;
            self.queue = next.queue;
            self.world_us = 0;
            self.caller_us = 0;
            self.tick_failed = false;
        } else {
            self.args = args;
        }
        Ok(())
    }
    /// Admit at most 1024 events, stable in timestamp order. No reserved queue at startup.
    pub fn input(&mut self, event: InputEvent) -> Result<(), DataError> {
        self.input.validate_event(&event)?;
        if self.queue.len() == 1024 {
            return Err(DataError::new("input queue limit (1024)"));
        }
        let index = self.queue.partition_point(|e| e.at_ms() <= event.at_ms());
        self.queue.insert(index, event);
        Ok(())
    }
    pub fn run(&mut self, elapsed_ms: f64) -> Result<u64, DataError> {
        let target = self
            .caller_us
            .checked_add(micros(elapsed_ms)?)
            .ok_or_else(|| DataError::new("clock overflow"))?;
        self.advance_us(target)
    }
    pub fn advance_to(&mut self, clock_ms: f64) -> Result<u64, DataError> {
        self.advance_us(micros(clock_ms)?)
    }
    fn check_clock(&self) -> Result<(), DataError> {
        self.world.healthy()?;
        if self.tick_failed {
            return Err(DataError::new("simulation poisoned by an incomplete tick"));
        }
        if self.world.hz() != G::HZ
            || self.world.tick() as u128 != self.world_us as u128 * G::HZ as u128 / 1_000_000
        {
            return Err(DataError::new("clock disagrees with world"));
        }
        Ok(())
    }
    fn advance_us(&mut self, target_us: i64) -> Result<u64, DataError> {
        self.check_clock()?;
        if target_us < self.caller_us {
            return Err(DataError::new("clock cannot retreat"));
        }
        if G::paused(&self.args) {
            let due = self
                .queue
                .partition_point(|e| micros(e.at_ms()).unwrap() <= target_us);
            let input = self.staged_input(due)?;
            self.input = input;
            self.input.clear_edges();
            self.queue.drain(..due);
            self.caller_us = target_us;
            return Ok(0);
        }
        let offset = self.caller_us - self.world_us;
        let simulation_us = target_us - offset;
        let target = (simulation_us as u128 * G::HZ as u128 / 1_000_000) as u64;
        let count = target
            .checked_sub(self.world.tick())
            .ok_or_else(|| DataError::new("clock disagrees with world"))?;
        if count > MAX_TICKS {
            return Err(DataError::new("clock request exceeds 216000 ticks"));
        }
        for _ in 0..count {
            let end = (self.world.tick() as u128 + 1) * 1_000_000;
            let due = self.queue.partition_point(|e| {
                ((micros(e.at_ms()).unwrap() as i128 - offset as i128) * G::HZ as i128)
                    < end as i128
            });
            // Preflight the bounded batch before any boundary state changes.
            let input = self.staged_input(due)?;
            self.world.begin_tick();
            self.input = input;
            self.queue.drain(..due);
            self.tick_failed = true;
            G::tick(&mut self.world, &self.input, &self.args);
            self.world.reap_orphans()?;
            self.world.step_clock();
            self.tick_failed = false;
            self.world_us = (self.world.tick() as u128 * 1_000_000).div_ceil(G::HZ as u128) as i64;
            self.caller_us = self.world_us + offset;
            if self.paranoid != Paranoid::Off {
                let pending = self.world.published_pending.get();
                let hash = self.world.hash();
                let bytes = self.save()?;
                if self.paranoid == Paranoid::FreshGame {
                    let next = Self::from_save(&bytes)?;
                    self.install(next, false)?;
                } else {
                    self.restore(&bytes)?;
                }
                self.world.published_pending.set(pending);
                assert_eq!(
                    hash,
                    self.world.hash(),
                    "paranoid {:?} tick {}",
                    self.paranoid,
                    self.world.tick()
                );
            }
        }
        self.world_us = simulation_us;
        self.caller_us = target_us;
        Ok(count)
    }
    fn staged_input(&self, due: usize) -> Result<Input, DataError> {
        let mut input = self.input.clone();
        input.clear_edges();
        for event in self.queue.iter().take(due) {
            input.apply(event.clone())?;
        }
        Ok(input)
    }
    /// Replace held input and rebase caller time without a tick. Clears all queued
    /// events and edges; at most 1024 events describing the complete held state.
    pub fn reconcile_input(&mut self, clock_ms: f64, held: &[InputEvent]) -> Result<(), DataError> {
        let caller_us = micros(clock_ms)?;
        if held.len() > 1024 {
            return Err(DataError::new("input reconciliation limit (1024)"));
        }
        let mut input = Input::new(G::ACTIONS)?;
        for event in held {
            input.apply(event.clone())?;
        }
        input.clear_edges();
        self.input = input;
        self.queue.clear();
        self.caller_us = caller_us;
        Ok(())
    }
    /// Adapter inputs only; no display pacing, look-ahead or frame policy.
    pub fn alpha_inputs(&self) -> (u64, u32, u32) {
        let phase = self.world_us as u128 * G::HZ as u128;
        (self.world.tick(), (phase % 1_000_000) as u32, 1_000_000)
    }
    /// External pending/failed readiness refuses immediately; virtual time cannot finish I/O.
    pub fn settle(&mut self, max_ticks: u32) -> Result<u64, DataError> {
        if max_ticks > 3600 {
            return Err(DataError::new("settle limit is 3600 ticks"));
        }
        let start = self.world.tick();
        let mut before = None;
        for _ in 0..max_ticks {
            match self.world.readiness() {
                crate::Readiness::Ready => {}
                other => return Err(DataError::new(format!("external readiness: {other:?}"))),
            }
            if self.world.quiescent() && self.queue.is_empty() {
                return Ok(self.world.tick() - start);
            }
            if G::paused(&self.args) {
                return Err(DataError::new("paused simulation cannot settle"));
            }
            let next = ((self.world.tick() as u128 + 1) * 1_000_000).div_ceil(G::HZ as u128) as i64;
            let sample = before.unwrap_or_else(|| self.world.observation_hash());
            self.advance_us(next + self.caller_us - self.world_us)?;
            before = Some(self.world.observe(sample));
        }
        if self.world.quiescent() && self.queue.is_empty() {
            Ok(self.world.tick() - start)
        } else {
            Err(DataError::new("settle tick budget exhausted"))
        }
    }
    /// EXSIM v9: identity → typed args → world → driver/delivery data.
    pub fn save(&self) -> Result<Vec<u8>, DataError> {
        self.check_clock()?;
        self.world.validate()?;
        let mut w = bin::Encoder::prefixed(MAGIC);
        w.begin_seq(9);
        w.item();
        w.string(G::ID);
        w.item();
        self.args.write(&mut w);
        w.item();
        self.world.write(&mut w, true);
        w.item();
        self.world_us.write(&mut w);
        w.item();
        self.input.write(&mut w);
        w.item();
        w.begin_seq(self.queue.len());
        for e in &self.queue {
            w.item();
            e.write(&mut w);
        }
        w.end_seq();
        w.item();
        self.world.write_publications(&mut w);
        w.item();
        w.begin_seq(2);
        // Two streamed fields share one sequence item; journal owns their framing.
        self.world.write_journal(&mut w);
        w.end_seq();
        w.item();
        self.caller_us.write(&mut w);
        w.end_seq();
        w.finish()
    }
    pub fn from_save(bytes: &[u8]) -> Result<Self, DataError> {
        Self::candidate(bytes, false)
    }
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        let next = Self::candidate(bytes, false)?;
        self.install(next, false)
    }
    /// Compatible-state carry preserves current live arguments; structural/setup changes refuse.
    pub fn carry(&mut self, bytes: &[u8]) -> Result<bool, DataError> {
        let next = Self::candidate(bytes, true)?;
        if next.args.setup_changed(&self.args) {
            return Err(DataError::new("carry setup arguments differ"));
        }
        let changed = next.save()? != bytes;
        self.install(next, true)?;
        Ok(changed)
    }
    fn install(&mut self, next: Self, keep_args: bool) -> Result<(), DataError> {
        self.world.adopt(next.world)?;
        if !keep_args {
            self.args = next.args;
        }
        self.input = next.input;
        self.queue = next.queue;
        self.world_us = next.world_us;
        self.caller_us = next.caller_us;
        self.tick_failed = false;
        Ok(())
    }
    fn candidate(bytes: &[u8], adapt: bool) -> Result<Self, DataError> {
        if bytes.len() > 128 * 1024 * 1024 {
            return Err(DataError::new("save exceeds 128 MiB"));
        }
        let payload = bytes
            .strip_prefix(MAGIC)
            .ok_or_else(|| DataError::new("unsupported Sim save version; expected EXSIM v9"))?;
        let mut r = bin::Decoder::with_budget(payload, 256 * 1024 * 1024);
        r.begin_seq()?;
        fn item(r: &mut dyn Reader) -> Result<(), DataError> {
            if r.item()? {
                Ok(())
            } else {
                Err(DataError::new("incomplete Sim save"))
            }
        }
        item(&mut r)?;
        if r.string()? != G::ID {
            return Err(DataError::new("game identity differs"));
        }
        item(&mut r)?;
        let args = G::Args::read_new(&mut r)?;
        Self::check(&args)?;
        let mut world = World::new(G::HZ, 0);
        G::register(&mut world, SetupArgs(&args))?;
        item(&mut r)?;
        world.read(&mut r)?;
        item(&mut r)?;
        let mut world_us = 0i64;
        world_us.read(&mut r)?;
        if world_us < 0
            || world.hz() != G::HZ
            || world.tick() as u128 != world_us as u128 * G::HZ as u128 / 1_000_000
        {
            return Err(DataError::new("saved clock disagrees with world"));
        }
        item(&mut r)?;
        let mut input = Input::default();
        input.read(&mut r)?;
        input.validate_saved(G::ACTIONS)?;
        item(&mut r)?;
        let mut queue = Vec::new();
        crate::data::limits::read_vec(&mut r, &mut queue, 1024)?;
        let mut last = 0.;
        for e in &queue {
            input.validate_event(e)?;
            if e.at_ms() < last {
                return Err(DataError::new("input queue is not ordered"));
            }
            last = e.at_ms();
        }
        item(&mut r)?;
        world.read_publications(&mut r)?;
        item(&mut r)?;
        r.begin_seq()?;
        world.read_journal(&mut r)?;
        if r.item()? {
            return Err(DataError::new("extra journal data"));
        }
        item(&mut r)?;
        let mut caller_us = 0i64;
        caller_us.read(&mut r)?;
        if caller_us < 0 {
            return Err(DataError::new("invalid caller clock"));
        }
        if r.item()? {
            return Err(DataError::new("extra Sim save data"));
        }
        r.finish()?;
        let next = Self {
            world,
            args,
            input,
            queue: queue.into(),
            world_us,
            caller_us,
            paranoid: Paranoid::Off,
            tick_failed: false,
            game: PhantomData,
        };
        let canonical = next.save()?;
        if !adapt && canonical != bytes {
            return Err(DataError::new(
                "exact save identity differs; use carry for schema adaptation",
            ));
        }
        Ok(next)
    }
}

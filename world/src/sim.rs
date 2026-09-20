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
    fn setup(world: &mut World, args: &Self::Args) -> Result<(), DataError>;
    fn tick(world: &mut World, input: &Input, args: &Self::Args) -> Result<(), DataError>;
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
    tick_error: Option<DataError>,
    game: PhantomData<G>,
}
const MAGIC: &[u8] = b"EXSIM\0\x0a";
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
        G::setup(&mut world, &args).map_err(|e| e.at("setup"))?;
        world.validate()?;
        world.driver_owned = true;
        Ok(Self {
            world,
            args,
            input,
            queue: VecDeque::new(),
            world_us: 0,
            caller_us: 0,
            paranoid: Paranoid::Off,
            tick_failed: false,
            tick_error: None,
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
    pub fn bind(&mut self, args: G::Args) -> Result<(), DataError> {
        Self::check(&args)?;
        if self.args.setup_changed(&args) {
            let mut next = Self::new(args)?;
            next.paranoid = self.paranoid;
            self.install(next, false)?;
        } else {
            let old = std::mem::replace(&mut self.args, args);
            self.world.mutated();
            drop(old);
        }
        Ok(())
    }
    pub fn input(&mut self, event: InputEvent) -> Result<(), DataError> {
        self.check_clock()?;
        self.input.validate_event(&event)?;
        if self.queue.len() == 1024 {
            return Err(DataError::new("input queue limit (1024)"));
        }
        let index = self.queue.partition_point(|e| e.at_ms() <= event.at_ms());
        self.queue.insert(index, event);
        self.world.mutated();
        Ok(())
    }
    pub fn run(&mut self, elapsed_ms: f64) -> Result<u64, DataError> {
        self.check_clock()?;
        let target = self
            .caller_us
            .checked_add(micros(elapsed_ms)?)
            .ok_or_else(|| DataError::new("clock overflow"))?;
        self.advance_us(target)
    }
    pub fn advance_to(&mut self, clock_ms: f64) -> Result<u64, DataError> {
        self.check_clock()?;
        self.advance_us(micros(clock_ms)?)
    }
    fn check_clock(&self) -> Result<(), DataError> {
        if let Some(error) = &self.tick_error {
            return Err(error.clone());
        }
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
                .partition_point(|e| micros(e.at_ms()).unwrap() < target_us);
            self.apply_input(due)?;
            self.input.clear_edges();
            self.caller_us = target_us;
            self.world.mutated();
            return Ok(0);
        }
        let offset = self.caller_us as i128 - self.world_us as i128;
        let simulation_us = i64::try_from(target_us as i128 - offset)
            .map_err(|_| DataError::new("simulation clock overflow"))?;
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
                ((micros(e.at_ms()).unwrap() as i128 - offset) * G::HZ as i128) < end as i128
            });
            let next_world = i64::try_from(end.div_ceil(G::HZ as u128))
                .map_err(|_| DataError::new("simulation clock overflow"))?;
            let next_caller = i64::try_from(next_world as i128 + offset)
                .map_err(|_| DataError::new("caller clock overflow"))?;
            // Preflight the bounded batch before any boundary state changes.
            self.apply_input(due)?;
            self.world.begin_tick();
            self.tick_failed = true;
            if let Err(error) = self.world.mutation(|w| G::tick(w, &self.input, &self.args)) {
                let error = error.at(format_args!("tick {}", self.world.tick() + 1));
                self.tick_error = Some(error.clone());
                // Telemetry refusal must not replace the original tick failure.
                let _ = self.world.session_log(&error.to_string());
                return Err(error);
            }
            self.world.reap_orphans()?;
            self.world.step_clock();
            self.tick_failed = false;
            self.world_us = next_world;
            self.caller_us = next_caller;
            if self.paranoid != Paranoid::Off {
                let hash = self.world.hash()?;
                let bytes = self.save()?;
                if self.paranoid == Paranoid::FreshGame {
                    let next = Self::from_save(&bytes)?;
                    self.install(next, false)?;
                } else {
                    self.restore(&bytes)?;
                }
                assert_eq!(
                    hash,
                    self.world.hash()?,
                    "paranoid {:?} tick {}",
                    self.paranoid,
                    self.world.tick()
                );
            }
        }
        self.world_us = simulation_us;
        self.caller_us = target_us;
        self.world.mutated();
        Ok(count)
    }
    fn apply_input(&mut self, due: usize) -> Result<(), DataError> {
        self.input.preflight(self.queue.iter().take(due))?;
        self.input.clear_edges();
        for event in self.queue.drain(..due) {
            self.input.apply(event);
        }
        Ok(())
    }
    /// Replace held input and rebase caller time without a tick. Clears all queued
    /// events and edges; at most 1024 events describing the complete held state.
    pub fn reconcile_input(&mut self, clock_ms: f64, held: &[InputEvent]) -> Result<(), DataError> {
        self.check_clock()?;
        let caller_us = micros(clock_ms)?;
        if held.len() > 1024 {
            return Err(DataError::new("input reconciliation limit (1024)"));
        }
        let mut input = Input::new(G::ACTIONS)?;
        input.preflight(held.iter())?;
        for event in held {
            input.apply(event.clone());
        }
        input.clear_edges();
        self.input = input;
        self.queue.clear();
        self.caller_us = caller_us;
        self.world.mutated();
        Ok(())
    }
    pub fn alpha_inputs(&self) -> (u64, u32, u32) {
        let phase = self.world_us as u128 * G::HZ as u128;
        (self.world.tick(), (phase % 1_000_000) as u32, 1_000_000)
    }
    pub fn settle(&mut self, max_ticks: u32) -> Result<u64, DataError> {
        self.check_clock()?;
        if max_ticks > 3600 {
            return Err(DataError::new("settle limit is 3600 ticks"));
        }
        let offset = self.caller_us as i128 - self.world_us as i128;
        let last =
            ((self.world.tick() as u128 + max_ticks as u128) * 1_000_000).div_ceil(G::HZ as u128);
        i64::try_from(last)
            .and_then(|last| i64::try_from(last as i128 + offset))
            .map_err(|_| DataError::new("settle clock overflow"))?;
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
            let sample = match before {
                Some(hash) => hash,
                None => self.world.sample()?.hash,
            };
            self.advance_us(
                i64::try_from(next as i128 + offset)
                    .map_err(|_| DataError::new("settle clock overflow"))?,
            )?;
            before = Some(self.world.observe(sample)?);
        }
        if self.world.quiescent() && self.queue.is_empty() {
            Ok(self.world.tick() - start)
        } else {
            Err(DataError::new("settle tick budget exhausted"))
        }
    }
    /// EXSIM v10: identity → typed args → world → driver/delivery data.
    pub fn save(&self) -> Result<Vec<u8>, DataError> {
        self.check_clock()?;
        self.world.validate()?;
        let mut w = bin::Encoder::prefixed(MAGIC);
        w.begin_seq(10);
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
            if w.stopped() {
                break;
            }
            w.item();
            e.write(&mut w);
        }
        w.end_seq();
        w.item();
        self.world.published.borrow().write(&mut w);
        w.item();
        w.begin_seq(2);
        // Two streamed fields share one sequence item; journal owns their framing.
        self.world.write_journal(&mut w);
        w.end_seq();
        w.item();
        self.caller_us.write(&mut w);
        w.item();
        self.world.published_pending.get().write(&mut w);
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
    pub fn carry(&mut self, bytes: &[u8]) -> Result<bool, DataError> {
        let next = Self::candidate(bytes, true)?;
        if next.args.setup_changed(&self.args) {
            return Err(DataError::new("carry setup arguments differ"));
        }
        let changed = next.save()? != bytes || bin::to_vec(&self.args)? != bin::to_vec(&next.args)?;
        self.install(next, true)?;
        Ok(changed)
    }
    fn install(&mut self, next: Self, keep_args: bool) -> Result<(), DataError> {
        let old_world = self.world.exchange(next.world)?;
        let old_args = if keep_args {
            next.args
        } else {
            std::mem::replace(&mut self.args, next.args)
        };
        self.input = next.input;
        self.queue = next.queue;
        self.world_us = next.world_us;
        self.caller_us = next.caller_us;
        self.tick_failed = false;
        self.tick_error = None;
        drop((old_world, old_args));
        Ok(())
    }
    fn candidate(bytes: &[u8], adapt: bool) -> Result<Self, DataError> {
        if bytes.len() > 128 * 1024 * 1024 {
            return Err(DataError::new("save exceeds 128 MiB"));
        }
        let payload = bytes
            .strip_prefix(MAGIC)
            .ok_or_else(|| DataError::new("unsupported Sim save version; expected EXSIM v10"))?;
        let mut r = bin::Decoder::with_budget(payload, 256 * 1024 * 1024);
        r.begin_seq()?;
        r.required_item("incomplete Sim save")?;
        if r.string()? != G::ID {
            return Err(DataError::new("game identity differs"));
        }
        r.required_item("incomplete Sim save")?;
        let args = G::Args::read_new(&mut r)?;
        Self::check(&args)?;
        let mut world = World::new(G::HZ, 0);
        G::register(&mut world, SetupArgs(&args))?;
        r.required_item("incomplete Sim save")?;
        world.read(&mut r)?;
        r.required_item("incomplete Sim save")?;
        let mut world_us = 0i64;
        world_us.read(&mut r)?;
        if world_us < 0
            || world.hz() != G::HZ
            || world.tick() as u128 != world_us as u128 * G::HZ as u128 / 1_000_000
        {
            return Err(DataError::new("saved clock disagrees with world"));
        }
        r.required_item("incomplete Sim save")?;
        let mut input = Input::default();
        input.read(&mut r)?;
        input.validate_saved(G::ACTIONS)?;
        r.required_item("incomplete Sim save")?;
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
        r.required_item("incomplete Sim save")?;
        world.read_publications(&mut r)?;
        r.required_item("incomplete Sim save")?;
        r.begin_seq()?;
        world.read_journal(&mut r)?;
        if r.item()? {
            return Err(DataError::new("extra journal data"));
        }
        r.required_item("incomplete Sim save")?;
        let mut caller_us = 0i64;
        caller_us.read(&mut r)?;
        if caller_us < 0 {
            return Err(DataError::new("invalid caller clock"));
        }
        r.required_item("missing publication delivery state")?;
        world.published_pending.set(r.boolean()?);
        if r.item()? {
            return Err(DataError::new("extra Sim save data"));
        }
        r.finish()?;
        world.driver_owned = true;
        let next = Self {
            world,
            args,
            input,
            queue: queue.into(),
            world_us,
            caller_us,
            paranoid: Paranoid::Off,
            tick_failed: false,
            tick_error: None,
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

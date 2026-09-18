//! Opt-in, bounded simulation captures. Imported recordings are data, never code.
use crate::{bin, hash, json, Clock, Data, DataError, Game, InputEvent, Sim, Value};

const MAX_BYTES: u32 = 8 * 1024 * 1024;
const MAX_EVENTS: u32 = 16_384;
const MAX_TICKS: u64 = 216_000;

/// Explicit bounds for one recording window; exhaustion retains the contiguous prefix.
#[derive(Clone, Copy, Data)]
pub struct CaptureLimits {
    /// Total serialized checkpoint and record budget, at most 8 MiB.
    pub bytes: u32,
    /// Number of accepted ingress/clock records, at most 16,384.
    pub events: u32,
    /// Simulation ticks since the checkpoint, at most 216,000.
    pub ticks: u64,
}
impl Default for CaptureLimits {
    fn default() -> Self {
        Self {
            bytes: 2 * 1024 * 1024,
            events: 4096,
            ticks: 36_000,
        }
    }
}
#[derive(Clone, Default, Data)]
pub(crate) enum Operation {
    #[default]
    Boundary,
    Input(InputEvent),
    Bind(Vec<Value>),
    Viewport {
        width: f32,
        height: f32,
    },
    Advance {
        at_us: i64,
        live: bool,
    },
}
#[derive(Clone, Default, Data)]
struct Record {
    index: u32,
    before: u64,
    after: u64,
    operation: Operation,
    world_hash: u64,
    state_hash: u64,
    // Bounded typed comparison at each sampled boundary, not a full shadow world.
    observed: String,
}
/// World-only checkpoint plus a contiguous ordered input/binding suffix.
/// Artifact identity is supplied by the trusted launcher from actual loaded bytes.
#[derive(Clone, Default, Data)]
pub struct Capture {
    format: u32,
    game: String,
    version: u32,
    build: String,
    hz: u32,
    seed: u64,
    first_tick: u64,
    checkpoint: Vec<u8>,
    checkpoint_hash: u64,
    checkpoint_world_hash: u64,
    records: Vec<Record>,
    limits: CaptureLimits,
    incomplete: String,
    complete: bool,
    completed_records: u32,
    end_state_hash: u64,
}
#[derive(Default, Data)]
struct Envelope {
    checksum: u64,
    capture: Capture,
}
pub(crate) struct Recorder {
    capture: Capture,
    origin_us: i64,
    bytes: usize,
    stopped: bool,
}
impl Recorder {
    pub(crate) fn fail(&mut self, why: &str) {
        if self.capture.incomplete.is_empty() {
            self.capture.incomplete = why.into();
        }
        self.stopped = true;
    }
    pub(crate) fn active(&self) -> bool {
        !self.stopped
    }
}
impl Capture {
    /// Encode a sealed recording; the checksum detects corruption and missing records.
    /// This checksum is not an authentication signature.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = b"EXCAP\0\x01".to_vec();
        bytes.extend(bin::to_vec(&Envelope {
            checksum: hash::of(self),
            capture: self.clone(),
        }));
        bytes
    }
    /// Decode within a fixed allocation budget and verify the complete recording.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DataError> {
        if bytes.len() > MAX_BYTES as usize + 4096 {
            return Err(DataError::new("capture exceeds 8 MiB"));
        }
        let payload = bytes
            .strip_prefix(b"EXCAP\0\x01")
            .ok_or_else(|| DataError::new("unsupported capture format"))?;
        let wire: Envelope = bin::from_slice(payload)?;
        if hash::of(&wire.capture) != wire.checksum {
            return Err(DataError::new(
                "capture checksum differs: corrupt checkpoint or dropped event",
            ));
        }
        wire.capture.validate()?;
        Ok(wire.capture)
    }
    fn validate(&self) -> Result<(), DataError> {
        let refuse = |s: &str| Err(DataError::new(s));
        if self.format != 1 {
            return refuse("unsupported capture version");
        }
        if !self.complete || !self.incomplete.is_empty() {
            return refuse(&format!("capture incomplete: {}", self.incomplete));
        }
        if self.records.len() != self.completed_records as usize {
            return refuse("capture has dropped records");
        }
        if self.records.len() > MAX_EVENTS as usize || self.hz == 0 {
            return refuse("capture bounds invalid");
        }
        let mut tick = self.first_tick;
        for (index, record) in self.records.iter().enumerate() {
            if record.index as usize != index || record.before != tick || record.after < tick {
                return refuse("capture event suffix is not contiguous");
            }
            tick = record.after;
            if tick - self.first_tick > MAX_TICKS {
                return refuse("capture tick budget exceeded");
            }
        }
        Ok(())
    }
    /// Last verified tick in the retained contiguous interval.
    pub fn last_tick(&self) -> u64 {
        self.records.last().map_or(self.first_tick, |r| r.after)
    }
    /// Number of ordered records; seek uses this boundary, including paused records.
    pub fn records(&self) -> usize {
        self.records.len()
    }
    /// Human-readable status, also available from state without exporting payload bytes.
    pub fn status(&self) -> String {
        format!("{{\"scope\":\"world-only\",\"class\":\"simulation\",\"game\":{},\"saveVersion\":{},\"build\":{},\"hz\":{},\"seed\":{},\"firstTick\":{},\"lastReliableTick\":{},\"hash\":\"0x{:016x}\",\"records\":{},\"complete\":{},\"incomplete\":{},\"uiState\":\"omitted\",\"externalResults\":\"unsupported\"}}",
            json::to_string(&self.game).unwrap(), self.version, json::to_string(&self.build).unwrap(), self.hz, self.seed, self.first_tick, self.last_tick(), self.records.last().map_or(self.checkpoint_world_hash, |r| r.world_hash), self.records.len(), self.complete && self.incomplete.is_empty(), json::to_string(&self.incomplete).unwrap())
    }
}
impl<G: Game> Sim<G> {
    /// Start an explicit capture window. `build` must identify actual loaded artifacts,
    /// never merely a Git commit or an identity read from the imported capture.
    /// Callers with hidden external results must refuse before starting this world-only path.
    pub fn start_capture(&mut self, build: &str, limits: CaptureLimits) -> Result<(), DataError> {
        if !G::CAPTURE_SUPPORTED {
            return Err(DataError::new("capture refused: game has not declared support for replay without external dependencies"));
        }
        if self.recorder.as_ref().is_some_and(Recorder::active) {
            return Err(DataError::new("capture already active; stop it first"));
        }
        if self.last_us.is_none() {
            return Err(DataError::new(
                "capture needs an established clock boundary; advance clock to current time first",
            ));
        }
        if build.is_empty() || build.len() > 4096 {
            return Err(DataError::new("capture requires a loaded artifact receipt"));
        }
        if limits.bytes == 0
            || limits.bytes > MAX_BYTES
            || limits.events == 0
            || limits.events > MAX_EVENTS
            || limits.ticks == 0
            || limits.ticks > MAX_TICKS
        {
            return Err(DataError::new(
                "capture limits exceed supported byte/event/tick bounds",
            ));
        }
        let checkpoint = self.save();
        if checkpoint.len() + 4096 > limits.bytes as usize {
            return Err(DataError::new("checkpoint exceeds capture byte budget"));
        }
        let capture = Capture {
            format: 1,
            game: G::ID.into(),
            version: G::SAVE_VERSION,
            build: build.into(),
            hz: G::HZ,
            seed: self.world.seed(),
            first_tick: self.world.tick(),
            checkpoint_hash: self.capture_hash(),
            checkpoint_world_hash: self.world.hash(),
            checkpoint,
            limits,
            ..Default::default()
        };
        let bytes = bin::to_vec(&capture).len() + 4096;
        self.recorder = Some(Recorder {
            capture,
            origin_us: self.last_us.unwrap_or(0),
            bytes,
            stopped: false,
        });
        Ok(())
    }
    /// Close a window at the current boundary; incomplete captures remain visibly incomplete.
    pub fn stop_capture(&mut self) -> Result<&Capture, DataError> {
        let end_state_hash = self.capture_hash();
        let recorder = self
            .recorder
            .as_mut()
            .ok_or_else(|| DataError::new("no capture window"))?;
        recorder.stopped = true;
        recorder.capture.complete = true;
        recorder.capture.completed_records = recorder.capture.records.len() as u32;
        recorder.capture.end_state_hash = end_state_hash;
        Ok(&recorder.capture)
    }
    /// Retained recording, available during or after the explicit window.
    pub fn capture(&self) -> Option<&Capture> {
        self.recorder.as_ref().map(|r| &r.capture)
    }
    pub(crate) fn capture_fail(&mut self, why: &str) {
        if let Some(r) = self.recorder.as_mut().filter(|r| r.active()) {
            r.fail(why);
        }
    }
    // No checkpoint byte comparison: journal entries and the host epoch aren't semantics.
    fn capture_hash(&self) -> u64 {
        #[derive(Default, Data)]
        struct State {
            world: u64,
            args: String,
            input: crate::Input,
            pending: Vec<u8>,
            world_us: i64,
        }
        hash::of(&State {
            world: self.world.hash(),
            args: json::to_string(&self.args).unwrap(),
            input: self.input.clone(),
            pending: self.capture_queue(),
            world_us: self.world_us,
        })
    }
    fn observed(&self) -> String {
        let resources = self
            .world
            .resources_json()
            .unwrap_or_else(|_| "null".into());
        let resources = if resources.len() <= 1024 {
            resources
        } else {
            "{\"unavailable\":\"resources exceed 1024 bytes\"}".into()
        };
        let mut entities = Vec::new();
        let mut bytes = resources.len();
        let mut truncated = false;
        for (index, entity) in self.world.entities().enumerate() {
            if index >= 16 {
                truncated = true;
                break;
            }
            let components = self
                .world
                .components_json(entity)
                .unwrap_or_else(|_| "null".into());
            let row = format!(
                "{{\"id\":{},\"name\":{},\"components\":{components}}}",
                entity.index(),
                json::to_string(&self.world.name(entity).unwrap_or("").to_string()).unwrap()
            );
            if bytes + row.len() > 4096 || entities.len() >= 16 {
                truncated = true;
                continue;
            }
            bytes += row.len();
            entities.push(row);
        }
        format!("{{\"resources\":{resources},\"entities\":[{}],\"truncated\":{truncated},\"inspect\":\"state world:*\"}}", entities.join(","))
    }
    pub(crate) fn record(&mut self, before: u64, mut operation: Operation) {
        let Some(r) = self.recorder.as_ref().filter(|r| r.active()) else {
            return;
        };
        let origin = r.origin_us;
        if let Operation::Input(event) = &mut operation {
            event.set_at_ms(
                crate::sim::micros(event.at_ms()).saturating_sub(origin) as f64 / 1000.0,
            );
        }
        if let Operation::Advance { at_us, .. } = &mut operation {
            *at_us = at_us.saturating_sub(origin);
        }
        let after = self.world.tick();
        let state_hash = self.capture_hash();
        let observed = self.observed();
        let record = Record {
            index: r.capture.records.len() as u32,
            before,
            after,
            operation,
            world_hash: self.world.hash(),
            state_hash,
            observed,
        };
        let bytes = bin::to_vec(&record).len();
        let r = self.recorder.as_mut().unwrap();
        if r.bytes + bytes > r.capture.limits.bytes as usize
            || r.capture.records.len() >= r.capture.limits.events as usize
            || after.saturating_sub(r.capture.first_tick) > r.capture.limits.ticks
        {
            r.fail("byte/event/tick limit reached; reliable interval ends at last retained record");
            return;
        }
        r.bytes += bytes;
        r.capture.records.push(record);
    }
    /// Replay into a fresh simulation without changing this or any player's save store.
    /// `through` is an ordered record boundary; seeking backward restores then runs forward.
    /// The trusted launcher supplies `loaded_build` from its current artifact inventory.
    pub fn replay_capture(
        capture: &Capture,
        loaded_build: &str,
        through: Option<usize>,
    ) -> Result<Self, DataError> {
        capture.validate()?;
        if capture.game != G::ID || capture.version != G::SAVE_VERSION || capture.hz != G::HZ {
            return Err(DataError::new(
                "capture game/save version/fixed-step rate differs; current world retained",
            ));
        }
        if capture.build != loaded_build {
            return Err(DataError::new(
                "capture build differs from loaded artifact receipt; exact replay refused",
            ));
        }
        let end = through.unwrap_or(capture.records.len());
        if end > capture.records.len() {
            return Err(DataError::new("seek boundary exceeds captured interval"));
        }
        if !G::CAPTURE_SUPPORTED {
            return Err(DataError::new(
                "replay refused: unsupported external game dependencies",
            ));
        }
        let mut sim = Self::from_save(&capture.checkpoint)?;
        // Isolated replay owns a controlled clock, but intentionally retains the
        // checkpoint's held input. A physical handoff would erase that evidence.
        sim.agent_owned = true;
        if sim.capture_hash() != capture.checkpoint_hash || sim.world.tick() != capture.first_tick {
            return Err(DataError::new(
                "corrupt checkpoint: semantic state/hash differs",
            ));
        }
        sim.advance(0.0, Clock::Seekable);
        for record in capture.records.iter().take(end) {
            if sim.world.tick() != record.before {
                return Err(DataError::new("capture boundary differs before event"));
            }
            match &record.operation {
                Operation::Boundary => {}
                Operation::Input(event) => {
                    validate_input(event)?;
                    sim.input(event.clone());
                }
                Operation::Bind(values) => sim.bind(values, None).map_err(DataError::new)?,
                Operation::Viewport { width, height } => {
                    if !width.is_finite() || !height.is_finite() || *width <= 0.0 || *height <= 0.0
                    {
                        return Err(DataError::new("invalid captured viewport"));
                    }
                    sim.viewport(*width, *height);
                }
                Operation::Advance { at_us, live } => {
                    if *at_us < sim.last_us.unwrap_or(0) || *at_us > 86_400_000_000 {
                        return Err(DataError::new("invalid captured clock"));
                    }
                    let clock = if *live { Clock::Live } else { Clock::Seekable };
                    if sim.ticks_due(*at_us as f64 / 1000.0, clock) as u64
                        != record.after - record.before
                    {
                        return Err(DataError::new(
                            "captured clock does not match recorded tick boundary",
                        ));
                    }
                    sim.advance(*at_us as f64 / 1000.0, clock);
                }
            }
            if sim.world.tick() != record.after
                || sim.world.hash() != record.world_hash
                || sim.capture_hash() != record.state_hash
            {
                return Err(DataError::new(format!("replay mismatch at sampled record {} tick {}: expected hash 0x{:016x}, actual 0x{:016x}; expected typed state {}; actual {}; no earlier divergent tick claimed", record.index, record.after, record.world_hash, sim.world.hash(), record.observed, sim.observed())));
            }
        }
        if end == capture.records.len() && sim.capture_hash() != capture.end_state_hash {
            return Err(DataError::new(
                "replay final state differs; capture may have an unrecorded dependency",
            ));
        }
        Ok(sim)
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 15) as usize] as char);
    }
    out
}
pub(crate) fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if text.len() > (MAX_BYTES as usize + 4096) * 2 || !text.len().is_multiple_of(2) {
        return Err("capture hex length invalid".into());
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|b| {
            let digit = |v: u8| match v {
                b'0'..=b'9' => Some(v - b'0'),
                b'a'..=b'f' => Some(v - b'a' + 10),
                _ => None,
            };
            Ok(digit(b[0]).ok_or("invalid capture hex")? * 16
                + digit(b[1]).ok_or("invalid capture hex")?)
        })
        .collect()
}

fn validate_input(event: &InputEvent) -> Result<(), DataError> {
    let valid = event.at_ms().is_finite()
        && event.at_ms().abs() <= 86_400_000.0
        && match event {
            InputEvent::Key { code, .. } => code.len() <= 128,
            InputEvent::Pointer { x, y, .. } => x.is_finite() && y.is_finite(),
            InputEvent::Wheel { dx, dy, .. } => dx.is_finite() && dy.is_finite(),
            InputEvent::Blur { .. } => true,
        };
    if valid {
        Ok(())
    } else {
        Err(DataError::new("invalid captured input fields/stamp"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture;
    impl Game for Fixture {
        const ID: &'static str = "capture-hardening";
        const CAPTURE_SUPPORTED: bool = true;
        type Args = ();
        fn setup(w: &mut crate::World, _: &()) {
            w.spawn_named("crate", (crate::Transform::default(),));
        }
        fn tick(_: &mut crate::World, _: &crate::Input, _: &()) {}
    }
    fn capture() -> Capture {
        let mut sim = Sim::<Fixture>::new(()).unwrap();
        sim.advance(0.0, Clock::Seekable);
        sim.start_capture("actual", CaptureLimits::default())
            .unwrap();
        sim.key_down("KeyW");
        sim.run(100.0);
        sim.stop_capture().unwrap().clone()
    }
    #[test]
    fn dropped_events_wrong_game_schema_and_corrupt_checkpoint_refuse() {
        let original = capture();
        let mut dropped = original.clone();
        dropped.records.pop();
        assert!(Sim::<Fixture>::replay_capture(&dropped, "actual", None)
            .err()
            .unwrap()
            .to_string()
            .contains("dropped"));
        let mut wrong = original.clone();
        wrong.game = "other".into();
        assert!(Sim::<Fixture>::replay_capture(&wrong, "actual", None).is_err());
        wrong = original.clone();
        wrong.version += 1;
        assert!(Sim::<Fixture>::replay_capture(&wrong, "actual", None).is_err());
        wrong = original.clone();
        wrong.checkpoint[0] ^= 1;
        assert!(Sim::<Fixture>::replay_capture(&wrong, "actual", None).is_err());
        let bytes = original.to_bytes();
        assert!(Capture::from_bytes(&bytes[..bytes.len() - 1]).is_err());
    }
    #[test]
    fn invalid_input_and_clock_refuse_before_execution_and_mismatch_names_typed_state() {
        let mut c = capture();
        c.records[0].operation = Operation::Input(InputEvent::Blur { at_ms: f64::NAN });
        assert!(Sim::<Fixture>::replay_capture(&c, "actual", None)
            .err()
            .unwrap()
            .to_string()
            .contains("input fields"));
        c = capture();
        c.records[1].operation = Operation::Advance {
            at_us: 86_400_000_000,
            live: false,
        };
        assert!(Sim::<Fixture>::replay_capture(&c, "actual", None)
            .err()
            .unwrap()
            .to_string()
            .contains("clock"));
        c = capture();
        c.records[1].world_hash ^= 1;
        let error = Sim::<Fixture>::replay_capture(&c, "actual", None)
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("sampled record 1")
                && error.contains("crate")
                && error.contains("Transform"),
            "{error}"
        );
    }
}

//! Opt-in, bounded simulation captures. Imported recordings are data, never code.
use crate::{
    bin, hash, json, Clock, Data, DataError, Game, InputEvent, Reader, Sim, Value, Writer,
};

const MAX_BYTES: u32 = 8 * 1024 * 1024;
const MAX_EVENTS: u32 = 16_384;
const MAX_TICKS: u64 = 216_000;
// Includes destination capacity, nested Values, and the decoder's name/field tables.
// Scale with the actual wire, not an untrusted declaration. Each pass has its own
// cumulative budget; the verification pass retains only one record at a time.
fn decoder(payload: &[u8]) -> bin::Decoder<'_> {
    bin::Decoder::with_budget(
        payload,
        (payload.len() * 32 + 65_536).min(128 * 1024 * 1024),
    )
}

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
impl CaptureLimits {
    fn validate(&self) -> Result<(), DataError> {
        if self.bytes == 0
            || self.bytes > MAX_BYTES
            || self.events == 0
            || self.events > MAX_EVENTS
            || self.ticks == 0
            || self.ticks > MAX_TICKS
        {
            Err(DataError::new("capture bounds invalid"))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Default, Data)]
pub(crate) enum Operation {
    #[default]
    Boundary,
    Input(InputEvent),
    DeviceInput(InputEvent),
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
        let (header, checksum, records) = read_header(payload)?;
        header.limits.validate()?;
        if bytes.len() > header.limits.bytes as usize {
            return Err(DataError::new("capture exceeds declared byte budget"));
        }
        if records > header.limits.events as usize {
            return Err(DataError::new("capture record count exceeds event budget"));
        }
        verify_checksum(payload, &header, checksum)?;
        // V1 hashes typed values (including omitted defaults), not wire bytes.
        // Only retain the record vector after the streaming checksum has passed.
        drop(header);
        #[cfg(test)]
        tests::COLLECTION_PASSES.with(|n| n.set(n.get() + 1));
        let mut r = decoder(payload);
        let mut wire = Envelope::default();
        wire.read(&mut r)?;
        r.finish()?;
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
        self.limits.validate()?;
        if self.records.len() > self.limits.events as usize || self.hz == 0 {
            return refuse("capture bounds invalid");
        }
        let mut tick = self.first_tick;
        for (index, record) in self.records.iter().enumerate() {
            if record.index as usize != index || record.before != tick || record.after < tick {
                return refuse("capture event suffix is not contiguous");
            }
            tick = record.after;
            if tick - self.first_tick > self.limits.ticks {
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

// Read the small header/checkpoint while walking (never collecting) record payloads.
// Fields may be reordered or omitted just as in Data; the decoder still detects
// duplicates and interns names in skipped values. The hard count is checked at
// the sequence header, even when the declared limits appear after the records.
fn read_header(payload: &[u8]) -> Result<(Capture, u64, usize), DataError> {
    let mut r = decoder(payload);
    let mut c = Capture::default();
    let mut checksum = 0;
    let mut records = 0;
    r.begin_struct()?;
    while let Some(name) = r.field()? {
        match name.as_str() {
            "checksum" => checksum.read(&mut r)?,
            "capture" => {
                r.begin_struct()?;
                while let Some(name) = r.field()? {
                    match name.as_str() {
                        "format" => c.format.read(&mut r)?,
                        "game" => c.game.read(&mut r)?,
                        "version" => c.version.read(&mut r)?,
                        "build" => c.build.read(&mut r)?,
                        "hz" => c.hz.read(&mut r)?,
                        "seed" => c.seed.read(&mut r)?,
                        "first_tick" => c.first_tick.read(&mut r)?,
                        "checkpoint" => c.checkpoint.read(&mut r)?,
                        "checkpoint_hash" => c.checkpoint_hash.read(&mut r)?,
                        "checkpoint_world_hash" => c.checkpoint_world_hash.read(&mut r)?,
                        "limits" => c.limits.read(&mut r)?,
                        "incomplete" => c.incomplete.read(&mut r)?,
                        "complete" => c.complete.read(&mut r)?,
                        "completed_records" => c.completed_records.read(&mut r)?,
                        "end_state_hash" => c.end_state_hash.read(&mut r)?,
                        "records" => {
                            r.begin_seq()?;
                            records = r.sequence_len().unwrap();
                            if records > MAX_EVENTS as usize {
                                return Err(DataError::new(
                                    "capture record count exceeds event budget",
                                ));
                            }
                            while r.item()? {
                                r.skip()?;
                            }
                        }
                        _ => r.skip()?,
                    }
                }
            }
            _ => r.skip()?,
        }
    }
    r.finish()?;
    Ok((c, checksum, records))
}

fn verify_checksum(payload: &[u8], c: &Capture, checksum: u64) -> Result<(), DataError> {
    // Match Capture's declaration-order Data hash without retaining its records.
    // A round-trip fixture below pins this to the derived v1 schema, including
    // reordered fields, missing defaults, and nested binding values.
    let mut h = hash::Hasher::default();
    h.begin_struct();
    c.format.write(&mut h);
    c.game.write(&mut h);
    c.version.write(&mut h);
    c.build.write(&mut h);
    c.hz.write(&mut h);
    c.seed.write(&mut h);
    c.first_tick.write(&mut h);
    c.checkpoint.write(&mut h);
    c.checkpoint_hash.write(&mut h);
    c.checkpoint_world_hash.write(&mut h);
    let mut r = decoder(payload);
    let mut found = false;
    r.begin_struct()?;
    while let Some(name) = r.field()? {
        if name != "capture" {
            r.skip()?;
            continue;
        }
        r.begin_struct()?;
        while let Some(name) = r.field()? {
            if name != "records" {
                r.skip()?;
                continue;
            }
            found = true;
            r.begin_seq()?;
            let count = r.sequence_len().unwrap();
            if count > c.limits.events as usize {
                return Err(DataError::new("capture record count exceeds event budget"));
            }
            h.begin_seq(count);
            while r.item()? {
                #[cfg(test)]
                tests::VERIFIED_RECORDS.with(|n| n.set(n.get() + 1));
                let mut record = Record::default();
                record.read(&mut r)?;
                record.write(&mut h);
            }
            h.end_seq();
        }
    }
    r.finish()?;
    if !found {
        h.begin_seq(0);
        h.end_seq();
    }
    c.limits.write(&mut h);
    c.incomplete.write(&mut h);
    c.complete.write(&mut h);
    c.completed_records.write(&mut h);
    c.end_state_hash.write(&mut h);
    h.end_struct();
    if h.finish() != checksum {
        return Err(DataError::new(
            "capture checksum differs: corrupt checkpoint or dropped event",
        ));
    }
    Ok(())
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
        let checkpoint = self.save()?;
        if checkpoint.len() + 4096 > limits.bytes as usize {
            return Err(DataError::new("checkpoint exceeds capture byte budget"));
        }
        let capture = Capture {
            format: 1,
            game: G::ID.into(),
            version: 6,
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
        if capture.game != G::ID || capture.version != 6 || capture.hz != G::HZ {
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
        // The checkpoint is a nested EXSIM/JSON/EXGAME payload. All its decoders
        // share consumption, including repeated passes and paged world storage.
        // The floor accommodates sparse component pages in small real saves.
        // Game setup/migration and custom Data/Default code remain executable
        // construction code: allocations outside Reader claims are not metered.
        let budget = crate::data::limits::LoadBudget::new(
            capture
                .checkpoint
                .len()
                .saturating_mul(32)
                .saturating_add(8 * 1024 * 1024)
                .min(128 * 1024 * 1024),
        );
        let mut sim = Self::from_save_in(&capture.checkpoint, Some(&budget))?;
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
                Operation::DeviceInput(event) => {
                    validate_input(event)?;
                    sim.device_input(event.clone());
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
    use std::cell::Cell;
    thread_local! { static EXECUTED_TICKS: Cell<u64> = const { Cell::new(0) }; }
    thread_local! {
        pub(super) static COLLECTION_PASSES: Cell<usize> = const { Cell::new(0) };
        pub(super) static VERIFIED_RECORDS: Cell<usize> = const { Cell::new(0) };
    }
    fn refused_before_collection(bytes: &[u8], expected: &str, verified: usize) {
        COLLECTION_PASSES.with(|n| n.set(0));
        VERIFIED_RECORDS.with(|n| n.set(0));
        let error = Capture::from_bytes(bytes).err().unwrap().to_string();
        assert!(error.contains(expected), "{error}");
        assert_eq!(COLLECTION_PASSES.with(Cell::get), 0);
        assert_eq!(VERIFIED_RECORDS.with(Cell::get), verified);
    }
    struct Fixture;
    impl Game for Fixture {
        const ID: &'static str = "capture-hardening";
        const CAPTURE_SUPPORTED: bool = true;
        type Args = ();
        fn setup(w: &mut crate::World, _: &()) {
            w.spawn_named("crate", (crate::Transform::default(),));
        }
        fn tick(_: &mut crate::World, _: &crate::Input, _: &()) {
            EXECUTED_TICKS.with(|ticks| ticks.set(ticks.get() + 1));
        }
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
    fn compact_record_counts_refuse_before_payload_decode() {
        #[derive(Default, Data)]
        struct Empty {}
        #[derive(Default, Data)]
        struct Compact {
            records: Vec<Empty>,
            limits: CaptureLimits,
        }
        #[derive(Default, Data)]
        struct Wire {
            capture: Compact,
            checksum: u64,
        }
        for (count, limit) in [(MAX_EVENTS + 1, MAX_EVENTS), (2, 1)] {
            let capture = Capture {
                records: (0..count).map(|_| Record::default()).collect(),
                limits: CaptureLimits {
                    events: limit,
                    ..Default::default()
                },
                ..Default::default()
            };
            // A valid semantic checksum over records encoded as just [8, 0].
            let mut bytes = b"EXCAP\0\x01".to_vec();
            bytes.extend(bin::to_vec(&Wire {
                checksum: hash::of(&capture),
                capture: Compact {
                    records: (0..count).map(|_| Empty {}).collect(),
                    limits: capture.limits,
                },
            }));
            assert!(bytes.len() < 34_000);
            refused_before_collection(&bytes, "record count exceeds event budget", 0);
        }

        let mut w = bin::Encoder::default();
        w.begin_struct();
        w.field("capture");
        w.begin_struct();
        w.field("records");
        w.begin_seq(MAX_EVENTS as usize + 1);
        let mut bytes = b"EXCAP\0\x01".to_vec();
        bytes.extend(w.finish());
        // Enough bytes for the count check, but the first value tag is invalid.
        // Refusing the count rather than that tag proves no item was walked.
        bytes.extend(std::iter::repeat_n(255, MAX_EVENTS as usize + 1));
        refused_before_collection(&bytes, "record count exceeds event budget", 0);
    }
    #[test]
    fn checksum_and_actual_wire_size_are_checked_before_retaining_records() {
        let original = capture();
        let mut corrupt = b"EXCAP\0\x01".to_vec();
        corrupt.extend(bin::to_vec(&Envelope {
            checksum: hash::of(&original) ^ 1,
            capture: original.clone(),
        }));
        refused_before_collection(&corrupt, "checksum differs", original.records.len());

        // The checksum is a varint too: changing the budget can shorten it,
        // so iterating length - 1 need not converge. Find adjacent refusing /
        // accepted budgets with identical wire length, keeping the exact edge.
        let mut boundary = None;
        'fixture: for padding in 0..64 {
            let mut candidate = original.clone();
            candidate.build.extend(std::iter::repeat_n('.', padding));
            let length = candidate.to_bytes().len() as u32;
            for budget in length.saturating_sub(16)..=length + 16 {
                candidate.limits.bytes = budget;
                if candidate.to_bytes().len() != budget as usize + 1 {
                    continue;
                }
                candidate.limits.bytes += 1;
                if candidate.to_bytes().len() == budget as usize + 1 {
                    candidate.limits.bytes -= 1;
                    boundary = Some(candidate);
                    break 'fixture;
                }
            }
        }
        let mut small = boundary.expect("adjacent byte-budget fixture within 64 padding bytes");
        refused_before_collection(&small.to_bytes(), "declared byte budget", 0);
        small.limits.bytes += 1;
        assert_eq!(small.to_bytes().len(), small.limits.bytes as usize);
        Capture::from_bytes(&small.to_bytes()).unwrap();

        #[derive(Default, Data)]
        struct Padded {
            checksum: u64,
            capture: Capture,
            ignored: Vec<u8>,
        }
        let mut padded = b"EXCAP\0\x01".to_vec();
        padded.extend(bin::to_vec(&Padded {
            checksum: hash::of(&small),
            capture: small,
            ignored: vec![0; 32],
        }));
        refused_before_collection(&padded, "declared byte budget", 0);
    }
    #[test]
    fn v1_checksum_preserves_nested_values_reordered_fields_and_defaults() {
        let mut c = capture();
        c.records[0].operation = Operation::Bind(vec![
            Value::Unit,
            Value::Number(-0.0),
            Value::Bool(true),
            Value::str("unicode 🌕"),
            Value::list(vec![Value::record(vec![Value::Number(7.0)])]),
            Value::Option(None),
            Value::Option(Some(std::rc::Rc::new(Value::str("nested")))),
        ]);
        let bytes = c.to_bytes();
        let decoded = Capture::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.to_bytes(), bytes);
        assert_eq!(hash::of(&decoded), hash::of(&c));

        #[derive(Default, Data)]
        struct Compact {
            limits: CaptureLimits,
            complete: bool,
            hz: u32,
            format: u32,
        }
        #[derive(Default, Data)]
        struct Reordered {
            capture: Compact,
            checksum: u64,
        }
        let c = Capture {
            format: 1,
            hz: 60,
            complete: true,
            ..Default::default()
        };
        let mut bytes = b"EXCAP\0\x01".to_vec();
        bytes.extend(bin::to_vec(&Reordered {
            checksum: hash::of(&c),
            capture: Compact {
                format: c.format,
                hz: c.hz,
                complete: c.complete,
                limits: c.limits,
            },
        }));
        assert_eq!(
            hash::of(&Capture::from_bytes(&bytes).unwrap()),
            hash::of(&c)
        );
    }
    #[test]
    fn legitimate_maximum_record_count_and_large_nested_bindings_still_import() {
        let mut c = Capture {
            format: 1,
            hz: 60,
            complete: true,
            completed_records: MAX_EVENTS,
            limits: CaptureLimits {
                bytes: MAX_BYTES,
                events: MAX_EVENTS,
                ..Default::default()
            },
            records: (0..MAX_EVENTS)
                .map(|index| Record {
                    index,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        assert_eq!(
            Capture::from_bytes(&c.to_bytes()).unwrap().records(),
            MAX_EVENTS as usize
        );
        // Nested collection cardinality is governed by allocation, not the
        // ingress event count. Large legitimate binding lists remain supported.
        c.records[0].operation = Operation::Bind(vec![Value::list(vec![Value::Unit; 131_072])]);
        let bytes = c.to_bytes();
        assert_eq!(
            hash::of(&Capture::from_bytes(&bytes).unwrap()),
            hash::of(&c)
        );
        c.records.clear();
        c.completed_records = 0;
        c.checkpoint = vec![0; MAX_BYTES as usize - 2048];
        let bytes = c.to_bytes();
        assert!(bytes.len() <= MAX_BYTES as usize);
        assert_eq!(
            Capture::from_bytes(&bytes).unwrap().checkpoint,
            c.checkpoint
        );
    }
    #[test]
    fn malformed_nested_binding_count_refuses_before_any_value() {
        let count = 6 * 1024 * 1024;
        assert!(count * std::mem::size_of::<Value>() > 128 * 1024 * 1024);
        let mut w = bin::Encoder::default();
        w.begin_struct();
        w.field("capture");
        w.begin_struct();
        w.field("limits");
        CaptureLimits {
            bytes: MAX_BYTES,
            ..Default::default()
        }
        .write(&mut w);
        w.field("records");
        w.begin_seq(1);
        w.begin_struct();
        w.field("operation");
        w.variant("Bind", 2);
        w.begin_seq(1);
        w.begin_seq(count);
        for _ in 0..count {
            // Syntactically skippable; cannot decode as a Value. The budget
            // refusal must precede even this first enum type check/reservation.
            w.boolean(false);
        }
        w.end_seq();
        w.end_seq();
        w.end_variant();
        w.end_struct();
        w.end_seq();
        w.end_struct();
        w.end_struct();
        let mut bytes = b"EXCAP\0\x01".to_vec();
        bytes.extend(w.finish());
        assert!(bytes.len() <= MAX_BYTES as usize);
        refused_before_collection(&bytes, "decoded size exceeds load budget", 1);
    }
    fn checkpoint_envelope(game: &str, world: Vec<u8>, args: &str) -> Vec<u8> {
        #[derive(Default, Data)]
        struct Saved {
            game: String,
            version: u32,
            world: Vec<u8>,
            args: String,
        }
        let mut bytes = b"EXSIM\0\x06".to_vec();
        bytes.extend(bin::to_vec(&Saved {
            game: game.into(),
            version: 1,
            world,
            args: args.into(),
        }));
        bytes
    }
    fn import_checkpoint(checkpoint: Vec<u8>, game: &str) -> Capture {
        let mut forged = capture();
        forged.checkpoint = checkpoint;
        forged.game = game.into();
        forged.limits.bytes = MAX_BYTES;
        // The outer checksum is valid; the payload reaches checkpoint replay.
        Capture::from_bytes(&forged.to_bytes()).unwrap()
    }
    #[test]
    fn imported_checkpoint_counts_refuse_before_exsim_or_world_amplification() {
        let mut w = bin::Encoder::default();
        w.begin_struct();
        w.field("queue");
        w.begin_seq(524_288);
        let mut checkpoint = b"EXSIM\0\x06".to_vec();
        checkpoint.extend(w.finish());
        // The count fits the wire, but its Queued storage cannot fit the budget.
        // Budget refusal rather than an invalid-tag error proves no item is read.
        checkpoint.extend(std::iter::repeat_n(255, 524_288));
        let queued = import_checkpoint(checkpoint, Fixture::ID);

        let mut w = bin::Encoder::default();
        w.begin_struct();
        w.field("state");
        w.begin_struct();
        w.field("slots");
        w.begin_seq(5 * 1024 * 1024);
        let mut world = b"EXGAME\0\x03".to_vec();
        world.extend(w.finish());
        world.extend(std::iter::repeat_n(255, 5 * 1024 * 1024));
        let slots = import_checkpoint(checkpoint_envelope(Fixture::ID, world, "[]"), Fixture::ID);

        let mut live = Sim::<Fixture>::new(()).unwrap();
        live.advance(0.0, Clock::Seekable);
        let before = live.world().hash();
        for (capture, field) in [(queued, "queue"), (slots, "slots")] {
            let error = Sim::<Fixture>::replay_capture(&capture, "actual", None)
                .err()
                .unwrap()
                .to_string();
            assert!(error.contains(field) && error.contains("budget"), "{error}");
            // Exercise the existing isolated agent replay path as well: refusal
            // must leave the retained world intact, not merely the scratch load.
            let reply = live.agent(&format!(
                r#"{{"op":"state","capture":"replay","build":"actual","data":"{}"}}"#,
                hex(&capture.to_bytes())
            ));
            assert!(reply.contains(field) && reply.contains("budget"), "{reply}");
            assert_eq!(live.world().hash(), before);
        }
    }
    #[test]
    fn imported_checkpoint_component_page_refuses_before_default_or_read() {
        #[derive(Data)]
        struct Wide {
            #[data(skip)]
            _inline: [[[u32; 32]; 32]; 32],
        }
        impl Default for Wide {
            fn default() -> Self {
                panic!("constructed an imported component before checking its 128 MiB page")
            }
        }
        impl crate::Component for Wide {
            const NAME: &'static str = "Wide";
        }
        struct Pages;
        impl Game for Pages {
            const ID: &'static str = "capture-pages";
            const CAPTURE_SUPPORTED: bool = true;
            type Args = ();
            fn setup(world: &mut crate::World, _: &()) {
                world.register::<Wide>();
            }
            fn tick(_: &mut crate::World, _: &crate::Input, _: &()) {}
        }
        let mut source = crate::World::new(60, 0);
        let entity = source.spawn(());
        let mut w = bin::Encoder::default();
        w.begin_struct();
        w.field("state");
        w.begin_struct();
        w.field("hz");
        60u32.write(&mut w);
        w.field("slots");
        w.begin_seq(1);
        w.begin_struct();
        w.field("alive");
        true.write(&mut w);
        w.end_struct();
        w.end_seq();
        w.end_struct();
        w.field("rng");
        source.rng().write(&mut w);
        w.field("components");
        w.begin_struct();
        w.field("Wide");
        w.begin_seq(1);
        w.begin_seq(2);
        entity.write(&mut w);
        w.begin_struct();
        w.end_struct();
        w.end_seq();
        w.end_seq();
        w.end_struct();
        w.field("resources");
        w.begin_struct();
        w.end_struct();
        w.end_struct();
        let mut world = b"EXGAME\0\x03".to_vec();
        world.extend(w.finish());
        let mut retained = crate::World::new(60, 0);
        retained.register::<Wide>();
        retained.spawn_named("keep", ());
        let before = retained.save();
        let error = retained
            .load_in(
                &world,
                Some(&crate::data::limits::LoadBudget::new(8 * 1024 * 1024)),
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Wide") && error.contains("budget"),
            "{error}"
        );
        assert_eq!(retained.save(), before);
        let capture = import_checkpoint(checkpoint_envelope(Pages::ID, world, "[]"), Pages::ID);
        assert!(capture.checkpoint.len() < 512);
        let error = Sim::<Pages>::replay_capture(&capture, "actual", None)
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("Wide") && error.contains("budget"),
            "{error}"
        );
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
    fn checksum_valid_oversized_advance_refuses_before_any_tick() {
        let mut forged = capture();
        forged.records[1].operation = Operation::Advance {
            at_us: 86_400_000_000,
            live: false,
        };
        // Re-encode a valid checksum: integrity alone cannot bound replay work.
        let imported = Capture::from_bytes(&forged.to_bytes()).unwrap();
        EXECUTED_TICKS.with(|ticks| ticks.set(0));
        let error = Sim::<Fixture>::replay_capture(&imported, "actual", None)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("clock does not match recorded tick boundary"));
        assert_eq!(EXECUTED_TICKS.with(Cell::get), 0);

        let mut over_budget = capture();
        over_budget.limits.ticks = over_budget.last_tick() - over_budget.first_tick - 1;
        EXECUTED_TICKS.with(|ticks| ticks.set(0));
        assert!(Capture::from_bytes(&over_budget.to_bytes())
            .err()
            .unwrap()
            .to_string()
            .contains("tick budget"));
        assert_eq!(EXECUTED_TICKS.with(Cell::get), 0);
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

//! MIDI (LLP 1125 D6–D8): the ports `exactMIDI()` answers, the queue of
//! timed messages `sendMIDI` and a sequence's notes and clock put on it, and
//! what the inputs delivered — kept by the runner, so it is the same on
//! every host and under the driver, which sends nothing.
//!
//! - **The queue.** Every message has an id and a runner time. A host hands
//!   a message to its output at most [`HOLD`] before it is due and holds it
//!   until then, so a message is *handed* once `at − HOLD ≤ t` (a rule, not
//!   a host's report) and *waiting* before. A retraction takes back waiting
//!   messages only; a handed one plays.
//! - **Bounds.** At most [`QUEUED`] messages wait; note-offs, Stop and clock
//!   ticks are exempt, so a note always ends. `sendMIDI` past the bound is
//!   dropped and journaled; a sequence stops expanding there.
//! - **Well-formed MIDI 1.0** ([`split`]): a status byte and its data bytes,
//!   no running status, no SysEx, nothing undefined.
//! - **Ports** keep every port seen this session: one a host no longer
//!   reports stays listed as `disconnected`.
//! - **A sequence's notes** (`crate::transport`) are a note-on at the step
//!   and a note-off `duration` beats later, both shifted by the track's
//!   `offset`. An edit at `t` takes back the waiting ones whose step is at
//!   or after `t`, but a note-off whose note-on was handed stays; a stop
//!   takes back every waiting one and moves such a note-off to the later of
//!   `t` and its note-on. A second note-on of one output, channel and note
//!   moves the earlier note's waiting note-off to it, sent first. No CC 123
//!   (D4): other notes on a shared channel are not the sequence's.
//! - **Clock out** (D7): Timing Clock on each beat `k / 24` from the
//!   arming's, Start before the first at a beat ≥ 0, Stop at a stop or the
//!   end no earlier than the last tick handed. Ticks are counted, not
//!   listed in the record or the journal one by one.
//!
//! @ref LLP 1125 D6–D8

use crate::agent::num;
use exact_plan::Value;
use std::collections::VecDeque;

/// Reserved resource source, answered by the runner.
pub const SOURCE: &str = "exactMIDI";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &["state", "inputs", "outputs"];
/// A `MIDIPort`'s fields, filled by name.
pub const PORT_FIELDS: &[&str] = &["id", "name", "manufacturer", "state"];
/// How long before its time a host hands a message to its output, ms (D7).
pub const HOLD: f64 = 50.0;
/// Messages that may wait to be handed.
pub const QUEUED: usize = 1024;
/// Messages, and messages heard, that the record keeps (D5).
pub const RECORD: usize = 1024;
/// The most clock ticks a second (a tempo of 2,500): a fill's work stays
/// finite.
pub const TICKS_PER_SECOND: f64 = 1000.0;

/// What `exactMIDI().state` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Not asked yet.
    Prompt,
    /// The ports are the app's.
    Granted,
    /// Refused: by the person, or the app grants no `device.midi`.
    Denied,
    /// No MIDI here (WebKit, tvOS, Linux and Windows).
    Unavailable,
}

impl Access {
    /// Its word.
    pub fn word(self) -> &'static str {
        match self {
            Access::Prompt => "prompt",
            Access::Granted => "granted",
            Access::Denied => "denied",
            Access::Unavailable => "unavailable",
        }
    }

    /// The access a word names.
    pub fn from_word(word: &str) -> Option<Access> {
        Some(match word {
            "prompt" => Access::Prompt,
            "granted" => Access::Granted,
            "denied" => Access::Denied,
            "unavailable" => Access::Unavailable,
            _ => return None,
        })
    }
}

/// One `MIDIPort`.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// Stable across replug: on Apple `kMIDIPropertyUniqueID`.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its manufacturer, or empty.
    pub manufacturer: String,
    /// `connected`, else `disconnected`.
    pub connected: bool,
}

impl Port {
    /// A port the agent names: its name is its id.
    pub fn named(name: &str) -> Port {
        Port {
            id: name.to_string(),
            name: name.to_string(),
            manufacturer: String::new(),
            connected: true,
        }
    }

    /// Its `state` word.
    pub fn state(&self) -> &'static str {
        if self.connected {
            "connected"
        } else {
            "disconnected"
        }
    }

    /// The record of `fields`, by name; `None` for a field it lacks.
    fn value(&self, fields: &[&str]) -> Option<Value> {
        let mut out = Vec::with_capacity(fields.len());
        for f in fields {
            out.push(match *f {
                "id" => Value::str(&self.id),
                "name" => Value::str(&self.name),
                "manufacturer" => Value::str(&self.manufacturer),
                "state" => Value::str(self.state()),
                _ => return None,
            });
        }
        Some(Value::record(out))
    }
}

/// The access and the ports, as a host reports them.
#[derive(Clone, Debug, PartialEq)]
pub struct Ports {
    /// The access.
    pub access: Access,
    /// `MIDIAccess.inputs`.
    pub inputs: Vec<Port>,
    /// `MIDIAccess.outputs`.
    pub outputs: Vec<Port>,
}

impl Ports {
    /// No ports, at `access`.
    pub fn none(access: Access) -> Ports {
        Ports {
            access,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// `reported` over `self`: its ports in its order, then each port seen
    /// before and no longer reported, disconnected, in its order.
    fn merged(&self, mut reported: Ports) -> Ports {
        for (now, was) in [
            (&mut reported.inputs, &self.inputs),
            (&mut reported.outputs, &self.outputs),
        ] {
            for p in was {
                if !now.iter().any(|q| q.id == p.id) {
                    now.push(Port {
                        connected: false,
                        ..p.clone()
                    });
                }
            }
        }
        reported
    }

    /// Whether `id` is an output a message can go to now.
    pub fn connected_output(&self, id: &str) -> bool {
        self.access == Access::Granted && self.outputs.iter().any(|p| p.id == id && p.connected)
    }

    /// Whether `id` is an input that can deliver now.
    pub fn connected_input(&self, id: &str) -> bool {
        self.access == Access::Granted && self.inputs.iter().any(|p| p.id == id && p.connected)
    }

    /// A host's report: `{"state":…,"inputs":[{"id","name","manufacturer",
    /// "state"}…],"outputs":[…]}`; a port's `state` is `connected` unless
    /// it says `disconnected`.
    pub fn from_json(v: &serde_json::Value) -> Result<Ports, String> {
        let access = v
            .get("state")
            .and_then(|s| s.as_str())
            .and_then(Access::from_word)
            .ok_or("midi ports: state is prompt, granted, denied or unavailable")?;
        let list = |key: &str| -> Result<Vec<Port>, String> {
            let Some(items) = v.get(key) else {
                return Ok(Vec::new());
            };
            let items = items
                .as_array()
                .ok_or(format!("midi ports: {key} is a list"))?;
            items
                .iter()
                .map(|p| {
                    let text =
                        |k: &str| p.get(k).and_then(|s| s.as_str()).unwrap_or("").to_string();
                    let id = text("id");
                    if id.is_empty() {
                        return Err(format!("midi ports: a port of {key} has no id"));
                    }
                    Ok(Port {
                        name: text("name"),
                        manufacturer: text("manufacturer"),
                        connected: text("state") != "disconnected",
                        id,
                    })
                })
                .collect()
        };
        Ok(Ports {
            access,
            inputs: list("inputs")?,
            outputs: list("outputs")?,
        })
    }
}

/// The bytes of one or more whole MIDI 1.0 messages, each its own, or why
/// they are not.
pub fn messages(data: &[Value]) -> Result<Vec<Vec<u8>>, String> {
    let mut bytes = Vec::with_capacity(data.len());
    for v in data {
        match v {
            Value::Number(n) if n.fract() == 0.0 && (0.0..=255.0).contains(n) => {
                bytes.push(*n as u8)
            }
            Value::Number(n) => return Err(format!("{} is not a byte", num(*n))),
            _ => return Err("a byte is not a number".into()),
        }
    }
    split(&bytes)
}

/// [`messages`] of bytes.
pub fn split(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if bytes.is_empty() {
        return Err("no message".into());
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let length = length(bytes[i]).map_err(|why| format!("byte {}: {why}", i + 1))?;
        if i + length > bytes.len() {
            return Err(format!(
                "the message at byte {} needs {length} bytes",
                i + 1
            ));
        }
        if let Some(k) = (i + 1..i + length).find(|&k| bytes[k] >= 0x80) {
            return Err(format!(
                "byte {} is {:02x}, not a data byte",
                k + 1,
                bytes[k]
            ));
        }
        out.push(bytes[i..i + length].to_vec());
        i += length;
    }
    Ok(out)
}

/// A message's length from its status byte, or why it has none.
pub fn length(status: u8) -> Result<usize, String> {
    match status {
        0x80..=0xBF | 0xE0..=0xEF | 0xF2 => Ok(3),
        0xC0..=0xDF | 0xF1 | 0xF3 => Ok(2),
        0xF6 | 0xF8 | 0xFA..=0xFC | 0xFE | 0xFF => Ok(1),
        0xF0 | 0xF7 => Err("SysEx is not sent".into()),
        0xF4 | 0xF5 | 0xF9 | 0xFD => Err(format!("{status:02x} is undefined")),
        _ => Err(format!(
            "{status:02x} is a data byte where a status belongs"
        )),
    }
}

/// Bytes as the journal and the driver write them: `90 3c 64`.
pub fn hex(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 3);
    for (i, b) in data.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Whether a message is a note-off (`8n`, or `9n` at velocity 0).
fn note_off(data: &[u8]) -> bool {
    matches!(data, [s, _, v] if s & 0xF0 == 0x80 || (s & 0xF0 == 0x90 && *v == 0))
}

/// A note message's channel and note.
fn note_key(data: &[u8]) -> Option<(u8, u8)> {
    match data {
        [s, n, _] if matches!(s & 0xF0, 0x80 | 0x90) => Some((s & 0x0F, *n)),
        _ => None,
    }
}

/// One message on the queue, as the record keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    /// Counts up from 1 per boot, clock ticks included.
    pub id: u64,
    /// The output's id.
    pub output: String,
    /// One message's bytes.
    pub data: Vec<u8>,
    /// When it is due, runner ms.
    pub at: f64,
    /// A sequence queued it.
    pub seq: bool,
    /// Why it was taken back before it was handed: `edit`, `cancelled` (a
    /// stop) or `overlap` (a later note-on moved it).
    pub by: Option<&'static str>,
    /// The step it is for, before the track's offset; its own time
    /// otherwise.
    step_at: f64,
    /// A clock tick's index (tick `k` is at beat `k / 24`).
    tick: Option<f64>,
    /// A sequence note-off's note-on: its time.
    on_at: Option<f64>,
}

impl Sent {
    /// `waiting`, `handed` or `retracted` at `t`.
    pub fn state(&self, t: f64) -> &'static str {
        match self.by {
            Some(_) => "retracted",
            None if self.at - HOLD > t => "waiting",
            None => "handed",
        }
    }

    /// Whether it is a clock tick.
    pub fn tick(&self) -> bool {
        self.tick.is_some()
    }

    fn waits(&self, t: f64) -> bool {
        self.by.is_none() && self.at - HOLD > t
    }

    fn handed(&self, t: f64) -> bool {
        self.by.is_none() && self.at - HOLD <= t
    }

    /// Exempt from [`QUEUED`]: a note-off, Stop, a tick.
    fn exempt(&self) -> bool {
        self.tick.is_some() || note_off(&self.data) || self.data == [0xFC]
    }

    /// A sequence note-off whose note-on was handed by `t`.
    fn off_of_handed(&self, t: f64) -> bool {
        self.on_at.is_some_and(|on| on - HOLD <= t)
    }
}

/// One message an input delivered and the runner dispatched.
#[derive(Clone, Debug, PartialEq)]
pub struct Received {
    /// The input's id.
    pub input: String,
    /// Its bytes.
    pub data: Vec<u8>,
    /// When it arrived, runner ms: the arm's stamp.
    pub time_stamp: f64,
    /// The transport's beat then, −1 while stopped.
    pub beat: f64,
    /// When it was dispatched, runner ms.
    pub at: f64,
}

/// What a host does with MIDI: the queue's changes, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum MidiOp {
    /// Hand `data` to `output` at runner time `at`, no earlier than
    /// [`HOLD`] before it.
    Send {
        /// The message.
        id: u64,
        /// The output's id.
        output: String,
        /// One message's bytes.
        data: Vec<u8>,
        /// When, runner ms.
        at: f64,
    },
    /// Message `id`, still held, is taken back.
    Retract {
        /// The message.
        id: u64,
    },
    /// Listen to exactly these inputs, by id.
    Open {
        /// The inputs.
        inputs: Vec<String>,
    },
    /// `requestMIDIAccess()`: ask the platform, then report the ports.
    Request,
}

/// A host batch's `{"op":"midi",…}` (LLP 1125 D7): `boot` at a boot whose
/// plan uses MIDI (the host loads its arm, and forgets what an earlier
/// runner left held), then the ops — `{"op":"send","id","output","data":
/// [bytes],"at"}` in runner ms, `{"op":"retract","id"}`, `{"op":"open",
/// "inputs":[…]}`, `{"op":"request"}`. `None` when there is nothing to say.
pub fn batch_op(ops: &[MidiOp], boot: bool) -> Option<String> {
    if ops.is_empty() && !boot {
        return None;
    }
    let mut s = String::from("{\"op\":\"midi\"");
    if boot {
        s.push_str(",\"boot\":true");
    }
    s.push_str(",\"ops\":[");
    for (i, op) in ops.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        match op {
            MidiOp::Send {
                id,
                output,
                data,
                at,
            } => {
                s.push_str(&format!("{{\"op\":\"send\",\"id\":{id},\"output\":"));
                crate::agent::quote(output, &mut s);
                s.push_str(",\"data\":[");
                for (k, b) in data.iter().enumerate() {
                    if k > 0 {
                        s.push(',');
                    }
                    s.push_str(&b.to_string());
                }
                s.push_str(&format!("],\"at\":{}}}", num(*at)));
            }
            MidiOp::Retract { id } => s.push_str(&format!("{{\"op\":\"retract\",\"id\":{id}}}")),
            MidiOp::Open { inputs } => {
                s.push_str("{\"op\":\"open\",\"inputs\":[");
                for (k, input) in inputs.iter().enumerate() {
                    if k > 0 {
                        s.push(',');
                    }
                    crate::agent::quote(input, &mut s);
                }
                s.push_str("]}");
            }
            MidiOp::Request => s.push_str("{\"op\":\"request\"}"),
        }
    }
    s.push_str("]}");
    Some(s)
}

/// Clock out's progress (D7).
#[derive(Clone, Debug)]
struct Clock {
    output: String,
    /// The next tick's index.
    next: f64,
    /// Start was sent.
    started: bool,
}

/// What became of a sequence's note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Queued {
    /// Queued, or dropped and journaled.
    Done,
    /// [`QUEUED`] messages already wait: the sequence stops expanding here.
    Full,
}

/// A message to queue.
struct New<'a> {
    output: &'a str,
    data: Vec<u8>,
    at: f64,
    step_at: f64,
    seq: bool,
    tick: Option<f64>,
    on_at: Option<f64>,
}

/// The ports, the queue and the record.
pub struct Midi {
    ports: Ports,
    /// The app grants `device.midi`.
    granted: bool,
    queue: VecDeque<Sent>,
    next: u64,
    received: VecDeque<Received>,
    heard: u64,
    ops: Vec<MidiOp>,
    /// The inputs a host was last told to open.
    open: Vec<String>,
    clock: Option<Clock>,
    ticks: u64,
    /// The last tick handed and let go from the queue: its index and time.
    played: Option<(f64, f64)>,
    /// When the `exactMIDI` readers are answered again.
    pub(crate) answer_due: f64,
    /// The resources reading `exactMIDI`.
    pub(crate) readers: Vec<usize>,
    /// The agent's environment (`prefer midi.*`): what a request answers.
    preferred: Ports,
}

impl Midi {
    /// The queue for a plan whose `exactMIDI` readers are `readers`;
    /// `granted` when the app grants `device.midi`.
    pub fn new(readers: Vec<usize>, granted: bool) -> Midi {
        Midi {
            ports: Ports::none(if granted {
                Access::Prompt
            } else {
                Access::Denied
            }),
            granted,
            queue: VecDeque::new(),
            next: 0,
            received: VecDeque::new(),
            heard: 0,
            ops: Vec::new(),
            open: Vec::new(),
            clock: None,
            ticks: 0,
            played: None,
            answer_due: f64::INFINITY,
            readers,
            preferred: Ports::none(Access::Granted),
        }
    }

    /// The ports as `exactMIDI()` answers them.
    pub fn ports(&self) -> &Ports {
        &self.ports
    }

    /// The queue's record, oldest first, clock ticks left out.
    pub fn sent(&self) -> impl DoubleEndedIterator<Item = &Sent> {
        self.queue.iter().filter(|s| s.tick.is_none())
    }

    /// Messages ever queued, clock ticks left out.
    pub fn recorded(&self) -> u64 {
        self.next - self.ticks
    }

    /// What the inputs delivered, oldest first, and how many ever.
    pub fn received(&self) -> (impl DoubleEndedIterator<Item = &Received>, u64) {
        (self.received.iter(), self.heard)
    }

    /// The inputs a host was last told to open.
    pub fn open(&self) -> &[String] {
        &self.open
    }

    /// Clock out's output while it plays, and the ticks ever queued.
    pub fn clock(&self) -> (Option<&str>, u64) {
        (self.clock.as_ref().map(|c| c.output.as_str()), self.ticks)
    }

    /// Messages waiting at `t`, the exempt left out.
    pub fn waiting(&self, t: f64) -> usize {
        self.queue
            .iter()
            .filter(|s| s.waits(t) && !s.exempt())
            .count()
    }

    /// When the first message waiting at `t` (the exempt left out) is
    /// handed, freeing a place.
    pub(crate) fn next_handed(&self, t: f64) -> Option<f64> {
        self.queue
            .iter()
            .filter(|s| s.waits(t) && !s.exempt())
            .map(|s| s.at - HOLD)
            .reduce(f64::min)
    }

    /// The ops since the last take.
    pub fn take(&mut self) -> Vec<MidiOp> {
        std::mem::take(&mut self.ops)
    }

    /// The inputs a host should listen to now; an `Open` when they changed.
    pub(crate) fn set_open(&mut self, inputs: Vec<String>) {
        if inputs != self.open {
            self.ops.push(MidiOp::Open {
                inputs: inputs.clone(),
            });
            self.open = inputs;
        }
    }

    /// One `exactMIDI` field, for a reader's shape: a port list's records
    /// carry `port_fields`.
    pub fn field(&self, name: &str, port_fields: &[&str]) -> Option<Value> {
        let list = |ports: &[Port]| -> Option<Value> {
            let items: Option<Vec<Value>> = ports.iter().map(|p| p.value(port_fields)).collect();
            items.map(Value::list)
        };
        match name {
            "state" => Some(Value::str(self.ports.access.word())),
            "inputs" => list(&self.ports.inputs),
            "outputs" => list(&self.ports.outputs),
            _ => None,
        }
    }

    /// What a host reported, merged over what was seen; `false` when
    /// nothing changed. Without the grant nothing is taken (rulings #5).
    pub(crate) fn set_ports(&mut self, reported: Ports, log: &mut Vec<String>) -> bool {
        if !self.granted {
            return false;
        }
        let ports = self.ports.merged(reported);
        if ports == self.ports {
            return false;
        }
        let count = |ports: &[Port]| ports.iter().filter(|p| p.connected).count();
        log.push(format!(
            "midi ports: {}, inputs {} of {}, outputs {} of {} connected",
            ports.access.word(),
            count(&ports.inputs),
            ports.inputs.len(),
            count(&ports.outputs),
            ports.outputs.len()
        ));
        self.ports = ports;
        true
    }

    /// The agent's `prefer midi.*`: the environment a request answers with,
    /// applied at once when access is granted (a port plugged in or pulled)
    /// or `state` was named. Whether `exactMIDI` changed.
    pub(crate) fn prefer(
        &mut self,
        access: Option<Access>,
        inputs: Option<Vec<Port>>,
        outputs: Option<Vec<Port>>,
        log: &mut Vec<String>,
    ) -> bool {
        if let Some(a) = access {
            self.preferred.access = a;
        }
        if let Some(i) = inputs {
            self.preferred.inputs = i;
        }
        if let Some(o) = outputs {
            self.preferred.outputs = o;
        }
        if access.is_some() || self.ports.access == Access::Granted {
            return self.set_ports(self.answered(), log);
        }
        false
    }

    /// The agent's environment as a request finds it: its state, and its
    /// ports when that is granted.
    fn answered(&self) -> Ports {
        match self.preferred.access {
            Access::Granted => self.preferred.clone(),
            a => Ports::none(a),
        }
    }

    /// `requestMIDIAccess()` at `t`, once its commit stood: refused without
    /// the grant; on a seek clock (the agent, a test) answered at once from
    /// the agent's environment (rulings #3), else asked of the host.
    pub(crate) fn request(&mut self, t: f64, seek: bool, log: &mut Vec<String>) {
        if !self.granted {
            log.push("midi refused: the grants name no device.midi (requestMIDIAccess)".into());
            return;
        }
        if seek {
            log.push(format!(
                "midi access: {} (agent)",
                self.preferred.access.word()
            ));
            if self.set_ports(self.answered(), log) {
                self.answer_due = self.answer_due.min(t);
            }
        } else {
            self.ops.push(MidiOp::Request);
        }
    }

    /// One `sendMIDI(output, data, at)` at its commit's time `t`.
    pub(crate) fn send(&mut self, args: &[Value], t: f64, log: &mut Vec<String>) {
        let output = match args.first() {
            Some(v) if v.is_str() => v.text().to_string(),
            _ => return log.push("midi dropped: the output is not a string (sendMIDI)".into()),
        };
        let data = match args.get(1) {
            Some(Value::List(items)) => messages(items),
            _ => Err("the data is not a list".into()),
        };
        let data = match data {
            Ok(d) => d,
            Err(why) => return log.push(format!("midi dropped: {why} (sendMIDI to {output})")),
        };
        let asked = match args.get(2) {
            None | Some(Value::Option(None)) => t,
            Some(Value::Number(n)) => *n,
            Some(Value::Option(Some(v))) => match **v {
                Value::Number(n) => n,
                _ => f64::NAN,
            },
            _ => f64::NAN,
        };
        if !asked.is_finite() {
            return log.push(format!(
                "midi dropped: at is not a number (sendMIDI to {output})"
            ));
        }
        if !self.ports.connected_output(&output) {
            let why = if self.ports.access != Access::Granted {
                format!("access is {}", self.ports.access.word())
            } else {
                format!("{output} is not a connected output")
            };
            return log.push(format!("midi dropped: {why} (sendMIDI)"));
        }
        let at = asked.max(t);
        let note = if asked < t {
            format!(" (asked {})", num(asked))
        } else {
            String::new()
        };
        for data in data {
            if !(note_off(&data) || data == [0xFC]) && self.waiting(t) >= QUEUED {
                log.push(format!(
                    "midi dropped: {QUEUED} messages wait (sendMIDI to {output} {})",
                    hex(&data)
                ));
                continue;
            }
            self.push(
                New {
                    output: &output,
                    data,
                    at,
                    step_at: at,
                    seq: false,
                    tick: None,
                    on_at: None,
                },
                t,
                &note,
                log,
            );
        }
    }

    /// Queue one message, journaled unless a tick; its id.
    fn push(&mut self, m: New<'_>, t: f64, note: &str, log: &mut Vec<String>) -> u64 {
        self.next += 1;
        let id = self.next;
        if m.tick.is_none() {
            log.push(format!(
                "midi {id} to {} {} at {}{note}",
                m.output,
                hex(&m.data),
                num(m.at)
            ));
        } else {
            self.ticks += 1;
        }
        self.ops.push(MidiOp::Send {
            id,
            output: m.output.to_string(),
            data: m.data.clone(),
            at: m.at,
        });
        self.queue.push_back(Sent {
            id,
            output: m.output.to_string(),
            data: m.data,
            at: m.at,
            seq: m.seq,
            by: None,
            step_at: m.step_at,
            tick: m.tick,
            on_at: m.on_at,
        });
        self.trim(t);
        id
    }

    /// Take back message `i` of the queue (`by`).
    fn retract(&mut self, i: usize, by: &'static str) {
        let s = &mut self.queue[i];
        s.by = Some(by);
        self.ops.push(MidiOp::Retract { id: s.id });
    }

    /// Keep the last [`RECORD`] messages, letting the oldest that no longer
    /// waits go first; a tick goes once it is handed.
    fn trim(&mut self, t: f64) {
        let mut i = 0;
        while i < self.queue.len() {
            let s = &self.queue[i];
            if s.tick.is_some() && s.handed(t) {
                let (k, at) = (s.tick.unwrap_or(0.0), s.at);
                if self.played.is_none_or(|(_, p)| p <= at) {
                    self.played = Some((k, at));
                }
                self.queue.remove(i);
            } else if s.tick.is_some() && s.by.is_some() {
                self.queue.remove(i);
            } else {
                i += 1;
            }
        }
        while self.queue.len() > RECORD {
            let Some(i) = self.queue.iter().position(|s| !s.waits(t)) else {
                return;
            };
            self.queue.remove(i);
        }
    }

    /// Whether a sequence's notes and clock can go to `output` now.
    pub(crate) fn can_send(&self, output: &str) -> bool {
        self.ports.connected_output(output)
    }

    /// One sequence note on `output` (LLP 1125 D7): `on` at `on_at`, its
    /// note-off at `off_at`, for the step at `step_at`, queued at `t`. A
    /// waiting note-off of the same output, channel and note that ends
    /// after `on_at` is moved to it first (rulings #7).
    #[allow(clippy::too_many_arguments)] // a note's fields
    pub(crate) fn note(
        &mut self,
        output: &str,
        on: [u8; 3],
        on_at: f64,
        off_at: f64,
        step_at: f64,
        t: f64,
        log: &mut Vec<String>,
    ) -> Queued {
        if self.waiting(t) >= QUEUED {
            return Queued::Full;
        }
        let key = note_key(&on);
        let moved: Vec<usize> = (0..self.queue.len())
            .filter(|&i| {
                let s = &self.queue[i];
                s.seq
                    && s.waits(t)
                    && s.output == output
                    && note_off(&s.data)
                    && note_key(&s.data) == key
                    && s.at > on_at
                    && s.on_at.is_some_and(|o| o <= on_at)
            })
            .collect();
        if !moved.is_empty() {
            log.push(format!("midi retracted: {} (overlap)", moved.len()));
        }
        // Taken back first: a push may let a handed tick go, moving places.
        let moved: Vec<Sent> = moved
            .into_iter()
            .map(|i| {
                self.retract(i, "overlap");
                self.queue[i].clone()
            })
            .collect();
        for s in moved {
            self.push(
                New {
                    output,
                    data: s.data,
                    at: on_at,
                    step_at: s.step_at,
                    seq: true,
                    tick: None,
                    on_at: s.on_at,
                },
                t,
                "",
                log,
            );
        }
        let off = [0x80 | (on[0] & 0x0F), on[1], 0];
        for (data, at, on_at) in [(on, on_at, None), (off, off_at, Some(on_at))] {
            self.push(
                New {
                    output,
                    data: data.to_vec(),
                    at,
                    step_at,
                    seq: true,
                    tick: None,
                    on_at,
                },
                t,
                "",
                log,
            );
        }
        Queued::Done
    }

    /// An edit at `t` (LLP 1125 D3, D7): every waiting message the sequence
    /// queued for a step at or after `t` is taken back, but a note-off whose
    /// note-on was handed stays (rulings #6); the clock goes on from its
    /// first tick taken back.
    pub(crate) fn retract_seq(&mut self, t: f64, log: &mut Vec<String>) {
        let mut n = 0;
        for i in 0..self.queue.len() {
            let s = &self.queue[i];
            if s.seq && s.waits(t) && s.step_at >= t && !s.off_of_handed(t) {
                if let (Some(k), Some(c)) = (s.tick, self.clock.as_mut()) {
                    c.next = c.next.min(k);
                } else {
                    n += 1;
                }
                self.retract(i, "edit");
            }
        }
        if n > 0 {
            log.push(format!("midi retracted: {n} (edit)"));
        }
    }

    /// A stop at `t` (D4): every waiting message the sequence queued is
    /// taken back (`cancelled`), a note-off whose note-on was handed moved
    /// to the later of `t` and its note-on; clock out sends Stop.
    pub(crate) fn stop_seq(&mut self, t: f64, log: &mut Vec<String>) {
        let (mut n, mut moved) = (0, Vec::new());
        for i in 0..self.queue.len() {
            let s = &self.queue[i];
            if !(s.seq && s.waits(t)) {
                continue;
            }
            if s.off_of_handed(t) {
                moved.push(i);
            } else if s.tick.is_none() {
                n += 1;
            }
            self.retract(i, "cancelled");
        }
        if n + moved.len() > 0 {
            log.push(format!("midi retracted: {} (cancelled)", n + moved.len()));
        }
        let moved: Vec<Sent> = moved.into_iter().map(|i| self.queue[i].clone()).collect();
        for s in moved {
            let on = s.on_at.unwrap_or(t);
            self.push(
                New {
                    output: &s.output,
                    data: s.data,
                    at: t.max(on),
                    step_at: s.step_at,
                    seq: true,
                    tick: None,
                    on_at: s.on_at,
                },
                t,
                "",
                log,
            );
        }
        self.stop_clock(t, log);
    }

    /// Clock out ends at `t`: Stop no earlier than the last tick handed,
    /// once Start was sent (rulings #9).
    fn stop_clock(&mut self, t: f64, log: &mut Vec<String>) {
        let Some(c) = self.clock.take() else {
            return;
        };
        for i in 0..self.queue.len() {
            if self.queue[i].tick.is_some() && self.queue[i].waits(t) {
                self.retract(i, "cancelled");
            }
        }
        if !c.started {
            return;
        }
        let last = self
            .queue
            .iter()
            .filter(|s| s.tick.is_some() && s.handed(t) && s.output == c.output)
            .map(|s| s.at)
            .chain(self.played.map(|(_, at)| at))
            .fold(t, f64::max);
        self.push(
            New {
                output: &c.output,
                data: vec![0xFC],
                at: last,
                step_at: last,
                seq: true,
                tick: None,
                on_at: None,
            },
            t,
            "",
            log,
        );
    }

    /// The last finite track ended at `t` (`ended=`): clock out sends Stop;
    /// note-offs still waiting stay.
    pub(crate) fn end_seq(&mut self, t: f64, log: &mut Vec<String>) {
        self.stop_clock(t, log);
    }

    /// Clock out to `output` from tick `first` (`""` for none), at an
    /// arming or an edit: a new output stops the old one's clock first.
    pub(crate) fn clock_to(&mut self, output: &str, first: f64, t: f64, log: &mut Vec<String>) {
        if self.clock.as_ref().is_some_and(|c| c.output == output) {
            return;
        }
        let next = self.clock.as_ref().map_or(first, |c| c.next);
        self.stop_clock(t, log);
        if !output.is_empty() {
            self.clock = Some(Clock {
                output: output.to_string(),
                next,
                started: false,
            });
        }
    }

    /// Queue clock out's ticks due before `to` under `time` (a beat's time),
    /// before beat `end` when the take is finite; one before `late` (a
    /// stalled live clock's fill) is skipped. Start goes before the first
    /// tick at a beat ≥ 0. A journal line says how many.
    pub(crate) fn ticks(
        &mut self,
        time: impl Fn(f64) -> f64,
        to: f64,
        end: Option<f64>,
        late: f64,
        t: f64,
        log: &mut Vec<String>,
    ) {
        let Some(mut c) = self.clock.take() else {
            return;
        };
        let can = self.can_send(&c.output);
        let (mut n, mut last) = (0, f64::NAN);
        loop {
            let k = c.next;
            let at = time(k / 24.0);
            if at.is_nan() || at >= to || end.is_some_and(|e| k / 24.0 >= e) {
                break;
            }
            c.next += 1.0;
            if !can || at < late {
                continue;
            }
            if !c.started && k >= 0.0 {
                c.started = true;
                self.push(
                    New {
                        output: &c.output,
                        data: vec![0xFA],
                        at,
                        step_at: at,
                        seq: true,
                        tick: None,
                        on_at: None,
                    },
                    t,
                    "",
                    log,
                );
            }
            self.push(
                New {
                    output: &c.output,
                    data: vec![0xF8],
                    at,
                    step_at: at,
                    seq: true,
                    tick: Some(k),
                    on_at: None,
                },
                t,
                "",
                log,
            );
            (n, last) = (n + 1, at);
        }
        if n > 0 {
            log.push(format!(
                "midi clock: {n} ticks to {} through {}",
                c.output,
                num(last)
            ));
        }
        self.clock = Some(c);
    }

    /// This runner is about to be replaced (a dev reload): the sequence's
    /// waiting messages go as at a stop, and every other waiting note-off is
    /// handed now; the rest is taken back.
    pub(crate) fn reload(&mut self, t: f64, log: &mut Vec<String>) {
        self.stop_seq(t, log);
        let (mut n, mut moved) = (0, Vec::new());
        for i in 0..self.queue.len() {
            if self.queue[i].waits(t) {
                if note_off(&self.queue[i].data) {
                    moved.push(i);
                }
                self.retract(i, "cancelled");
                n += 1;
            }
        }
        if n > 0 {
            log.push(format!("midi retracted: {n} (reload)"));
        }
        let moved: Vec<Sent> = moved.into_iter().map(|i| self.queue[i].clone()).collect();
        for s in moved {
            self.push(
                New {
                    output: &s.output,
                    data: s.data,
                    at: t,
                    step_at: t,
                    seq: s.seq,
                    tick: None,
                    on_at: None,
                },
                t,
                "",
                log,
            );
        }
    }

    /// Record what an input delivered.
    pub(crate) fn heard(&mut self, r: Received) {
        self.heard += 1;
        self.received.push_back(r);
        if self.received.len() > RECORD {
            self.received.pop_front();
        }
    }
}

/// One message an input delivered, as a host hands it over.
#[derive(Clone, Debug, PartialEq)]
pub struct Incoming {
    /// The input's id.
    pub input: String,
    /// When it arrived, runner ms.
    pub time_stamp: f64,
    /// Its bytes.
    pub data: Vec<u8>,
}

/// One host delivery's messages, coalesced (D8; rulings #2): a Control
/// Change, pitch bend, channel pressure or polyphonic aftertouch keeps only
/// the latest of its input, channel and controller or note, never across a
/// note or transport message of the same input and channel (a transport
/// message has none, so it bounds every channel of its input). Clock and
/// Active Sensing are left out (they are clock in's, stage 4); what is not
/// one whole message is dropped and journaled. Returns the messages to
/// dispatch, in order, and how many each input coalesced.
pub fn coalesce(batch: Vec<Incoming>, log: &mut Vec<String>) -> Vec<Incoming> {
    let mut out: Vec<Option<Incoming>> = Vec::with_capacity(batch.len());
    // (input, status kind, channel, controller or note) → its place in `out`.
    let mut latest: Vec<((String, u8, u8, u8), usize)> = Vec::new();
    let mut coalesced: Vec<(String, usize)> = Vec::new();
    for m in batch {
        match split(&m.data) {
            Ok(parts) if parts.len() == 1 => {}
            Ok(_) => {
                log.push(format!(
                    "midi in dropped: one message at a time ({})",
                    m.input
                ));
                continue;
            }
            Err(why) => {
                log.push(format!("midi in dropped: {why} ({})", m.input));
                continue;
            }
        }
        let status = m.data[0];
        if matches!(status, 0xF8 | 0xFE) {
            continue;
        }
        let (kind, channel) = (status & 0xF0, status & 0x0F);
        let key = match kind {
            0xB0 | 0xA0 => Some((m.input.clone(), kind, channel, m.data[1])),
            0xE0 | 0xD0 => Some((m.input.clone(), kind, channel, 0)),
            _ => None,
        };
        if let Some(key) = key {
            if let Some(slot) = latest.iter().position(|(k, _)| *k == key) {
                out[latest[slot].1] = None;
                latest[slot].1 = out.len();
                match coalesced.iter_mut().find(|(i, _)| *i == m.input) {
                    Some((_, n)) => *n += 1,
                    None => coalesced.push((m.input.clone(), 1)),
                }
            } else {
                latest.push((key, out.len()));
            }
        } else if status < 0xF0 {
            // A note, program change: bounds its input and channel.
            latest.retain(|((input, _, ch, _), _)| !(*input == m.input && *ch == channel));
        } else {
            latest.retain(|((input, ..), _)| *input != m.input);
        }
        out.push(Some(m));
    }
    for (input, n) in coalesced {
        log.push(format!("midi coalesced: {n} ({input})"));
    }
    out.into_iter().flatten().collect()
}

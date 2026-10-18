//! MIDI in the runner's commits (LLP 1125 D6–D8): what a host reports of
//! the ports re-answers `exactMIDI`'s readers in one commit, as a page fact
//! does; a host delivery of input messages is coalesced, and each message
//! is dispatched to every mounted element that hears its input, one commit
//! per element in tree order, at the same instant (rulings #1); the queue's
//! ops go to the host through [`Runner::take_midi`].

use super::{CommitReceipt, DataError, DataSource, Event, Runner, RunnerError};
use crate::midi::{coalesce, Access, Incoming, Midi, Port, Ports, Received, SOURCE};
use exact_kernel::{PropId, ViewId};
use exact_plan::{EventKind, TypeKind, Value};

/// The queue for `plan`, its readers found, its access by the grant.
pub(super) fn queue<D: DataSource>(plan: &exact_plan::Plan, data: &D) -> Midi {
    let readers = (0..plan.resources.len())
        .filter(|&i| plan.str(plan.resources[i].source) == SOURCE)
        .collect();
    Midi::new(readers, crate::device::granted(data.grants(), "midi"))
}

/// Whether `data` is one whole message clock in reads (LLP 1125 D9):
/// Timing Clock, Start, Continue, Stop, or Song Position.
fn clock_message(data: &[u8]) -> bool {
    match data {
        [0xF8 | 0xFA | 0xFB | 0xFC] => true,
        [0xF2, lsb, msb] => *lsb < 0x80 && *msb < 0x80,
        _ => false,
    }
}

/// What a `midimessage` handler's action hears (LLP 1125 D8): Web MIDI's
/// `MIDIMessageEvent`, with the input, runner time and the transport's beat.
#[derive(Clone, Debug, PartialEq)]
pub struct MidiMessage {
    /// The input's id.
    pub input: String,
    /// The message's bytes.
    pub data: Vec<u8>,
    /// When it arrived, runner ms.
    pub time_stamp: f64,
    /// The transport's beat then, −1 while stopped.
    pub beat: f64,
}

impl MidiMessage {
    /// `MIDIMessageEvent`'s record, in `contract_types`' order.
    pub fn value(&self) -> Value {
        Value::record(vec![
            Value::str(&self.input),
            Value::list(
                self.data
                    .iter()
                    .map(|b| Value::Number(f64::from(*b)))
                    .collect(),
            ),
            Value::Number(self.time_stamp),
            Value::Number(self.beat),
        ])
    }
}

impl<D: DataSource> Runner<D> {
    /// The MIDI queue and record, for `state.midi` and a test.
    pub fn midi(&self) -> &Midi {
        &self.midi
    }

    /// `exactMIDI()`'s answer, by the declared fields and, in its port
    /// lists, the `MIDIPort` shape's.
    pub(super) fn midi_answer(&self, i: usize) -> Result<Value, DataError> {
        let row = &self.plan.resources[i];
        if row.args.len > 0 {
            return Err(DataError::BadArguments(format!(
                "{SOURCE} takes no arguments"
            )));
        }
        let ty = self.plan.type_(row.ty);
        if ty.kind != TypeKind::Record {
            return Err(DataError::Unavailable(format!(
                "{SOURCE} answers a record; this one is declared `{}`",
                self.plan.str(ty.name)
            )));
        }
        let mut fields = Vec::with_capacity(ty.fields.len as usize);
        for f in ty.fields.iter() {
            let field = self.plan.field(f);
            let name = self.plan.str(field.name);
            // A port list's records carry the fields its item shape names.
            let item = self.plan.type_(field.ty);
            let item = match item.kind {
                TypeKind::List => item.elem.map(|e| self.plan.type_(e)),
                _ => None,
            };
            let port_fields: Vec<&str> = item
                .filter(|t| t.kind == TypeKind::Record)
                .map(|t| {
                    t.fields
                        .iter()
                        .map(|f| self.plan.str(self.plan.field(f).name))
                        .collect()
                })
                .unwrap_or_default();
            fields.push(self.midi.field(name, &port_fields).ok_or_else(|| {
                DataError::Unavailable(format!("{SOURCE} has no field `{name}`"))
            })?);
        }
        Ok(Value::record(fields))
    }

    /// What a host reported of the ports (LLP 1125 D6): when `exactMIDI`
    /// changed, its readers are asked again in one commit.
    pub fn set_midi(&mut self, ports: Ports) -> Result<Option<CommitReceipt>, RunnerError> {
        let mut lines = Vec::new();
        let changed = self.midi.set_ports(ports, &mut lines);
        lines.into_iter().for_each(|l| self.log(l));
        if !changed {
            return Ok(None);
        }
        let readers = self.midi.readers.clone();
        self.recommit(readers, "midi")
    }

    /// The `exactMIDI` readers answered again by a commit of their own (a
    /// request the agent answered), as `exactTransport`'s are.
    pub(super) fn midi_answered(&mut self) -> Result<CommitReceipt, RunnerError> {
        self.midi.answer_due = f64::INFINITY;
        let readers = self.midi.readers.clone();
        self.commit_again(readers, "midi")
    }

    /// The elements that hear `input` now, in tree order: mounted, with a
    /// `midimessage` handler, and `midiinputs` naming it.
    fn midi_listeners(&self, input: Option<&str>) -> Vec<(ViewId, Vec<String>)> {
        if !self.kernel.has_prop(PropId::MidiInputs) {
            return Vec::new();
        }
        let views = self
            .kernel
            .preorder_where(&self.kernel.roots(), |_, props| {
                props.str(PropId::MidiInputs).is_some()
            });
        views
            .into_iter()
            .filter_map(|view| {
                let (node, _) = self.find(view)?;
                let heard = self
                    .plan
                    .node(node)
                    .handlers
                    .iter()
                    .any(|h| self.plan.handler(h).event == EventKind::Midimessage);
                let inputs: Vec<String> = self
                    .kernel
                    .node(view)?
                    .props
                    .str(PropId::MidiInputs)?
                    .split('\n')
                    .filter(|i| !i.is_empty())
                    .map(str::to_string)
                    .collect();
                (heard && input.is_none_or(|i| inputs.iter().any(|x| x == i)))
                    .then_some((view, inputs))
            })
            .collect()
    }

    /// The inputs the mounted listeners name, each once, in tree order, then
    /// the input a sequence follows (LLP 1125 D9), opened with or without a
    /// listener.
    pub fn midi_inputs(&self) -> Vec<String> {
        let mut open: Vec<String> = Vec::new();
        for (_, inputs) in self.midi_listeners(None) {
            for i in inputs {
                if !open.contains(&i) {
                    open.push(i);
                }
            }
        }
        if let Some(i) = self.transport.clock_input() {
            if !open.iter().any(|o| o == i) {
                open.push(i.to_string());
            }
        }
        open
    }

    /// [`Runner::midi_inputs`] for the host, with the input whose clock it
    /// forwards: an `Open` when they changed.
    pub(super) fn midi_open(&mut self) {
        let open = self.midi_inputs();
        let clock = self.transport.clock_input().map(str::to_string);
        self.midi.set_open(open, clock.into_iter().collect());
    }

    /// What MIDI does since the last take: sends and retractions, the
    /// inputs to open, a request (LLP 1125 D7).
    pub fn take_midi(&mut self) -> Vec<crate::midi::MidiOp> {
        self.midi_open();
        self.midi.take()
    }

    /// One host delivery of input messages (D8): the followed input's clock
    /// and transport messages to the transport first, in arrival order (D9),
    /// then the delivery coalesced and each message dispatched, in arrival
    /// order, to every element that hears its input, a commit each; one no
    /// element hears is dropped unrecorded. A refusal does not stop the rest
    /// (a note-off still arrives); the first is returned.
    pub fn midi_input(
        &mut self,
        batch: Vec<Incoming>,
    ) -> (Vec<CommitReceipt>, Option<RunnerError>) {
        let clock: Vec<(f64, Vec<u8>)> = match self.transport.clock_input() {
            Some(input) if self.midi.ports().connected_input(input) => batch
                .iter()
                .filter(|m| m.input == input && clock_message(&m.data))
                .map(|m| (m.time_stamp, m.data.clone()))
                .collect(),
            _ => Vec::new(),
        };
        if !clock.is_empty() {
            // The delivery's runner time is the host's now, not the last commit's.
            self.sample();
            self.transport_clock(&clock);
        }
        let mut lines = Vec::new();
        let batch = coalesce(batch, &mut lines);
        lines.into_iter().for_each(|l| self.log(l));
        let (mut receipts, mut first) = (Vec::new(), None);
        for m in batch {
            if !self.midi.ports().connected_input(&m.input) {
                self.log(format!(
                    "midi in dropped: {} is not a connected input",
                    m.input
                ));
                continue;
            }
            let listeners = self.midi_listeners(Some(&m.input));
            if listeners.is_empty() {
                continue;
            }
            let beat = self.transport.beat_at(m.time_stamp);
            let line = format!(
                "midi in {} {} at {} beat {}",
                m.input,
                crate::midi::hex(&m.data),
                crate::agent::num(m.time_stamp),
                crate::agent::num(beat)
            );
            self.log(line);
            self.midi.heard(Received {
                input: m.input.clone(),
                data: m.data.clone(),
                time_stamp: m.time_stamp,
                beat,
                at: self.now_ms,
            });
            let message = MidiMessage {
                input: m.input,
                data: m.data,
                time_stamp: m.time_stamp,
                beat,
            };
            for (view, _) in listeners {
                // An earlier handler's commit may have unmounted it.
                if self.find(view).is_none() {
                    continue;
                }
                match self.dispatch(view, Event::MidiMessage(message.clone())) {
                    Ok(r) => receipts.push(r),
                    Err(e) => {
                        first.get_or_insert(e);
                    }
                }
            }
        }
        (receipts, first)
    }

    /// A host's message (its MIDI arm's, or the agent's on its behalf):
    /// `{"ports":{…}}` ([`Ports::from_json`]) or `{"messages":[[input,
    /// timeStamp, [bytes…]], …]}`, one delivery. The receipts and the first
    /// refusal, or why the message is none.
    pub fn midi_host(
        &mut self,
        json: &str,
    ) -> Result<(Vec<CommitReceipt>, Option<RunnerError>), String> {
        let v: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("midi: not JSON: {e}"))?;
        if let Some(p) = v.get("ports") {
            let ports = Ports::from_json(p)?;
            return Ok(match self.set_midi(ports) {
                Ok(r) => (r.into_iter().collect(), None),
                Err(e) => (Vec::new(), Some(e)),
            });
        }
        let Some(list) = v.get("messages").and_then(|m| m.as_array()) else {
            return Err("midi: a host says ports or messages".into());
        };
        let mut batch = Vec::with_capacity(list.len());
        for m in list {
            let (Some(input), Some(time_stamp), Some(bytes)) = (
                m.get(0).and_then(|i| i.as_str()),
                m.get(1).and_then(|t| t.as_f64()),
                m.get(2).and_then(|b| b.as_array()),
            ) else {
                return Err("midi: a message is [input, timeStamp, [bytes…]]".into());
            };
            let data: Option<Vec<u8>> = bytes
                .iter()
                .map(|b| b.as_u64().filter(|b| *b < 256).map(|b| b as u8))
                .collect();
            batch.push(Incoming {
                input: input.to_string(),
                time_stamp,
                data: data.ok_or("midi: a byte is a whole number 0–255")?,
            });
        }
        Ok(self.midi_input(batch))
    }

    /// The agent's MIDI (LLP 1125 §4): `{"op":"prefer","midi":{"state"?,
    /// "inputs"?, "outputs"?}}`, the ports a request finds, by name; and
    /// `{"op":"midi","input":…,"data":[…]}`, a message from that input now
    /// (`tap midi`). `None` for any other request; else the receipts and
    /// the reply, or the refusal.
    pub fn midi_agent(
        &mut self,
        request: &str,
    ) -> Option<(Vec<CommitReceipt>, Result<String, String>)> {
        let v: serde_json::Value = serde_json::from_str(request).ok()?;
        match v.get("op").and_then(|o| o.as_str()) {
            Some("prefer") => {
                let m = v.get("midi")?;
                Some(self.midi_prefer(m))
            }
            Some("midi") => Some(self.midi_tap(&v)),
            _ => None,
        }
    }

    fn midi_prefer(
        &mut self,
        m: &serde_json::Value,
    ) -> (Vec<CommitReceipt>, Result<String, String>) {
        let access = match m.get("state") {
            None => None,
            Some(s) => match s.as_str().and_then(Access::from_word) {
                Some(a) => Some(a),
                None => {
                    return (
                        Vec::new(),
                        Err("prefer midi.state: prompt, granted, denied or unavailable".into()),
                    )
                }
            },
        };
        let names = |key: &str| -> Option<Vec<Port>> {
            let list = m.get(key)?.as_array()?;
            Some(
                list.iter()
                    .filter_map(|n| n.as_str())
                    .map(Port::named)
                    .collect(),
            )
        };
        let mut lines = Vec::new();
        let changed = self
            .midi
            .prefer(access, names("inputs"), names("outputs"), &mut lines);
        lines.into_iter().for_each(|l| self.log(l));
        let receipts = if changed {
            let readers = self.midi.readers.clone();
            match self.recommit(readers, "midi") {
                Ok(r) => r.into_iter().collect(),
                Err(e) => return (Vec::new(), Err(format!("prefer midi: {e:?}"))),
            }
        } else {
            Vec::new()
        };
        (receipts, Ok(self.midi_reply()))
    }

    /// `tap midi "<input>" 90 3c 64`: one delivery of the messages the bytes
    /// make, refused by name unless they are whole, the input is connected
    /// and a mounted element hears it or a sequence follows it. With
    /// `"clock":[[stamp, byte, (lsb, msb)?]…]` instead (the driver's `tap
    /// midi "<input>" clock <bpm> <ms>`, a slice at a time), one delivery of
    /// clock and transport messages at those stamps (LLP 1125 D9).
    fn midi_tap(&mut self, v: &serde_json::Value) -> (Vec<CommitReceipt>, Result<String, String>) {
        let input = v.get("input").and_then(|i| i.as_str()).unwrap_or("");
        if let Some(list) = v.get("clock") {
            return self.midi_tap_clock(input, list);
        }
        let data: Option<Vec<u8>> = v.get("data").and_then(|d| d.as_array()).and_then(|d| {
            d.iter()
                .map(|b| b.as_u64().filter(|b| *b < 256).map(|b| b as u8))
                .collect()
        });
        let Some(data) = data.filter(|d| !d.is_empty()) else {
            return (
                Vec::new(),
                Err("tap midi: the bytes are whole numbers 0–255".into()),
            );
        };
        // One delivery of one or more whole messages, coalesced as an arm's.
        let messages = match crate::midi::split(&data) {
            Ok(m) => m,
            Err(why) => return (Vec::new(), Err(format!("tap midi: {why}"))),
        };
        if let Some(why) = self.midi_unconnected(input) {
            return (Vec::new(), Err(why));
        }
        if self.midi_listeners(Some(input)).is_empty()
            && self.transport.clock_input() != Some(input)
        {
            return (
                Vec::new(),
                Err(format!(
                    "tap midi: no mounted element with a midimessage handler lists {input:?} in midiinputs"
                )),
            );
        }
        let now = self.now_ms;
        let batch = messages
            .into_iter()
            .map(|data| Incoming {
                input: input.to_string(),
                time_stamp: now,
                data,
            })
            .collect();
        let (receipts, error) = self.midi_input(batch);
        match error {
            Some(e) => (receipts, Err(format!("tap midi: {e:?}"))),
            None => (receipts, Ok(self.midi_reply())),
        }
    }

    /// Why `tap midi` cannot deliver from `input`, unless it can.
    fn midi_unconnected(&self, input: &str) -> Option<String> {
        if self.midi.ports().connected_input(input) {
            return None;
        }
        let why = match self.midi.ports().access {
            Access::Granted => format!("{input:?} is not a connected input (prefer midi.inputs)"),
            a => format!(
                "MIDI access is {} (requestMIDIAccess, prefer midi.state)",
                a.word()
            ),
        };
        Some(format!("tap midi: {why}"))
    }

    /// `tap midi`'s clock form: one delivery from the input a sequence
    /// follows, stamps rising from its last tick to now.
    fn midi_tap_clock(
        &mut self,
        input: &str,
        list: &serde_json::Value,
    ) -> (Vec<CommitReceipt>, Result<String, String>) {
        let Some(follow) = self.transport.follower().filter(|f| f.input == input) else {
            return (
                Vec::new(),
                Err(format!("tap midi: no sequence follows {input:?} (clock=)")),
            );
        };
        let mut last = follow.last_tick().unwrap_or(f64::NEG_INFINITY);
        if let Some(why) = self.midi_unconnected(input) {
            return (Vec::new(), Err(why));
        }
        let now = self.now_ms;
        let mut batch = Vec::new();
        for m in list.as_array().map_or(&[][..], Vec::as_slice) {
            let a = m.as_array().map_or(&[][..], Vec::as_slice);
            let f = |k: usize| a.get(k).and_then(serde_json::Value::as_f64);
            let byte = |k: usize| f(k).filter(|b| b.fract() == 0.0 && (0.0..128.0).contains(b));
            let data = match (a.len(), f(1), byte(2), byte(3)) {
                (2, Some(b), ..) if [248.0, 250.0, 251.0, 252.0].contains(&b) => vec![b as u8],
                (4, Some(242.0), Some(lsb), Some(msb)) => vec![0xF2, lsb as u8, msb as u8],
                _ => {
                    return (
                        Vec::new(),
                        Err("tap midi: a clock message is [stamp, f8|fa|fb|fc] or [stamp, f2, lsb, msb]".into()),
                    )
                }
            };
            let stamp = f(0).unwrap_or(f64::NAN);
            if !(stamp >= last && stamp <= now) {
                return (
                    Vec::new(),
                    Err(format!(
                        "tap midi: clock stamps rise from the last tick to now ({}), not {}",
                        crate::agent::num(now),
                        crate::agent::num(stamp)
                    )),
                );
            }
            last = stamp;
            batch.push(Incoming {
                input: input.to_string(),
                time_stamp: stamp,
                data,
            });
        }
        let (receipts, error) = self.midi_input(batch);
        match error {
            Some(e) => (receipts, Err(format!("tap midi: {e:?}"))),
            None => (receipts, Ok(self.midi_reply())),
        }
    }

    /// `{"midi":{"state":…,"inputs":[…],"outputs":[…]}}`: the ports now.
    fn midi_reply(&self) -> String {
        let p = self.midi.ports();
        let names = |ports: &[Port]| {
            let mut s = String::from("[");
            for (i, port) in ports.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                crate::agent::quote(&port.id, &mut s);
            }
            s.push(']');
            s
        };
        format!(
            "{{\"midi\":{{\"state\":\"{}\",\"inputs\":{},\"outputs\":{}}}}}",
            p.access.word(),
            names(&p.inputs),
            names(&p.outputs)
        )
    }
}

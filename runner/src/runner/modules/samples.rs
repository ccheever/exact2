//! Sampled sources (LLP 1133.013 D5, D6, §9): a polled source that declares
//! `"samples"` answers each reader with the latest reading and, when the
//! reader asks a bound (`history`), every sample since that reader's last
//! delivered answer, with a count of what the bound left out (`dropped`)
//! and whether acquisition paused meanwhile (`interrupted`).
//!
//! **The cursor is the runner's.** Each reader's cursor is the `sequence` of
//! the last answer delivered to it: a Contract reader's once the commit that
//! carries the answer stands, so an answer superseded, let go or refused
//! consumed nothing; a data-code call's when its reply leaves the
//! implementation, since the runner cannot see it land. An implementation
//! receives it as `after` and answers statelessly from it; a reader's first
//! ask carries none and gets the latest reading with an empty batch.
//!
//! **The substitute** (§9) answers a sampled source on the reader's grid:
//! every 1 / `frequency` of the session clock from its group's first ask.
//! Its value at an instant is the agent's preference set last before it: a
//! reading, or a list of steps, each `at` milliseconds after the moment it
//! was preferred and holding until the next. The runner computes nothing
//! about the values; it copies the step's fields and stamps them.
//!
//! **A canvas input** (D6) is a send that declares `"feeds"`: under the
//! substitute the runner keeps it, and a host advancing the agent's clock
//! takes each input's samples on its grid as the surface's input events
//! ([`Runner::surface_sensor_events`]), a display frame at a time.

use super::super::{DataSource, Runner};
use super::{error_code, CallOrigin, Json, ModulesRow, Route};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The most samples a batch or a canvas input's step carries, whatever its
/// declaration says.
const MOST: usize = 1024;

/// Who reads a sampled source.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::runner) enum Reader {
    /// A Contract resource.
    Resource(usize),
    /// Data code's call, by `extension.source` and its arguments.
    Data(String),
}

/// One canvas input the substitute feeds.
#[derive(Clone, Debug)]
struct Input {
    surface: String,
    sensor: String,
    /// The sampled source whose values feed it.
    source: String,
    /// Samples a second, as asked; `period` is its reciprocal in ms.
    frequency: f64,
    period: f64,
    start: f64,
    /// The next grid point to deliver.
    next: u64,
    /// Points skipped while the page was hidden, owed to the next sample.
    dropped: u32,
    origin: CallOrigin,
}

#[derive(Default)]
pub(in crate::runner) struct Samples {
    /// Each reader's cursor.
    cursors: BTreeMap<Reader, f64>,
    /// The cursors of answers whose commit has not stood yet.
    landing: Vec<(Reader, f64)>,
    /// Data code's cursors, which a reply leaving the implementation sets.
    data: Arc<Mutex<BTreeMap<String, f64>>>,
    /// The reader being asked now, and its origin.
    pub(in crate::runner) asking: Option<(Reader, CallOrigin)>,
    /// Each reader group's grid start, by `extension.source` and arguments.
    grids: BTreeMap<(String, String), f64>,
    /// Each sampled source's preferences, oldest first: when each was set,
    /// and the reading or steps (`Null` for the default).
    tracks: BTreeMap<String, Vec<(f64, Json)>>,
    /// When the page was hidden: from, and to once shown again.
    hidden: Vec<(f64, Option<f64>)>,
    inputs: Vec<Input>,
    /// Inputs ended and not yet delivered: surface, sensor, when.
    ended: Vec<(String, String, f64)>,
}

/// `args` without the cursor and without what the call left out: the group
/// a reader's grid and release are kept by.
pub(in crate::runner) fn group(args: &Json) -> String {
    let mut args = args.clone();
    if let Some(map) = args.as_object_mut() {
        map.retain(|k, v| k != "after" && !v.is_null());
    }
    args.to_string()
}

impl<D: DataSource> Runner<D> {
    /// `row`'s `samples` declaration, if it is a sampled source.
    pub(in crate::runner) fn samples_of(&self, row: &ModulesRow) -> Option<Json> {
        let decl = self.substitute(row)["samples"].clone();
        decl.is_object().then_some(decl)
    }

    /// Clamp each computed option to its declared bounds (D12 item 2), with
    /// a journal line; a literal outside them was refused at bake.
    pub(in crate::runner) fn clamp_bounds(&mut self, row: &ModulesRow, args: &mut Json) {
        let sub = self.substitute(row);
        let Some(bounds) = sub["bounds"].as_object() else {
            return;
        };
        for (option, b) in bounds {
            let Some(given) = args.get(option).and_then(Json::as_f64) else {
                continue;
            };
            let low = b["min"].as_f64().unwrap_or(f64::NEG_INFINITY);
            let high = b["max"].as_f64().unwrap_or(f64::INFINITY);
            let clamped = given.clamp(low, high);
            if clamped != given {
                let key = self.key(row);
                self.log(format!("{key}: {option} {given} clamped to {clamped}"));
                args[option] = json!(clamped);
            }
        }
    }

    /// A reader of a sampled source asks: its grid starts at its group's
    /// first ask, and its cursor goes to the implementation as `after`.
    pub(in crate::runner) fn sampled_ask(&mut self, row: &ModulesRow, args: &Json) {
        if !self.substitute_polls(row) {
            return;
        }
        let at = (self.key(row), group(args));
        self.extensions
            .samples
            .grids
            .entry(at)
            .or_insert(self.now_ms);
    }

    /// The cursor the reader being asked carries, if any.
    pub(in crate::runner) fn cursor(&self, row: &ModulesRow) -> Option<f64> {
        let (reader, _) = self.extensions.samples.asking.as_ref()?;
        match reader {
            Reader::Data(id) => {
                let data = self.extensions.samples.data.lock().ok()?;
                data.get(id).copied()
            }
            reader => self.extensions.samples.cursors.get(reader).copied(),
        }
        .filter(|_| self.samples_of(row).is_some())
    }

    /// An answer for the reader being asked: its cursor, delivered once its
    /// commit stands (a data-code call's at once).
    pub(in crate::runner) fn sampled_answered(&mut self, row: &ModulesRow, answer: &Json) {
        let Some(decl) = self.samples_of(row) else {
            return;
        };
        let field = decl["cursor"].as_str().unwrap_or("sequence");
        let (Some(sequence), Some((reader, _))) = (
            answer[field].as_f64(),
            self.extensions.samples.asking.clone(),
        ) else {
            return;
        };
        match reader {
            Reader::Data(id) => {
                if let Ok(mut data) = self.extensions.samples.data.lock() {
                    data.insert(id, sequence);
                }
            }
            reader => self.extensions.samples.landing.push((reader, sequence)),
        }
    }

    /// A data-code reader's id: its source's key and its arguments.
    pub(in crate::runner) fn data_reader(&self, row: &ModulesRow, args: &Json) -> Reader {
        Reader::Data(format!("{} {}", self.key(row), group(args)))
    }

    /// Where a data-code call's reply, leaving the implementation, sets its
    /// reader's cursor.
    pub(in crate::runner) fn data_cursors(&self) -> Arc<Mutex<BTreeMap<String, f64>>> {
        self.extensions.samples.data.clone()
    }

    /// The commit stood: the cursors of the answers it delivered hold.
    pub(in crate::runner) fn samples_stood(&mut self) {
        let s = &mut self.extensions.samples;
        for (reader, sequence) in s.landing.drain(..) {
            s.cursors.insert(reader, sequence);
        }
    }

    /// The commit was refused: its answers delivered nothing.
    pub(in crate::runner) fn samples_refused(&mut self) {
        self.extensions.samples.landing.clear();
    }

    /// The page was hidden or shown at the session clock's now.
    pub(in crate::runner) fn samples_visibility(&mut self, hidden: bool) {
        let now = self.now_ms;
        let spans = &mut self.extensions.samples.hidden;
        match (hidden, spans.last_mut()) {
            (true, Some((_, None))) => {}
            (true, _) => spans.push((now, None)),
            (false, Some((_, end @ None))) => *end = Some(now),
            (false, _) => {}
        }
        // A span no grid reaches back to any longer is forgotten.
        if spans.len() > 64 {
            spans.remove(0);
        }
    }

    fn hidden_at(&self, t: f64) -> bool {
        (self.extensions.samples.hidden.iter())
            .any(|(from, to)| *from <= t && to.is_none_or(|to| t < to))
    }

    /// Whether acquisition paused at any instant in `(from, to]`.
    fn paused_within(&self, from: f64, to: f64) -> bool {
        (self.extensions.samples.hidden.iter()).any(|(a, b)| *a <= to && b.is_none_or(|b| b > from))
    }

    /// The agent preferred `value` for `key` now: a sampled source keeps it
    /// as a step of its track.
    pub(in crate::runner) fn note_track(&mut self, key: &str, value: &Json) {
        let tracked = (self.plan.modules.iter())
            .any(|m| self.key(m) == key && self.substitute(m)["track"] == true);
        if !tracked {
            return;
        }
        let value = if value == "clear" {
            Json::Null
        } else {
            value.clone()
        };
        let track = self
            .extensions
            .samples
            .tracks
            .entry(key.to_owned())
            .or_default();
        track.push((self.now_ms, value));
        if track.len() > 64 {
            track.remove(0);
        }
    }

    /// `row`'s reading at instant `t`: the track's step that holds then,
    /// else its default.
    fn reading_at(&self, row: &ModulesRow, t: f64) -> Json {
        let sub = self.substitute(row);
        let track = self.extensions.samples.tracks.get(&self.key(row));
        for (set, value) in track.into_iter().flatten().rev() {
            if *set > t {
                continue;
            }
            match value {
                Json::Null => break,
                Json::Array(steps) => {
                    let step = (steps.iter()).rfind(|s| set + s["at"].as_f64().unwrap_or(0.0) <= t);
                    if let Some(step) = step {
                        let mut reading = step.clone();
                        if let Some(map) = reading.as_object_mut() {
                            map.remove("at");
                        }
                        return reading;
                    }
                }
                reading => return reading.clone(),
            }
        }
        sub["default"].clone()
    }

    /// A reader's period, in ms: its frequency, else the declared rate; a
    /// tool's or a wake's at most the app's own fastest (D4).
    fn sampled_period(&self, decl: &Json, args: &Json, origin: CallOrigin) -> f64 {
        let rate = decl["rate"].as_f64().unwrap_or(60.0);
        let asked = (decl["frequency"].as_str())
            .and_then(|o| args[o].as_f64())
            .filter(|f| *f > 0.0)
            .unwrap_or(rate);
        let frequency = match origin {
            CallOrigin::App => asked,
            _ => asked.min(
                self.app_rate(decl["stream"].as_str().unwrap_or_default())
                    .unwrap_or(asked),
            ),
        };
        1000.0 / frequency.max(1e-3)
    }

    /// The substitute's answer to a reader of sampled source `row` (§9).
    pub(in crate::runner) fn sampled(&self, row: &ModulesRow, args: &Json) -> Json {
        let Some(decl) = self.samples_of(row) else {
            return json!({ "error": "NotSupportedError" });
        };
        let origin = (self.extensions.samples.asking.as_ref()).map_or(CallOrigin::App, |a| a.1);
        let period = self.sampled_period(&decl, args, origin);
        let stamp = decl["stamp"].as_str().unwrap_or("timestamp");
        let bound = (decl["bound"].as_str())
            .and_then(|o| {
                args[o]
                    .as_f64()
                    .or(self.substitute(row)["bounds"][o]["default"].as_f64())
            })
            .unwrap_or(0.0)
            .clamp(0.0, MOST as f64) as usize;
        let start = (self
            .extensions
            .samples
            .grids
            .get(&(self.key(row), group(args))))
        .copied()
        .unwrap_or(self.now_ms);
        let point = |n: u64| start + n as f64 * period;
        let at_or_before = |t: f64| ((t - start) / period + 1e-9).floor().max(0.0) as u64;
        let now = at_or_before(self.now_ms);
        let reading = |n: u64| {
            let mut r = self.reading_at(row, point(n));
            if let Some(map) = r.as_object_mut() {
                map.insert(stamp.into(), json!(point(n)));
            }
            r
        };
        // While hidden, the last reading is the one before the pause.
        let latest = match (self.extensions.samples.hidden.last()).filter(|(_, to)| to.is_none()) {
            Some((from, _)) if *from > start => at_or_before(*from - 1e-6),
            _ => now,
        };
        let mut answer = reading(latest);
        let (mut samples, mut dropped, mut interrupted) = (Vec::new(), 0usize, false);
        if let Some(after) = self.cursor(row) {
            let after = after.max(0.0) as u64;
            let visible = |n: &u64| !self.hidden_at(point(*n));
            let newest: Vec<u64> = (after + 1..=now)
                .rev()
                .filter(visible)
                .take(bound)
                .collect();
            if bound > 0 {
                samples = newest.iter().rev().map(|n| reading(*n)).collect();
                dropped = (after + 1..=now).filter(visible).count() - samples.len();
            }
            interrupted = self.paused_within(point(after), self.now_ms);
        }
        if let Some(map) = answer.as_object_mut() {
            // When the group's zero was taken (`since`, LLP 1133.026 D13).
            if let Some(field) = decl["since"].as_str() {
                map.insert(field.into(), json!(self.time.epoch_at_zero + start));
            }
            map.insert(
                decl["cursor"].as_str().unwrap_or("sequence").into(),
                json!(latest),
            );
            map.insert("samples".into(), Json::Array(samples));
            map.insert("dropped".into(), json!(dropped));
            map.insert("interrupted".into(), json!(interrupted));
        }
        answer
    }

    /// Whether the substitute polls `row` itself: a sampled source, or a
    /// count (`counts`).
    pub(in crate::runner) fn substitute_polls(&self, row: &ModulesRow) -> bool {
        self.samples_of(row).is_some()
            || self.substitute(row)["counts"].is_object()
            || matches!(
                self.substitute(row)["device"]["role"].as_str(),
                Some("stream" | "reports")
            )
    }

    /// The substitute's answer to a reader of a count (`counts`, LLP
    /// 1133.026 §11): the track's reading now, which holds while the page is
    /// hidden too, counted from `since`: where the step names none, the
    /// later of the reader's `start` and the group's first ask less `reach`
    /// (Core Motion's seven days), on the epoch.
    pub(in crate::runner) fn counted(&self, row: &ModulesRow, sub: &Json, args: &Json) -> Json {
        let counts = &sub["counts"];
        let epoch = self.time.epoch_at_zero;
        let first = epoch
            + (self
                .extensions
                .samples
                .grids
                .get(&(self.key(row), group(args))))
            .copied()
            .unwrap_or(self.now_ms);
        let start = (counts["start"].as_str())
            .and_then(|o| args[o].as_f64())
            .unwrap_or(first);
        let reach = counts["reach"].as_f64().unwrap_or(f64::INFINITY);
        let mut reading = self.reading_at(row, self.now_ms);
        if let Some(map) = reading.as_object_mut() {
            if let Some(field) = counts["since"].as_str() {
                map.entry(field).or_insert(json!(start.max(first - reach)));
            }
            if let Some(field) = counts["stamp"].as_str() {
                map.insert(field.into(), json!(epoch + self.now_ms));
            }
        }
        reading
    }

    /// Whether a value preferred for a sampled source is a reading, or a
    /// list of steps each a reading with its `at`.
    pub(in crate::runner) fn track_checked(
        &self,
        row: &ModulesRow,
        value: &Json,
    ) -> Result<(), String> {
        let decl = self.samples_of(row).unwrap_or_default();
        let counts = self.substitute(row)["counts"].clone();
        let stamp = decl["stamp"].as_str().unwrap_or("timestamp");
        let key = self.key(row);
        let one = |reading: &Json| -> Result<(), String> {
            let mut full = reading.clone();
            let Some(map) = full.as_object_mut() else {
                return Err(format!("{key}: a reading is an object"));
            };
            map.remove("at");
            // What the runner adds to a count's step, or a batch's.
            let added: Vec<(&str, Json)> = match counts.is_object() {
                true => ["since", "stamp"]
                    .iter()
                    .filter_map(|f| Some((counts[*f].as_str()?, json!(0))))
                    .filter(|(f, _)| !map.contains_key(*f))
                    .collect(),
                false => vec![
                    (stamp, json!(0)),
                    (decl["cursor"].as_str().unwrap_or("sequence"), json!(0)),
                    ("samples", json!([])),
                    ("dropped", json!(0)),
                    ("interrupted", json!(false)),
                ]
                .into_iter()
                .chain(decl["since"].as_str().map(|f| (f, json!(0))))
                .collect(),
            };
            for (field, value) in added {
                map.insert(field.into(), value);
            }
            let ty = self.plan.sources[row.source.0 as usize].ty;
            self.answer_json(ty, &key, &full)
                .map(|_| ())
                .map_err(|e| e.to_string())
        };
        match value {
            Json::Array(steps) if !steps.is_empty() => {
                for step in steps {
                    if !step["at"].as_f64().is_some_and(|at| at >= 0.0) {
                        return Err(format!(
                            "{key}: each step has `at`, ms after it is preferred"
                        ));
                    }
                    one(step)?;
                }
                Ok(())
            }
            Json::Array(_) => Err(format!("{key}: a track has at least one step")),
            reading => one(reading),
        }
    }

    /// The fastest frequency an app-origin consumer of `stream` reads it at:
    /// a Contract reader whose last ask was the app's, or a canvas input.
    /// `None` when none does: the stream is not running for the app.
    pub(in crate::runner) fn app_rate(&self, stream: &str) -> Option<f64> {
        let mut best: Option<f64> = None;
        let mut take = |f: f64| best = Some(best.map_or(f, |b| b.max(f)));
        for row in &self.plan.modules {
            let Some(decl) = self.samples_of(row) else {
                continue;
            };
            if decl["stream"] != stream {
                continue;
            }
            let rate = decl["rate"].as_f64().unwrap_or(60.0);
            let readers = self.extensions.readers.get(&self.key(row));
            for (i, args) in readers.into_iter().flatten() {
                let app = !self.origin_asked(*i).is_some_and(|o| o != CallOrigin::App);
                if !app || !self.resources.get(*i).is_some_and(Option::is_some) {
                    continue;
                }
                let args: Json = serde_json::from_str(args).unwrap_or_default();
                let f = (decl["frequency"].as_str()).and_then(|o| args[o].as_f64());
                take(f.unwrap_or(rate));
            }
        }
        for input in self
            .extensions
            .samples
            .inputs
            .iter()
            .filter(|i| i.sensor == stream)
        {
            if input.origin == CallOrigin::App {
                take(input.frequency);
            }
        }
        best
    }

    /// The reader of a sampled source `row` asking now is kept as one, under
    /// the substitute too, so its period is polled and a tool's join can
    /// find the app's consumers.
    pub(in crate::runner) fn sampled_reader(&mut self, row: &ModulesRow, args: &Json) {
        let joinable = self.substitute(row)["join"] == true;
        if !(self.substitute_polls(row) || joinable) || self.route(row) != Route::Substitute {
            return;
        }
        if let Some((Reader::Resource(i), _)) = self.extensions.samples.asking.clone() {
            let key = self.key(row);
            (self.extensions.readers.entry(key.clone()).or_default()).insert(i, group(args));
            self.note_poll(row);
        }
    }

    /// The substitute's readers no resource holds any longer are let go, with
    /// their grids and cursors (a released source's acquisition stops).
    pub(in crate::runner) fn release_sampled(&mut self) {
        let keys: Vec<String> = (self.plan.modules.iter())
            .filter(|m| self.substitute_polls(m) && self.route(m) == Route::Substitute)
            .map(|m| self.key(m))
            .collect();
        let mut held = std::collections::BTreeSet::new();
        for key in keys {
            let resources = &self.resources;
            let readers = self.extensions.readers.entry(key.clone()).or_default();
            readers.retain(|i, _| resources.get(*i).is_some_and(Option::is_some));
            held.extend(readers.keys().copied());
            let groups: Vec<String> = readers.values().cloned().collect();
            let samples = &mut self.extensions.samples;
            // A data-code reader's grid is kept while its cursor is.
            let data = samples.data.lock().map(|d| d.clone()).unwrap_or_default();
            samples.grids.retain(|(k, g), _| {
                *k != key || groups.contains(g) || data.contains_key(&format!("{k} {g}"))
            });
            // A device's stream or reports are data code's, with no Contract
            // reader: polled while a connection reads (`devices.rs`).
            if readers.is_empty() {
                self.extensions.readers.remove(&key);
                if !self.device_polled(&key) {
                    self.extensions.polls.remove(&key);
                }
            }
        }
        // A reader's cursor goes with it; another source's readers keep theirs.
        (self.extensions.samples.cursors).retain(|r, _| match r {
            Reader::Resource(i) => held.contains(i),
            Reader::Data(_) => true,
        });
    }

    /// A send that `feeds` a canvas's input, answered by the substitute: the
    /// input starts, on a fresh grid, or ends (D6).
    pub(in crate::runner) fn feed(&mut self, row: &ModulesRow, args: &Json, origin: CallOrigin) {
        let feeds = self.substitute(row)["feeds"].clone();
        let field = |k: &str| {
            (feeds[k].as_str())
                .and_then(|o| args[o].as_str())
                .unwrap_or_default()
                .to_owned()
        };
        let (surface, sensor) = (field("surface"), field("sensor"));
        let samples = &mut self.extensions.samples;
        let was = samples
            .inputs
            .iter()
            .position(|i| i.surface == surface && i.sensor == sensor);
        if feeds["stop"] == true {
            if let Some(at) = was {
                samples.inputs.remove(at);
                samples
                    .ended
                    .push((surface.clone(), sensor.clone(), self.now_ms));
                self.log(format!("sensor input {surface} {sensor}: ended"));
            }
            return;
        }
        let source = (self.plan.modules.iter())
            .find(|m| {
                self.samples_of(m)
                    .is_some_and(|d| d["stream"] == sensor.as_str())
            })
            .map(|m| {
                self.plan
                    .str(self.plan.sources[m.source.0 as usize].name)
                    .to_owned()
            });
        let Some(source) = source else {
            return;
        };
        let rate = feeds["rate"].as_f64().unwrap_or(60.0);
        let frequency = (feeds["frequency"].as_str())
            .and_then(|o| args[o].as_f64())
            .filter(|f| *f > 0.0)
            .unwrap_or(rate);
        let input = Input {
            surface: surface.clone(),
            sensor: sensor.clone(),
            source,
            frequency,
            period: 1000.0 / frequency,
            start: self.now_ms,
            next: 0,
            dropped: 0,
            origin,
        };
        let samples = &mut self.extensions.samples;
        match was {
            Some(at) => samples.inputs[at] = input,
            None => samples.inputs.push(input),
        }
        self.log(format!("sensor input {surface} {sensor}: {frequency} Hz"));
    }

    /// Whether the substitute feeds a canvas input: a host advancing the
    /// agent's clock then steps each world a display frame at a time.
    pub fn surface_sensor_inputs(&self) -> bool {
        !self.extensions.samples.inputs.is_empty() || !self.extensions.samples.ended.is_empty()
    }

    /// What the substitute's canvas inputs owe their surfaces up to `to_ms`
    /// on the session clock, oldest first: each a surface's name and its
    /// input event as the hosts write it, `{"t":"sensor","sensor","values",
    /// "dropped","at"}` or `{"t":"sensorEnded","sensor","at"}` (D6). Values
    /// are the declared `values` fields of the sensor's source, a list one
    /// spread, padded with zeros to four.
    pub fn surface_sensor_events(&mut self, to_ms: f64) -> Vec<(String, String)> {
        let mut out: Vec<(f64, String, String)> = Vec::new();
        let inputs = self.extensions.samples.inputs.clone();
        let mut advanced = Vec::new();
        for mut input in inputs {
            let row = self.module_row(&input.source);
            let decl = self.samples_of(&row).unwrap_or_default();
            let fields: Vec<String> = (decl["values"].as_array().into_iter().flatten())
                .filter_map(|f| f.as_str().map(str::to_owned))
                .collect();
            for _ in 0..MOST {
                let t = input.start + input.next as f64 * input.period;
                if t > to_ms + 1e-9 {
                    break;
                }
                input.next += 1;
                // The page hidden, or the canvas by its lease (`fed.rs`): a gap.
                if self.hidden_at(t) || self.fed_paused(&input.surface) {
                    input.dropped += 1;
                    continue;
                }
                let reading = self.reading_at(&row, t);
                let mut values: Vec<f64> = Vec::new();
                for f in &fields {
                    match &reading[f.as_str()] {
                        Json::Array(items) => values.extend(items.iter().filter_map(Json::as_f64)),
                        v => values.extend(v.as_f64()),
                    }
                }
                values.resize(4, 0.0);
                let event = json!({ "t": "sensor", "sensor": input.sensor, "values": values,
                    "dropped": std::mem::take(&mut input.dropped), "at": t });
                out.push((t, input.surface.clone(), event.to_string()));
            }
            advanced.push(input);
        }
        self.extensions.samples.inputs = advanced;
        let ended = std::mem::take(&mut self.extensions.samples.ended);
        for (surface, sensor, at) in ended {
            if at > to_ms + 1e-9 {
                self.extensions.samples.ended.push((surface, sensor, at));
                continue;
            }
            let event = json!({ "t": "sensorEnded", "sensor": sensor, "at": at });
            out.push((at, surface, event.to_string()));
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.into_iter().map(|(_, s, e)| (s, e)).collect()
    }

    /// `state.extensions`' streams, readers' cursors and canvas inputs (§9,
    /// Disclosure), for `extension`.
    pub(in crate::runner) fn samples_json(&self, extension: &str) -> Option<Json> {
        let mine = |row: &ModulesRow| self.plan.str(row.extension) == extension;
        let sampled: Vec<&ModulesRow> = (self.plan.modules.iter())
            .filter(|m| mine(m) && self.samples_of(m).is_some())
            .collect();
        if sampled.is_empty() {
            return None;
        }
        let mut streams = serde_json::Map::new();
        let mut readers = Vec::new();
        for row in &sampled {
            let decl = self.samples_of(row).unwrap_or_default();
            let stream = decl["stream"].as_str().unwrap_or_default().to_owned();
            for (i, args) in self
                .extensions
                .readers
                .get(&self.key(row))
                .into_iter()
                .flatten()
            {
                let origin = self.origin_asked(*i).unwrap_or_default();
                let cursor = self.extensions.samples.cursors.get(&Reader::Resource(*i));
                readers.push(json!({ "name": self.plan.str(row.declared),
                    "resource": self.plan.str(self.plan.resources[*i].name),
                    "args": serde_json::from_str::<Json>(args).unwrap_or_default(),
                    "cursor": cursor, "origin": origin.word() }));
            }
            if let Some(rate) = self.app_rate(&stream) {
                streams.insert(stream, json!({ "frequency": rate }));
            }
        }
        let inputs: Vec<Json> = (self.extensions.samples.inputs.iter())
            .map(|i| {
                json!({ "surface": i.surface, "sensor": i.sensor,
                "frequency": i.frequency, "origin": i.origin.word() })
            })
            .collect();
        let tracks: serde_json::Map<String, Json> = (self.extensions.samples.tracks.iter())
            .filter(|(k, _)| k.starts_with(&format!("{extension}.")))
            .filter_map(|(k, t)| Some((k.clone(), t.last()?.1.clone())))
            .filter(|(_, v)| !v.is_null())
            .collect();
        Some(json!({ "streams": streams, "sampled": readers, "inputs": inputs, "tracks": tracks }))
    }

    /// A sampled source or a canvas input whose interface's status says the
    /// device has none (`available: false`, D14): it fails
    /// `NotSupportedError`. Other interfaces answer such a device their own
    /// way (local authentication's `NOT_AVAILABLE`).
    pub(in crate::runner) fn unavailable(&self, row: &ModulesRow) -> bool {
        let sub = self.substitute(row);
        if sub["samples"].is_null() && sub["feeds"].is_null() && sub["counts"].is_null() {
            return false;
        }
        let status = sub["permission"]["status"].as_str();
        if status == Some(self.plan.str(row.declared)) {
            return false;
        }
        let value = status
            .and_then(|s| self.sibling(row, s))
            .and_then(|s| self.value_of(&s));
        value.is_some_and(|v| error_code(&v).is_none() && v["available"] == false)
    }
}

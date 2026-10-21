//! What Exact measures of each hatch call, and what hatch code says of
//! itself (@ref LLP 1075.003.000.001 §3.1–§3.3): one store a session, owned
//! by the host loop's thread. Development only, as `perf` is (LLP 1079 D1):
//! a production build's store keeps nothing and each call returns on one
//! flag. Its bounds are §3.2's, fixed: past one a record is refused and
//! counted, never grown. Reads are cumulative and change nothing.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/// Counter names, and timing names, a module.
pub(crate) const NAMES: usize = 64;
/// A timing's ring of last samples.
pub(crate) const SAMPLES: usize = 256;
/// Spans open at once.
pub(crate) const OPEN_SPANS: usize = 32;
/// Snapshot names, and each one's bytes of JSON.
pub(crate) const SNAPSHOTS: usize = 16;
pub(crate) const SNAPSHOT_BYTES: usize = 4096;
/// A log line's bytes, and a scope's lines a second of session clock.
pub(crate) const LINE_BYTES: usize = 256;
pub(crate) const LINES_PER_SECOND: u32 = 20;
/// A `state.hatches` or `perf hatches` reply.
pub(crate) const REPLY_BYTES: usize = 65536;

#[derive(Default)]
struct Timed {
    calls: u64,
    ms: f64,
    worst: f64,
}

#[derive(Default)]
struct Timing {
    count: u64,
    sum: f64,
    max: f64,
    ring: Vec<f64>,
    at: usize,
    dropped: u64,
    measured: bool,
}

struct Span {
    scope: String,
    name: String,
    from: f64,
    node: u32,
}

#[derive(Default)]
struct Bucket {
    from: f64,
    lines: u32,
    more: u32,
}

type Key = (String, String);

/// A session's store.
#[derive(Default)]
pub(crate) struct Store {
    /// Whether anything is kept: a development build.
    pub(crate) measuring: bool,
    /// Lines for the journal, each without its `hatch ` prefix; the host
    /// takes them after every call.
    pub(crate) lines: Vec<String>,
    /// A hatch asked for Save Trace, and when one was last written.
    pub(crate) trace_asked: bool,
    trace_at: Option<Instant>,
    timing: BTreeMap<(String, Option<u32>, &'static str), Timed>,
    /// The time spent in calls nested in each open one.
    inner: Vec<f64>,
    scope_calls: BTreeMap<String, BTreeMap<&'static str, u64>>,
    counters: BTreeMap<Key, u64>,
    timings: BTreeMap<Key, Timing>,
    snapshots: BTreeMap<Key, Value>,
    spans: BTreeMap<u64, Span>,
    next_span: u64,
    buckets: BTreeMap<String, Bucket>,
    bad_names: BTreeSet<String>,
    rejected: u64,
    abandoned: u64,
    limited: u64,
}

/// `text` in at most `max` bytes of UTF-8, cut at a character with `…`.
pub(crate) fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut out = String::new();
    for ch in text.chars() {
        if out.len() + ch.len_utf8() > max - 3 {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
}

impl Store {
    /// A store that measures, or one that keeps nothing.
    pub(crate) fn new(measuring: bool) -> Store {
        Store {
            measuring,
            next_span: 1,
            ..Store::default()
        }
    }

    /// A line for the journal, whatever the build: an act, a refusal.
    pub(crate) fn say(&mut self, line: String) {
        self.lines.push(line);
    }

    // What Exact times (§3.1).

    /// A hatch call begins: its start, when measuring.
    pub(crate) fn begin_call(&mut self) -> Option<Instant> {
        if !self.measuring {
            return None;
        }
        self.inner.push(0.0);
        Some(Instant::now())
    }

    /// The call returned. Its time is its own: what calls nested in it took
    /// is theirs. `counts` adds it to its scope's calls by moment (a node's
    /// are counted by its word instead).
    pub(crate) fn end_call(
        &mut self,
        started: Option<Instant>,
        hatch: &str,
        site: Option<u32>,
        moment: &'static str,
        counts: bool,
    ) {
        let Some(started) = started else { return };
        let whole = started.elapsed().as_secs_f64() * 1000.0;
        let own = (whole - self.inner.pop().unwrap_or(0.0)).max(0.0);
        if let Some(outer) = self.inner.last_mut() {
            *outer += whole;
        }
        if counts {
            *self
                .scope_calls
                .entry(hatch.to_owned())
                .or_default()
                .entry(moment)
                .or_default() += 1;
        }
        let timed = self
            .timing
            .entry((hatch.to_owned(), site, moment))
            .or_default();
        timed.calls += 1;
        timed.ms += own;
        timed.worst = timed.worst.max(own);
    }

    /// `perf <target>`'s row for a hatched site: its hatch's calls and time.
    pub(crate) fn site(&self, site: u32) -> Option<(u64, f64)> {
        let (mut calls, mut ms) = (0, 0.0);
        for ((_, at, _), timed) in &self.timing {
            if *at == Some(site) {
                calls += timed.calls;
                ms += timed.ms;
            }
        }
        (calls > 0).then_some((calls, ms))
    }

    // What hatch code records (§3.2).

    fn named(&mut self, scope: &str, name: &str) -> bool {
        if valid(name) {
            return true;
        }
        self.rejected += 1;
        let shown = cut(name, 64);
        if self.bad_names.len() < 8 && self.bad_names.insert(shown.clone()) {
            self.lines.push(format!(
                "{scope}: diagnostics refused the name {} (lowercase letters, digits, \"-\" and \".\", at most 64 bytes)",
                Value::from(shown)
            ));
        }
        false
    }

    /// A journal line under its scope: 20 a second of session clock, then
    /// one line for the rest.
    pub(crate) fn log(&mut self, scope: &str, text: &str, now: f64) {
        if !self.measuring {
            return;
        }
        let bucket = self.buckets.entry(scope.to_owned()).or_insert(Bucket {
            from: now,
            ..Bucket::default()
        });
        if now - bucket.from >= 1000.0 || now < bucket.from {
            if bucket.more > 0 {
                self.lines.push(format!("{scope}: … {} more", bucket.more));
            }
            *bucket = Bucket {
                from: now,
                ..Bucket::default()
            };
        }
        if bucket.lines >= LINES_PER_SECOND {
            bucket.more += 1;
            self.limited += 1;
            return;
        }
        bucket.lines += 1;
        self.lines
            .push(format!("{scope}: {}", cut(text, LINE_BYTES)));
    }

    /// A counter, cumulative.
    pub(crate) fn count(&mut self, scope: &str, name: &str, by: u64) {
        if !self.measuring || !self.named(scope, name) {
            return;
        }
        let key = (scope.to_owned(), name.to_owned());
        if !self.counters.contains_key(&key) && self.counters.len() >= NAMES {
            self.rejected += 1;
            return;
        }
        let counter = self.counters.entry(key).or_default();
        *counter = counter.saturating_add(by);
    }

    /// One sample of a timing: `measured` when the hatch took it itself.
    pub(crate) fn sample(&mut self, scope: &str, name: &str, ms: f64, measured: bool) {
        if !self.measuring || !self.named(scope, name) {
            return;
        }
        if !(ms >= 0.0 && ms.is_finite()) {
            self.rejected += 1;
            return;
        }
        let key = (scope.to_owned(), name.to_owned());
        if !self.timings.contains_key(&key) && self.timings.len() >= NAMES {
            self.rejected += 1;
            return;
        }
        let t = self.timings.entry(key).or_default();
        t.count += 1;
        t.sum += ms;
        t.max = t.max.max(ms);
        t.measured |= measured;
        if t.ring.len() < SAMPLES {
            t.ring.push(ms);
        } else {
            t.ring[t.at] = ms;
            t.at = (t.at + 1) % SAMPLES;
            t.dropped += 1;
        }
    }

    /// A span begins on the session clock: its id, 0 for an inert one.
    pub(crate) fn begin(&mut self, scope: &str, name: &str, node: u32, now: f64) -> u64 {
        if !self.measuring || !self.named(scope, name) {
            return 0;
        }
        if self.spans.len() >= OPEN_SPANS {
            self.rejected += 1;
            return 0;
        }
        let id = self.next_span;
        self.next_span += 1;
        self.spans.insert(
            id,
            Span {
                scope: scope.to_owned(),
                name: name.to_owned(),
                from: now,
                node,
            },
        );
        id
    }

    /// The span ends: an id the store does not hold is an inert span's, or
    /// one ended already.
    pub(crate) fn end(&mut self, id: u64, now: f64) {
        if let Some(span) = self.spans.remove(&id) {
            self.sample(&span.scope, &span.name, (now - span.from).max(0.0), false);
        }
    }

    /// A snapshot, the latest kept; one that does not fit, or is not JSON,
    /// is refused whole.
    pub(crate) fn publish(&mut self, scope: &str, name: &str, json: &str) {
        if !self.measuring || !self.named(scope, name) {
            return;
        }
        let key = (scope.to_owned(), name.to_owned());
        let value = (json.len() <= SNAPSHOT_BYTES)
            .then(|| serde_json::from_str::<Value>(json).ok())
            .flatten();
        match value {
            Some(value)
                if self.snapshots.contains_key(&key) || self.snapshots.len() < SNAPSHOTS =>
            {
                self.snapshots.insert(key, value);
            }
            _ => self.rejected += 1,
        }
    }

    /// A hatch asks for Save Trace: at most one a second of wall time, a
    /// sooner ask joining the pending one.
    pub(crate) fn ask_trace(&mut self) {
        if self.measuring {
            self.trace_asked = true;
        }
    }

    /// Whether the display loop writes a trace now: asked, and a second
    /// since the last.
    pub(crate) fn take_trace(&mut self) -> bool {
        if !self.trace_asked
            || self
                .trace_at
                .is_some_and(|at| at.elapsed().as_secs_f64() < 1.0)
        {
            return false;
        }
        self.trace_asked = false;
        self.trace_at = Some(Instant::now());
        true
    }

    /// A node ended: its open spans are closed as abandoned, counted, not timed.
    pub(crate) fn ended(&mut self, node: u32) {
        let before = self.spans.len();
        self.spans.retain(|_, span| span.node != node || node == 0);
        self.abandoned += (before - self.spans.len()) as u64;
    }

    /// A reload or a new module: a new incarnation's counts start at nothing.
    pub(crate) fn reset(&mut self) {
        let lines = std::mem::take(&mut self.lines);
        *self = Store::new(self.measuring);
        self.lines = lines;
    }

    // Reads, which change nothing (§3.3).

    fn grouped<T>(map: &BTreeMap<Key, T>, value: impl Fn(&T) -> Value) -> Map<String, Value> {
        let mut out = Map::new();
        for ((scope, name), v) in map {
            let names = out.entry(scope.clone()).or_insert_with(|| json!({}));
            names[name.as_str()] = value(v);
        }
        out
    }

    fn counted(&self) -> Map<String, Value> {
        Self::grouped(&self.counters, |n| json!(n))
    }

    /// `state.hatches`: `words` as the session counts them, each with what
    /// its hatch counted and published, and the same for the other scopes.
    pub(crate) fn state(&self, mut words: Map<String, Value>) -> Map<String, Value> {
        let counted = self.counted();
        let published = Self::grouped(&self.snapshots, Value::clone);
        let add = |entry: &mut Value, scope: &str| {
            if let Some(c) = counted.get(scope) {
                entry["counters"] = c.clone();
            }
            if let Some(p) = published.get(scope) {
                entry["published"] = p.clone();
            }
        };
        for (word, entry) in words.iter_mut() {
            add(entry, &format!("element {word}"));
        }
        let mut others = Map::new();
        let scopes: BTreeSet<&String> = self
            .scope_calls
            .keys()
            .chain(counted.keys())
            .chain(published.keys())
            .collect();
        for scope in scopes {
            if let Some(word) = scope.strip_prefix("element ") {
                if !words.contains_key(word) {
                    let mut entry = json!({ "live": 0, "reusable": 0, "lost": [], "calls": {} });
                    add(&mut entry, scope);
                    words.insert(word.to_owned(), entry);
                }
                continue;
            }
            let mut entry = match self.scope_calls.get(scope) {
                Some(calls) => json!({ "calls": calls }),
                None => json!({}),
            };
            add(&mut entry, scope);
            others.insert(scope.clone(), entry);
        }
        let mut reply = Map::new();
        reply.insert("words".into(), Value::Object(words));
        reply.insert("scopes".into(), Value::Object(others));
        reply.insert("measuring".into(), self.measuring.into());
        reply.insert("rejected".into(), self.rejected.into());
        reply.insert("abandoned".into(), self.abandoned.into());
        reply.insert("limited".into(), self.limited.into());
        reply
    }

    /// `perf hatches`: every call Exact timed, by hatch and by site and
    /// moment, and the hatches' counters and timings. Cumulative.
    pub(crate) fn perf(&self, tags: Map<String, Value>, tickets: usize) -> Value {
        let mut by: BTreeMap<&str, Timed> = BTreeMap::new();
        let mut calls = Vec::new();
        for ((hatch, site, moment), t) in &self.timing {
            let sum = by.entry(hatch).or_default();
            sum.calls += t.calls;
            sum.ms += t.ms;
            sum.worst = sum.worst.max(t.worst);
            let mut row = json!({ "hatch": hatch, "moment": moment, "calls": t.calls, "ms": t.ms, "worst": t.worst });
            if let Some(site) = site {
                row["site"] = json!(site);
            }
            calls.push(row);
        }
        let hatches: Map<String, Value> = by
            .into_iter()
            .map(|(hatch, t)| {
                (
                    hatch.to_owned(),
                    json!({ "calls": t.calls, "ms": t.ms, "worst": t.worst }),
                )
            })
            .collect();
        let timings = Self::grouped(&self.timings, |t| {
            let mut sorted = t.ring.clone();
            sorted.sort_by(f64::total_cmp);
            let q = |f: f64| match sorted.is_empty() {
                true => 0.0,
                false => sorted[(sorted.len() - 1).min((f * sorted.len() as f64) as usize)],
            };
            let mut out = json!({ "count": t.count, "sum": t.sum, "max": t.max, "p50": q(0.5), "p95": q(0.95),
                "samples": sorted.len(), "dropped": t.dropped });
            if t.measured {
                out["measured"] = true.into();
            }
            out
        });
        let mut reply = tags;
        reply.insert("measuring".into(), self.measuring.into());
        reply.insert("hatches".into(), Value::Object(hatches));
        reply.insert("calls".into(), Value::Array(calls));
        reply.insert("tickets".into(), tickets.into());
        reply.insert("counters".into(), Value::Object(self.counted()));
        reply.insert("timings".into(), Value::Object(timings));
        reply.insert("rejected".into(), self.rejected.into());
        reply.insert("abandoned".into(), self.abandoned.into());
        reply.insert("limited".into(), self.limited.into());
        Value::Object(fit(reply, &["calls", "hatches", "counters", "timings"]))
    }
}

/// A reply of at most 64 KB: the longest of `lists` is halved until it fits,
/// and the reply says `truncated`.
pub(crate) fn fit(mut reply: Map<String, Value>, lists: &[&str]) -> Map<String, Value> {
    let size = |v: &Value| v.to_string().len();
    while size(&Value::Object(reply.clone())) > REPLY_BYTES {
        let longest = lists
            .iter()
            .filter(|key| match reply.get(**key) {
                Some(Value::Array(list)) => !list.is_empty(),
                Some(Value::Object(map)) => !map.is_empty(),
                _ => false,
            })
            .max_by_key(|key| size(&reply[**key]));
        let Some(longest) = longest else { break };
        match reply.get_mut(*longest) {
            Some(Value::Array(list)) => list.truncate(list.len() / 2),
            Some(Value::Object(map)) => {
                let keep = map.len() / 2;
                let gone: Vec<String> = map.keys().skip(keep).cloned().collect();
                for key in gone {
                    map.remove(&key);
                }
            }
            _ => break,
        }
        reply.insert("truncated".into(), true.into());
    }
    reply
}

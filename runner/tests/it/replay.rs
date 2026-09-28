//! Two runs of the same calls agree (LLP 1069.009, first slice): every input
//! is a runner call, so a fixed sequence of calls made twice, from two fresh
//! runners on Caltrain's plan, must leave equal state, hand out equal
//! requests after every call, and write equal journals. And an answer that
//! reads the device (D2a) replays from its recorded value, not by running
//! the source again.
//!
//! @ref LLP 1069.009 D1 (the calls), D2a (recorded answers), D4 (the
//! `take_requests` comparison is the divergence check)
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    agent, Answer, DataError, DataSource, Event, Outcome, Request, RequestOut, Response, Runner,
    Store,
};
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

fn station(id: &str, name: &str, zone: f64, distance: f64) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::str(name),
        Value::Number(zone),
        Value::Number(distance),
    ])
}

const STATIONS: &[(&str, &str, f64)] = &[
    ("mv", "Mountain View", 3.0),
    ("pa", "Palo Alto", 3.0),
    ("sf", "San Francisco", 1.0),
];

fn stations(near: f64) -> Vec<Value> {
    STATIONS
        .iter()
        .enumerate()
        .map(|(i, (id, name, zone))| station(id, name, *zone, near + i as f64))
        .collect()
}

/// A device read: somewhere else each time it is asked, as a GPS fix is.
static FIX: AtomicU64 = AtomicU64::new(1);

/// Caltrain's sources in miniature. `search` goes out as a request the host
/// runs (a `fulfill` answers it); `defaultLocation` reads the device, marks
/// it (`observe_external_read`), and watches the `location` topic.
struct Device {
    /// Whether each location read is a new fix, or the same one.
    moving: bool,
}

impl DataSource for Device {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let distance = match args.last() {
            Some(Value::Record(fields)) => match fields.first() {
                Some(Value::Number(lat)) => *lat,
                _ => 0.0,
            },
            _ => 0.0,
        };
        Ok(Answer::Now(match source {
            "defaultLocation" => {
                store.observe_external_read();
                store.observe_topic("location");
                let fix = if self.moving {
                    FIX.fetch_add(1, Ordering::SeqCst) as f64
                } else {
                    0.0
                };
                Value::record(vec![
                    Value::Number(37.0 + fix / 1000.0),
                    Value::Number(-122.0),
                ])
            }
            "station" => {
                let id = args.first().and_then(Value::as_str).unwrap_or("mv");
                stations(distance)
                    .into_iter()
                    .find(|s| matches!(s, Value::Record(f) if f[0].as_str() == Some(id)))
                    .ok_or_else(|| DataError::Unavailable(id.into()))?
            }
            "board" => {
                let now = match args.get(2) {
                    Some(Value::Number(n)) => *n,
                    _ => 0.0,
                };
                let train = if args.get(1).and_then(Value::as_str) == Some("north") {
                    101.0
                } else {
                    102.0
                };
                Value::list(
                    (0..3)
                        .map(|k| {
                            let at = now + 60_000.0 * (k as f64 + 1.0);
                            Value::record(vec![
                                Value::str(&format!("d{train}-{k}")),
                                Value::Number(train + k as f64 * 2.0),
                                Value::str("Local"),
                                Value::str("San Francisco"),
                                Value::Number(at),
                            ])
                        })
                        .collect(),
                )
            }
            "nearest" | "stations" => Value::list(stations(distance)),
            "search" => {
                let q = args.first().and_then(Value::as_str).unwrap_or("");
                return Ok(Answer::Later(Request::get(&format!(
                    "https://stations.test/search?q={q}"
                ))));
            }
            other => return Err(DataError::UnknownSource(other.into())),
        }))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Outcome::Response(r) = outcome else {
            return Err(DataError::Unavailable(source.into()));
        };
        // `id:name` per line.
        let body = String::from_utf8(r.body).map_err(|_| DataError::BadArguments("utf8".into()))?;
        Ok(Answer::Now(Value::list(
            body.lines()
                .filter_map(|l| l.split_once(':'))
                .map(|(id, name)| station(id, name, 3.0, 0.0))
                .collect(),
        )))
    }
}

/// One recorded answer (D3's `answer` record): made while the device-read
/// count rose, so a replay takes the value rather than running the source.
/// It carries the topics the answer watched, or a replay would not know to
/// ask again when one changes.
#[derive(Clone, Debug)]
struct Recorded {
    value: Value,
    topics: Vec<String>,
}

type Key = (String, String);

fn key(source: &str, args: &[Value]) -> Key {
    (source.to_owned(), format!("{args:?}"))
}

/// Wraps a source. Recording, it keeps every answer made with a device read,
/// keyed by source and arguments, in order. Replaying, it answers those from
/// the record and runs the source for the rest.
struct Tape<D> {
    inner: D,
    replay: bool,
    answers: BTreeMap<Key, VecDeque<Recorded>>,
    replayed: usize,
}

impl<D> Tape<D> {
    fn recording(inner: D) -> Self {
        Self {
            inner,
            replay: false,
            answers: BTreeMap::new(),
            replayed: 0,
        }
    }
}

impl<D: DataSource> DataSource for Tape<D> {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.inner.query(source, args)
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let k = key(source, args);
        if self.replay {
            if let Some(r) = self.answers.get_mut(&k).and_then(VecDeque::pop_front) {
                self.replayed += 1;
                store.observe_external_read();
                for t in &r.topics {
                    store.observe_topic(t);
                }
                return Ok(Answer::Now(r.value));
            }
            return self.inner.answer(store, source, args);
        }
        let before = store.reads();
        let answer = self.inner.answer(store, source, args)?;
        if store.reads() > before {
            let Answer::Now(value) = &answer else {
                panic!("a device read answered later: record its fulfill instead");
            };
            // Read the topics this answer watched, and give them back.
            let topics = store.take_topics();
            for t in &topics {
                store.observe_topic(t);
            }
            self.answers.entry(k).or_default().push_back(Recorded {
                value: value.clone(),
                topics,
            });
        }
        Ok(answer)
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.inner.parse(store, source, args, outcome)
    }
}

fn boot<D: DataSource>(data: D) -> Runner<D> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps/caltrain/app.contract");
    let plan = contract::compile_path(&path).unwrap();
    Runner::boot(
        plan,
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view<D: DataSource>(r: &Runner<D>, test_id: &str) -> u32 {
    let k = r.kernel();
    let key = *k
        .find_by_test_id(test_id)
        .first()
        .unwrap_or_else(|| panic!("no view {test_id}"));
    k.node_by_key(key).unwrap().id
}

/// What one call left: its result, the requests it handed out, and the
/// whole state (every slot, derive and resource, the facts, what's pending).
#[derive(Debug, PartialEq)]
struct Step {
    call: &'static str,
    result: String,
    requests: Vec<RequestOut>,
    state: String,
}

/// The fixed sequence of calls: the host's facts, the clock, a press and a
/// typed search, the reply to the search, the device saying the location
/// changed, and more clock. Returns each call's step and the journal.
fn run<D: DataSource>(r: &mut Runner<D>) -> (Vec<Step>, Vec<String>) {
    let mut steps = Vec::new();
    let mut step = |r: &mut Runner<D>, call: &'static str, result: String| {
        let requests = r.take_requests();
        steps.push(Step {
            call,
            result,
            requests: requests.clone(),
            state: agent::state(r),
        });
        requests
    };
    step(r, "boot", String::new());
    let res = format!("{:?}", r.set_viewport(402.0, 874.0));
    step(r, "viewport", res);
    let res = format!(
        "{:?}",
        r.set_place("en-US", "America/Los_Angeles", Some(3_710_452_291.0))
    );
    step(r, "place", res);
    let res = format!("{:?}", r.set_time(1_790_553_600_000.0, -420.0));
    step(r, "time", res);
    let res = format!("{:?}", r.advance_timed(412.0));
    step(r, "advance 412", res);
    let v = view(r, "change-station");
    let res = format!("{:?}", r.dispatch(v, Event::Press));
    step(r, "press change-station", res);
    let v = view(r, "station-search");
    let res = format!("{:?}", r.dispatch(v, Event::Change("Palo".into())));
    let ticket = step(r, "change station-search", res)
        .iter()
        .find(|q| q.target == "matches")
        .expect("the search went out as a request")
        .ticket;
    let reply = Outcome::Response(Response {
        status: 200,
        headers: Vec::new(),
        body: b"pa:Palo Alto\nmv:Mountain View".to_vec(),
    });
    let res = format!("{:?}", r.fulfill(ticket, reply));
    step(r, "fulfill search", res);
    let res = format!("{:?}", r.changed("location"));
    step(r, "location changed", res);
    let res = format!("{:?}", r.advance_timed(3_412.0));
    step(r, "advance 3412", res);
    (steps, r.journal().map(str::to_owned).collect())
}

fn assert_agree(a: &(Vec<Step>, Vec<String>), b: &(Vec<Step>, Vec<String>)) {
    assert_eq!(a.0.len(), b.0.len());
    for (x, y) in a.0.iter().zip(&b.0) {
        assert_eq!(x, y, "the runs diverge at `{}`", x.call);
    }
    assert_eq!(a.1, b.1, "the journals differ");
}

/// Two fresh runners, the same calls, the same sources' answers: equal
/// state, equal requests after every call, equal journals.
#[test]
fn two_runs_of_the_same_calls_agree() {
    let a = run(&mut boot(Device { moving: false }));
    let b = run(&mut boot(Device { moving: false }));
    assert_agree(&a, &b);
    // The sequence exercised what it names: the search went out as a
    // request, its reply filled `matches`, and the clock ticked the boards.
    let at = |call| &a.0.iter().find(|s| s.call == call).unwrap().state;
    // A resource's list, as `state` writes it.
    let list = |call, name: &str| {
        let s = at(call);
        let from = s.rfind(&format!("\"{name}\":[")).unwrap();
        s[from..from + s[from..].find(']').unwrap() + 1].to_owned()
    };
    assert_eq!(list("change station-search", "matches"), "\"matches\":[]");
    assert!(list("fulfill search", "matches").contains("Palo Alto"));
    assert_ne!(
        list("advance 412", "northBoard"),
        list("advance 3412", "northBoard")
    );
}

/// A source whose answer reads the device returns something new each call,
/// so running it again is not a replay. Answering from the recorded value
/// is: equal state, requests and journal, call by call.
#[test]
fn a_device_read_replays_from_its_recorded_value() {
    let mut recorded = boot(Tape::recording(Device { moving: true }));
    let a = run(&mut recorded);
    let tape = std::mem::take(&mut recorded.data().answers);
    let reads: usize = tape.values().map(VecDeque::len).sum();
    // Boot and `changed("location")` each asked the device.
    assert_eq!(reads, 2, "{tape:?}");

    // Running the source again reads a different device: the runs diverge.
    let rerun = run(&mut boot(Device { moving: true }));
    assert_ne!(a.0, rerun.0, "a moving device read the same twice");

    let mut replayed = boot(Tape {
        replay: true,
        answers: tape,
        ..Tape::recording(Device { moving: true })
    });
    let c = run(&mut replayed);
    assert_agree(&a, &c);
    assert_eq!(replayed.data().replayed, reads);
}

//! The corpus (LLP 1004 D6): every accept fixture compiles byte-identically,
//! round-trips, and runs; every reject fixture is refused with exactly its id.

use exact_kernel::{Kernel, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn reject_fixtures_are_refused_by_exactly_their_id() {
    let text = corpus("rejects.txt");
    let mut checked = 0;
    for case in text.split("\n---\n") {
        let case = case.trim();
        let Some(rest) = case.strip_prefix("== ") else {
            continue;
        };
        let (id, src) = rest.split_once('\n').unwrap();
        let src: String = src
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        match contract::compile(&src) {
            Err(e) => assert_eq!(e.id, id, "fixture `{id}` was refused as `{}`: {e}", e.id),
            Ok(_) => panic!("fixture `{id}` compiled"),
        }
        checked += 1;
    }
    assert!(checked >= 26, "{checked} reject fixtures");
}

/// The same miniature data source the runner's hand-built test uses.
#[derive(Default)]
struct Schedule;

fn station(id: &str, name: &str) -> Value {
    Value::record(vec![Value::str(id), Value::str(name)])
}

fn departure(id: &str, train: f64, at_ms: f64) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::Number(train),
        Value::Number(at_ms),
    ])
}

impl DataSource for Schedule {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "stations" => Ok(Value::list(vec![
                station("mv", "Mountain View"),
                station("pa", "Palo Alto"),
            ])),
            "departures" => match args.first().and_then(Value::as_str) {
                Some("mv") => Ok(Value::list(vec![
                    departure("d1", 101.0, 600_000.0),
                    departure("d2", 103.0, 1_500_000.0),
                ])),
                Some("pa") => Ok(Value::list(vec![
                    departure("d2", 103.0, 1_200_000.0),
                    departure("d1", 101.0, 300_000.0),
                    departure("d9", 109.0, 9_000_000.0),
                ])),
                other => Err(DataError::Unavailable(format!("{other:?}"))),
            },
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn text_of(r: &Runner<Schedule>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    k.node_by_key(key)?
        .props
        .str(PropId::Text)
        .map(str::to_string)
}

fn ids(r: &Runner<Schedule>, prefix: &str) -> Vec<(String, u32)> {
    let k = r.kernel();
    let mut out = Vec::new();
    for root in k.roots() {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let n = k.node(id).unwrap();
            if let Some(t) = n.props.str(PropId::TestId) {
                if t.starts_with(prefix) {
                    out.push((t.to_string(), id));
                }
            }
            let mut c = n.children();
            c.reverse();
            stack.extend(c);
        }
    }
    out
}

#[test]
fn the_now_screen_fixture_compiles_and_behaves_like_the_hand_built_plan() {
    let src = corpus("now-screen.contract");
    let plan = contract::compile(&src).unwrap();
    assert_eq!(
        contract::compile(&src).unwrap().encode(),
        plan.encode(),
        "byte-identical"
    );
    let plan = Plan::decode(&plan.encode()).unwrap();
    let baked = contract::bake(plan, Schedule).unwrap();
    assert!(baked.resources.iter().all(|r| r.initial.len > 0));

    let mut r = Runner::boot(baked, Schedule, Kernel::with_monospace()).unwrap();
    assert_eq!(text_of(&r, "count").as_deref(), Some("2 trains"));
    assert_eq!(text_of(&r, "nearest").as_deref(), Some("nearest"));
    let rows = ids(&r, "dep-");
    assert_eq!(
        rows.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        ["dep-d1", "dep-d2"]
    );
    let (d1, d2) = (rows[0].1, rows[1].1);

    // Press → the curried row id → the source refuses "d2" → rolled back.
    assert!(r.dispatch(d2, Event::Press).is_err());
    assert_eq!(r.slot("stationId"), Some(&Value::NONE));
    r.act("selectStation", vec![Value::str("pa")]).unwrap();
    assert_eq!(text_of(&r, "count").as_deref(), Some("3 trains"));
    assert_eq!(text_of(&r, "selected").as_deref(), Some("at pa"));
    let rows = ids(&r, "dep-");
    assert_eq!(
        rows.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        ["dep-d2", "dep-d1", "dep-d9"]
    );
    assert_eq!(
        (rows[0].1, rows[1].1),
        (d2, d1),
        "keyed rows keep their views"
    );

    // Search flips the `when`; the timer ticks under the clock.
    let search = ids(&r, "search")[0].1;
    r.dispatch(search, Event::Change("pal".into())).unwrap();
    assert_eq!(text_of(&r, "searching").as_deref(), Some("searching"));
    r.dispatch(search, Event::Change(String::new())).unwrap();
    assert_eq!(ids(&r, "dep-").len(), 3);
    assert_eq!(r.advance(2_500.0).unwrap().len(), 2);
    assert_eq!(r.slot("nowMs"), Some(&Value::Number(2_000.0)));

    r.act("setDark", vec![Value::str("dark")]).unwrap();
    assert_eq!(r.take_commands()[0].name, "setScheme");
}

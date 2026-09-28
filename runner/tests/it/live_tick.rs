//! One live tick on a 10,000-row virtualized list costs what changed, not
//! the list (LLP 1053 §0 G8). The shape is the heavy list benchmark's: a
//! source answers a mutation with the whole feed, one message inserted at the
//! top and one replaced, every other record the previous answer's allocation.
//! The runner keys only the rows whose allocation is new, and type-checks
//! only the records it has not already checked.
use exact_kernel::{Kernel, Offer};
use exact_plan::{Items, Value};
use exact_runner::{DataError, DataSource, Runner, RunnerError};
use std::time::Instant;

const SOURCE: &str = r##"shape Run
  id: string
  t: string
  s: string
shape Para
  id: string
  runs: list<Run>
shape Reaction
  emoji: string
  count: number
shape Message
  id: string
  author: string
  minutesAgo: number
  paragraphs: list<Para>
  quote: option<string>
  reactions: list<Reaction>
shape Feed
  rows: list<Message>
component App
  resource initial = messages() as shape Feed
  mutation changed as shape Feed
  derive feed = match changed { case some(f) => f, case none => initial }
  action tick writes changed
    send changed = tick()
  view
    column width="100%" height="100%" overflow="hidden"
      list testId="messages" virtualized=true estimated-item-height=160 flex=1 min-height=0 width="100%"
        each m in feed.rows key=m.id
          column width="100%"
            text m.author
            each p in m.paragraphs key=p.id
              text
                each r in p.runs key=r.id
                  text r.t
            row
              each r in m.reactions key=r.emoji
                text `${r.emoji} ${r.count}`
"##;

const COUNT: usize = 10_000;

fn message(id: &str, i: usize, bump: f64) -> Value {
    let paragraphs = (0..3)
        .map(|p| {
            let runs = (0..6usize)
                .map(|r| {
                    Value::record(vec![
                        Value::str(&format!("r{r}")),
                        Value::str(&format!("run {r} of paragraph {p} in {i}")),
                        Value::str(if r % 3 == 0 { "bold" } else { "" }),
                    ])
                })
                .collect();
            Value::record(vec![Value::str(&format!("p{p}")), Value::list(runs)])
        })
        .collect();
    Value::record(vec![
        Value::str(id),
        Value::str(&format!("Author {}", i % 97)),
        Value::Number(i as f64),
        Value::list(paragraphs),
        if i.is_multiple_of(4) {
            Value::some(Value::str("a quote"))
        } else {
            Value::NONE
        },
        Value::list(vec![
            Value::record(vec![Value::str("👍"), Value::Number(1.0 + bump)]),
            Value::record(vec![Value::str("🎉"), Value::Number(2.0)]),
        ]),
    ])
}

/// The feed a source keeps: the rows it answered last, shared.
struct Feed {
    rows: Items,
    ticks: usize,
    /// The next tick answers one record of the wrong shape.
    bad: bool,
}
impl Feed {
    fn new() -> Self {
        Self {
            rows: Items::from(
                (0..COUNT)
                    .map(|i| message(&format!("m{i}"), i, 0.0))
                    .collect::<Vec<_>>(),
            ),
            ticks: 0,
            bad: false,
        }
    }
    fn feed(&self) -> Value {
        Value::record(vec![Value::List(self.rows.clone())])
    }
}
impl DataSource for Feed {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        if source == "tick" {
            // One message on top, one reaction bumped; every other record is
            // the previous answer's allocation.
            self.ticks += 1;
            let k = self.ticks;
            let mut rows = Vec::with_capacity(self.rows.len() + 1);
            rows.push(message(&format!("live-{k}"), k, 0.0));
            rows.extend(self.rows.iter().cloned());
            let at = (k * 101) % COUNT + 1;
            let Value::Record(old) = &rows[at] else {
                unreachable!()
            };
            let id = old[0].as_str().unwrap().to_owned();
            rows[at] = message(&id, at, k as f64);
            if std::mem::take(&mut self.bad) {
                // `minutesAgo` as a string, deep in a mostly shared answer.
                let Value::Record(fields) = &rows[at] else {
                    unreachable!()
                };
                let mut fields = fields.to_vec();
                fields[2] = Value::str("now");
                rows[at] = Value::record(fields);
                return Ok(Value::record(vec![Value::list(rows)]));
            }
            self.rows = Items::from(rows);
        }
        Ok(self.feed())
    }
}

fn boot() -> Runner<Feed> {
    let plan = contract::compile(SOURCE).unwrap();
    let mut r = Runner::boot(
        plan,
        Feed::new(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let root = r.roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    r
}

/// Every live tick keys only its new rows (the collection's unit tests show
/// the result equals a full re-key), and a bad record among the shared ones
/// is still refused. `cargo test --release -p exact-runner --test it
/// live_tick -- --nocapture` prints the per-tick time.
#[test]
fn a_live_tick_keys_and_checks_what_changed() {
    let mut r = boot();
    // The first tick replaces the baked answer with the source's own: every
    // allocation is new to the runner.
    r.act("tick", vec![]).unwrap();
    let mut times = Vec::new();
    let ticks = 20;
    for _ in 0..ticks {
        let at = Instant::now();
        r.act("tick", vec![]).unwrap();
        times.push(at.elapsed().as_secs_f64() * 1000.0);
        let work = r.last_instance_work();
        // Its two new messages, and the paragraphs, runs and reactions of
        // the rows it mounted: not the list.
        assert!(
            work.rows_keyed <= 100,
            "a tick keyed {} rows",
            work.rows_keyed
        );
    }
    times.sort_by(f64::total_cmp);
    eprintln!(
        "live tick on {COUNT} rows: median {:.3} ms, max {:.3} ms, keyed {}",
        times[times.len() / 2],
        times[times.len() - 1],
        r.last_instance_work().rows_keyed
    );
    let snapshot = r.collections().pop().unwrap();
    assert_eq!(snapshot.count, COUNT + ticks + 1);
    // One bad record among the shared ones is refused as the shape it is,
    // and the list stays as it was.
    r.data().bad = true;
    let refused = r.act("tick", vec![]);
    assert!(
        matches!(&refused, Err(RunnerError::Shape { resource }) if resource == "changed"),
        "{refused:?}"
    );
    assert_eq!(r.collections().pop().unwrap().count, COUNT + ticks + 1);
    r.act("tick", vec![]).unwrap();
    assert_eq!(r.collections().pop().unwrap().count, COUNT + ticks + 2);
}

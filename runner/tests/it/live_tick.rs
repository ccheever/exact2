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
  action tick
    send changed = tick()
  view
    column width="100%" height="100%" overflow="hidden"
      list testId="messages" virtualized=true estimated-item-height=160 flex=1 min-height=0 width="100%"
        for m in feed.rows key=m.id
          column width="100%"
            text m.author
            for p in m.paragraphs key=p.id
              text
                for r in p.runs key=r.id
                  text r.t
            row
              for r in m.reactions key=r.emoji
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
        matches!(&refused, Err(RunnerError::Shape { resource, .. }) if resource == "changed"),
        "{refused:?}"
    );
    assert_eq!(r.collections().pop().unwrap().count, COUNT + ticks + 1);
    r.act("tick", vec![]).unwrap();
    assert_eq!(r.collections().pop().unwrap().count, COUNT + ticks + 2);
}

#[test]
fn unchanged_key_sequences_keep_duplicate_rows_and_rebind_their_contents() {
    use exact_kernel::PropId;
    use exact_runner::Event;

    let source = r#"shape Item
  id: number
  label: string
  shown: bool
component App
  state rows = [Item(id=0, label="a", shown=false), Item(id=0, label="b", shown=false), Item(id=1, label="c", shown=false)]
  action replace(items: list<Item>)
    rows = items
  view
    column
      for item, i in rows key=item.id
        Card(item=item, position=i)
component Card
  props
    item: Item
    position: number
  state taps = 0
  action bump
    taps = taps + 1
  view
    column
      text `${item.label}:${taps}:${position}:${1 / item.id > 0}` testId=item.label press=bump
      if item.shown
        text "extra" testId=`${item.label}-extra`
"#;
    struct Unused;
    impl DataSource for Unused {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("unexpected source {name}")
        }
    }
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Unused,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let shown = |r: &Runner<Unused>, label: &str| {
        let key = r.kernel().find_by_test_id(label)[0];
        let node = r.kernel().node_by_key(key).unwrap();
        (key, node.props.str(PropId::Text).unwrap().to_owned())
    };
    let a = shown(&r, "a").0;
    let b = shown(&r, "b").0;
    let c = shown(&r, "c").0;
    r.dispatch(r.kernel().node_by_key(b).unwrap().id, Event::Press)
        .unwrap();
    let item = |id, label, visible| {
        Value::record(vec![
            Value::Number(id),
            Value::str(label),
            Value::Bool(visible),
        ])
    };
    r.act(
        "replace",
        vec![Value::list(vec![
            item(-0.0, "A", true),
            item(0.0, "B", false),
            item(1.0, "C", false),
        ])],
    )
    .unwrap();
    assert_eq!(shown(&r, "A"), (a, "A:0:0:false".into()));
    assert_eq!(shown(&r, "B"), (b, "B:1:1:true".into()));
    assert_eq!(shown(&r, "C"), (c, "C:0:2:true".into()));
    assert_eq!(shown(&r, "A-extra").1, "extra");
    // Leaving the unchanged-order path must keep both duplicate identities,
    // the row's private state and its new position.
    r.act(
        "replace",
        vec![Value::list(vec![
            item(1.0, "C", false),
            item(-0.0, "A", false),
            item(0.0, "B", false),
        ])],
    )
    .unwrap();
    assert_eq!(shown(&r, "C"), (c, "C:0:0:true".into()));
    assert_eq!(shown(&r, "A"), (a, "A:0:1:false".into()));
    assert_eq!(shown(&r, "B"), (b, "B:1:2:true".into()));
    assert!(r.kernel().find_by_test_id("A-extra").is_empty());
    r.act(
        "replace",
        vec![Value::list(vec![
            item(1.0, "C", false),
            item(2.0, "D", true),
        ])],
    )
    .unwrap();
    assert_eq!(shown(&r, "C").0, c);
    assert_eq!(shown(&r, "D").1, "D:0:1:true");
    assert!(r.kernel().node_by_key(a).is_none());
    assert!(r.kernel().node_by_key(b).is_none());
}

#[test]
fn positional_rows_match_general_keys_through_replacement_growth_and_removal() {
    use exact_kernel::PropId;
    use exact_runner::Event;

    let source = r#"component App
  state rows = ["a", "b"]
  action replace(items: list<string>)
    rows = items
  view
    column
      for item, i in rows key=KEY
        Card(item=item, position=i)
component Card
  props
    item: string
    position: number
  state initial = item
  state taps = 0
  action bump
    taps = taps + 1
  view
    column
      text `${item}:${initial}:${taps}:${position}` testId=`row-${position}` press=bump
      if item == "extra"
        text "extra" testId=`extra-${position}`
"#;
    struct Unused;
    impl DataSource for Unused {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("unexpected source {name}")
        }
    }
    let boot = |key| {
        Runner::boot(
            contract::compile(&source.replace("KEY", key)).unwrap(),
            Unused,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    };
    let (mut indexed, mut general) = (boot("i"), boot("i + 0"));
    assert_eq!(indexed.last_instance_work().rows_keyed, 0);
    assert_eq!(general.last_instance_work().rows_keyed, 2);
    let original = indexed.kernel().find_by_test_id("row-0")[0];
    for r in [&mut indexed, &mut general] {
        let key = r.kernel().find_by_test_id("row-0")[0];
        let id = r.kernel().node_by_key(key).unwrap().id;
        r.dispatch(id, Event::Press).unwrap();
    }
    for labels in [
        vec!["b", "a"],
        vec!["extra", "b", "a"],
        vec!["a"],
        vec![],
        vec!["new", "extra"],
    ] {
        for r in [&mut indexed, &mut general] {
            r.act(
                "replace",
                vec![Value::list(labels.iter().map(|s| Value::str(s)).collect())],
            )
            .unwrap();
        }
        assert_eq!(indexed.last_instance_work().rows_keyed, 0);
        assert_eq!(general.last_instance_work().rows_keyed, labels.len());
        assert_eq!(
            indexed.kernel().export(None).unwrap(),
            general.kernel().export(None).unwrap()
        );
        if labels.first() == Some(&"b") {
            let n = indexed.kernel().node_by_key(original).unwrap();
            assert_eq!(n.props.str(PropId::Text), Some("b:a:1:0"));
        }
    }
    assert!(indexed.kernel().node_by_key(original).is_none());
    let key = indexed.kernel().find_by_test_id("row-0")[0];
    let n = indexed.kernel().node_by_key(key).unwrap();
    assert_eq!(n.props.str(PropId::Text), Some("new:new:0:0"));
}

#[test]
fn nested_row_state_does_not_visit_an_unrelated_stateless_parent_subtree() {
    use exact_kernel::PropId;
    use exact_runner::Event;

    let source = r#"shape Group
  id: number
  labels: list<string>
component App
  state groups = [Group(id=0, labels=["target"]), Group(id=1, labels=["other"])]
  action replace(items: list<Group>)
    groups = items
  view
    column
      for group, i in groups key=KEY
        column
          for label in group.labels key=label
            Counter(label=label)
component Counter
  props
    label: string
  state taps = 0
  action bump
    taps = taps + 1
  view
    text `${label}:${taps}` testId=label press=bump
"#;
    struct Unused;
    impl DataSource for Unused {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("unexpected source {name}")
        }
    }
    for key in ["group.id", "i"] {
        let mut r = Runner::boot(
            contract::compile(&source.replace("KEY", key)).unwrap(),
            Unused,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let target = r.kernel().find_by_test_id("target")[0];
        let id = r.kernel().node_by_key(target).unwrap().id;
        r.dispatch(id, Event::Press).unwrap();
        let before_growth = r.last_instance_work().rows_scanned;
        let group = |id, labels| Value::record(vec![Value::Number(id), Value::list(labels)]);
        r.act(
            "replace",
            vec![Value::list(vec![
                group(0.0, vec![Value::str("target")]),
                group(
                    1.0,
                    (0..512)
                        .map(|i| Value::str(&format!("other-{i}")))
                        .collect(),
                ),
            ])],
        )
        .unwrap();
        let target = r.kernel().find_by_test_id("target")[0];
        let id = r.kernel().node_by_key(target).unwrap().id;
        r.dispatch(id, Event::Press).unwrap();
        assert_eq!(
            r.kernel()
                .node_by_key(target)
                .unwrap()
                .props
                .str(PropId::Text),
            Some("target:2")
        );
        let other = r.kernel().find_by_test_id("other-511")[0];
        assert_eq!(
            r.kernel()
                .node_by_key(other)
                .unwrap()
                .props
                .str(PropId::Text),
            Some("other-511:0")
        );
        // The sibling's 512 children cannot make one local write do more work.
        assert_eq!(
            r.last_instance_work().rows_scanned,
            before_growth,
            "{key}: {:?}",
            r.last_instance_work()
        );
    }
}

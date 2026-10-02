//! An incremental update equals a full one (LLP 1005 §8): every app plan,
//! compiled through the Contract API, runs twice in lockstep — once
//! evaluating only what changed, once evaluating everything — through seeded
//! random host events, clock seeks, late replies, store reads and writes,
//! deferred activation, collection feedback (stale reports included) and
//! reorder gestures. Receipts, the kernel tree, carried state, effects, the
//! journal and every collection's protocol state (revision, epochs,
//! measurements) must agree after every step; every run must boot, never
//! poison, and commit.

use exact_kernel::{Kernel, NodeKey, PropId, ViewId};
use exact_plan::{EventKind, Plan, TypeKind, TypesId, Value};
use exact_runner::{
    Answer, CollectionFeedback, DataError, DataSource, Event, Outcome, Request, RowMeasurement,
    Runner, RunnerError, Store,
};
use std::path::Path;

/// Answers every declared source with a value of its declared shape: small
/// lists of scalars that are partly stable across answers (strings are hex
/// colours, so any style row takes them), every other answer an equal but
/// freshly allocated copy of the last one for the same arguments, and —
/// once `later` is set — every third answer a request the test replies to.
/// Every third answer reads the store and every reply writes it; `deferred`
/// makes the source not ready until the test activates it.
#[derive(Default)]
struct Fake {
    plan: Option<Plan>,
    counter: u64,
    later: bool,
    deferred: bool,
    last: std::collections::BTreeMap<String, Value>,
}

fn copy(v: &Value) -> Value {
    match v {
        s @ exact_plan::str_value!() => Value::str(s.text()),
        Value::Option(Some(v)) => Value::some(copy(v)),
        Value::List(items) => Value::list(items.iter().map(copy).collect()),
        Value::Record(fields) => Value::record(fields.iter().map(copy).collect()),
        other => other.clone(),
    }
}

fn mix(a: u64, b: u64) -> u64 {
    (a ^ b.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .rotate_left(27)
        .wrapping_mul(0x2545_f491_4f6c_dd1d)
}

impl Fake {
    /// A value of type `ty` at structural position `path`. About half the
    /// positions are the same in every answer (so a row's key often
    /// survives), the rest differ with `version` (so its contents change).
    fn gen(&mut self, plan: &Plan, ty: TypesId, path: u64, version: u64, depth: usize) -> Value {
        let row = plan.type_(ty).clone();
        let at = mix(path, 1);
        let n = if at.is_multiple_of(2) {
            at
        } else {
            mix(at, version)
        };
        match row.kind {
            TypeKind::Number => Value::Number((n % 997) as f64),
            TypeKind::Bool => Value::Bool(n % 3 == 0),
            TypeKind::String => Value::str(&format!("#{:06x}", n & 0xff_ffff)),
            TypeKind::Unit => Value::Unit,
            TypeKind::Option if depth < 4 && n % 2 == 0 => {
                let elem = row.elem.expect("option element");
                Value::some(self.gen(plan, elem, mix(path, 2), version, depth + 1))
            }
            TypeKind::Option => Value::NONE,
            TypeKind::List => {
                let len = if depth < 3 { (n % 4) as usize } else { 0 };
                let elem = row.elem.expect("list element");
                Value::list(
                    (0..len)
                        .map(|i| self.gen(plan, elem, mix(path, 3 + i as u64), version, depth + 1))
                        .collect(),
                )
            }
            TypeKind::Record => Value::record(
                row.fields
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let ty = plan.field(f).ty;
                        self.gen(plan, ty, mix(path, 100 + i as u64), version, depth + 1)
                    })
                    .collect(),
            ),
        }
    }

    fn value(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let key = format!("{source} {args:?}");
        self.counter += 1;
        if self.counter.is_multiple_of(2) {
            if let Some(last) = self.last.get(&key) {
                return Ok(copy(last));
            }
        }
        let plan = self.plan.take().expect("bound");
        let ty = plan
            .resources
            .iter()
            .find(|r| plan.str(r.source) == source)
            .map(|r| r.ty)
            .or_else(|| {
                plan.mutations
                    .iter()
                    .find(|m| plan.str(m.name) == source)
                    .map(|m| m.ty)
            });
        let root = source.bytes().fold(7, |h, b| mix(h, b as u64));
        let result = match ty {
            Some(ty) => Ok(self.gen(&plan, ty, root, self.counter, 0)),
            None => Err(DataError::UnknownSource(source.into())),
        };
        self.plan = Some(plan);
        if let Ok(value) = &result {
            self.last.insert(key, value.clone());
        }
        result
    }
}

impl DataSource for Fake {
    fn bind(&mut self, plan: &Plan) {
        self.plan = Some(plan.clone());
    }
    fn app_id(&self) -> &str {
        ""
    }
    fn grants(&self) -> &str {
        "secret.keep fake\n"
    }
    fn ready(&self) -> bool {
        !self.deferred
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.value(source, args)
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.counter += 1;
        if self.counter.is_multiple_of(3) {
            let _ = store.get("fake");
        }
        if self.later && self.counter.is_multiple_of(3) {
            return Ok(Answer::Later(Request::get("https://fixture.invalid/")));
        }
        self.value(source, args).map(Answer::Now)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        store.set("fake", &self.counter.to_string())?;
        self.value(source, args).map(Answer::Now)
    }
}

/// A small deterministic generator (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

fn event(rng: &mut Rng, kind: EventKind) -> Option<Event> {
    let text = |rng: &mut Rng| rng.pick(&["", "a", "ab", "hello", "/", "#fff"]).to_string();
    Some(match kind {
        EventKind::Press => Event::Press,
        EventKind::Change => Event::Change(text(rng).into()),
        EventKind::Input => Event::Input(text(rng).into()),
        EventKind::Hover => Event::Hover(rng.below(2) == 0),
        EventKind::Focus => Event::Focus,
        EventKind::Blur => Event::Blur,
        EventKind::Key => Event::Key(rng.pick(&["Enter", "Escape", "ArrowDown", "a"]).to_string()),
        EventKind::Submit => Event::Submit,
        EventKind::Load => Event::Load,
        EventKind::Message => Event::Message(text(rng)),
        EventKind::Contextmenu => Event::Contextmenu,
        EventKind::Dblclick => Event::Dblclick,
        EventKind::Swiperight => Event::Swiperight,
        EventKind::Scroll => Event::Scroll(0.0, (rng.below(5) * 40) as f64),
        EventKind::Navigate => Event::Navigate(rng.pick(&["/", "/t/1", "/nowhere"]).to_string()),
        EventKind::Pan => Event::Pan(rng.below(9) as f64 - 4.0, 0.0),
        EventKind::Panrelease => Event::PanRelease((rng.below(9) as f64 - 4.0) * 250.0, 0.0),
        EventKind::Select => Event::Select {
            formats: text(rng),
            mixed: rng.below(2) == 0,
            link: String::new(),
            unavailable: String::new(),
        },
        EventKind::Heightrelease => Event::HeightRelease {
            height: (rng.below(5) * 50) as f64,
            velocity: 0.0,
        },
        _ => return None,
    })
}

/// Everything observable after one step, for comparison across modes, and
/// the tickets of the requests it handed out.
fn observe<D: DataSource>(r: &mut Runner<D>) -> (String, Vec<u64>) {
    let requests = r.take_requests();
    let tickets = requests.iter().map(|q| q.ticket).collect();
    // Revisions, epochs and measurements included: protocol state is the
    // same in both modes (re-measuring follows inputs that actually changed).
    let collections = r.collections();
    let seen = format!(
        "poisoned {}\nkernel {:?}\ncarry {:?}\ncommands {:?}\nrequests {:?}\nsurfaces {:?}\nrouter {:?}\nstore {:?}\ncollections {:?}\njournal {:?}",
        r.is_poisoned(),
        r.kernel().export(None),
        r.carry(),
        r.take_commands(),
        requests,
        r.take_surface_updates(),
        r.take_router_change(),
        r.take_store_writes(),
        collections,
        r.journal().collect::<Vec<_>>(),
    );
    (seen, tickets)
}

fn outcome(result: Result<exact_kernel::CommitReceipt, RunnerError>) -> String {
    format!("{result:?}")
}

fn boot(plan: &Plan, full: bool, deferred: bool) -> Runner<Fake> {
    let mut r = Runner::boot(
        plan.clone(),
        Fake {
            deferred,
            ..Fake::default()
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap_or_else(|e| panic!("boot (full: {full}): {e:?}"));
    r.set_full_evaluation(full);
    r.data().later = true;
    r
}

/// A host report for a random collection: a scroll offset, a port, a row
/// width that sometimes changes, measured heights for every mounted row
/// (sometimes zero) and, sometimes, the interaction pin on a reorder handle.
fn feedback(r: &Runner<Fake>, rng: &mut Rng) -> Option<CollectionFeedback> {
    let collections = r.collections();
    let c = collections.get(rng.below(collections.len().max(1)))?;
    let handle = handles(r, c.rows.iter().map(|row| row.root).collect())
        .into_iter()
        .next()
        .filter(|_| rng.below(2) == 0)
        .and_then(|key| r.kernel().node_by_key(key).map(|n| n.id));
    Some(CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset: *rng.pick(&[0.0, 30.0, 400.0, 1e5]),
        port_cross: 320.0,
        port_main: *rng.pick(&[120.0, 400.0]),
        cross: *rng.pick(&[320.0, 320.0, 640.0]),
        measurements: c
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                size: *rng.pick(&[0.0, 18.0, 24.0, 60.0]),
            })
            .collect(),
        focus_view: None,
        interaction_view: handle,
    })
}

/// Reorder handles (`reorderFor`) under the given views, in tree order.
fn handles(r: &Runner<Fake>, roots: Vec<ViewId>) -> Vec<NodeKey> {
    let mut out = Vec::new();
    let mut stack = roots;
    while let Some(view) = stack.pop() {
        let Some(node) = r.kernel().node(view) else {
            continue;
        };
        if node.props.str(PropId::ReorderFor).is_some() {
            out.push(node.key);
        }
        stack.extend(node.children().into_iter().rev());
    }
    out
}

/// One reorder gesture on both runners, compared step by step: the host
/// reports the handle's row measured and pinned, then begin, preview at
/// `y`, drop and finish. Tokens carry a process-wide serial, so only what
/// each step committed is compared.
fn reorder(full: &mut Runner<Fake>, incremental: &mut Runner<Fake>, y: f64) -> (String, String) {
    let mut sides = [String::new(), String::new()];
    let roots = full.roots();
    let Some(handle) = handles(full, roots).into_iter().next() else {
        return (String::new(), String::new());
    };
    let pin = full.kernel().node_by_key(handle).map(|n| n.id);
    let list = full.kernel().node_by_key(handle).and_then(|n| {
        let mut at = n;
        while at.node_type != exact_kernel::NodeType::List {
            at = full.kernel().node(at.parent?)?;
        }
        Some(at.id)
    });
    for (side, r) in [full, incremental].into_iter().enumerate() {
        let out = &mut sides[side];
        if let Some(c) = r.collections().into_iter().find(|c| Some(c.view) == list) {
            let report = CollectionFeedback {
                view: c.view,
                revision: c.revision,
                scroll_sequence: c.scroll_sequence + 1,
                offset: 0.0,
                port_cross: 320.0,
                port_main: 400.0,
                cross: 320.0,
                measurements: c
                    .rows
                    .iter()
                    .map(|row| RowMeasurement {
                        view: row.view,
                        epoch: row.epoch,
                        size: 24.0,
                    })
                    .collect(),
                focus_view: None,
                interaction_view: pin,
            };
            out.push_str(&format!("pin {:?}; ", r.collection_feedback(report)));
        }
        let Some(binding) = r.reorder_binding(handle) else {
            out.push_str("no binding");
            continue;
        };
        let geometry = r.reorder_geometry(binding.list).unwrap();
        let start = r.begin_reorder(binding, geometry);
        out.push_str(&format!(
            "begin {:?}; ",
            start.as_ref().map(|s| s.as_ref().map(|s| &s.receipt))
        ));
        let Ok(Some(start)) = start else { continue };
        let geometry = r.reorder_geometry(binding.list).unwrap();
        let preview = r.preview_reorder(start.token, geometry, y);
        out.push_str(&format!("preview {preview:?}; "));
        let geometry = r.reorder_geometry(binding.list).unwrap();
        out.push_str(&format!(
            "drop {:?}; ",
            r.drop_reorder(start.token, geometry)
        ));
        out.push_str(&format!("finish {:?}", r.finish_reorder(start.token)));
    }
    let [a, b] = sides;
    (a, b)
}

/// Drive both runners through `steps` seeded steps; the number of commits.
fn lockstep(app: &str, plan: &Plan, seed: u64, steps: usize, deferred: bool) -> usize {
    let mut full = boot(plan, true, deferred);
    let mut incremental = boot(plan, false, deferred);
    assert_eq!(observe(&mut full), observe(&mut incremental), "{app}: boot");
    let mut rng = Rng(seed);
    let mut tickets: Vec<u64> = Vec::new();
    let mut reports: Vec<CollectionFeedback> = Vec::new();
    let mut commits = 0;
    for step in 0..steps {
        let handlers = full.handlers();
        assert_eq!(handlers, incremental.handlers(), "{app} step {step}");
        let what;
        let (a, b) = match rng.below(12) {
            _ if deferred && step == steps / 3 => {
                what = "activation".to_string();
                full.data().deferred = false;
                incremental.data().deferred = false;
                (
                    format!("{:?}", full.data_ready()),
                    format!("{:?}", incremental.data_ready()),
                )
            }
            0 if full.has_timers() => {
                let to = full.now_ms() + (rng.below(4) * 250) as f64;
                what = format!("advance {to}");
                let a = full.advance(to).map(|r| r.len());
                let b = incremental.advance(to).map(|r| r.len());
                (format!("{a:?}"), format!("{b:?}"))
            }
            1 if !tickets.is_empty() => {
                let ticket = tickets.remove(rng.below(tickets.len()));
                what = format!("fulfill {ticket}");
                let reply = || Outcome::Storage(Vec::new());
                let a = full.fulfill(ticket, reply());
                let b = incremental.fulfill(ticket, reply());
                (format!("{a:?}"), format!("{b:?}"))
            }
            2 | 3 => {
                let Some(report) = feedback(&full, &mut rng) else {
                    continue;
                };
                what = format!("collection feedback {report:?}");
                reports.push(report.clone());
                (
                    format!("{:?}", full.collection_feedback(report.clone())),
                    format!("{:?}", incremental.collection_feedback(report)),
                )
            }
            4 if !reports.is_empty() => {
                // An old report: stale revisions and epochs are ignored alike.
                let report = reports[rng.below(reports.len())].clone();
                what = format!("stale feedback {report:?}");
                (
                    format!("{:?}", full.collection_feedback(report.clone())),
                    format!("{:?}", incremental.collection_feedback(report)),
                )
            }
            5 => {
                let y = *rng.pick(&[0.0, 40.0, 500.0]);
                what = format!("reorder to {y}");
                reorder(&mut full, &mut incremental, y)
            }
            _ => {
                let views: Vec<(&ViewId, &Vec<EventKind>)> = handlers.iter().collect();
                if views.is_empty() {
                    continue;
                }
                let (view, kinds) = *rng.pick(&views);
                let kind = *rng.pick(kinds);
                let Some(event) = event(&mut rng, kind) else {
                    continue;
                };
                what = format!("{event:?} on view {view}");
                let result = incremental.dispatch(*view, event.clone());
                // A view with listeners is always found where it lives.
                assert!(
                    !matches!(result, Err(RunnerError::UnknownView(_))),
                    "{app} step {step}: {what} found no instance"
                );
                (outcome(full.dispatch(*view, event)), outcome(result))
            }
        };
        assert_eq!(a, b, "{app} step {step}: {}", clip(&what));
        commits +=
            usize::from(a.contains("Ok(") && !a.contains("Ok(None)") && !a.contains("Ok([])"));
        let (seen, handed) = observe(&mut full);
        tickets.extend(handed);
        let (other, _) = observe(&mut incremental);
        if seen != other {
            let diff = seen
                .lines()
                .zip(other.lines())
                .find(|(a, b)| a != b)
                .map(|(a, b)| format!("full:        {}\nincremental: {}", clip(a), clip(b)));
            panic!(
                "{app} step {step} ({}) diverged:\n{}",
                clip(&what),
                diff.unwrap_or_default()
            );
        }
        assert!(
            !full.is_poisoned(),
            "{app} step {step}: {} poisoned the runner: {}",
            clip(&what),
            clip(&a)
        );
    }
    commits
}

fn clip(s: &str) -> &str {
    &s[..s.len().min(2000)]
}

/// What the apps may not exercise: row-owned state, `now()` read by a
/// binding under a timer, nested lists reading the outer item,
/// `when`/`match` inside rows, both windowed lists (one scrolled by a
/// binding), and a list reordered by its rows' grips.
const DECK: &str = r#"
shape Tag
  id: string
  name: string
shape Item
  id: string
  label: string
  n: number
  note: option<string>
  tags: list<Tag>
component Deck
  state query = ""
  state picked = ""
  state revision = 0
  state shown = true
  state stamp = 0
  state scrollTo = 0
  state moved = ""
  resource items = items(revision) as shape list<Item>
  derive title = `${query} ${length(items)} ${stamp}`
  task clock mount
    every(250, tick)
  action tick
    stamp = stamp + 1
  action typeQuery(value)
    query = value
  action pick(id: string)
    picked = id
  action revise
    revision = revision + 1
  action toggle
    shown = not shown
  action jump
    scrollTo = scrollTo + 40
  action moveItem(item: string, before: option<string>)
    moved = item
  view
    column
      input value=query change=typeQuery testId="q"
      text title
      text `${now()}`
      button press=revise
        text "revise"
      button press=toggle
        text "toggle"
      button press=jump
        text "jump"
      list virtualized=true estimated-item-height=24 height=200 scrollTop=scrollTo
        each it in items key=it.id
          column width="100%"
            text `${it.label} ${picked}`
      list virtualized=true height=200
        each it in items key=it.id
          column width="100%"
            text `${it.n} ${picked}`
            text join(map(it.tags, (t, i) => `${i}:${t.name}`), ",")
      text moved
      list id="deck-list" reorderdrop=moveItem virtualized=true height=200
        each it in items key=it.id
          column width="100%"
            box reorderFor="deck-list" padding=4
              text it.label
      when shown
        column
          each it in items key=it.id
            column
              Card(item=it, picked=picked)
              text join(filter(map(it.tags, t => t.name), n => n != picked), " ")
              button press=pick(it.id)
                text `${now()} ${it.n}`
component Card
  props
    item: Item
    picked: string
  state open = false
  state taps = 0
  action flip
    open = not open
    taps = taps + 1
  view
    column
      button press=flip
        text `${item.label} ${taps}`
      text (picked == item.id ? "picked" : "")
      when open
        each t in item.tags key=t.id
          text `${t.name} ${item.n} ${taps}`
      match item.note
        case some(s)
          text s
        case none
          text "no note"
"#;

#[test]
fn every_app_plan_updates_incrementally_exactly_as_it_does_in_full() {
    let apps = Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps");
    let mut names: Vec<_> = std::fs::read_dir(&apps)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("app.contract").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(names.len() > 10, "{names:?}");
    names.insert(0, "deck (synthetic)".into());
    // One thread per app and seed: each builds its own runners.
    let total: usize = std::thread::scope(|scope| {
        let apps = &apps;
        let runs: Vec<_> = names
            .iter()
            .map(|app| {
                scope.spawn(move || {
                    let plan = if app.starts_with("deck") {
                        contract::compile(DECK).unwrap_or_else(|e| panic!("{app}: {e}"))
                    } else {
                        contract::compile_path(&apps.join(app).join("app.contract"))
                            .unwrap_or_else(|e| panic!("{app}: {e}"))
                    };
                    // Seeds 2 and 4 boot a baked plan against a source that
                    // is not ready, and activate it a third of the way in.
                    let baked = contract::bake(plan.clone(), Fake::default())
                        .unwrap_or_else(|e| panic!("{app}: bake: {e:?}"));
                    let (plan, baked) = (&plan, &baked);
                    // And one per seed.
                    std::thread::scope(|seeds| {
                        let runs: Vec<_> = (1..=4u64)
                            .map(|seed| {
                                seeds.spawn(move || {
                                    let deferred = seed % 2 == 0;
                                    let steps = 60;
                                    let commits = lockstep(
                                        app,
                                        if deferred { baked } else { plan },
                                        seed.wrapping_mul(0x9e37_79b9_7f4a_7c15),
                                        steps,
                                        deferred,
                                    );
                                    eprintln!("{app} seed {seed}: {commits} commits");
                                    assert!(
                                        commits >= steps / 4,
                                        "{app} seed {seed}: only {commits} commits"
                                    );
                                    commits
                                })
                            })
                            .collect();
                        runs.into_iter()
                            .map(|run| run.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
                            .sum::<usize>()
                    })
                })
            })
            .collect();
        runs.into_iter()
            .map(|run| run.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .sum()
    });
    assert!(
        total > 100,
        "only {total} commits across {} apps",
        names.len()
    );
}

/// Store provenance travels through derives and resource arguments even
/// when no value changes (Astra, 2026-09-22): input → first → second →
/// dependent, and input starts reading the store while answering the same.
#[test]
fn store_provenance_propagates_through_unchanged_values() {
    #[derive(Default)]
    struct Seed {
        read: bool,
    }
    impl DataSource for Seed {
        fn grants(&self) -> &str {
            "secret.keep token\n"
        }
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!("answer is implemented")
        }
        fn answer(
            &mut self,
            store: &mut Store,
            source: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            if source == "seed" && self.read {
                let _ = store.get("token");
            }
            Ok(Answer::Now(Value::Number(7.0)))
        }
    }
    let plan = contract::compile(
        "component Probe
  state revision = 0
  resource input = seed(revision) as shape number
  derive first = input
  derive second = first
  resource dependent = consume(second) as shape number
  action change
    revision = revision + 1
  view
    text `${dependent}`
",
    )
    .unwrap();
    for full in [false, true] {
        let mut r = Runner::boot(
            plan.clone(),
            Seed::default(),
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.set_full_evaluation(full);
        r.data().read = true;
        r.act("change", vec![]).unwrap();
        assert!(r.resource_reads_store("input"), "full={full}");
        assert!(r.resource_reads_store("dependent"), "full={full}");
    }
}

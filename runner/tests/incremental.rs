//! An incremental update equals a full one (LLP 1005 §8): every app plan,
//! compiled through the Contract API, runs twice in lockstep — once
//! evaluating only what changed, once evaluating everything — through seeded
//! random host events, clock seeks and late replies. Receipts, the kernel
//! tree, carried state, effects and the journal must agree after every step.

use exact_kernel::{Kernel, ViewId};
use exact_plan::{EventKind, Plan, TypeKind, TypesId, Value};
use exact_runner::{
    Answer, DataError, DataSource, Event, Outcome, Request, Runner, RunnerError, Store,
};
use std::path::Path;

/// Answers every declared source with a value of its declared shape: small
/// lists of scalars that are partly stable across answers (strings are hex
/// colours, so any style row takes them), every other answer an equal but
/// freshly allocated copy of the last one for the same arguments, and —
/// once `later` is set — every third answer a request the test replies to.
#[derive(Default)]
struct Fake {
    plan: Option<Plan>,
    counter: u64,
    later: bool,
    last: std::collections::BTreeMap<String, Value>,
}

fn copy(v: &Value) -> Value {
    match v {
        Value::Str(s) => Value::str(s),
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
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.value(source, args)
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        self.counter += 1;
        if self.later && self.counter.is_multiple_of(3) {
            return Ok(Answer::Later(Request::get("https://fixture.invalid/")));
        }
        self.value(source, args).map(Answer::Now)
    }
    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
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
        EventKind::Change => Event::Change(text(rng)),
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
    let mut collections = r.collections();
    for c in &mut collections {
        // Protocol counters: a full update re-measures every collection on
        // every commit; they order host feedback and are not what it shows.
        c.revision = 0;
        for row in &mut c.rows {
            row.epoch = 0;
        }
    }
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

fn boot(plan: &Plan, full: bool) -> Result<Runner<Fake>, RunnerError> {
    let mut r = Runner::boot(
        plan.clone(),
        Fake::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )?;
    r.set_full_evaluation(full);
    r.data().later = true;
    Ok(r)
}

/// Drive both runners through `steps` seeded steps; the number of commits.
fn lockstep(app: &str, plan: &Plan, seed: u64, steps: usize) -> usize {
    let (mut full, mut incremental) = match (boot(plan, true), boot(plan, false)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(a), Err(b)) => {
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{app}: boot");
            eprintln!("{app}: boot refused: {}", clip(&format!("{a:?}")));
            return 0;
        }
        (a, b) => panic!("{app}: boot differs: {:?} / {:?}", a.err(), b.err()),
    };
    assert_eq!(observe(&mut full), observe(&mut incremental), "{app}: boot");
    let mut rng = Rng(seed);
    let mut tickets: Vec<u64> = Vec::new();
    let mut commits = 0;
    for step in 0..steps {
        let handlers = full.handlers();
        assert_eq!(handlers, incremental.handlers(), "{app} step {step}");
        let what;
        let (a, b) = match rng.below(10) {
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
        assert_eq!(a, b, "{app} step {step}: {what}");
        commits += usize::from(a.starts_with("Ok"));
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
                "{app} step {step} ({what}) diverged:\n{}",
                diff.unwrap_or_default()
            );
        }
        if full.is_poisoned() {
            eprintln!("{app} poisoned at step {step} by {what}: {}", clip(&a));
            break;
        }
    }
    commits
}

fn clip(s: &str) -> &str {
    &s[..s.len().min(2000)]
}

/// What the apps may not exercise: row-owned state, `now()` read by a
/// binding under a timer, nested lists reading the outer item, and
/// `when`/`match` inside rows.
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
  resource items = items(revision) as shape list<Item>
  derive title = `${query} ${length(items)} ${stamp}`
  task clock mount
    every(250, tick)
  action tick writes stamp
    stamp = stamp + 1
  action typeQuery(value) writes query
    query = value
  action pick(id: string) writes picked
    picked = id
  action revise writes revision
    revision = revision + 1
  action toggle writes shown
    shown = not shown
  view
    column
      input value=query change=typeQuery testId="q"
      text title
      text `${now()}`
      button press=revise
        text "revise"
      button press=toggle
        text "toggle"
      when shown
        column
          each it in items key=it.id
            column
              Card(item=it, picked=picked)
              button press=pick(it.id)
                text `${now()} ${it.n}`
component Card
  props
    item: Item
    picked: string
  state open = false
  state taps = 0
  action flip writes open, taps
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
    let mut total = 0;
    for app in &names {
        let plan = if app.starts_with("deck") {
            contract::compile(DECK).unwrap_or_else(|e| panic!("{app}: {e}"))
        } else {
            contract::compile_path(&apps.join(app).join("app.contract"))
                .unwrap_or_else(|e| panic!("{app}: {e}"))
        };
        for seed in 1..=4u64 {
            let commits = lockstep(app, &plan, seed.wrapping_mul(0x9e37_79b9_7f4a_7c15), 60);
            eprintln!("{app} seed {seed}: {commits} commits");
            total += commits;
        }
    }
    assert!(
        total > 100,
        "only {total} commits across {} apps",
        names.len()
    );
}

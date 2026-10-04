//! A child component's state lives exactly as long as its instance (LLP
//! 1017 P4c): created when the instance is — an `each` row, a `when` or
//! `match` arm, or the root at the boot render — its initializer evaluated
//! then, after settlement, in the use site's scope; dropped with it.

use exact_kernel::{Kernel, PropId};
use exact_runner::{
    Answer, DataError, DataSource, Event, Outcome, Request, Response, Runner, Store, Value,
};
use std::cell::Cell;
use std::rc::Rc;

/// `items()` answers two records now; `wait()` answers later; `tick(n)`
/// refuses once `n` reaches `refuse_from`.
#[derive(Default)]
struct Items {
    refuse_from: Rc<Cell<f64>>,
}

impl DataSource for Items {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "items" => Ok(Value::list(
                [2.0, 3.0]
                    .map(|n| Value::record(vec![Value::Number(n)]))
                    .to_vec(),
            )),
            "tick" => {
                let n = args[0].as_number().unwrap_or(0.0);
                if n >= self.refuse_from.get() {
                    return Err(DataError::Unavailable("tick".into()));
                }
                Ok(Value::Number(n))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if source == "wait" {
            return Ok(Answer::Later(Request::get("https://wait.test/")));
        }
        self.query(source, args).map(Answer::Now)
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(Value::list(
            [5.0, 6.0, 7.0]
                .map(|n| Value::record(vec![Value::Number(n)]))
                .to_vec(),
        )))
    }
}

fn boot(src: &str) -> Runner<Items> {
    boot_with(src, Items::default())
}

fn boot_with(src: &str, data: Items) -> Runner<Items> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e:?}"));
    Runner::boot(
        plan,
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn texts(r: &Runner<Items>, id: &str) -> Vec<String> {
    let k = r.kernel();
    k.find_by_test_id(id)
        .into_iter()
        .map(|key| {
            k.node_by_key(key)
                .unwrap()
                .props
                .str(PropId::Text)
                .unwrap()
                .to_string()
        })
        .collect()
}

fn text(r: &Runner<Items>, id: &str) -> String {
    let all = texts(r, id);
    assert_eq!(all.len(), 1, "{id}: {all:?}");
    all[0].clone()
}

fn press(r: &mut Runner<Items>, id: &str) -> Result<(), exact_runner::RunnerError> {
    let key = r.kernel().find_by_test_id(id)[0];
    let view = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(view, Event::Press).map(|_| ())
}

const COUNTER: &str = r#"
component Counter
  props
    start: number
    id: string
  state c = start
  action bump
    c = c + 1
  view
    text `${c}` press=bump testId=id
"#;

#[test]
fn a_child_state_initializes_from_a_resource_a_derive_and_a_binding_everywhere() {
    let src = format!(
        r#"
shape P
  n: number
component App
  state picked = some(P(n=40))
  resource items = items() as shape list<P>
  derive total = length(items) * 10
  view
    column
      Counter(start=length(items), id="top-resource")
      Counter(start=total, id="top-derive")
      when total > 0
        Counter(start=total + 1, id="when-derive")
        Counter(start=length(items) + 1, id="when-resource")
      match picked
        case some(p)
          Counter(start=p.n, id="match-binding")
          Counter(start=p.n + total, id="match-both")
        case none
          text "none"
      each p in items key=toString(p.n)
        Counter(start=p.n + total, id="each")
{COUNTER}"#
    );
    let r = boot(&src);
    assert_eq!(text(&r, "top-resource"), "2");
    assert_eq!(text(&r, "top-derive"), "20");
    assert_eq!(text(&r, "when-derive"), "21");
    assert_eq!(text(&r, "when-resource"), "3");
    assert_eq!(text(&r, "match-binding"), "40");
    assert_eq!(text(&r, "match-both"), "60");
    assert_eq!(texts(&r, "each"), ["22", "23"]);
}

#[test]
fn hiding_a_when_arm_drops_its_childs_state_and_showing_it_starts_again() {
    let src = format!(
        r#"
component App
  state shown = true
  state base = 5
  action toggle
    shown = !shown
  action rebase
    base = base + 100
  view
    column
      when shown
        Counter(start=base, id="c")
      else
        Counter(start=base * 2, id="other")
{COUNTER}"#
    );
    let mut r = boot(&src);
    assert_eq!(text(&r, "c"), "5");
    // Writes from the child's own action persist while the arm shows.
    press(&mut r, "c").unwrap();
    press(&mut r, "c").unwrap();
    assert_eq!(text(&r, "c"), "7");
    // An initializer runs once per instance: a later change to what it
    // read does not run it again.
    r.act("rebase", vec![]).unwrap();
    assert_eq!(text(&r, "c"), "7");
    // Hidden: the arm's state is gone; the else arm starts its own.
    r.act("toggle", vec![]).unwrap();
    assert!(texts(&r, "c").is_empty());
    assert_eq!(text(&r, "other"), "210");
    press(&mut r, "other").unwrap();
    assert_eq!(text(&r, "other"), "211");
    // Shown again: a new instance, initialized now, from what it reads now.
    r.act("toggle", vec![]).unwrap();
    assert_eq!(text(&r, "c"), "105");
    r.act("toggle", vec![]).unwrap();
    assert_eq!(text(&r, "other"), "210", "the else arm restarted too");
}

#[test]
fn a_match_arm_keeps_its_childs_state_while_its_binding_changes() {
    let src = format!(
        r#"
component App
  state v = some(1)
  action next
    v = match v {{ case some(n) => some(n + 1), case none => none }}
  action clear
    v = none
  action set
    v = some(10)
  view
    column
      match v
        case some(n)
          Counter(start=n, id="c")
          text `${{n}}` testId="n"
        case none
          text "none"
{COUNTER}"#
    );
    let mut r = boot(&src);
    assert_eq!(text(&r, "c"), "1");
    press(&mut r, "c").unwrap();
    r.act("next", vec![]).unwrap();
    assert_eq!(text(&r, "n"), "2");
    assert_eq!(
        text(&r, "c"),
        "2",
        "same arm, same instance: its state stays"
    );
    r.act("clear", vec![]).unwrap();
    r.act("set", vec![]).unwrap();
    assert_eq!(
        text(&r, "c"),
        "10",
        "a new instance starts from its binding"
    );
}

#[test]
fn a_refused_commit_rolls_back_a_write_to_an_arms_child_state() {
    let src = r#"
component App
  state shown = true
  resource t = tick(floor(now() / 1000)) as shape number
  view
    column
      text `${t}` testId="t"
      when shown
        Counter(start=1, id="c")

component Counter
  props
    start: number
    id: string
  state c = start
  action bump
    c = c + 1
  view
    text `${c}` press=bump testId=id
"#;
    let refuse_from = Rc::new(Cell::new(f64::INFINITY));
    let mut r = boot_with(
        src,
        Items {
            refuse_from: refuse_from.clone(),
        },
    );
    press(&mut r, "c").unwrap();
    assert_eq!(text(&r, "c"), "2");
    // The next commit asks `tick` again, at a later second, and it refuses:
    // the child's write is undone with the rest of the commit.
    refuse_from.set(1.0);
    r.advance(5_000.0).unwrap();
    let before = r.kernel().export(None).unwrap();
    let refused = press(&mut r, "c").unwrap_err();
    assert!(
        matches!(&refused, exact_runner::RunnerError::Data { resource, .. } if resource == "t"),
        "{refused:?}"
    );
    assert_eq!(r.kernel().export(None).unwrap(), before);
    assert!(!r.is_poisoned());
    refuse_from.set(f64::INFINITY);
    press(&mut r, "c").unwrap();
    assert_eq!(text(&r, "c"), "3", "the refused bump left no trace");
}

#[test]
fn an_initializer_reads_a_resource_that_has_not_answered_as_its_placeholder() {
    let src = format!(
        r#"
shape P
  n: number
component App
  resource later = wait() as shape list<P>
  view
    column
      text `${{length(later)}}` testId="len"
      Counter(start=length(later), id="c")
{COUNTER}"#
    );
    let mut r = boot(&src);
    assert_eq!(text(&r, "len"), "0");
    assert_eq!(text(&r, "c"), "0", "the placeholder: an empty list");
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(
        ticket,
        Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: Vec::new(),
        }),
    )
    .unwrap();
    assert_eq!(text(&r, "len"), "3");
    assert_eq!(text(&r, "c"), "0", "the answer does not run it again");
}

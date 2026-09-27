//! A resource's compiled value is released once nothing outside the runner
//! holds it (`exact_runner::held`): a source that adopted it (LLP 1027 D11)
//! answers its edits through a mutation, as a live list does, and the
//! baked copy of what it replaced is freed. A later read decodes the
//! plan's bytes again: the value never changes for a reader.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};
use std::rc::{Rc, Weak};

const SRC: &str = r#"
shape Row
  id: string
  text: string
shape Feed
  rows: list<Row>

component App
  state peek = false
  resource initial = feed() as shape Feed
  mutation changed as shape Feed
  derive feed = match changed { case some(m) => m, case none => initial }
  derive baked = peek ? length(initial.rows) : 0
  action edit writes changed
    send changed = edit()
  action look writes peek
    peek = true

  view
    column
      text `${baked}` testId="baked"
      each r in feed.rows key=r.id
        text r.text
"#;

fn row(id: usize, text: &str) -> Value {
    Value::record(vec![Value::str(&id.to_string()), Value::str(text)])
}

/// The feed, and after adopting the runner's copy, its edits: each `edit`
/// replaces the first row and shares the rest.
#[derive(Default)]
struct Feed {
    rows: Vec<Value>,
    edits: usize,
}

impl Feed {
    fn value(&self) -> Value {
        Value::record(vec![Value::list(self.rows.clone())])
    }
}

impl DataSource for Feed {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        if self.rows.is_empty() {
            self.rows = (0..3).map(|i| row(i, "baked")).collect();
        }
        match source {
            "feed" => Ok(self.value()),
            "edit" => {
                self.edits += 1;
                self.rows[0] = row(0, &format!("edit {}", self.edits));
                Ok(self.value())
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn adopt(&mut self, source: &str, _: &[Value], value: &Value) {
        if let ("feed", Value::Record(fields)) = (source, value) {
            if let Value::List(rows) = &fields[0] {
                self.rows = rows.to_vec();
            }
        }
    }
}

/// The resource's record and its first row, weakly: whether the runner's
/// decoded copy is still alive.
fn weak(r: &Runner<Feed>) -> (Weak<[Value]>, Weak<[Value]>) {
    let Some(Value::Record(record)) = r.resource("initial") else {
        panic!("a record")
    };
    let Value::List(rows) = &record[0] else {
        panic!("a list")
    };
    let Value::Record(first) = &rows[0] else {
        panic!("a row")
    };
    (Rc::downgrade(record), Rc::downgrade(first))
}

fn text(r: &Runner<Feed>, id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}

#[test]
fn an_adopted_value_replaced_on_screen_is_released_and_reads_the_same() {
    let plan = contract::bake(contract::compile(SRC).unwrap(), Feed::default()).unwrap();
    let baked = Value::from_bytes(plan.bytes(plan.resources[0].initial)).unwrap();
    let mut r = Runner::boot(
        plan.clone(),
        Feed::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let (record, first) = weak(&r);
    // The list shows it: held.
    r.advance(0.).unwrap();
    assert!(record.upgrade().is_some() && first.upgrade().is_some());

    r.act("edit", vec![]).unwrap();
    // Replaced on screen by the source's edit, which shares rows 1 and 2:
    // the record and the row it replaced are freed.
    assert!(record.upgrade().is_none(), "the baked record is released");
    assert!(first.upgrade().is_none(), "the replaced row with it");

    // Read for inspection, it is the same value, decoded again, and
    // released again after the next update.
    assert_eq!(r.resource("initial"), Some(&baked));
    assert!(exact_runner::agent::state(&r).contains("\"baked\""));
    let (again, _) = weak(&r);
    r.act("edit", vec![]).unwrap();
    assert!(
        again.upgrade().is_none(),
        "an inspection's copy is released"
    );

    // A reload carries the value as it was compiled.
    let carried = r.carry();
    let reloaded = Runner::boot_carrying(
        plan.clone(),
        Feed::default(),
        Kernel::with_monospace(),
        &carried,
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(reloaded.resource("initial"), Some(&baked));

    // An expression that reads it after the release sees the same value,
    // and keeps it from then on: no decode per evaluation.
    r.act("look", vec![]).unwrap();
    assert_eq!(text(&r, "baked"), "3");
    let (kept, _) = weak(&r);
    r.act("edit", vec![]).unwrap();
    assert!(kept.upgrade().is_some(), "read by an expression: kept");
    assert_eq!(r.resource("initial"), Some(&baked));
}

#[test]
fn a_value_still_on_screen_is_never_released() {
    let plan = contract::bake(contract::compile(SRC).unwrap(), Feed::default()).unwrap();
    let mut r = Runner::boot(
        plan,
        Feed::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let (record, first) = weak(&r);
    for _ in 0..5 {
        r.act("look", vec![]).unwrap();
        r.advance(100.).unwrap();
    }
    assert!(record.upgrade().is_some() && first.upgrade().is_some());
}

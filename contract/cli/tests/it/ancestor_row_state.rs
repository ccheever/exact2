//! A write to a row's state reaches every binding below that row that reads
//! it: one in the row itself, one in a keyed row nested inside it, one in an
//! arm, one in a child component given it as a prop — and an event inside a
//! nested row writing the outer row's state, with sibling rows untouched.
//! Incremental evaluation answers what full evaluation does. c39fc7ffc fixed
//! the nested rows; these are the shapes the Brooks port found it in.

use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

/// `items()` answers `a` and `b`, each with one tag.
struct Items;
impl DataSource for Items {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "items" => Ok(Value::list(
                ["a", "b"]
                    .map(|id| {
                        Value::record(vec![
                            Value::str(id),
                            Value::list(vec![Value::record(vec![Value::str(&format!("{id}1"))])]),
                        ])
                    })
                    .to_vec(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

const ROWS: &str = r#"shape Tag
  id: string
shape Item
  id: string
  tags: list<Tag>

component App
  resource items = items() as shape list<Item>
  view
    column
      each it in items key=it.id
        Row(item=it)

component Row
  props
    item: Item
  state n = 0
  action bump
    n = n + 1
  view
    column
      button press=bump testId=`bump-${item.id}`
        text "bump"
      text `${n}` testId=`direct-${item.id}`
      each t in item.tags key=t.id
        column
          text `${n}` testId=`keyed-${t.id}`
          button press=bump testId=`inner-${t.id}`
            text "inner"
      when n >= 0
        text `${n}` testId=`arm-${item.id}`
      Count(n=n, id=item.id)

component Count
  props
    n: number
    id: string
  view
    text `${n}` testId=`child-${id}`
"#;

fn boot(full: bool) -> Runner<Items> {
    let plan = contract::compile(ROWS).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        Items,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_full_evaluation(full);
    r
}

fn text_of(r: &Runner<Items>, id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .iter()
        .find_map(|(p, v)| match v {
            PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
            _ => None,
        })
        .unwrap()
}

fn press(r: &mut Runner<Items>, id: &str) {
    let k = r.kernel();
    let view = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}

/// Every reader of each row's `n`, in order: direct, keyed, arm, child.
fn readers(r: &Runner<Items>) -> Vec<String> {
    ["a", "b"]
        .iter()
        .flat_map(|id| {
            [
                format!("direct-{id}"),
                format!("keyed-{id}1"),
                format!("arm-{id}"),
                format!("child-{id}"),
            ]
        })
        .map(|test_id| format!("{test_id}={}", text_of(r, &test_id)))
        .collect()
}

#[test]
fn a_row_state_write_reaches_its_nested_readers_and_no_sibling() {
    for full in [true, false] {
        let mut r = boot(full);
        press(&mut r, "bump-a");
        assert_eq!(
            readers(&r),
            [
                "direct-a=1",
                "keyed-a1=1",
                "arm-a=1",
                "child-a=1",
                "direct-b=0",
                "keyed-b1=0",
                "arm-b=0",
                "child-b=0"
            ],
            "after the row's own press (full: {full})"
        );
        // An event inside the nested keyed row writes the outer row's state.
        press(&mut r, "inner-b1");
        assert_eq!(
            readers(&r),
            [
                "direct-a=1",
                "keyed-a1=1",
                "arm-a=1",
                "child-a=1",
                "direct-b=1",
                "keyed-b1=1",
                "arm-b=1",
                "child-b=1"
            ],
            "after the nested row's press (full: {full})"
        );
    }
}

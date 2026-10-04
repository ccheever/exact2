//! What the checker learns late still types what reads it early: a state
//! typed only by an action's write, as derives, view heads and handler
//! arguments read it; and a prop's declared type, as the `none` passed to
//! it (LLP 1004 D3).

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct Items;
impl DataSource for Items {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "ps" => Ok(Value::list(
                [2.0, 3.0]
                    .map(|n| Value::record(vec![Value::Number(n)]))
                    .to_vec(),
            )),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn boot(src: &str) -> Runner<Items> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e:?}"));
    let plan = contract::bake(plan, Items).unwrap();
    Runner::boot(
        plan,
        Items,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn text(r: &Runner<Items>, id: &str) -> String {
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
fn a_state_typed_by_a_write_types_the_derives_that_read_it() {
    let mut r = boot(
        r#"
shape P
  n: number
component App
  state s = none
  state l = []
  resource ps = ps() as shape list<P>
  derive d = match s { case some(x) => x.n, case none => 0 }
  derive m = map(l, x => x.n)
  derive e = length(m)
  action a
    s = some(P(n=1))
    l = ps
  view
    column
      text `${d}` testId="d"
      text `${e}` testId="e"
      when e > 0
        text "some" testId="when"
      match s
        case some(p)
          text `${p.n}` testId="match"
        case none
          text "none" testId="match"
      each p in l key=toString(p.n)
        text `${p.n}` testId="each"
"#,
    );
    assert_eq!(text(&r, "d"), "0");
    assert_eq!(text(&r, "e"), "0");
    assert_eq!(text(&r, "match"), "none");
    r.act("a", vec![]).unwrap();
    assert_eq!(text(&r, "d"), "1");
    assert_eq!(text(&r, "e"), "2");
    assert_eq!(text(&r, "when"), "some");
    assert_eq!(text(&r, "match"), "1");
    assert_eq!(r.kernel().find_by_test_id("each").len(), 2);
}

#[test]
fn a_state_typed_by_a_write_types_handler_arguments() {
    let mut r = boot(
        r#"
shape P
  n: number
component App
  state s = none
  state got = 0
  action a
    s = some(P(n=4))
  action take(v)
    got = v
  view
    column
      match s
        case some(p)
          button "take" press=take(p.n) testId="take"
        case none
          text "none"
      text `${got}` testId="got"
"#,
    );
    r.act("a", vec![]).unwrap();
    r.act("take", vec![Value::Number(4.0)]).unwrap();
    assert_eq!(text(&r, "got"), "4");
}

#[test]
fn a_props_declared_type_types_its_argument() {
    let r = boot(
        r#"
component App
  view
    column
      C(o=none)
      C(o=some(3))
component C
  props
    o: option<number>
  view
    text (match o { case some(v) => toString(v), case none => "none" }) testId="o"
"#,
    );
    let k = r.kernel();
    let shown: Vec<String> = k
        .find_by_test_id("o")
        .into_iter()
        .map(|key| {
            k.node_by_key(key)
                .unwrap()
                .props
                .str(PropId::Text)
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(shown, ["none", "3"]);
}

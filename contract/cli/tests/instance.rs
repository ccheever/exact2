//! LLP 1017 P4c: per-instance state, proven on the runner — a row's state
//! follows its key, a use's own state is its own, and neither leaks.

use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

#[derive(Default)]
struct Stations;

impl DataSource for Stations {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let station = |id: &str, name: &str| Value::record(vec![Value::str(id), Value::str(name)]);
        match source {
            "stations" => {
                let asc = matches!(args.first(), Some(Value::Str(s)) if s.as_ref() == "asc");
                let mut rows = vec![station("mv", "Mountain View"), station("pa", "Palo Alto")];
                if !asc {
                    rows.reverse();
                }
                Ok(Value::list(rows))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn text_of(r: &Runner<Stations>, id: &str) -> String {
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

fn view_of(r: &Runner<Stations>, id: &str) -> u32 {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    k.node_by_key(key).unwrap().id
}

#[test]
fn a_use_owns_its_state_and_a_row_owns_its_own_which_follows_its_key() {
    let plan = contract::compile(&corpus("instance.contract")).unwrap();
    let plan = contract::bake(plan, Stations).unwrap();
    let mut r = Runner::boot(plan, Stations, Kernel::with_monospace()).unwrap();
    // Two uses of Counter: two states, two derives, two actions.
    assert_eq!(text_of(&r, "count-text-a"), "a 0 0");
    let a = view_of(&r, "count-a");
    r.dispatch(a, Event::Press).unwrap();
    r.dispatch(a, Event::Press).unwrap();
    assert_eq!(text_of(&r, "count-text-a"), "a 2 4");
    assert_eq!(text_of(&r, "count-text-b"), "b 0 0");
    // A singleton's state is a root slot by its lifted name; carried by name.
    assert_eq!(r.slot("n__1"), Some(&Value::Number(2.0)));
    assert!(r.carry().slots.iter().any(|(n, _)| n == "n__1"));
    // Rows: hover one, the other is untouched.
    let mv = view_of(&r, "station-mv");
    r.dispatch(mv, Event::Hover(true)).unwrap();
    assert_eq!(text_of(&r, "name-mv"), "Mountain View !");
    assert_eq!(text_of(&r, "name-pa"), "Palo Alto");
    // A row slot is the row's: not a root slot, not carried.
    assert_eq!(r.slot("hot__3"), None);
    assert!(!r.carry().slots.iter().any(|(n, _)| n == "hot__3"));
    // Reorder: the state followed the key.
    r.act("flip", vec![]).unwrap();
    assert_eq!(r.slot("order"), Some(&Value::str("desc")));
    assert_eq!(text_of(&r, "name-mv"), "Mountain View !");
    assert_eq!(text_of(&r, "name-pa"), "Palo Alto");
    let pa = view_of(&r, "station-pa");
    r.dispatch(pa, Event::Hover(true)).unwrap();
    r.dispatch(mv, Event::Hover(false)).unwrap();
    assert_eq!(text_of(&r, "name-mv"), "Mountain View");
    assert_eq!(text_of(&r, "name-pa"), "Palo Alto !");
    // A row action run with no row has nothing to write: a typed refusal,
    // and the kernel untouched.
    assert!(r.act("setHot__3", vec![Value::Bool(true)]).is_err());
    assert_eq!(text_of(&r, "name-mv"), "Mountain View");
}

#[test]
fn a_child_may_not_own_a_resource() {
    let src = "shape S\n  id: string\ncomponent A\n  view\n    Row()\ncomponent Row\n  resource s = s() as shape S\n  view\n    text s.id\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-child-resource");
}

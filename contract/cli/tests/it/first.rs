//! `first(list)` (LLP 1054.000 C4): `some` of a list's first item, `none`
//! for an empty list, and a type error for anything but a list.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

/// `names(true)` is two names, `names(false)` none.
struct Names;
impl DataSource for Names {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "names" if args[0].as_bool() == Some(true) => {
                Ok(Value::list(vec![Value::str("ada"), Value::str("grace")]))
            }
            "names" => Ok(Value::list(vec![])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

const SRC: &str = r#"
component App
  state full = true
  resource names = names(full) as shape list<string>
  action clear writes full
    full = false
  view
    column
      match first(names)
        case some(n)
          text n testId="first"
        case none
          text "nobody" testId="first"
"#;

#[test]
fn first_is_some_of_the_first_item_or_none() {
    let plan = contract::bake(contract::compile(SRC).unwrap(), Names).unwrap();
    let mut r = Runner::boot(
        plan,
        Names,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let text = |r: &Runner<Names>| {
        let k = r.kernel();
        let key = k.find_by_test_id("first")[0];
        k.node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text)
            .unwrap()
            .to_string()
    };
    assert_eq!(text(&r), "ada");
    r.act("clear", vec![]).unwrap();
    assert_eq!(text(&r), "nobody");
}

#[test]
fn first_takes_a_list() {
    let src = SRC.replace("first(names)", "first(\"ada\")");
    let e = contract::compile(&src).unwrap_err();
    assert_eq!(e.id, "type-argument");
    assert!(e.message.contains("expects `list`"), "{}", e.message);
}

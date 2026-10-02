//! `at(list, i)` (LLP 1006 §3): JavaScript's `Array.prototype.at`, `some` of
//! the item or `none`, and a type error for anything but a list and a number.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct Names;
impl DataSource for Names {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "names" => Ok(Value::list(
                ["ada", "grace", "barbara"].map(Value::str).to_vec(),
            )),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

const SRC: &str = r#"
component App
  state i = 1
  resource names = names() as shape list<string>
  action go(n: number)
    i = n
  view
    column
      match at(names, i)
        case some(n)
          text n testId="at"
        case none
          text "nobody" testId="at"
"#;

#[test]
fn at_is_some_of_the_item_or_none() {
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
        let key = k.find_by_test_id("at")[0];
        k.node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text)
            .unwrap()
            .to_string()
    };
    assert_eq!(text(&r), "grace");
    for (i, expected) in [
        (-1.0, "barbara"),
        (3.0, "nobody"),
        (0.5, "ada"),
        (-4.0, "nobody"),
    ] {
        r.act("go", vec![Value::Number(i)]).unwrap();
        assert_eq!(text(&r), expected, "at(names, {i})");
    }
}

/// A record's field, read through `at`'s option (the lowerer types it too).
#[test]
fn at_reads_a_record_s_field() {
    let source = r#"
shape Msg
  name: string
component App
  resource msgs = msgs() as shape list<Msg>
  view
    column
      match at(msgs, -1)
        case some(m)
          text m.name testId="at"
        case none
          text "nobody" testId="at"
"#;
    struct Msgs;
    impl DataSource for Msgs {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(vec![
                Value::record(vec![Value::str("ada")]),
                Value::record(vec![Value::str("grace")]),
            ]))
        }
    }
    let plan = contract::bake(contract::compile(source).unwrap(), Msgs).unwrap();
    let r = Runner::boot(
        plan,
        Msgs,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let key = k.find_by_test_id("at")[0];
    assert_eq!(
        k.node_by_key(key).unwrap().props.str(PropId::Text),
        Some("grace")
    );
}

#[test]
fn at_takes_a_list_and_a_number() {
    for (call, id) in [
        (r#"at("abc", 1)"#, "type-arg"),
        (r#"at(names, "1")"#, "type-arg"),
        ("at(names)", "type-arity"),
    ] {
        let source = SRC.replace("at(names, i)", call);
        let error = contract::compile(&source).unwrap_err().to_string();
        assert!(error.contains(id), "{call}: {error}");
    }
}

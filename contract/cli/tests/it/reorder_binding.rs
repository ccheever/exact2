//! Arrange payloads preserve exact string keys; physical admission is Runner-owned.
use exact_kernel::{Kernel, PropId};
use exact_plan::{EventKind, Value};
use exact_runner::{DataError, DataSource, Event, Runner};

struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(vec![Value::str(""), Value::str("a,🦀\n")]))
    }
}
fn source(params: &str, target: &str, callback: &str) -> String {
    format!("component App\n  state received = \"initial\"\n  resource rows = rows() as shape list<string>\n  action receive({params}) writes received\n    received = item\n  view\n    list id=\"arrange\" virtualized=true height=200 reorderdrop={callback} testId=\"list\"\n      each x in rows key=x\n        column reorderFor={target} testId=`grip-${{x}}`\n          text x\n")
}
#[test]
fn reorder_prop_event_roundtrip_and_exact_payload_types() {
    let p = contract::compile(&source(
        "item: string, before: option<string>",
        "\"arrange\"",
        "receive",
    ))
    .unwrap();
    let p = exact_plan::Plan::decode(&p.encode()).unwrap();
    let mut r = Runner::boot(p, Rows, Kernel::with_monospace(), Default::default(), "/").unwrap();
    assert_eq!(PropId::from_name("reorderFor").unwrap() as u16, 77);
    assert_eq!(EventKind::from_name("reorderdrop").unwrap() as u8, 17);
    let list = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("list")[0])
        .unwrap()
        .id;
    assert_eq!(r.handlers_of(list), vec![EventKind::Reorderdrop]);
    r.dispatch(
        list,
        Event::ReorderDrop {
            item: "a,🦀\n".into(),
            before: Some(String::new()),
        },
    )
    .unwrap();
    assert_eq!(r.slot("received"), Some(&Value::str("a,🦀\n")));
    assert_eq!(EventKind::Transformrelease as u8, 16);
    assert_eq!(PropId::TransformDragFor as u16, 76);
}
#[test]
fn reorder_handler_rejects_wrong_option_and_key_types_after_currying() {
    for params in [
        "item: number, before: option<string>",
        "item: string, before: string",
        "item: string, before: option<number>",
        "item: string",
    ] {
        let invalid = source(params, "\"arrange\"", "receive")
            .replace("received = item", "received = \"constant\"");
        let error = contract::compile(&invalid).unwrap_err().to_string();
        assert!(
            error.contains("handler-type") || error.contains("handler-arity"),
            "{error}"
        );
    }
    assert!(contract::compile(&source(
        "item: string, before: option<string>",
        "12",
        "receive"
    ))
    .is_err());
    contract::compile(&source(
        "origin: string, item: string, before: option<string>",
        "\"arrange\"",
        "receive(\"curried\")",
    ))
    .unwrap();
}

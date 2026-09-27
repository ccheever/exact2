//! LLP 1010 §6.2: logical text over a virtualized list's rows, mounted or not.
use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value, Viewport};
use std::cell::Cell;
use std::rc::Rc;

const SOURCE: &str = r#"shape Item
  id: number
component App
  state reverse = false
  state skip = 0
  state jump = 0
  resource rows = rows(reverse, skip) as shape list<Item>
  action reorder writes reverse
    reverse = !reverse
  action trim(n: number) writes skip
    skip = n
  action go(n: number) writes jump
    jump = n
  view
    list virtualized=true estimated-item-height=24 height=240 width=390 overflow-x="hidden" testId="list" scrollTop=jump
      each item in rows key=item.id
        Cell(item=item)
component Cell
  props
    item: Item
  state count = 0
  action bump writes count
    count = count + 1
  view
    button press=bump testId=`row-${item.id}` height=24
      text `${item.id}:${count}` testId=`value-${item.id}`
"#;

#[derive(Clone)]
struct Data {
    count: usize,
    calls: Rc<Cell<usize>>,
}
impl DataSource for Data {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls.set(self.calls.get() + 1);
        let skip = match args.get(1) {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        let mut rows: Vec<_> = (skip..self.count)
            .map(|id| Value::record(vec![Value::Number(id as f64)]))
            .collect();
        if args.first() == Some(&Value::Bool(true)) {
            rows.reverse();
        }
        Ok(Value::list(rows))
    }
}
fn data(count: usize) -> Data {
    Data {
        count,
        calls: Rc::new(Cell::new(0)),
    }
}
fn view(r: &Runner<Data>, name: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
#[test]
fn logical_text_copy_spans_unmounted_rows_without_changing_the_window() {
    use exact_runner::{CollectionFeedback, ListTextPosition, RowMeasurement};
    let d = data(1000);
    let calls = d.calls.clone();
    let mut r = Runner::boot(
        contract::compile(SOURCE).unwrap(),
        d,
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    let list = view(&r, "list");
    let snapshot = r.collections().remove(0);
    r.collection_feedback(CollectionFeedback {
        view: list,
        revision: snapshot.revision,
        scroll_sequence: snapshot.scroll_sequence + 1,
        offset: 12000.0,
        port_cross: 390.0,
        port_main: 240.0,
        cross: 390.0,
        measurements: snapshot
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                size: 24.0,
            })
            .collect(),
        focus_view: None,
        interaction_view: None,
    })
    .unwrap();
    r.dispatch(view(&r, "row-500"), Event::Press).unwrap();
    let count = r.kernel().live_count();
    let slots = r.kernel().arena().slot_count();
    let copy = r.list_text(list, None).unwrap();
    let expected = (0..1000)
        .map(|i| format!("{i}:{}", usize::from(i == 500)))
        .collect::<Vec<_>>()
        .join("\n\n");
    assert_eq!(copy, expected);
    assert_eq!(count, r.kernel().live_count());
    assert_eq!(slots, r.kernel().arena().slot_count());
    assert_eq!(calls.get(), 1);
    assert!(r.kernel().find_by_test_id("row-0").is_empty());
    let a = ListTextPosition {
        key: "n:498",
        paragraph: 0,
        offset: 2,
    };
    let b = ListTextPosition {
        key: "n:502",
        paragraph: 0,
        offset: 3,
    };
    assert_eq!(
        r.list_text(list, Some((a, b))).unwrap(),
        "8:0\n\n499:0\n\n500:1\n\n501:0\n\n502"
    );
    assert_eq!(
        r.list_text(list, Some((b, a))).unwrap(),
        r.list_text(list, Some((a, b))).unwrap()
    );
    r.act("reorder", vec![]).unwrap();
    assert_eq!(r.list_index(list, "n:498"), Some(501));
    assert_eq!(
        r.list_text(list, Some((a, b))).unwrap(),
        ":0\n\n501:0\n\n500:1\n\n499:0\n\n49"
    );
    r.act("trim", vec![Value::Number(500.0)]).unwrap();
    assert!(r.list_text(list, Some((a, b))).is_err());
    assert!(!r.is_poisoned());
}

#[test]
fn logical_text_joins_inline_runs_skips_hidden_text_and_uses_utf16_positions() {
    use exact_runner::ListTextPosition;
    let source = SOURCE.replace(
        "      text `${item.id}:${count}` testId=`value-${item.id}`",
        r#"      text
        text "A🦊"
        text `${item.id}`
      view display="none"
        text "secret"
      text "tail""#,
    );
    let r = Runner::boot(
        contract::compile(&source).unwrap(),
        data(3),
        Kernel::with_monospace(),
        Viewport::default(),
        "/",
    )
    .unwrap();
    let list = view(&r, "list");
    assert_eq!(
        r.list_text(list, None).unwrap(),
        "A🦊0\n\ntail\n\nA🦊1\n\ntail\n\nA🦊2\n\ntail"
    );
    let a = ListTextPosition {
        key: "n:0",
        paragraph: 0,
        offset: 1,
    };
    let b = ListTextPosition {
        key: "n:2",
        paragraph: 1,
        offset: 2,
    };
    assert_eq!(
        r.list_text(list, Some((a, b))).unwrap(),
        "🦊0\n\ntail\n\nA🦊1\n\ntail\n\nA🦊2\n\nta"
    );
}

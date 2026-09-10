//! Component lifetime is observable through real compiled plans and host replies.
use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Event, Outcome, Request, Response, Runner, Store,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct Source {
    calls: Rc<RefCell<Vec<String>>>,
    parses: Rc<RefCell<usize>>,
    later: bool,
}
impl DataSource for Source {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let id = args.first().and_then(Value::as_str).unwrap_or("");
        self.calls.borrow_mut().push(format!("{source}:{id}"));
        Ok(Value::str(&format!("{id}:{}", self.calls.borrow().len())))
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if self.later {
            let id = args.first().and_then(Value::as_str).unwrap_or("");
            self.calls.borrow_mut().push(format!("{source}:{id}"));
            Ok(Answer::Later(Request::get(&format!(
                "https://example.test/{source}/{id}"
            ))))
        } else {
            self.query(source, args).map(Answer::Now)
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        *self.parses.borrow_mut() += 1;
        match outcome {
            Outcome::Response(response) => Ok(Answer::Now(Value::str(
                &String::from_utf8(response.body).unwrap(),
            ))),
            _ => Err(DataError::Unavailable("test response failed".into())),
        }
    }
}
fn text<D: DataSource>(r: &Runner<D>, id: &str) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel()
        .node_by_key(key)
        .unwrap()
        .props
        .iter()
        .find_map(|(p, v)| match v {
            PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
            _ => None,
        })
        .unwrap()
}
fn press<D: DataSource>(r: &mut Runner<D>, id: &str) {
    let key = r.kernel().find_by_test_id(id)[0];
    let view = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}
fn reply<D: DataSource>(r: &mut Runner<D>, ticket: u64, body: &str) {
    r.fulfill(
        ticket,
        Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: body.as_bytes().to_vec(),
        }),
    )
    .unwrap();
}
const CHILD: &str = r#"
component Card
  props
    id: string
  state ticks = 0
  resource value = read(id) as shape string else `loading-${id}`
  mutation result as shape string
  action refreshValue writes ticks
    refresh value
  action submit writes result
    send result = save(id)
  action tick writes ticks
    ticks = ticks + 1
  task ticker mount
    every(100, tick)
  view
    column
      text value testId=`value-${id}`
      text `${ticks}` testId=`ticks-${id}`
      text `${pending(value)}` testId=`pending-${id}`
      text `${pending(result)}` testId=`saving-${id}`
      button "refresh" press=refreshValue testId=`refresh-${id}`
      button "save" press=submit testId=`save-${id}`
      match result
        case some(v)
          text v testId=`result-${id}`
        case none
          text "none" testId=`result-${id}`
"#;
fn compile(root: &str) -> exact_plan::Plan {
    contract::compile(&format!("{root}\n{CHILD}")).unwrap()
}

#[test]
fn independent_children_keep_requests_mutations_and_refresh_local() {
    let plan =
        compile("component App\n  view\n    column\n      Card(id=\"a\")\n      Card(id=\"b\")\n");
    let source = Source {
        later: true,
        ..Default::default()
    };
    let mut r = Runner::boot(plan, source, Kernel::with_monospace()).unwrap();
    assert_eq!(text(&r, "value-a"), "loading-a");
    assert_eq!(text(&r, "pending-a"), "true");
    let requests = r.take_requests();
    assert_eq!(requests.len(), 2);
    reply(&mut r, requests[1].ticket, "B");
    assert_eq!(text(&r, "value-a"), "loading-a");
    assert_eq!(text(&r, "value-b"), "B");
    reply(&mut r, requests[0].ticket, "A");
    press(&mut r, "refresh-a");
    let refresh = r.take_requests();
    assert_eq!(refresh.len(), 1);
    assert_eq!(text(&r, "pending-b"), "false");
    assert_eq!(text(&r, "value-a"), "A");
    reply(&mut r, refresh[0].ticket, "A2");
    press(&mut r, "save-a");
    let save = r.take_requests();
    assert_eq!(save.len(), 1);
    assert_eq!(text(&r, "saving-a"), "true");
    assert_eq!(text(&r, "saving-b"), "false");
    reply(&mut r, save[0].ticket, "saved A");
    assert_eq!(text(&r, "result-a"), "saved A");
    assert_eq!(text(&r, "result-b"), "none");
}

#[test]
fn conditional_remount_starts_fresh_and_drops_taken_and_undrained_work() {
    let plan = compile("component App\n  state shown = false\n  action toggle writes shown\n    shown = !shown\n  view\n    column\n      when shown\n        Card(id=\"a\")\n");
    let source = Source {
        later: true,
        ..Default::default()
    };
    let parses = source.parses.clone();
    let mut r = Runner::boot(plan, source, Kernel::with_monospace()).unwrap();
    assert!(r.take_requests().is_empty());
    r.advance(250.0).unwrap();
    r.act("toggle", vec![]).unwrap();
    let old = r.take_requests()[0].ticket;
    assert_eq!(text(&r, "ticks-a"), "0");
    r.advance(349.0).unwrap();
    assert_eq!(text(&r, "ticks-a"), "0");
    r.advance(350.0).unwrap();
    assert_eq!(text(&r, "ticks-a"), "1");
    press(&mut r, "save-a");
    r.act("toggle", vec![]).unwrap();
    assert!(
        r.take_requests().is_empty(),
        "unmounted mutation must never leave the outbox"
    );
    r.advance(1000.0).unwrap();
    r.act("toggle", vec![]).unwrap();
    let fresh = r.take_requests()[0].ticket;
    assert_ne!(old, fresh);
    reply(&mut r, old, "obsolete");
    assert_eq!(*parses.borrow(), 0);
    assert_eq!(text(&r, "value-a"), "loading-a");
    assert_eq!(text(&r, "ticks-a"), "0");
    assert_eq!(text(&r, "result-a"), "none");
    reply(&mut r, fresh, "new");
    assert_eq!(text(&r, "value-a"), "new");
}

#[test]
fn changed_props_supersede_only_that_instances_old_reply() {
    let plan = compile("component App\n  state id = \"a\"\n  action change writes id\n    id = \"b\"\n  view\n    column\n      Card(id=id)\n");
    let source = Source {
        later: true,
        ..Default::default()
    };
    let parses = source.parses.clone();
    let mut r = Runner::boot(plan, source, Kernel::with_monospace()).unwrap();
    let old = r.take_requests()[0].ticket;
    r.act("change", vec![]).unwrap();
    let new = r.take_requests()[0].ticket;
    reply(&mut r, new, "new B");
    reply(&mut r, old, "old A");
    assert_eq!(*parses.borrow(), 1);
    assert_eq!(text(&r, "value-b"), "new B");
}

#[test]
fn a_stateful_child_without_effects_also_resets_on_remount() {
    let source = r#"component App
  state shown = true
  action toggle writes shown
    shown = !shown
  view
    column
      when shown
        Counter()
component Counter
  state n = 0
  action bump writes n
    n = n + 1
  view
    column
      button "bump" press=bump testId="bump"
      text `${n}` testId="count"
"#;
    let plan = contract::compile(source).unwrap();
    let mut r = Runner::boot(plan, Source::default(), Kernel::with_monospace()).unwrap();
    press(&mut r, "bump");
    assert_eq!(text(&r, "count"), "1");
    assert!(!r
        .carry()
        .slots
        .iter()
        .any(|(name, _)| name.starts_with("n__")));
    r.act("toggle", vec![]).unwrap();
    r.act("toggle", vec![]).unwrap();
    assert_eq!(text(&r, "count"), "0");
}

#[test]
fn baked_pending_fallback_is_a_placeholder_and_still_launches_a_request() {
    let plan = compile("component App\n  view\n    column\n      Card(id=\"a\")\n");
    let source = Source {
        later: true,
        ..Default::default()
    };
    let plan = contract::bake(plan, source.clone()).unwrap();
    let mut r = Runner::boot(plan, source, Kernel::with_monospace()).unwrap();
    assert_eq!(text(&r, "value-a"), "loading-a");
    assert_eq!(
        r.take_requests().len(),
        1,
        "a baked fallback is not a settled answer"
    );
}

#[test]
fn keyed_bake_keeps_equal_argument_answers_distinct_and_reorder_keeps_identity() {
    let source = r#"component App
  state reverse = false
  derive ids = reverse ? ["b", "a"] : ["a", "b"]
  action flip writes reverse
    reverse = !reverse
  view
    column
      each id in ids key=id
        Row(id=id)
component Row
  props
    id: string
  resource value = read("same") as shape string
  view
    text value testId=id
"#;
    let source_data = Source::default();
    let calls = source_data.calls.clone();
    let plan = contract::bake(contract::compile(source).unwrap(), source_data.clone()).unwrap();
    assert_eq!(calls.borrow().len(), 2);
    let mut r = Runner::boot(plan, source_data, Kernel::with_monospace()).unwrap();
    assert_eq!(text(&r, "a"), "same:1");
    assert_eq!(text(&r, "b"), "same:2");
    assert_eq!(
        calls.borrow().len(),
        2,
        "boot must use each instance's compiled answer"
    );
    let key_a = r.kernel().find_by_test_id("a")[0];
    let receipt = r.act("flip", vec![]).unwrap();
    assert!(receipt.created.is_empty() && receipt.destroyed.is_empty());
    assert_eq!(r.kernel().find_by_test_id("a")[0], key_a);
    assert_eq!(text(&r, "a"), "same:1");
    assert_eq!(text(&r, "b"), "same:2");
    assert_eq!(calls.borrow().len(), 2);
}

#[test]
fn timer_uses_live_captures_and_preserves_cadence_across_keyed_reorder() {
    let source = r#"component App
  state reverse = false
  state label = "before"
  derive ids = reverse ? ["b", "a"] : ["a", "b"]
  action change writes reverse, label
    reverse = !reverse
    label = "after"
  view
    column
      each id in ids key=id
        Ticker(id=id, label=label)
component Ticker
  props
    id: string
    label: string
  state seen = "initial"
  action tick writes seen
    seen = `${id}-${label}`
  task ticker mount
    every(100, tick)
  view
    text seen testId=id
"#;
    let plan = contract::compile(source).unwrap();
    let mut r = Runner::boot(plan, Source::default(), Kernel::with_monospace()).unwrap();
    r.advance(50.0).unwrap();
    r.act("change", vec![]).unwrap();
    let receipts = r.advance(100.0).unwrap();
    assert_eq!(receipts.len(), 2);
    assert_eq!(text(&r, "a"), "a-after");
    assert_eq!(text(&r, "b"), "b-after");
}

#[test]
fn wrong_fallback_type_and_missing_first_pending_value_refuse() {
    let wrong = "component App\n  view\n    C()\ncomponent C\n  resource x = read() as shape number else \"wrong\"\n  view\n    text `${x}`\n";
    assert!(contract::compile(wrong).is_err());
    let missing = "component App\n  view\n    C()\ncomponent C\n  resource x = read() as shape string\n  view\n    text x\n";
    let plan = contract::compile(missing).unwrap();
    let source = Source {
        later: true,
        ..Default::default()
    };
    assert!(Runner::boot(plan, source, Kernel::with_monospace()).is_err());
}

#[test]
fn root_resources_can_use_the_same_explicit_fallback() {
    let source = "component App\n  resource x = read(\"root\") as shape string else \"loading\"\n  view\n    text x testId=\"x\"\n";
    let plan = contract::compile(source).unwrap();
    let source = Source {
        later: true,
        ..Default::default()
    };
    let mut r = Runner::boot(plan, source, Kernel::with_monospace()).unwrap();
    assert_eq!(text(&r, "x"), "loading");
    let ticket = r.take_requests()[0].ticket;
    reply(&mut r, ticket, "loaded");
    assert_eq!(text(&r, "x"), "loaded");
}

#[test]
fn nested_resources_control_children_through_match_slots_and_providers() {
    struct Gates;
    impl DataSource for Gates {
        fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            match source {
                "enabled" => Ok(Value::Bool(true)),
                "read" => Ok(args[0].clone()),
                _ => Err(DataError::UnknownSource(source.into())),
            }
        }
    }
    let source = r#"component App
  state selected = some("selected")
  view
    column
      match selected
        case some(id)
          provide label = id
            Gate()
        case none
          text "empty"
component Gate
  inject
    label: string
  resource enabled = enabled(label) as shape bool
  view
    column
      when enabled
        Shell()
          Data()
component Shell
  slot
  state mounted = true
  view
    column
      children
component Data
  inject
    label: string
  resource value = read(label) as shape string
  view
    text value testId="data"
"#;
    let plan = contract::bake(contract::compile(source).unwrap(), Gates).unwrap();
    let r = Runner::boot(plan, Gates, Kernel::with_monospace()).unwrap();
    assert_eq!(text(&r, "data"), "selected");
}

#[test]
fn forward_resource_dependencies_settle_against_new_props_in_one_commit() {
    let source = r#"component App
  state id = "a"
  action change writes id
    id = "b"
  view
    Dependent(id=id)
component Dependent
  props
    id: string
  resource derived = read(base) as shape string
  resource base = read(id) as shape string
  view
    text derived testId="derived"
"#;
    let plan = contract::compile(source).unwrap();
    let mut r = Runner::boot(plan, Source::default(), Kernel::with_monospace()).unwrap();
    assert_eq!(text(&r, "derived"), "a:1:2");
    r.act("change", vec![]).unwrap();
    assert_eq!(text(&r, "derived"), "b:3:4");
}

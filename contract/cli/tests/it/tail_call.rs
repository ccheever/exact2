//! LLP 1017 P4c's tail call (Charlie, 2026-10-03; the Signal Clone's photo
//! viewer is the consumer): a child's action may end by calling one of its
//! `action` props. The call is the named action's statements, run last in
//! the caller's commit; anywhere else an action prop is still refused.

use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn text_of(r: &Runner<NoData>, id: &str) -> String {
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
fn press(r: &mut Runner<NoData>, id: &str) {
    let k = r.kernel();
    let view = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}

const VIEWER: &str = r#"component App
  state open = true
  state closes = 0
  state last = ""
  action dismiss(why: string)
    open = false
    closes = closes + 1
    last = why
  view
    column
      when open
        Viewer(close=dismiss)
      text `${closes} ${last} ${open}` testId="closes"

component Viewer
  props
    close: action
  state drags = 0
  action release(dy: number)
    drags = drags + 1
    if dy > 100
      close("swiped")
  view
    column
      button press=release(200) testId="far"
        text "far"
      button press=release(10) testId="near"
        text "near"
"#;

#[test]
fn an_action_prop_called_last_runs_the_named_action_in_the_same_commit() {
    let mut r = boot(VIEWER);
    press(&mut r, "near");
    assert_eq!(
        text_of(&r, "closes"),
        "0  true",
        "not called: the branch was not taken"
    );
    press(&mut r, "far");
    assert_eq!(
        text_of(&r, "closes"),
        "1 swiped false",
        "the root's action ran with the call's argument"
    );
}

#[test]
fn curried_arguments_reach_the_call_ahead_of_its_own() {
    let src = r#"component App
  state log = ""
  action note(who: string, what: string)
    log = `${who}:${what}`
  view
    column
      Row(done=note("ada"))
      text log testId="log"

component Row
  props
    done: action
  state n = 0
  action finish
    n = n + 1
    done("finished")
  view
    button press=finish testId="finish"
      text "finish"
"#;
    let mut r = boot(src);
    press(&mut r, "finish");
    assert_eq!(text_of(&r, "log"), "ada:finished");
}

#[test]
fn the_call_reads_the_state_as_the_commit_found_it_and_writes_after_the_caller() {
    let src = r#"component App
  state count = 1
  state seen = 0
  action record
    seen = count
    count = 10
  view
    column
      Bump(after=record)
      text `${count} ${seen}` testId="state"

component Bump
  props
    after: action
  action go
    count = 5
    after()
  view
    button press=go testId="go"
      text "go"
"#;
    // `count` is the root's: the child cannot write it, so this one is refused.
    assert!(contract::compile(src).is_err());
    let src = r#"component App
  state count = 1
  state seen = 0
  action record
    seen = count
    count = count + 10
  view
    column
      Bump(after=record)
      text `${count} ${seen}` testId="state"

component Bump
  props
    after: action
  state mine = 0
  action go
    mine = mine + 1
    after()
  view
    button press=go testId="go"
      text "go"
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "state"), "11 1");
}

#[test]
fn an_action_prop_anywhere_but_the_tail_is_still_refused() {
    let src = r#"component App
  state n = 0
  action bump
    n = n + 1
  view
    Child(cb=bump)

component Child
  props
    cb: action
  state m = 0
  action go
    cb()
    m = m + 1
  view
    button press=go testId="go"
      text "go"
"#;
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-unknown-command", "{e}");
    assert!(e.message.contains("last statement"), "{e}");
}

#[test]
fn a_local_that_would_hide_what_the_called_action_reads_is_refused() {
    let src = r#"component App
  state n = 0
  action bump
    n = n + 1
  view
    Child(cb=bump)

component Child
  props
    cb: action
  state m = 0
  action go
    let n = 5
    m = n
    cb()
  view
    button press=go testId="go"
      text "go"
"#;
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "syntax-tail-capture", "{e}");
}

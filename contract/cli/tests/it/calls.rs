//! LLP 1089: an action calls an action of its own component, an `action`
//! prop or an injected action, as a statement, anywhere. The call is the
//! callee's statements expanded in place: one commit, every statement
//! reading the state the action started with. LLP 1017 §11's tail call (the
//! Signal Clone's photo viewer) is one case of it, and keeps its tests.

use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

/// `items()` answers two `Item`s, `x` and `y`; nothing else answers.
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "items" => Ok(Value::list(
                ["x", "y"]
                    .map(|id| Value::record(vec![Value::str(id)]))
                    .to_vec(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

/// The declarations `items()` needs, ahead of a test's root.
const ITEMS: &str = "shape Item\n  id: string\n\n";

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
        Viewer(hide=dismiss)
      text `${closes} ${last} ${open}` testId="closes"

component Viewer
  props
    hide: action
  state drags = 0
  action release(dy: number)
    drags = drags + 1
    if dy > 100
      hide("swiped")
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
fn an_action_prop_called_before_the_childs_own_writes_runs_first() {
    let src = r#"component App
  state n = 0
  action bump
    n = n + 1
  view
    column
      Child(cb=bump)
      text `${n}` testId="n"

component Child
  props
    cb: action
  state m = 0
  action go
    cb()
    m = m + 1
  view
    button press=go testId="go"
      text `${m}`
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    press(&mut r, "go");
    assert_eq!(text_of(&r, "n"), "2");
}

#[test]
fn the_called_action_reads_the_root_never_a_caller_local_or_parameter_of_its_name() {
    // The child's parameter `n` and its local `m` share the spelling of root
    // state the root's action reads: the root's are read.
    let src = r#"component App
  state n = 7
  state m = 3
  state seen = ""
  action report
    seen = `${n} ${m}`
  view
    column
      Child(done=report)
      text seen testId="seen"

component Child
  props
    done: action
  state mine = 0
  action go(n: number)
    let m = n + 1
    mine = m
    done()
  view
    button press=go(42) testId="go"
      text "go"
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "seen"), "7 3");
}

#[test]
fn a_chain_of_tail_calls_runs_in_order_each_reading_its_own_names() {
    // Outer's parameter `why` shares the spelling of root state the inner
    // action reads.
    let src = r#"component App
  state why = "root"
  state last = ""
  state log = ""
  action inner
    last = why
  view
    column
      Outer(next=inner)
      text `${last} ${log}` testId="state"

component Outer
  props
    next: action
  state relays = 0
  action relay(why: string)
    relays = relays + 1
    next()
  view
    Inner(done=relay("arg"))

component Inner
  props
    done: action
  state n = 0
  action go
    n = n + 1
    done()
  view
    button press=go testId="go"
      text "go"
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "state"), "root ");
}

#[test]
fn the_arguments_are_held_to_the_called_actions_parameter_types() {
    let src = r#"component App
  state result = ""
  action record(value: string)
    result = value
  view
    Child(cb=record)

component Child
  props
    cb: action
  state m = 0
  action go
    m = m + 1
    cb(42)
  view
    button press=go testId="go"
      text "go"
"#;
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-argument", "{e}");
}

#[test]
fn an_argument_must_be_a_name_the_child_has() {
    let src = r#"component App
  state secret = "root"
  state got = ""
  action record(value: string)
    got = value
  view
    Child(cb=record)

component Child
  props
    cb: action
  state m = 0
  action go
    m = m + 1
    cb(secret)
  view
    button press=go testId="go"
      text "go"
"#;
    assert!(
        contract::compile(src).is_err(),
        "the child never had `secret`"
    );
}

#[test]
fn renaming_apart_leaves_a_function_call_of_the_same_spelling_alone() {
    // A caller parameter and a local spelled like a library function: the
    // call still names the function.
    let src = r#"component App
  state seen = ""
  action report
    seen = "done"
  view
    column
      Child(done=report)
      text seen testId="seen"

component Child
  props
    done: action
  state out = ""
  action go(length: number)
    let toString = length + 1
    out = `${toString} ${toString(length)}`
    done()
  view
    column
      button press=go(4) testId="go"
        text "go"
      text out testId="out"
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "out"), "5 4");
    assert_eq!(text_of(&r, "seen"), "done");
}

fn refused(src: &str) -> contract::CompileError {
    contract::compile(src).expect_err("refused")
}

#[test]
fn a_call_stands_first_in_the_middle_in_an_arm_and_twice() {
    let src = r#"component App
  state log = ""
  state n = 0
  action note(what)
    log = `${log}${what}`
  action bump
    n = n + 1
  action first
    note("a")
    n = 10
  action middle
    n = 5
    note("b")
    let k = n
    log = `${k}`
  action arm(go: bool)
    if go
      bump()
    else
      note("c")
  action twice
    bump()
    note("d")
  view
    column
      button press=first testId="first"
        text "first"
      button press=middle testId="middle"
        text "middle"
      button press=arm(true) testId="yes"
        text "yes"
      button press=arm(false) testId="no"
        text "no"
      button press=twice testId="twice"
        text "twice"
      text `${n} ${log}` testId="out"
"#;
    let mut r = boot(src);
    press(&mut r, "first");
    assert_eq!(text_of(&r, "out"), "10 a");
    // Every statement reads the starting state; the last write wins.
    press(&mut r, "middle");
    assert_eq!(text_of(&r, "out"), "5 10");
    press(&mut r, "yes");
    assert_eq!(text_of(&r, "out"), "6 10");
    press(&mut r, "no");
    assert_eq!(text_of(&r, "out"), "6 10c");
    press(&mut r, "twice");
    assert_eq!(text_of(&r, "out"), "7 10cd");
}

#[test]
fn a_chain_of_three_runs_in_one_commit_each_with_its_own_names() {
    let src = r#"component App
  state a = ""
  state b = ""
  state c = ""
  action third(x: string)
    c = x
  action second(x: string)
    let y = `${x}2`
    b = y
    third(`${y}3`)
  action first(x: string)
    a = x
    second(`${x}1`)
  view
    column
      button press=first("go") testId="go"
        text "go"
      text `${a} ${b} ${c}` testId="out"
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "out"), "go go12 go123");
}

#[test]
fn a_child_calls_its_own_action_in_a_row() {
    // `type-unknown-command` before LLP 1089: the child's body was checked
    // with the call a command.
    let src = &format!(
        "{ITEMS}{}",
        r#"component App
  resource items = items() as shape list<Item>
  view
    column
      each item in items key=item.id
        Row(label=item.id)

component Row
  props
    label: string
  state taps = 0
  state last = ""
  action count(by: number)
    taps = taps + by
  action tap
    count(2)
    last = label
  view
    button press=tap testId=`row-${label}`
      text `${label} ${taps} ${last}` testId=`text-${label}`
"#
    );
    let mut r = boot(src);
    press(&mut r, "row-y");
    assert_eq!(text_of(&r, "text-y"), "y 2 y");
    assert_eq!(text_of(&r, "text-x"), "x 0 ");
}

#[test]
fn a_prop_called_from_a_same_component_callee_runs_the_owners_action_in_the_row() {
    let src = &format!(
        "{ITEMS}{}",
        r#"component App
  state picked = ""
  resource items = items() as shape list<Item>
  action pick(id: string)
    picked = id
  view
    column
      each item in items key=item.id
        Row(label=item.id, choose=pick(item.id))
      text picked testId="picked"

component Row
  props
    label: string
    choose: action
  state n = 0
  action report
    choose()
  action tap
    n = n + 1
    report()
  view
    button press=tap testId=`row-${label}`
      text `${n}` testId=`n-${label}`
"#
    );
    let mut r = boot(src);
    press(&mut r, "row-x");
    assert_eq!(text_of(&r, "picked"), "x");
    assert_eq!(text_of(&r, "n-x"), "1");
}

#[test]
fn a_let_after_a_call_reads_its_own_value_in_the_callees_freed_local() {
    let src = r#"component App
  state out = ""
  action helper(x: number)
    let doubled = x * 2
    out = `${doubled}`
  action go
    let before = 1
    helper(20)
    let after = before + 2
    out = `${before} ${after}`
  view
    column
      button press=go testId="go"
        text "go"
      text out testId="out"
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    // The caller's assignment is the last write.
    assert_eq!(text_of(&r, "out"), "1 3");
}

#[test]
fn an_arrow_parameter_spelled_like_a_written_slot_is_not_a_stale_read() {
    let src = &format!(
        "{ITEMS}{}",
        r#"component App
  state count = 0
  state shown = ""
  resource items = items() as shape list<Item>
  action reset
    count = 0
  action go
    reset()
    shown = join(map(items, count => count.id), ",")
  view
    column
      button press=go testId="go"
        text "go"
      text shown testId="shown"
"#
    );
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "shown"), "x,y");
}

#[test]
fn an_injected_action_is_called_like_a_prop() {
    let src = r#"component App
  state hits = 0
  action hit(by: number)
    hits = hits + by
  provide
    onHit = hit(3)
  view
    column
      Deep()
      text `${hits}` testId="hits"

component Deep
  inject
    onHit: action
  state mine = 0
  action go
    onHit()
    mine = mine + 1
  view
    button press=go testId="go"
      text `${mine}`
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "hits"), "3");
}

#[test]
fn curried_and_payload_arguments_are_passed_explicitly() {
    let src = &format!(
        "{ITEMS}{}",
        r#"component App
  state seen = ""
  resource items = items() as shape list<Item>
  action flip(id: string, checked: bool)
    seen = `${id} ${checked}`
  action toggled(id: string, checked: bool)
    flip(id, checked)
  view
    column
      each item in items key=item.id
        Row(label=item.id, set=toggled(item.id))
      text seen testId="seen"

component Row
  props
    label: string
    set: action
  state n = 0
  action change(on: bool)
    n = n + 1
    set(on)
  view
    input type="checkbox" change=change testId=`box-${label}`
"#
    );
    let mut r = boot(src);
    let k = r.kernel();
    let view = k.node_by_key(k.find_by_test_id("box-x")[0]).unwrap().id;
    r.dispatch(view, Event::Change(true.into())).unwrap();
    // `x` was curried where `set` was bound; `true` is the payload `change`
    // passed on.
    assert_eq!(text_of(&r, "seen"), "x true");
}

#[test]
fn an_untyped_helper_is_typed_by_its_calls() {
    let src = r#"component App
  state r = 0
  state c = 0
  action move(dr, dc)
    r = r + dr
    c = c + dc
  action clipKey(k: string)
    if k == "ArrowDown"
      move(1, 0)
    if k == "ArrowRight"
      move(0, 1)
  view
    column
      button press=clipKey("ArrowDown") testId="down"
        text "down"
      button press=clipKey("ArrowRight") testId="right"
        text "right"
      text `${r} ${c}` testId="at"
"#;
    let mut r = boot(src);
    press(&mut r, "down");
    press(&mut r, "right");
    assert_eq!(text_of(&r, "at"), "1 1");
}

#[test]
fn a_call_naming_a_host_command_and_an_action_is_refused_even_in_its_wrapper() {
    // Caltrain's old `setScheme` wrapper: no self-wrapper exemption (LLP 1089 D1).
    let src = r#"component App
  state scheme = "light"
  action setScheme(s: string)
    scheme = s
    setScheme(s)
  view
    button press=setScheme("dark") testId="dark"
      text scheme
"#;
    let e = refused(src);
    assert_eq!(e.id, "syntax-call-ambiguous", "{e}");
    assert_eq!(
        e.message,
        "`setScheme()` names both a host command and action `App.setScheme`. Rename the action and update its bindings. Call the renamed action to invoke it; keep `setScheme()` to invoke the host command"
    );
    assert_eq!((e.span.line, e.span.col), (5, 5));
    let related: Vec<_> = e
        .related
        .iter()
        .map(|r| (r.span.line, r.note.as_str()))
        .collect();
    assert_eq!(related, [(3, "the action `setScheme` is declared here")]);
}

#[test]
fn a_renamed_wrapper_calls_the_host_command() {
    // Caltrain's wrapper as it is now: `chooseScheme` wraps `setScheme`.
    let src = r#"component App
  state scheme = "light"
  action chooseScheme(s: string)
    scheme = s
    setScheme(s)
  view
    button press=chooseScheme("dark") testId="dark"
      text scheme testId="scheme"
"#;
    let mut r = boot(src);
    press(&mut r, "dark");
    assert_eq!(text_of(&r, "scheme"), "dark");
}

#[test]
fn a_call_naming_a_host_command_and_an_action_prop_or_inject_is_refused_anywhere() {
    // The Signal Clone's viewer: an `action` prop named like `close()`,
    // called inside an arm, with and without arguments.
    let prop = r#"component App
  state open = true
  action dismiss
    open = false
  view
    Viewer(close=dismiss)

component Viewer
  props
    close: action
  state n = 0
  action swiped(far: bool)
    if far
      close()
    else
      n = n + 1
  view
    button press=swiped(true) testId="swipe"
      text "x"
"#;
    let e = refused(prop);
    assert_eq!(e.id, "syntax-call-ambiguous", "{e}");
    assert_eq!(
        e.message,
        "`close()` names both a host command and action prop `Viewer.close`. Rename the prop and update its bindings. Call the renamed action prop to invoke it; keep `close()` to invoke the host command"
    );
    assert_eq!(e.related.first().map(|r| r.span.line), Some(10));
    let inject = r#"component App
  state n = 0
  action again
    n = n + 1
  provide
    reload = again
  view
    Row()

component Row
  inject
    reload: action
  action go
    match some(1)
      case some(x)
        reload()
      case none
        reload()
  view
    button press=go testId="go"
      text "go"
"#;
    let e = refused(inject);
    assert_eq!(e.id, "syntax-call-ambiguous", "{e}");
    assert!(e.message.contains("injected action `Row.reload`"), "{e}");
}

#[test]
fn binding_such_a_name_or_calling_the_command_elsewhere_compiles() {
    // Declared and bound, never called: legal. Another component's action
    // of the name does not make `close()` here ambiguous.
    let src = r#"component App
  state open = true
  action close
    open = false
  view
    column
      Viewer(close=close)
      Quit()

component Viewer
  props
    close: action
  view
    button press=close testId="close"
      text "x"

component Quit
  action leave
    close()
  view
    button press=leave testId="leave"
      text "quit"
"#;
    contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
}

/// The refusal's id and whole message.
fn says(src: &str, id: &str, message: &str) {
    let e = refused(src);
    assert_eq!((e.id.as_str(), e.message.as_str()), (id, message), "{e}");
}

const BUTTON: &str = "  view\n    button press=go testId=\"go\"\n      text \"go\"\n";

#[test]
fn a_read_another_frame_made_stale_is_refused_in_each_of_d3s_cases() {
    let follow = "component App\n  state sel = 0\n  state seen = 0\n  action follow\n    seen = sel\n  action go\n    sel = 3\n    follow()\n";
    says(
        &format!("{follow}{BUTTON}"),
        "analyze-call-stale-read",
        "`follow` (called at line 8) reads `sel`, which `go` assigns at line 7; `follow` sees the value `sel` had when the action started. Pass the value it should see: a `let` bound before line 7 keeps the starting value, and the value assigned at line 7 gives the new one",
    );
    // The argument is read in the callee's frame.
    let argument = "component App\n  state sel = 0\n  state seen = 0\n  action follow(v: number)\n    seen = v\n  action go\n    sel = 3\n    follow(sel)\n";
    says(
        &format!("{argument}{BUTTON}"),
        "analyze-call-stale-read",
        "`follow` (called at line 8) reads `sel`, which `go` assigns at line 7; `follow` sees the value `sel` had when the action started. Pass the value it should see: a `let` bound before line 7 keeps the starting value, and the value assigned at line 7 gives the new one",
    );
    let reset = "component App\n  state count = 0\n  action reset\n    count = 0\n  action go\n    reset()\n    count = count + 1\n";
    says(
        &format!("{reset}{BUTTON}"),
        "analyze-call-stale-read",
        "`go` reads `count` at line 7, which `reset` (called at line 6) assigns at line 4; `go` sees the value `count` had when the action started. Pass the value it should see: a `let` bound before line 6 keeps the starting value, and the value assigned at line 4 gives the new one",
    );
    // Both read the starting cell and the later write wins: not a diagonal.
    let diagonal = "component App\n  state r = 0\n  state c = 0\n  action move(dr: number, dc: number)\n    r = r + dr\n    c = c + dc\n  action go\n    move(1, 0)\n    move(0, 1)\n";
    says(
        &format!("{diagonal}{BUTTON}"),
        "analyze-call-stale-read",
        "`move` (called at line 9) reads `r`, which `move` (called at line 8) assigns at line 5; `move` sees the value `r` had when the action started. Pass the value it should see: a `let` bound before line 8 keeps the starting value, and the value assigned at line 5 gives the new one",
    );
}

#[test]
fn reads_d3_leaves_alone_compile() {
    for (case, src) in [
        (
            "exclusive literal arms",
            "component App\n  state r = 0\n  action move(dr: number)\n    r = r + dr\n  action key(k: string)\n    if k == \"ArrowDown\" or k == \"Enter\"\n      move(1)\n    if k == \"ArrowUp\"\n      move(-1)\n  view\n    button press=key(\"Enter\") testId=\"go\"\n      text \"go\"\n",
        ),
        (
            "one frame",
            "component App\n  state a = 1\n  state b = 2\n  state location = \"\"\n  state back = \"\"\n  action go(to: string)\n    a = b\n    b = a\n    location = to\n    back = `${back}/${location}`\n  view\n    button press=go(\"x\") testId=\"go\"\n      text \"go\"\n",
        ),
        (
            "a prop read after calling the owner",
            "component App\n  state n = 0\n  action bump\n    n = n + 1\n  view\n    Child(label=\"a\", cb=bump)\n\ncomponent Child\n  props\n    label: string\n    cb: action\n  state seen = \"\"\n  action go\n    cb()\n    seen = label\n  view\n    button press=go testId=\"go\"\n      text seen\n",
        ),
        (
            "a derive is a settled value",
            "component App\n  state message = \"x\"\n  state shown = \"\"\n  derive status = `s ${message}`\n  action showStatus\n    shown = status\n  action go\n    message = \"\"\n    showStatus()\n  view\n    button press=go testId=\"go\"\n      text shown\n",
        ),
    ] {
        contract::compile(src).unwrap_or_else(|e| panic!("{case}: {e}"));
    }
}

#[test]
fn each_call_refusal_says_what_to_write() {
    says(
        "component App\n  state r = 0\n  action move(dr: number, dc: number)\n    r = dr + dc\n  action go\n    move(1)\n  view\n    button press=go testId=\"go\"\n      text \"go\"\n",
        "type-call-arity",
        "`move` takes 2 argument(s), given 1",
    );
    says(
        "component App\n  state r = 0\n  action move(id: string, dr: number, dc: number)\n    r = dr + dc\n  view\n    Child(go=move(\"a\"))\n\ncomponent Child\n  props\n    go: action\n  state n = 0\n  action tap\n    n = n + 1\n    go(1)\n  view\n    button press=tap testId=\"go\"\n      text \"go\"\n",
        "type-call-arity",
        "`move` takes 2 argument(s) after the 1 curried at line 6, given 1",
    );
    says(
        "component App\n  state n = 0\n  action archive\n    n = n + 1\n  view\n    Row()\n\ncomponent Row\n  state m = 0\n  action tap\n    m = 1\n    archive()\n  view\n    button press=tap testId=\"go\"\n      text \"go\"\n",
        "type-unknown-command",
        "`archive` is an action of `App`, not in `Row`'s scope; pass it as an `action` prop or `provide` it",
    );
    let e = refused(&format!(
        "component App\n  state n = 0\n  action go\n    teleport()\n{BUTTON}"
    ));
    assert_eq!(e.id, "type-unknown-command");
    assert!(
        e.message
            .starts_with("`teleport` is not a host command; the hosts answer blur, "),
        "{e}"
    );
    says(
        "component App\n  state n = 0\n  action save\n    n = 1\n  action go\n    let x = save()\n    n = 2\n  view\n    button press=go testId=\"go\"\n      text \"go\"\n",
        "type-call-value",
        "`save(…)` is a call of an action: a call is a statement and returns nothing; compute values with `fn`",
    );
    says(
        "component App\n  state n = 0\n  action save\n    n = 1\n  action run(what)\n    n = 2\n  action go\n    run(save)\n  view\n    button press=go testId=\"go\"\n      text \"go\"\n",
        "type-call-action-arg",
        "an argument of `run` is an action: a call passes values; call the action itself, or bind it where the view passes it (`go=act`)",
    );
    says(
        "component App\n  state n = 0\n  action a\n    n = 1\n    b()\n  action b\n    a()\n  view\n    button press=a testId=\"go\"\n      text \"go\"\n",
        "syntax-call-cycle",
        "calling `a` comes back to an action already on the way: a → b → a; an action never calls itself, directly or through others",
    );
    says(
        "component App\n  state n = 0\n  view\n    Child(go=n + 1)\n\ncomponent Child\n  props\n    go: action\n  state m = 0\n  action tap\n    go()\n    m = 1\n  view\n    button press=tap testId=\"go\"\n      text \"go\"\n",
        "syntax-call-target",
        "`go` is called by an action, so it must name an action, as `go=act` or `go=act(args)` does",
    );
}

#[test]
fn an_action_that_grows_past_the_bound_is_refused_before_it_is_built() {
    // Each action calls the next twice: 2^12 copies of the last, were they
    // built.
    let mut src = String::from("component App\n  state n = 0\n  action a12\n    n = n + 1\n");
    for i in (0..12).rev() {
        src += &format!("  action a{i}\n    a{}()\n    a{}()\n", i + 1, i + 1);
    }
    src += "  view\n    button press=a0 testId=\"go\"\n      text \"go\"\n";
    // `a3` is the first, in declaration order, whose expansion passes the
    // bound: it would hold 1,534 statements (`a4`'s 766, twice, and its two
    // calls).
    says(
        &src,
        "syntax-call-size",
        "`a3` grows past 1024 statements as its calls expand, at this call of `a12`: an action and every action it calls are one body; call fewer, or move the shared work into one action",
    );
}

#[test]
fn a_mutation_sent_twice_through_calls_names_the_calls() {
    says(
        "component App\n  mutation saved as shape string\n  action commitEdit\n    send saved = save(\"a\")\n  action openSheet(id: string)\n    send saved = save(id)\n  action enter\n    commitEdit()\n    openSheet(\"b\")\n  view\n    button press=enter testId=\"go\"\n      text \"go\"\n",
        "analyze-send-twice",
        "`enter` sends `saved` twice on one path: in `commitEdit` (called at line 8) and in `openSheet` (called at line 9). Only the last send's reply reaches `saved` (LLP 1016 D5): send once, or use a mutation per request",
    );
    says(
        "component App\n  mutation saved as shape string\n  action save\n    send saved = save(\"a\")\n  action enter\n    save()\n    save()\n  view\n    button press=enter testId=\"go\"\n      text \"go\"\n",
        "analyze-send-twice",
        "`enter` sends `saved` twice on one path: in `save` (called at line 6) and in `save` (called at line 7). Only the last send's reply reaches `saved` (LLP 1016 D5): send once, or use a mutation per request",
    );
    says(
        "component App\n  mutation saved as shape string then after\n  action commit\n    send saved = save(\"a\")\n  action after\n    commit()\n  view\n    button press=commit testId=\"go\"\n      text \"go\"\n",
        "analyze-then-self-send",
        "`after` cannot send `saved` (it calls `commit` at line 6, which sends it): it runs when that mutation answers",
    );
}

#[test]
fn an_ambiguous_call_is_refused_once_and_a_shape_named_action_is_no_action() {
    // Checked as the host command too, `close("swiped")` would also be told
    // `close()` takes no arguments; the ambiguity alone is reported.
    let src = r#"component App
  state open = true
  action dismiss(why: string)
    open = false
  view
    Viewer(close=dismiss)

component Viewer
  props
    close: action
  action swiped
    close("swiped")
  view
    button press=swiped testId="swipe"
      text "x"
"#;
    let dir = std::env::temp_dir().join(format!("contract-ambiguous-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, src).unwrap();
    let all = contract::compile_path_all(&path, false)
        .map(|_| ())
        .expect_err("refused");
    let ids: Vec<&str> = all.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["syntax-call-ambiguous"], "{all:?}");
    // Nor checked as the command where it checks the arguments themselves
    // (`share`'s named arguments, `postMessage`'s surface), in a child and
    // in the root.
    let argued = r#"component App
  state n = 0
  action share(v: number)
    n = v
  action go
    share(1)
  view
    column
      button press=go testId="go"
        text "go"
      Viewer(close=go, postMessage=go, value=1)

component Viewer
  props
    close: action
    value: number
    postMessage: action
  action swiped
    close("swiped")
    postMessage("hi", "nowhere")
  view
    button press=swiped testId="swipe"
      text `${value + 1}`
"#;
    std::fs::write(&path, argued).unwrap();
    let all = contract::compile_path_all(&path, false)
        .map(|_| ())
        .expect_err("refused");
    std::fs::remove_dir_all(&dir).unwrap();
    let ids: Vec<&str> = all.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(
        ids, ["syntax-call-ambiguous"; 3],
        "share, close and postMessage, and nothing else: {all:#?}"
    );
    // A prop whose type is a shape the file names `action` is a record.
    let record = r#"shape action
  value: number

component App
  view
    Child(close=action(value=1))

component Child
  props
    close: action
  action leave
    close()
  view
    button press=leave testId="leave"
      text "quit"
"#;
    contract::compile(record).unwrap_or_else(|e| panic!("{e}"));
}

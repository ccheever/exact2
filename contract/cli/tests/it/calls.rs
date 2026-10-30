//! LLP 1089: an action calls an action of its own component, an `action`
//! prop or an injected action, as a statement, anywhere. The call is the
//! callee's statements expanded in place: one commit, every statement
//! reading the state the action started with. LLP 1017 §11's tail call (the
//! Signal Clone's photo viewer) is one case of it, and keeps its tests.

use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

mod values;

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

#[test]
fn relaunch_is_a_host_command_without_arguments() {
    let src = "component App\n  action restart\n    relaunch()\n  view\n    button press=restart\n      text \"Restart\"\n";
    let mut r = boot(src);
    r.act("restart", vec![]).unwrap();
    let commands = r.take_commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "relaunch");
    assert!(commands[0].args.is_empty());
    let error = contract::compile(&src.replace("relaunch()", "relaunch(1)"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("takes no arguments"), "{error}");
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
            PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
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
      if open
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
fn an_empty_list_or_none_argument_takes_the_parameters_type() {
    let src = r#"shape Item
  done: bool

component App
  state n = 0
  action count(items: Item[], first: option<Item>)
    n = items.filter(x => x.done).length + (match first { case some(i) => (i.done ? 1 : 0), case none => 10 })
  action go
    count([], none)
  view
    column
      button press=go testId="go"
        text "go"
      text `${n}` testId="n"
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "n"), "10");
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
    out = `${toString} ${length.toString()}`
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
  resource items = items() as shape Item[]
  view
    column
      for item in items key=item.id
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
  resource items = items() as shape Item[]
  action pick(id: string)
    picked = id
  view
    column
      for item in items key=item.id
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
  resource items = items() as shape Item[]
  action reset
    count = 0
  action go
    reset()
    shown = items.map(count => count.id).join(",")
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
  resource items = items() as shape Item[]
  action flip(id: string, checked: bool)
    seen = `${id} ${checked}`
  action toggled(id: string, checked: bool)
    flip(id, checked)
  view
    column
      for item in items key=item.id
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
    for body in [
        r#"    if k == "ArrowDown"
      move(1, 0)
    if k == "ArrowRight"
      move(0, 1)"#,
        r#"    if k == ""
      let ignored = k
    else
      let dr = k == "ArrowDown" ? 1 : 0
      let dc = k == "ArrowRight" ? 1 : 0
      move(dr, dc)"#,
        r#"    match some(k)
      case some(key)
        if key != ""
          let dr = key == "ArrowDown" ? 1 : 0
          let dc = key == "ArrowRight" ? 1 : 0
          move(dr, dc)
      case none
        let ignored = k"#,
        r#"    match (k == "" ? some(k) : none)
      case some(key)
        let ignored = key
      case none
        let dr = k == "ArrowDown" ? 1 : 0
        let dc = k == "ArrowRight" ? 1 : 0
        move(dr, dc)"#,
    ] {
        let src = format!(
            r#"component App
  state r = 0
  state c = 0
  action move(dr, dc)
    r = r + dr
    c = c + dc
  action clipKey(k: string)
{body}
  view
    column
      button press=clipKey("ArrowDown") testId="down"
        text "down"
      button press=clipKey("ArrowRight") testId="right"
        text "right"
      text `${{r}} ${{c}}` testId="at"
"#
        );
        let mut r = boot(&src);
        press(&mut r, "down");
        press(&mut r, "right");
        assert_eq!(text_of(&r, "at"), "1 1", "{body}");
    }
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

/// The one warning `src` earns, by id and whole message; it compiles.
fn warns(src: &str, id: &str, message: &str) {
    let (result, warnings) = contract::compile_warned(src);
    result.unwrap_or_else(|e| panic!("a warning never fails a build: {e}"));
    let got: Vec<(&str, &str)> = warnings
        .iter()
        .map(|w| (w.id.as_str(), w.message.as_str()))
        .collect();
    assert_eq!(got, [(id, message)], "{src}");
}

#[test]
fn a_read_another_frame_made_stale_is_warned_of_in_each_of_d3s_cases() {
    // One rule with the body's read (LLP 1035.005.002 C2): the same id, a
    // warning, naming both frames.
    const STALE: &str = "analyze-read-after-assign";
    let follow = "component App\n  state sel = 0\n  state seen = 0\n  action follow\n    seen = sel\n  action go\n    sel = 3\n    follow()\n";
    warns(
        &format!("{follow}{BUTTON}"),
        STALE,
        "`follow` (called at line 8) reads `sel`, which `go` assigns at line 7; `follow` sees the value `sel` had when the action started: writes land when the action ends. Pass the value it should see: a `let` bound before line 7 keeps the starting value, and the value assigned at line 7 gives the new one",
    );
    // The argument is read in the callee's frame, and named as the argument.
    let argument = "component App\n  state sel = 0\n  state seen = 0\n  action follow(v: number)\n    seen = v\n  action go\n    sel = 3\n    follow(sel)\n";
    warns(
        &format!("{argument}{BUTTON}"),
        STALE,
        "an argument of `follow(…)` at line 8 reads `sel`, which `go` assigns at line 7; the argument of `follow` sees the value `sel` had when the action started: writes land when the action ends. Pass the value it should see: a `let` bound before line 7 keeps the starting value, and the value assigned at line 7 gives the new one",
    );
    let reset = "component App\n  state count = 0\n  action reset\n    count = 0\n  action go\n    reset()\n    count = count + 1\n";
    warns(
        &format!("{reset}{BUTTON}"),
        STALE,
        "`go` reads `count` at line 7, which `reset` (called at line 6) assigns at line 4; `go` sees the value `count` had when the action started: writes land when the action ends. Pass the value it should see: a `let` bound before line 6 keeps the starting value, and the value assigned at line 4 gives the new one",
    );
    // Both read the starting cell and the later write wins: not a diagonal.
    // A warning names every such read, where the refusal stopped at one.
    let diagonal = "component App\n  state r = 0\n  state c = 0\n  action move(dr: number, dc: number)\n    r = r + dr\n    c = c + dc\n  action go\n    move(1, 0)\n    move(0, 1)\n";
    let (result, warnings) = contract::compile_warned(&format!("{diagonal}{BUTTON}"));
    result.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        warnings.iter().map(|w| w.message.as_str()).collect::<Vec<_>>(),
        [
            "`move` (called at line 9) reads `r`, which `move` (called at line 8) assigns at line 5; `move` sees the value `r` had when the action started: writes land when the action ends. Pass the value it should see: a `let` bound before line 8 keeps the starting value, and the value assigned at line 5 gives the new one",
            "`move` (called at line 9) reads `c`, which `move` (called at line 8) assigns at line 6; `move` sees the value `c` had when the action started: writes land when the action ends. Pass the value it should see: a `let` bound before line 8 keeps the starting value, and the value assigned at line 6 gives the new one",
        ]
    );
}

#[test]
fn the_bodys_exemptions_hold_behind_a_call_too() {
    // A field the copy left alone is the same value either way (`rereads.rs`);
    // a condition the assignment's path decides steps once an action.
    for (case, src) in [
        (
            "a copied record's unchanged field",
            "shape Item\n  id: string\n  n: number\ncomponent App\n  state item = Item(id=\"x\", n=0)\n  state shown = \"\"\n  action show\n    shown = item.id\n  action go\n    item = Item(item, n=item.n + 1)\n    show()\n",
        ),
        (
            "a phase chain through a call",
            "component App\n  state phase = 1\n  state shown = \"\"\n  action step\n    if phase == 2\n      shown = \"two\"\n  action go\n    if phase == 1\n      phase = 2\n    step()\n",
        ),
    ] {
        let (result, warnings) = contract::compile_warned(&format!("{src}{BUTTON}"));
        result.unwrap_or_else(|e| panic!("{case}: {e}"));
        assert!(warnings.is_empty(), "{case}: {warnings:?}");
    }
    // The replaced field is the stale read.
    let replaced = "shape Item\n  id: string\n  n: number\ncomponent App\n  state item = Item(id=\"x\", n=0)\n  state shown = \"\"\n  action show\n    shown = `${item.n}`\n  action go\n    item = Item(item, n=item.n + 1)\n    show()\n";
    let (_, warnings) = contract::compile_warned(&format!("{replaced}{BUTTON}"));
    assert_eq!(
        warnings
            .iter()
            .map(|w| (w.id.as_str(), w.span.line))
            .collect::<Vec<_>>(),
        [("analyze-read-after-assign", 8)]
    );
}

#[test]
fn reads_d3_leaves_alone_compile() {
    for (case, src) in [
        (
            "exclusive literal arms",
            "component App\n  state r = 0\n  action move(dr: number)\n    r = r + dr\n  action key(k: string)\n    if k == \"ArrowDown\" || k == \"Enter\"\n      move(1)\n    if k == \"ArrowUp\"\n      move(-1)\n  view\n    button press=key(\"Enter\") testId=\"go\"\n      text \"go\"\n",
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
        "`enter` sends `saved` twice on one path: in `commitEdit` (called at line 8) and in `openSheet` (called at line 9). Only the last send's reply reaches `saved` (LLP 1016 D5): send once, use a mutation per request, or declare `mutation saved … queue` to run both in order",
    );
    says(
        "component App\n  mutation saved as shape string\n  action save\n    send saved = save(\"a\")\n  action enter\n    save()\n    save()\n  view\n    button press=enter testId=\"go\"\n      text \"go\"\n",
        "analyze-send-twice",
        "`enter` sends `saved` twice on one path: in `save` (called at line 6) and in `save` (called at line 7). Only the last send's reply reaches `saved` (LLP 1016 D5): send once, use a mutation per request, or declare `mutation saved … queue` to run both in order",
    );
    // A call in an arm says why both can run (chess #2).
    says(
        "component App\n  state n = 0\n  mutation saved as shape string\n  action save\n    send saved = save(\"a\")\n  action enter\n    if n > 0\n      save()\n    save()\n  view\n    button press=enter testId=\"go\"\n      text \"go\"\n",
        "analyze-send-twice",
        "`enter` sends `saved` twice on one path, in `save` (called at line 8) and in `save` (called at line 9): every `if` is read as one that can run, unless the two sends are arms of one `if`/`else` or `match`, or sit in `if`s testing one unchanged name against different literals. Only the last send's reply reaches `saved` (LLP 1016 D5): send once, make the sends arms of one `if … else if`, use a mutation per request, or declare `mutation saved … queue` to run both in order",
    );
    says(
        "component App\n  mutation saved as shape string then after\n  action commit\n    send saved = save(\"a\")\n  action after\n    commit()\n  view\n    button press=commit testId=\"go\"\n      text \"go\"\n",
        "analyze-then-self-send",
        "`after` cannot send `saved` (it calls `commit` at line 6, which sends it): it runs when that mutation answers. To repeat while a condition holds, declare a root task: `task NAME when COND`, with `every(ms, action)` on the next indented line",
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

/// What an action prop was curried with is a hidden parameter of every
/// lifted action of the use (D7). One action a handler binds types it for
/// its siblings, one nothing binds included; when none is bound or called,
/// the refusal names the prop's binding, never the capture.
#[test]
fn a_childs_hidden_capture_is_typed_by_any_bound_sibling_and_never_named() {
    let src = r#"component App
  view
    column
      Panel(title="Panel")

component Panel
  props
    title: string
  state last = ""
  action note(label: string)
    last = `${title} ${label}`
  view
    column
      text last testId="last"
      Leaf(label="a", tell=note)

component Leaf
  props
    label: string
    tell: action
  state n = 0
  action spare
    tell("spare")
  action idle
    n = n + 1
  action go
    tell(label)
  view
    button "Go" press=go testId="go"
"#;
    let mut r = boot(src);
    press(&mut r, "go");
    assert_eq!(text_of(&r, "last"), "Panel a");
    // Nothing binds or calls any of Leaf's actions: one refusal, at
    // `tell=note`, naming the first action that calls `tell`.
    let unbound = src.replace(" press=go", "");
    let e = refused(&unbound);
    assert_eq!(e.id, "type-cannot-infer", "{e}");
    assert_eq!(
        e.message,
        "cannot infer the type of what `note` carries from this binding into `spare`, which calls it: bind `spare` to a handler or call it from one"
    );
    assert_eq!((e.span.line, e.span.col), (15, 23), "{e}");
    let dir = std::env::temp_dir().join(format!("contract-capture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, &unbound).unwrap();
    let all = contract::compile_path_all(&path, false)
        .map(|_| ())
        .expect_err("refused");
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(all.len(), 1, "{all:#?}");
}

/// A handler names an action, as lowering binds it, though an app `fn` of
/// the same name is what a call in an expression reaches.
#[test]
fn a_handler_calls_the_action_an_app_fn_shares_a_name_with() {
    let src = "fn pick(xs: string[], i: number): string = match xs.at(i) { case some(s) => s, case none => \"\" }\ncomponent App\n  state v = \"\"\n  action pick(s: string)\n    v = s\n  view\n    column\n      text v testId=\"v\"\n      text pick([\"a\", \"b\"], 1) testId=\"fn\"\n      button \"pick\" press=pick(\"x\") testId=\"pick\"\n";
    let mut r = boot(src);
    assert_eq!(text_of(&r, "fn"), "b");
    press(&mut r, "pick");
    assert_eq!(text_of(&r, "v"), "x");
    // An argument the action does not take is refused as the action's.
    let e = refused(&src.replace("press=pick(\"x\")", "press=pick(1)"));
    assert_eq!(e.message, "`pick` expects `string`, given `number`", "{e}");
}

/// An action prop named like an app `fn` is refused where it is declared
/// when its component calls the name in an expression: inlined, that call
/// is the bound action's, where the checker typed it as the fn (the euxy
/// port's `pick`). A handler's own call is the action's both ways, so a
/// prop only handlers call, or nothing does, is no clash.
#[test]
fn an_action_prop_named_like_an_app_fn_is_refused_where_an_expression_calls_it() {
    const FN: &str = "fn pick(xs: string[], i: number): string = match xs.at(i) { case some(s) => s, case none => \"\" }\n";
    let src = format!("{FN}component App\n  state v = \"\"\n  action choose(s: string)\n    v = s\n  view\n    Seg(labels=[\"a\", \"b\"], pick=choose)\ncomponent Seg\n  props\n    labels: string[]\n    pick: action\n  view\n    row\n      for o, i in labels key=o\n        button press=pick(o)\n          text pick(labels, i)\n");
    let e = refused(&src);
    assert_eq!(e.id, "type-prop-hides-fn", "{e}");
    assert_eq!(
        e.message,
        "rename the action prop `pick` (as in `onPick`) or `fn pick`: in `Seg` the call `pick(…)` at line 16 cannot be both the prop and the fn"
    );
    assert_eq!(e.span.line, 11);
    // A call in a handler's argument, a derive, or a use's argument is an
    // expression's too; an injected action is checked as a prop is.
    let argument = src
        .replace("text pick(labels, i)", "text o")
        .replace("press=pick(o)", "press=pick(pick(labels, i))");
    assert_eq!(refused(&argument).id, "type-prop-hides-fn");
    let derive = src.replace("text pick(labels, i)", "text o").replace(
        "  view\n    row",
        "  derive first = pick(labels, 0)\n  view\n    row",
    );
    assert_eq!(refused(&derive).id, "type-prop-hides-fn");
    let inject = format!("{FN}component App\n  state v = \"\"\n  action choose(s: string)\n    v = s\n  provide\n    pick = choose\n  view\n    Seg()\ncomponent Seg\n  inject\n    pick: action\n  view\n    text pick([\"a\"], 0)\n");
    let e = refused(&inject);
    assert_eq!(e.id, "type-prop-hides-fn", "{e}");
    assert_eq!(
        e.message,
        "rename the action inject `pick` (as in `onPick`) or `fn pick`: in `Seg` the call `pick(…)` at line 14 cannot be both the inject and the fn"
    );
    // Handlers alone: the prop is the action, curried or bare, and the fn
    // is the fn where the app calls it.
    let handlers = format!("{FN}component App\n  state v = \"\"\n  action choose(s: string)\n    v = s\n  view\n    column\n      text v testId=\"v\"\n      text pick([\"x\", \"y\"], 1) testId=\"fn\"\n      Seg(labels=[\"a\", \"b\"], pick=choose)\ncomponent Seg\n  props\n    labels: string[]\n    pick: action\n  view\n    row\n      for o in labels key=o\n        button o press=pick(o) testId=`pick-${{o}}`\n");
    let mut r = boot(&handlers);
    assert_eq!(text_of(&r, "fn"), "y");
    press(&mut r, "pick-b");
    assert_eq!(text_of(&r, "v"), "b");
    let bare = handlers
        .replace("press=pick(o)", "press=pick")
        .replace("pick=choose)", "pick=choose(\"bare\"))");
    let mut r = boot(&bare);
    press(&mut r, "pick-a");
    assert_eq!(text_of(&r, "v"), "bare");
    // Never called: nothing clashes.
    let unused = src.replace(
        "        button press=pick(o)\n          text pick(labels, i)\n",
        "        text o\n",
    );
    assert!(
        contract::compile(&unused).is_ok(),
        "{:?}",
        contract::compile(&unused).err()
    );
}

/// A child's handler names an action of its own component or an action
/// prop (analyze's `check_handler`), never the root's, and never a `fn`:
/// where the root declares the action and an app `fn` shares its name, the
/// child's `press=pick("x")` was typed as the fn and refused for its arity.
#[test]
fn a_childs_handler_naming_the_roots_action_is_refused_as_a_handler_not_typed_as_a_fn() {
    const FN: &str = "fn pick(xs: string[], i: number): string = match xs.at(i) { case some(s) => s, case none => \"\" }\n";
    let app = "component App\n  state v = \"\"\n  action pick(s: string)\n    v = s\n  view\n    column\n      text v testId=\"v\"\n      Row()\ncomponent Row\n  view\n    button \"pick\" press=pick(\"x\") testId=\"pick\"\n";
    let message = "`pick` is an action of `App`, not in `Row`'s scope: `press=` names an action of its own component or an `action` prop; pass it as one (`pick: action` under `props`, `pick=pick` at the use) or `provide` it";
    for src in [format!("{FN}{app}"), app.to_string()] {
        let e = refused(&src);
        assert_eq!(e.id, "type-handler-action", "{e}");
        assert_eq!(e.message, message);
        assert_eq!(e.span.col, 25);
    }
    // Bare, and with the fn's own arity, the same.
    let e = refused(&format!("{FN}{app}").replace("press=pick(\"x\")", "press=pick([\"a\"], 0)"));
    assert_eq!(e.id, "type-handler-action", "{e}");
    let e = refused(&app.replace("press=pick(\"x\")", "press=pick"));
    assert_eq!(
        (e.id.as_str(), e.message.as_str()),
        ("type-handler-action", message)
    );
    // A handler naming only a fn is told it is one.
    let e = refused(&format!(
        "{FN}component App\n  state v = \"\"\n  view\n    button \"pick\" press=pick([\"a\"], 0)\n"
    ));
    assert_eq!(e.id, "type-handler-action", "{e}");
    assert_eq!(
        e.message,
        "`pick` is a `fn`, not an action: `press=` names an action of its own component or an `action` prop"
    );
    // Passed as the prop the message names, it runs.
    let passed = format!("{FN}{app}")
        .replace("Row()", "Row(pick=pick)")
        .replace(
            "component Row\n",
            "component Row\n  props\n    pick: action\n",
        );
    let mut r = boot(&passed);
    press(&mut r, "pick");
    assert_eq!(text_of(&r, "v"), "x");
}

/// `press=pick()` is the action `pick()`, as `press=pick` is, though an app
/// `fn pick` takes an argument: a local action, an action prop and an
/// injected action alike.
#[test]
fn a_handler_called_with_no_arguments_is_the_action_not_the_fn() {
    const FN: &str = "fn pick(n: number): number = n + 1\n";
    let local = format!("{FN}component App\n  state v = 0\n  action pick\n    v = pick(v)\n  view\n    column\n      text `${{v}}` testId=\"v\"\n      button \"pick\" press=pick() testId=\"pick\"\n");
    let mut r = boot(&local);
    press(&mut r, "pick");
    assert_eq!(text_of(&r, "v"), "1");
    let prop = format!("{FN}component App\n  state v = 0\n  action bump\n    v = pick(v)\n  view\n    column\n      text `${{v}}` testId=\"v\"\n      Seg(pick=bump)\ncomponent Seg\n  props\n    pick: action\n  view\n    button \"pick\" press=pick() testId=\"pick\"\n");
    let mut r = boot(&prop);
    press(&mut r, "pick");
    assert_eq!(text_of(&r, "v"), "1");
    let inject = format!("{FN}component App\n  state v = 0\n  action bump\n    v = pick(v)\n  provide\n    pick = bump\n  view\n    column\n      text `${{v}}` testId=\"v\"\n      Seg()\ncomponent Seg\n  inject\n    pick: action\n  view\n    button \"pick\" press=pick() testId=\"pick\"\n");
    let mut r = boot(&inject);
    press(&mut r, "pick");
    assert_eq!(text_of(&r, "v"), "1");
    // An argument the parameterless action does not take is the handler's
    // refusal (analyze's, which knows the event's payload), never the fn's.
    let e = refused(&local.replace("press=pick()", "press=pick(1)"));
    assert_eq!(e.id, "analyze-handler-arity", "{e}");
}

/// A child's `provide` is substituted with its props as its view is, so a
/// provided value calling an action prop named like an app `fn` is the
/// clash too: typed as the fn, lifted as the action's name.
#[test]
fn an_action_prop_named_like_an_app_fn_is_refused_where_a_provide_calls_it() {
    let src = "fn pick(xs: string[], i: number): string = match xs.at(i) { case some(s) => s, case none => \"\" }\ncomponent App\n  state v = \"\"\n  action choose(s: string)\n    v = s\n  view\n    Seg(labels=[\"a\", \"b\"], pick=choose)\ncomponent Seg\n  props\n    labels: string[]\n    pick: action\n  provide\n    label = pick(labels, 0)\n  view\n    Leaf()\ncomponent Leaf\n  inject\n    label: string\n  view\n    text label\n";
    let e = refused(src);
    assert_eq!(e.id, "type-prop-hides-fn", "{e}");
    assert_eq!(
        e.message,
        "rename the action prop `pick` (as in `onPick`) or `fn pick`: in `Seg` the call `pick(…)` at line 13 cannot be both the prop and the fn"
    );
}

#[test]
fn value_props_and_injects_named_like_an_app_fn_refuse_expression_calls() {
    let prop = "fn pick(n: number): number = n + 1\nfn other(n: number): number = n + 100\ncomponent App\n  state other = 7\n  view\n    Seg(pick=other)\ncomponent Seg\n  props\n    pick: number\n  view\n    text `${pick(1)}` testId=\"result\"\n";
    let inject = prop
        .replace(
            "  view\n    Seg(pick=other)",
            "  provide\n    pick = other\n  view\n    Seg()",
        )
        .replace("  props\n", "  inject\n");
    for (src, kind, declaration, call) in [(prop, "prop", 9, 11), (&inject, "inject", 11, 13)] {
        let e = refused(src);
        assert_eq!(e.id, "type-prop-hides-fn", "{e}");
        assert_eq!(e.span.line, declaration);
        assert_eq!(e.message, format!(
            "rename the {kind} `pick` or `fn pick`: in `Seg` the call `pick(…)` at line {call} cannot be both the {kind} and the fn"
        ));
        let value = src.replace("${pick(1)}", "${pick}");
        assert_eq!(text_of(&boot(&value), "result"), "7");
    }
}

#[test]
fn expression_binders_protect_calls_from_a_prop_named_like_an_app_fn() {
    let src = "fn pick(n: number): number = n + 1\ncomponent App\n  state v = 0\n  action choose(n: number)\n    v = n\n  view\n    Seg(pick=choose)\ncomponent Seg\n  props\n    pick: action\n  view\n    text `${EXPR}` testId=\"result\"\n";
    let same_action = src.replace("choose", "pick");
    let extra = src
        .replace("Seg(pick=choose)", "Seg(pick=choose, extra=pick(3))")
        .replace(
            "    pick: action\n",
            "    pick: action\n    extra: number\n",
        );
    for source in [src, &same_action, &extra] {
        for expr in [
            "match some(7) { case some(pick) => pick(1), case none => 0 }",
            "match [7].map(pick => pick(1)).at(0) { case some(n) => n, case none => 0 }",
        ] {
            assert_eq!(text_of(&boot(&source.replace("EXPR", expr)), "result"), "2");
        }
    }
    // The binder covers only the some arm, and ends with the expression.
    for expr in [
        "match some(pick(1)) { case some(pick) => pick, case none => 0 }",
        "match some(7) { case some(pick) => pick, case none => pick(1) }",
        "[7].map(pick => pick(1)).length + pick(1)",
    ] {
        assert_eq!(refused(&src.replace("EXPR", expr)).id, "type-prop-hides-fn");
    }
}

#[test]
fn node_binders_do_not_protect_calls_from_a_prop_named_like_an_app_fn() {
    let src = "fn pick(n: number): number = n + 1\ncomponent App\n  state v = 0\n  action choose(n: number)\n    v = n\n  view\n    Seg(pick=choose)\ncomponent Seg\n  props\n    pick: action\n  view\nBODY";
    for body in [
        "    for pick in [7] key=pick\n      text `${pick(1)}`\n",
        "    for n, pick in [7] key=pick\n      text `${pick(1)}`\n",
        "    match some(7)\n      case some(pick)\n        text `${pick(1)}`\n      case none\n        text \"none\"\n",
    ] {
        assert_eq!(refused(&src.replace("BODY", body)).id, "type-prop-hides-fn");
    }
}

#[test]
fn derive_binders_protect_calls_from_a_prop_named_like_an_app_fn() {
    let src = "fn pick(n: number): number = n + 1\ncomponent App\n  state v = 0\n  action choose(n: number)\n    v = n\n  view\n    Seg(pick=choose)\ncomponent Seg\n  props\n    pick: action\n  derive shown = EXPR\n  view\n    text `${shown}` testId=\"result\"\n";
    let no_props = src
        .replace("Seg(pick=choose)", "Seg()")
        .replace("  props\n    pick: action\n", "");
    let same_derive = no_props.replace("shown", "pick");
    for source in [src, &no_props, &same_derive] {
        for expr in [
            "match some(7) { case some(pick) => pick(1), case none => 0 }",
            "match [7].map(pick => pick(1)).at(0) { case some(n) => n, case none => 0 }",
        ] {
            assert_eq!(text_of(&boot(&source.replace("EXPR", expr)), "result"), "2");
        }
    }
}

fn derive_call_source(body: &str, declarations: &str, view: &str) -> String {
    format!(
        "fn pick(n: number): number = n + 1\nfn other(n: number): number = n + 100\ncomponent App\n  view\n    Seg()\ncomponent Seg\n  derive pick = {body}\n{declarations}  view\n{view}\ncomponent Frame\n  slot\n  view\n    children\ncomponent Label\n  props\n    value: string\n  view\n    text value testId=\"result\"\n"
    )
}

#[test]
fn a_derive_named_like_an_app_fn_is_not_a_dependency_of_its_call() {
    for (body, expected) in [("pick(1)", "2 6"), ("other(1)", "101 6")] {
        for view in [
            "    text `${pick} ${pick(5)}` testId=\"result\"",
            "    Frame()\n      text `${pick} ${pick(5)}` testId=\"result\"",
            "    Label(value=`${pick} ${pick(5)}`)",
        ] {
            let src = derive_call_source(body, "", view);
            assert_eq!(text_of(&boot(&src), "result"), expected);
        }
    }
}

#[test]
fn a_derive_named_like_an_app_fn_leaves_state_initializer_calls_alone() {
    for (body, expected) in [("pick(1)", "2 6"), ("other(1)", "101 6")] {
        let src = derive_call_source(
            body,
            "  state shown = pick(5)\n",
            "    text `${pick} ${shown}` testId=\"result\"",
        );
        assert_eq!(text_of(&boot(&src), "result"), expected);
    }
}

#[test]
fn a_derive_named_like_an_app_fn_leaves_action_body_calls_alone() {
    for (body, value) in [("pick(1)", "2"), ("other(1)", "101")] {
        let src = derive_call_source(
            body,
            "  state shown = 0\n  action update(n: number)\n    shown = pick(n)\n  action relay()\n    update(5)\n",
            "    column\n      text `${pick} ${shown}` testId=\"result\"\n      button \"Update\" press=update(5) testId=\"update\"\n      button \"Reset\" press=update(0) testId=\"reset\"\n      button \"Relay\" press=relay testId=\"relay\"",
        );
        let mut r = boot(&src);
        assert_eq!(text_of(&r, "result"), format!("{value} 0"));
        for (button, expected) in [("update", 6), ("reset", 1), ("relay", 6)] {
            press(&mut r, button);
            assert_eq!(text_of(&r, "result"), format!("{value} {expected}"));
        }
    }
}

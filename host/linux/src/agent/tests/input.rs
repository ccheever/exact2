//! The Linux agent protocol's input tests: the clipboard, keys, modifiers and
//! mouse contacts, driven through `handle`.
use super::*;

#[test]
fn the_clipboard_events_reach_the_nearest_handler() {
    let plan = contract::compile("component App\n  state log = \"\"\n  action pasted(at: string, e: ClipboardEvent)\n    log = `${log}${at}:${e.text};`\n  action copied\n    log = `${log}copy;`\n  action seen\n    log = log\n  view\n    column width=300 paste=pasted(\"grid\") copy=copied testId=\"grid\"\n      box testId=\"cell\" width=50 height=20 focus=seen\n      text log testId=\"log\" height=20\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<NoData>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let (cell, log) = (id(&p, "cell"), id(&p, "log"));
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{cell},"clipboard":"paste","text":"a\tb"}}"#),
    );
    assert!(reply.contains("\"clipboard\":\"paste\""), "{reply}");
    handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{cell},"clipboard":"copy"}}"#),
    );
    let k = p.host().kernel();
    assert_eq!(
        k.node(log).unwrap().props.str(exact_kernel::PropId::Text),
        Some("grid:a\tb;copy;")
    );
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{log},"clipboard":"cut"}}"#),
    );
    assert!(reply.contains("no cut handler"), "{reply}");
}

/// A paste is Ctrl+V first. A `key` handler that `preventDefault()`s that
/// chord keeps the clipboard event from landing; one that does not still
/// hears the key and the paste. Copy stays the clipboard event alone
/// (drums R15: the driver's paste skipped the key and hid that bug).
#[test]
fn a_paste_is_ctrl_v_and_a_prevented_chord_skips_the_clipboard() {
    let head = r#"component App
  state log = ""
  action keyed(k: string, e: KeyboardEvent)
    log = `${log}key:${k}:${e.ctrlKey};`
"#;
    let tail = r#"  action pasted(e: ClipboardEvent)
    log = `${log}paste:${e.text};`
  action copied
    log = `${log}copy;`
  view
    column width=300
      box key=keyed paste=pasted copy=copied testId="cell" width=80 height=24
      text log testId="log" height=20
"#;
    let text_of = |p: &Presenter<NoData>| {
        let k = p.host().kernel();
        let log = k.node_by_key(k.find_by_test_id("log")[0]).unwrap().id;
        k.node(log)
            .unwrap()
            .props
            .str(exact_kernel::PropId::Text)
            .unwrap()
            .to_string()
    };
    for prevent in [false, true] {
        let guard = if prevent {
            "    if k == \"v\" and e.ctrlKey\n      preventDefault()\n"
        } else {
            ""
        };
        let plan = contract::compile(&format!("{head}{guard}{tail}")).unwrap();
        let (mut p, boot_error) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(boot_error.is_none(), "{boot_error:?}");
        let k = p.host().kernel();
        let cell = k.node_by_key(k.find_by_test_id("cell")[0]).unwrap().id;
        let reply = handle(
            &mut p,
            &format!(r#"{{"op":"type","id":{cell},"clipboard":"paste","text":"secret"}}"#),
        );
        assert!(reply.contains("\"clipboard\":\"paste\""), "{reply}");
        assert!(!reply.contains("error"), "{reply}");
        let log = text_of(&p);
        assert!(
            log.starts_with("key:v:true;"),
            "prevent={prevent} the chord was not delivered: {log}"
        );
        if prevent {
            assert!(
                !log.contains("paste:") && !log.contains("secret"),
                "a prevented chord still pasted: {log}"
            );
        } else {
            assert_eq!(log, "key:v:true;paste:secret;");
        }
        handle(
            &mut p,
            &format!(r#"{{"op":"type","id":{cell},"clipboard":"copy"}}"#),
        );
        let log = text_of(&p);
        assert!(
            log.ends_with("copy;") && log.matches("key:").count() == 1,
            "copy sent a key or dropped the chord: {log}"
        );
        let reply = handle(&mut p, &format!(r#"{{"op":"type","id":{cell},"key":"a"}}"#));
        assert!(!reply.contains("error"), "{reply}");
        let log = text_of(&p);
        assert!(
            log.ends_with("key:a:false;"),
            "Ctrl stayed down after the paste: {log}"
        );
    }
}

/// Gallery F20: `tap <id> modifiers Shift+Meta` presses with the keys held,
/// which the action's `MouseEvent` reports; the keys are released after.
#[test]
fn a_tap_holds_its_modifiers_for_the_press() {
    let plan = contract::compile("component App\n  state log = \"\"\n  action pick(e: MouseEvent)\n    log = `${log}${e.shiftKey}${e.metaKey}${e.altKey};`\n  view\n    column width=300\n      button press=pick testId=\"b\" width=50 height=20\n      text log testId=\"log\" height=20\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<NoData>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let (b, log) = (id(&p, "b"), id(&p, "log"));
    handle(
        &mut p,
        &format!(r#"{{"op":"tap","id":{b},"modifiers":"Shift+Meta"}}"#),
    );
    handle(&mut p, &format!(r#"{{"op":"tap","id":{b}}}"#));
    let k = p.host().kernel();
    assert_eq!(
        k.node(log).unwrap().props.str(exact_kernel::PropId::Text),
        Some("truetruefalse;falsefalsefalse;")
    );
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"tap","id":{b},"modifiers":"Hyper"}}"#),
    );
    assert!(reply.contains("error"), "{reply}");
}

/// A request that answers long after the drive moves on.
#[derive(Default)]
struct Stalled;
impl DataSource for Stalled {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(exact_runner::Answer::Later(
            exact_runner::Request::continuation(1),
        ))
    }
    fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
        exact_runner::Dispatch::Run(exact_runner::Work::Later(Box::new(|reply| {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(30));
                reply.send(exact_runner::Outcome::Storage(b"1".to_vec()));
            });
        })))
    }
}

/// A jump waits for no request with no timer due before it, and its reply
/// names what it left in flight, as the web hosts' do (calendar F10, workout
/// F6); `clock settle` waits for it and says `requests` past its bound.
#[test]
fn a_jump_names_the_requests_it_left_in_flight() {
    let plan = contract::compile(
        "component App\n  resource item = item() as shape number\n  view\n    text `${item}` testId=\"log\"\n",
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        Stalled,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let reply = json(handle(&mut p, r#"{"op":"clock","to":100}"#));
    assert_eq!(reply["clock"], 100, "{reply}");
    assert_eq!(reply["inflight"], 1, "{reply}");
    let reply = json(clock_within(
        &mut p,
        r#"{"op":"clock","settle":true}"#,
        std::time::Duration::from_millis(100),
    ));
    assert_eq!(reply["settled"], false, "{reply}");
    assert_eq!(reply["reason"], "requests", "{reply}");
}

/// Review A1: a drag's contact with `mouse` holds the left button through its
/// phases. Every phase says `mouse`, which a contact takes (a click's `mouse`
/// form stays the click's), and the box hears the press and the release.
#[test]
fn a_mouse_contact_goes_down_holds_and_lifts() {
    let plan = contract::compile("component App\n  state log = \"\"\n  action at(kind: string, e: PointerEvent)\n    log = `${log}${kind}:${e.pointerType};`\n  view\n    column width=300\n      box testId=\"pad\" width=200 height=100 touch-action=\"none\" pointerdown=at(\"down\") pointerup=at(\"up\")\n      text log testId=\"log\" height=20\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<NoData>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let (pad, log) = (id(&p, "pad"), id(&p, "log"));
    let down = handle(
        &mut p,
        &format!(r#"{{"op":"tap","phase":"down","id":{pad},"x":20,"y":20,"mouse":true}}"#),
    );
    assert!(!down.contains("\"error\""), "{down}");
    // A hold seeks the presenter clock and reports it, so the driver does not seek again (platformer R7).
    let before = p.host().now();
    let held = handle(
        &mut p,
        r#"{"op":"tap","phase":"hold","ms":32,"mouse":true,"virtual":true}"#,
    );
    assert!(!held.contains("\"error\""), "{held}");
    let held: serde_json::Value = serde_json::from_str(&held).unwrap();
    assert_eq!(held["clock"].as_f64(), Some(before + 32.0), "{held}");
    let up = handle(&mut p, r#"{"op":"tap","phase":"up","mouse":true}"#);
    assert!(!up.contains("\"error\""), "{up}");
    let k = p.host().kernel();
    assert_eq!(
        k.node(log).unwrap().props.str(exact_kernel::PropId::Text),
        Some("down:mouse;up:mouse;")
    );
}

#[test]
fn driver_key_uses_the_web_vocabulary() {
    assert_eq!(driver_key("p"), Some(("KeyP", "p")));
    assert_eq!(driver_key("P"), Some(("KeyP", "P")));
    assert_eq!(driver_key("KeyP"), Some(("KeyP", "p")));
    assert_eq!(driver_key("7"), Some(("Digit7", "7")));
    assert_eq!(driver_key("Digit7"), Some(("Digit7", "7")));
    assert_eq!(driver_key("End"), Some(("End", "End")));
    assert_eq!(driver_key("Home"), Some(("Home", "Home")));
    assert_eq!(driver_key("Delete"), Some(("Delete", "Delete")));
    assert_eq!(driver_key(" "), Some(("Space", " ")));
    assert_eq!(driver_key("Space"), Some(("Space", " ")));
    assert_eq!(driver_key("-"), Some(("Minus", "-")));
    assert_eq!(driver_key("+"), Some(("Equal", "+")));
    assert_eq!(driver_key("!"), Some(("Digit1", "!")));
    assert_eq!(driver_key("F1"), Some(("F1", "F1")));
    assert_eq!(driver_key("F12"), Some(("F12", "F12")));
    assert_eq!(driver_key("F24"), Some(("F24", "F24")));
    assert_eq!(driver_key("Shift"), Some(("ShiftLeft", "Shift")));
    assert_eq!(driver_key("ArrowLeft"), Some(("ArrowLeft", "ArrowLeft")));
    assert_eq!(driver_key("Nope"), None);
    assert_eq!(driver_key("Endd"), None);
}

/// `aria-keyshortcuts` (the web's input-glue rule, the Apple hosts'
/// `Shortcuts.swift`): a declared chord presses its button before the
/// focus's `key` handlers hear it, and goes no further — F13–F24 as F1 —
/// with or without a focus; the paste chord is one, so a button declaring
/// Control+V takes it and the clipboard event never lands; a text field
/// keeps its plain keys; nothing behind a shown `aria-modal`.
#[test]
fn aria_keyshortcuts_press_their_button_before_the_key_handlers() {
    let plan = contract::compile(
        r#"component App
  state log = ""
  state armed = false
  state modal = false
  state text = ""
  action keyed(k: string)
    log = `${log}key:${k};`
  action pasted(e: ClipboardEvent)
    log = `${log}paste:${e.text};`
  action pressed(what: string)
    log = `${log}${what};`
  action arm
    armed = not armed
  action edit(v: string)
    text = v
  action open
    modal = true
  view
    column width=300
      box key=keyed paste=pasted tabindex=0 testId="pad" width=80 height=24
      input value=text input=edit testId="field" height=24
      when armed
        button "Paste" aria-keyshortcuts="Meta+V Control+V" press=pressed("button-paste") testId="paste" height=24
      button "F13" aria-keyshortcuts="F13" press=pressed("f13") testId="f13" height=24
      button "F24" aria-keyshortcuts="Shift+F24" press=pressed("f24") testId="f24" height=24
      button "S" aria-keyshortcuts="s" press=pressed("s") testId="s" height=24
      button "arm" press=arm testId="arm" height=24
      button "open" press=open testId="open" height=24
      when modal
        column aria-modal=true testId="dialog" height=40
          button "Close" aria-keyshortcuts="Escape" press=pressed("close") testId="close" height=24
      button "Away" aria-keyshortcuts="Escape" press=pressed("away") testId="away" height=24
      text log testId="log" height=20
"#,
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 600.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let named = |p: &Presenter<NoData>, id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id
    };
    let log = |p: &mut Presenter<NoData>| {
        let k = p.host().kernel();
        let id = k.node_by_key(k.find_by_test_id("log")[0]).unwrap().id;
        let text = k.node(id).unwrap().props.str(exact_kernel::PropId::Text);
        text.unwrap_or("").to_string()
    };
    let (pad, field, root) = (named(&p, "pad"), named(&p, "field"), named(&p, "log"));
    let key = |p: &mut Presenter<NoData>, id: u32, chord: &str| {
        handle(p, &format!(r#"{{"op":"type","id":{id},"key":"{chord}"}}"#))
    };
    for (chord, want) in [
        ("F13", "f13;"),
        ("Shift+F24", "f24;"),
        ("F24", "key:F24;"),
        ("s", "s;"),
    ] {
        let before = log(&mut p);
        let reply = key(&mut p, pad, chord);
        assert!(!reply.contains("error"), "{chord}: {reply}");
        assert_eq!(log(&mut p), format!("{before}{want}"), "{chord}");
    }
    // A target that takes no focus (a text) leaves it where it is; the
    // page's shortcuts hear the key all the same.
    let before = log(&mut p);
    assert!(!key(&mut p, root, "F13").contains("error"));
    assert_eq!(log(&mut p), format!("{before}f13;"));
    // A field keeps its plain keys: `s` is typed, not the shortcut.
    let before = log(&mut p);
    key(&mut p, field, "s");
    assert_eq!(log(&mut p), before, "a field's plain key is its typing");
    // The paste chord: the key handler, then the paste, until a button declares it.
    let paste = |p: &mut Presenter<NoData>| {
        handle(
            p,
            &format!(r#"{{"op":"type","id":{pad},"clipboard":"paste","text":"x"}}"#),
        )
    };
    let before = log(&mut p);
    paste(&mut p);
    assert_eq!(log(&mut p), format!("{before}key:v;paste:x;"));
    p.tap(named(&p, "arm")).unwrap();
    let before = log(&mut p);
    paste(&mut p);
    assert_eq!(
        log(&mut p),
        format!("{before}button-paste;"),
        "the button took the chord"
    );
    // Behind a shown `aria-modal`, only its own shortcuts.
    let before = log(&mut p);
    key(&mut p, pad, "Escape");
    assert_eq!(log(&mut p), format!("{before}away;"));
    p.tap(named(&p, "open")).unwrap();
    let before = log(&mut p);
    key(&mut p, pad, "Escape");
    key(&mut p, pad, "F13");
    assert_eq!(log(&mut p), format!("{before}close;key:F13;"));
}

/// #140: a key's release runs the focus's `keyup` handlers, a modifier's
/// too, and both events carry the physical key and the auto-repeat — from
/// the agent's requests and from a hardware keyboard alike. A modifier's own
/// keydown holds it; its keyup no longer does, as DOM's flags say.
#[test]
fn keyup_hears_a_release_and_both_carry_code_and_repeat() {
    let plan = contract::compile(
        r#"component App
  state log = ""
  action down(k: string, e: KeyboardEvent)
    log = `${log}d:${k}:${e.code}:${e.repeat}:${e.metaKey};`
  action up(k: string, e: KeyboardEvent)
    log = `${log}u:${k}:${e.code}:${e.repeat}:${e.metaKey};`
  view
    column width=300
      box key=down keyup=up testId="cell" width=80 height=24
      text log testId="log" height=20
"#,
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let k = p.host().kernel();
    let cell = k.node_by_key(k.find_by_test_id("cell")[0]).unwrap().id;
    let log = |p: &mut Presenter<NoData>| {
        let k = p.host().kernel();
        let id = k.node_by_key(k.find_by_test_id("log")[0]).unwrap().id;
        let text = k.node(id).unwrap().props.str(exact_kernel::PropId::Text);
        text.unwrap_or("").to_string()
    };
    for request in [
        r#""key":"Meta","phase":"down""#,
        r#""key":"Meta","phase":"up""#,
        r#""key":"a","phase":"down""#,
        r#""key":"a","phase":"down","repeat":true"#,
        r#""key":"a","phase":"up""#,
        r#""key":"Meta+b""#,
    ] {
        let reply = handle(&mut p, &format!(r#"{{"op":"type","id":{cell},{request}}}"#));
        assert!(!reply.contains("error"), "{request}: {reply}");
    }
    assert_eq!(
        log(&mut p),
        "d:Meta:MetaLeft:false:true;u:Meta:MetaLeft:false:false;\
         d:a:KeyA:false:false;d:a:KeyA:true:false;u:a:KeyA:false:false;\
         d:b:KeyB:false:true;u:b:KeyB:false:true;"
    );
    // A hardware keyboard's ⌘W, held then released, then ⌘ released.
    let before = log(&mut p).len();
    p.hardware_key("MetaLeft", "Meta", true, false);
    p.hardware_key("KeyW", "w", true, false);
    p.hardware_key("KeyW", "w", true, true);
    p.hardware_key("KeyW", "w", false, false);
    p.hardware_key("MetaLeft", "Meta", false, false);
    assert_eq!(
        &log(&mut p)[before..],
        "d:Meta:MetaLeft:false:true;d:w:KeyW:false:true;d:w:KeyW:true:true;\
         u:w:KeyW:false:true;u:Meta:MetaLeft:false:false;"
    );
}

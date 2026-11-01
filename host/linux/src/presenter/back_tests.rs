//! System Back, in order (LLP 1115.001 D6): the authored control, the root's
//! `navigate`, the runner's own `back`, and who takes nothing; and a
//! desktop's Back keys, whose default action it is (LLP 1115.000 D5.4).
use super::*;

/// A two-route app. `ROOT` is the navigation root's own attributes, `BACK`
/// what the pushed route holds beside its text.
const APP: &str = r#"routes nav
  home "/"
    detail "/detail"
component App
  state pressed = 0
  state followed = 0
  state kept = 0
  action open
    nav = push(nav, "/detail")
  action keep(k: string, e: KeyboardEvent)
    if k == "ArrowLeft" and e.altKey
      kept = kept + 1
      preventDefault()
  action claim
    kept = kept + 10
  action back
    pressed = pressed + 1
    nav = back(nav)
  action follow(url: string)
    followed = followed + 1
    nav = go(nav, url)
  view
    main navigationKey=`${top(nav).id}` ROOT width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0
          if e.name == "home"
            button press=open testId="open"
              text "Open"
          if e.name == "detail"
            text "Detail" testId="detail"
            BACK
"#;

fn boot(root: &str, back: &str) -> Presenter<()> {
    let source = APP.replace("ROOT", root).replace("BACK", back);
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn id(p: &Presenter<()>, name: &str) -> Option<ViewId> {
    let kernel = p.host().kernel();
    let key = kernel.find_by_test_id(name).first().copied()?;
    Some(kernel.node_by_key(key)?.id)
}

/// Whether the pushed route is the one that shows.
fn on_detail(p: &Presenter<()>) -> bool {
    id(p, "detail").is_some_and(|id| !p.host().route_visibility(id).0)
}

/// A number slot's value, read from the runner.
fn slot(p: &Presenter<()>, name: &str) -> f64 {
    match p.host().runner().slot(name) {
        Some(exact_runner::Value::Number(n)) => *n,
        other => panic!("slot {name}: {other:?}"),
    }
}

fn open(p: &mut Presenter<()>) {
    let button = id(p, "open").unwrap();
    p.dispatch_press(button, 0., false);
    assert!(on_detail(p), "the button pushed the route");
}

const BUTTON: &str = "button id=\"back\" press=back testId=\"back\"\n              text \"Back\"";

#[test]
fn back_presses_the_roots_authored_control() {
    let mut p = boot("navigationBack=\"back\" navigate=follow", BUTTON);
    assert!(!p.back_goes(), "nowhere to go from the first screen");
    assert!(!p.system_back(1.), "and Back there is the shell's");
    open(&mut p);
    assert!(p.back_goes());
    assert!(p.system_back(2.));
    assert!(!on_detail(&p), "back on the first screen");
    // The author's control took it, not the root's `navigate`.
    assert_eq!(slot(&p, "pressed"), 1.0);
    assert_eq!(slot(&p, "followed"), 0.0);
    assert!(!p.system_back(3.), "and from there Back leaves the app");
}

/// A route with no control: Back goes to the location beneath through the root's `navigate`,
/// or, with no handler on the root, by the runner's own back (the router pops, nothing pressed).
#[test]
fn a_route_with_no_control_goes_back_by_the_roots_navigate_or_the_runners_own_back() {
    for (root, read, expected) in [("navigate=follow", "followed", 1.0), ("", "pressed", 0.0)] {
        let mut p = boot(root, "text \"no control here\"");
        assert!(!p.system_back(1.), "{root:?}");
        open(&mut p);
        assert!(p.system_back(2.), "{root:?}");
        assert!(!on_detail(&p), "{root:?}: back on the first screen");
        assert_eq!(slot(&p, read), expected, "{root:?}: {read}");
        assert!(!p.system_back(3.), "{root:?}");
    }
}

#[test]
fn a_control_that_does_not_resolve_refuses_back_and_keeps_the_app() {
    // Disabled: the author's say. Back is taken and nothing moves; it does
    // not fall through to `navigate`, and the app is not left.
    let disabled = BUTTON.replace("press=back", "press=back disabled=true");
    let mut p = boot("navigationBack=\"back\" navigate=follow", &disabled);
    open(&mut p);
    assert!(p.system_back(2.));
    assert!(on_detail(&p), "still on the pushed screen");
    assert_eq!(slot(&p, "followed"), 0.0);
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    assert!(journal.contains("system Back refused"), "{journal}");
}

#[test]
fn a_node_that_only_has_the_id_back_is_no_back_control() {
    // No navigation root names it: what the host pressed before LLP 1115.001 D6.
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(
            "component App\n  state pressed = 0\n  action back\n    pressed = pressed + 1\n  view\n    column width=300 height=300\n      button id=\"back\" press=back testId=\"back\"\n        text \"Back\"\n",
        )
        .unwrap()
        .encode(),
        (),
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    assert!(!p.back_goes() && !p.system_back(1.));
    assert_eq!(slot(&p, "pressed"), 0.0);
    // Nor is one outside the selected route, under a root that names it.
    let mut q = boot("navigationBack=\"back\"", "text \"no control here\"");
    open(&mut q);
    assert!(q.system_back(2.), "the visit beneath is still gone back to");
    assert!(!on_detail(&q));
}

/// A key at the device, down and up, inside the modifiers `held` (each its
/// code and its name), as the display loop and VNC deliver them.
fn device_key(p: &mut Presenter<()>, held: &[(&str, &str)], key: &str) {
    for (code, name) in held {
        p.hardware_key(code, name, true, false);
    }
    p.hardware_key(key, key, true, false);
    p.hardware_key(key, key, false, false);
    for (code, name) in held.iter().rev() {
        p.hardware_key(code, name, false, false);
    }
}
const ALT: &[(&str, &str)] = &[("AltLeft", "Alt")];

#[test]
fn a_desktops_back_keys_go_back_in_the_same_order() {
    // () focused, no control, no `navigate`: the runner's own back.
    let mut p = boot("", "text \"no control here\"");
    device_key(&mut p, ALT, "ArrowLeft"); // nowhere to go: the app stays as it is
    open(&mut p);
    device_key(&mut p, &[], "ArrowLeft");
    assert!(on_detail(&p), "a bare arrow is no Back");
    device_key(
        &mut p,
        &[("ShiftLeft", "Shift"), ("AltLeft", "Alt")],
        "ArrowLeft",
    );
    assert!(on_detail(&p), "nor is Alt+Left under another modifier");
    device_key(&mut p, ALT, "ArrowLeft");
    assert!(!on_detail(&p), "Alt+Left popped the route");
    // The keyboard's own Back key, here to the control the root names.
    let mut p = boot("navigationBack=\"back\" navigate=follow", BUTTON);
    open(&mut p);
    device_key(&mut p, &[], "BrowserBack");
    assert!(!on_detail(&p));
    assert_eq!(slot(&p, "pressed"), 1.0);
    assert_eq!(slot(&p, "followed"), 0.0);
    // A held key goes back once: its repeats pop nothing more.
    let again = "button press=open testId=\"again\"\n              text \"Again\"";
    let mut p = boot("navigate=follow", again);
    open(&mut p);
    let button = id(&p, "again").unwrap();
    p.dispatch_press(button, 1., false);
    p.hardware_key("AltLeft", "Alt", true, false);
    p.hardware_key("ArrowLeft", "ArrowLeft", true, false);
    p.hardware_key("ArrowLeft", "ArrowLeft", true, true);
    p.hardware_key("ArrowLeft", "ArrowLeft", false, false);
    p.hardware_key("AltLeft", "Alt", false, false);
    assert_eq!(slot(&p, "followed"), 1.0, "one visit back, of two");
}

/// LLP 1115.000 D5.5: a mouse's back and forward buttons step history at
/// their release, as Chrome's do, unless a `pointerup` prevented it; Forward
/// goes where Back left, by the root's `navigate` or the runner's own `go`,
/// and Alt+Right and the Forward key go there too.
#[test]
fn the_mouses_back_and_forward_buttons_step_history_at_their_release() {
    for root in ["navigate=follow", ""] {
        let mut p = boot(root, "text \"no control here\"");
        open(&mut p);
        p.pointer_aux(8, true, 300., 700., 1.);
        assert!(on_detail(&p), "{root:?}: not at the press");
        p.pointer_aux(8, false, 300., 700., 2.);
        assert!(!on_detail(&p), "{root:?}: back at the release");
        p.pointer_aux(16, true, 300., 700., 3.);
        p.pointer_aux(16, false, 300., 700., 4.);
        assert!(on_detail(&p), "{root:?}: forward to the detail");
        p.pointer_aux(16, true, 300., 700., 5.);
        p.pointer_aux(16, false, 300., 700., 6.);
        assert!(on_detail(&p), "{root:?}: nothing further ahead");
        device_key(&mut p, ALT, "ArrowLeft");
        assert!(!on_detail(&p), "{root:?}");
        device_key(&mut p, ALT, "ArrowRight");
        assert!(on_detail(&p), "{root:?}: Alt+Right went forward");
        device_key(&mut p, ALT, "ArrowLeft");
        device_key(&mut p, &[], "BrowserForward");
        assert!(on_detail(&p), "{root:?}: the Forward key");
        // Back went by `navigate` too: three each way.
        assert_eq!(
            slot(&p, "followed"),
            if root.is_empty() { 0. } else { 6. },
            "{root:?}"
        );
    }
    // A `pointerup` that prevents its default keeps the page, as in Chrome;
    // a `pointerdown` that does goes back all the same.
    let hold = "  action hold(e: PointerEvent)\n    kept = e.button * 10 + e.buttons\n    preventDefault()\n";
    for (handler, kept, back) in [("pointerup", 30.0, false), ("pointerdown", 38.0, true)] {
        let pad = format!("box testId=\"pad\" width=400 height=400 {handler}=hold");
        let source = (APP.replace("ROOT", "").replace("BACK", &pad))
            .replace("  action claim\n", &format!("{hold}  action claim\n"));
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            (),
            (400., 800.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        open(&mut p);
        p.pointer_aux(8, true, 20., 20., 1.);
        p.pointer_aux(8, false, 20., 20., 2.);
        assert_eq!(on_detail(&p), !back, "{handler}");
        assert_eq!(
            slot(&p, "kept"),
            kept,
            "{handler}: button 3, buttons 8 down, 0 up"
        );
    }
}

#[test]
fn back_is_its_keys_default_action_after_the_authors() {
    // In a text field Alt+Left goes back; the caret's keys are the bare arrows.
    let mut p = boot("navigate=follow", "input value=\"abc\" testId=\"field\"");
    open(&mut p);
    let field = id(&p, "field").unwrap();
    assert_eq!(p.set_focus(Some(field), 1.), None);
    device_key(&mut p, &[], "ArrowLeft");
    assert!(on_detail(&p));
    device_key(&mut p, ALT, "ArrowLeft");
    assert!(!on_detail(&p));
    assert_eq!(slot(&p, "followed"), 1.0);
    // A `key` handler at the focus that prevents the default keeps the key.
    let editor = "box key=keep tabindex=0 testId=\"editor\" width=100 height=40";
    let mut p = boot("navigate=follow", editor);
    open(&mut p);
    let editor = id(&p, "editor").unwrap();
    assert_eq!(p.set_focus(Some(editor), 1.), None);
    device_key(&mut p, ALT, "ArrowLeft");
    assert!(on_detail(&p), "the editor's own Alt+Left");
    assert_eq!(slot(&p, "kept"), 1.0);
    assert_eq!(slot(&p, "followed"), 0.0);
    // A button that declares the chord takes it before Back, focus or none.
    let claimed = "button aria-keyshortcuts=\"Alt+ArrowLeft\" press=claim testId=\"claim\"\n              text \"Older\"";
    let mut p = boot("navigate=follow", claimed);
    open(&mut p);
    device_key(&mut p, ALT, "ArrowLeft");
    assert!(on_detail(&p), "the page's shortcut, not Back");
    assert_eq!(slot(&p, "kept"), 10.0);
    // The agent's key is the same key: a drive asks for Back as a person does.
    let mut p = boot("", "text \"no control here\"");
    open(&mut p);
    let detail = id(&p, "detail").unwrap();
    let reply = crate::agent::handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{detail},"key":"Alt+ArrowLeft"}}"#),
    );
    assert!(reply.contains("\"typed\""), "{reply}");
    assert!(!on_detail(&p), "the drive's Alt+Left popped the route");
}

/// LLP 1116.006 D4: a test's `back` is system Back, the wire's `tap` with
/// `back`; with nowhere to go back to it is an error that says so, and a
/// control that refuses it is the reply's `refused`.
#[test]
fn the_agents_back_is_system_back() {
    let back = r#"{"op":"tap","back":true}"#;
    let mut p = boot("navigationBack=\"back\" navigate=follow", BUTTON);
    let reply = crate::agent::handle(&mut p, back);
    assert!(reply.contains("nothing to go back from"), "{reply}");
    open(&mut p);
    let reply = crate::agent::handle(&mut p, back);
    assert!(
        reply.contains(r#""back":"route""#) && !reply.contains("refused"),
        "{reply}"
    );
    assert!(!on_detail(&p), "the drive's back popped the route");
    assert_eq!(slot(&p, "pressed"), 1.0, "by the author's control");
    let disabled = BUTTON.replace("press=back", "press=back disabled=true");
    let mut p = boot("navigationBack=\"back\" navigate=follow", &disabled);
    open(&mut p);
    let reply = crate::agent::handle(&mut p, back);
    assert!(reply.contains(r#""refused""#), "{reply}");
    assert!(on_detail(&p), "a disabled control keeps the route");
    let reply = crate::agent::handle(&mut p, r#"{"op":"tap","back":true,"id":3}"#);
    assert!(reply.contains("takes no other input fields"), "{reply}");
}

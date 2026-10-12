//! System Back, in order (LLP 1115.001 D6): the authored control, the root's
//! `navigate`, the runner's own `back`, and who takes nothing; and a
//! desktop's Back keys, whose default action it is (LLP 1115.000 D5.4).
use super::*;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct Nothing;
impl DataSource for Nothing {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

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
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0
          when e.name == "home"
            button press=open testId="open"
              text "Open"
          when e.name == "detail"
            text "Detail" testId="detail"
            BACK
"#;

fn boot(root: &str, back: &str) -> Presenter<Nothing> {
    let source = APP.replace("ROOT", root).replace("BACK", back);
    let (p, error) = Presenter::boot_with(
        &contract::compile(&source).unwrap().encode(),
        Nothing,
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn id(p: &Presenter<Nothing>, name: &str) -> Option<ViewId> {
    let kernel = p.host().kernel();
    let key = kernel.find_by_test_id(name).first().copied()?;
    Some(kernel.node_by_key(key)?.id)
}

/// Whether the pushed route is the one that shows.
fn on_detail(p: &Presenter<Nothing>) -> bool {
    id(p, "detail").is_some_and(|id| !p.host().route_visibility(id).0)
}

fn slot(p: &mut Presenter<Nothing>, name: &str) -> serde_json::Value {
    let state: serde_json::Value =
        serde_json::from_str(&crate::agent::handle(p, r#"{"op":"state"}"#)).unwrap();
    state["slots"][name].clone()
}

fn open(p: &mut Presenter<Nothing>) {
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
    assert_eq!(slot(&mut p, "pressed"), 1);
    assert_eq!(slot(&mut p, "followed"), 0);
    assert!(!p.system_back(3.), "and from there Back leaves the app");
}

#[test]
fn a_route_with_no_control_goes_back_through_the_roots_navigate() {
    let mut p = boot("navigate=follow", "text \"no control here\"");
    assert!(!p.system_back(1.));
    open(&mut p);
    assert!(p.system_back(2.));
    assert!(!on_detail(&p));
    assert_eq!(
        slot(&mut p, "followed"),
        1,
        "the location beneath, to navigate"
    );
    assert!(!p.system_back(3.));
}

#[test]
fn a_root_with_no_handler_goes_back_by_the_runners_own_back() {
    let mut p = boot("", "text \"no control here\"");
    assert!(!p.system_back(1.));
    open(&mut p);
    assert!(p.system_back(2.));
    assert!(!on_detail(&p), "the router popped");
    assert_eq!(slot(&mut p, "pressed"), 0);
    assert!(!p.system_back(3.));
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
    assert_eq!(slot(&mut p, "followed"), 0);
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
        Nothing,
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    assert!(!p.back_goes() && !p.system_back(1.));
    assert_eq!(slot(&mut p, "pressed"), 0);
    // Nor is one outside the selected route, under a root that names it.
    let mut q = boot("navigationBack=\"back\"", "text \"no control here\"");
    open(&mut q);
    assert!(q.system_back(2.), "the visit beneath is still gone back to");
    assert!(!on_detail(&q));
}

/// A key at the device, down and up, inside the modifiers `held` (each its
/// code and its name), as the display loop and VNC deliver them.
fn device_key(p: &mut Presenter<Nothing>, held: &[(&str, &str)], key: &str) {
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
    // Nothing focused, no control, no `navigate`: the runner's own back.
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
    assert_eq!(slot(&mut p, "pressed"), 1);
    assert_eq!(slot(&mut p, "followed"), 0);
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
    assert_eq!(slot(&mut p, "followed"), 1, "one visit back, of two");
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
    assert_eq!(slot(&mut p, "followed"), 1);
    // A `key` handler at the focus that prevents the default keeps the key.
    let editor = "box key=keep tabindex=0 testId=\"editor\" width=100 height=40";
    let mut p = boot("navigate=follow", editor);
    open(&mut p);
    let editor = id(&p, "editor").unwrap();
    assert_eq!(p.set_focus(Some(editor), 1.), None);
    device_key(&mut p, ALT, "ArrowLeft");
    assert!(on_detail(&p), "the editor's own Alt+Left");
    assert_eq!(slot(&mut p, "kept"), 1);
    assert_eq!(slot(&mut p, "followed"), 0);
    // A button that declares the chord takes it before Back, focus or none.
    let claimed = "button aria-keyshortcuts=\"Alt+ArrowLeft\" press=claim testId=\"claim\"\n              text \"Older\"";
    let mut p = boot("navigate=follow", claimed);
    open(&mut p);
    device_key(&mut p, ALT, "ArrowLeft");
    assert!(on_detail(&p), "the page's shortcut, not Back");
    assert_eq!(slot(&mut p, "kept"), 10);
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
    assert_eq!(slot(&mut p, "pressed"), 1, "by the author's control");
    let disabled = BUTTON.replace("press=back", "press=back disabled=true");
    let mut p = boot("navigationBack=\"back\" navigate=follow", &disabled);
    open(&mut p);
    let reply = crate::agent::handle(&mut p, back);
    assert!(reply.contains(r#""refused""#), "{reply}");
    assert!(on_detail(&p), "a disabled control keeps the route");
    let reply = crate::agent::handle(&mut p, r#"{"op":"tap","back":true,"id":3}"#);
    assert!(reply.contains("takes no other input fields"), "{reply}");
}

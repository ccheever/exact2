//! System Back, in order (LLP 1115.001 D6): the authored control, the root's
//! `navigate`, the runner's own `back`, and who takes nothing.
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
  action open
    nav = push(nav, "/detail")
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

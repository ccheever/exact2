//! The focus follows the route on the painted hosts (LLP 1115.000 D4.2,
//! ruling 5): the cases the web's rule is held by in Chrome
//! (`host/web/tests/route-focus.test.mjs`), over this presenter. A pointer's
//! push and a key's, Back to the pressed control, a route with no heading,
//! a focus the commit set, a route with no press, a press too old to explain
//! one, a replace, and another tab.
use super::*;

/// Two tabs and a field beside the routes. Home pushes Detail (a header
/// with the Back control and a heading), Bare (no header) and Form (a
/// heading and an `autofocus` field); Detail is replaced by Bare.
const APP: &str = r#"routes nav
  tab home "/"
    detail "/detail"
    bare "/bare"
    form "/form"
  tab other "/other"
component App
  state note = ""
  action pick(name: string)
    nav = select(nav, name)
  action open(url: string)
    nav = push(nav, url)
  action swap(url: string)
    nav = replace(nav, url)
  action back
    nav = back(nav)
  action follow(url: string)
    nav = go(nav, url)
  action typed(value: string)
    note = value
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow testId="nav" width="100%" height="100%" display="flex" flex-direction="column"
      input testId="aside" value=note input=typed height=30
      column flex=1 min-height=0 position="relative"
        for t in nav.tabs key = t.name
          column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
            for e in t.stack key = e.id
              if e.name == "home"
                column navigationKey=`${e.id}` testId="route-home" position="absolute" inset=0
                  button press=open("/detail") testId="open" height=40
                    text "Open"
                  button press=open("/bare") testId="open-bare" height=40
                    text "Bare"
                  button press=open("/form") testId="open-form" height=40
                    text "Form"
              if e.name == "detail"
                column navigationKey=`${e.id}` testId="route-detail" position="absolute" inset=0
                  header display="flex"
                    button id="back" press=back testId="back" height=40
                      text "Back"
                    text "Detail" role="heading" testId="detail-title"
                  button press=swap("/bare") testId="swap" height=40
                    text "Swap"
              if e.name == "bare"
                column navigationKey=`${e.id}` testId="route-bare" position="absolute" inset=0
                  button id="back" press=back testId="bare-back" height=40
                    text "Back"
              if e.name == "form"
                column navigationKey=`${e.id}` testId="route-form" position="absolute" inset=0
                  header
                    text "Form" role="heading" testId="form-title"
                  input autofocus=true testId="field" height=30
              if e.name == "other"
                column navigationKey=`${e.id}` testId="route-other" position="absolute" inset=0
                  header
                    text "Other" role="heading" testId="other-title"
      row role="tablist" testId="tabs"
        button role="tab" aria-controls="panel-home" aria-selected=(nav.tab == "home") press=pick("home") testId="tab-home" flex=1 height=40
          text "Home"
        button role="tab" aria-controls="panel-other" aria-selected=(nav.tab == "other") press=pick("other") testId="tab-other" flex=1 height=40
          text "Other"
"#;

fn boot() -> Presenter<()> {
    boot_source(APP)
}

fn boot_source(source: &str) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    settle(&mut p);
    p
}

fn settle(p: &mut Presenter<()>) {
    for _ in 0..4 {
        assert!(p.pump(p.host.now()).is_none());
        if p.dirty() {
            let _ = p.frame();
        }
    }
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    let key = k.find_by_test_id(name).first().copied();
    let node = key.and_then(|key| k.node_by_key(key));
    node.unwrap_or_else(|| panic!("no node {name}")).id
}

/// The node that has the focus, by its `testId`.
fn focused(p: &Presenter<()>) -> String {
    let Some(focus) = p.focus() else {
        return "nothing".into();
    };
    let node = p.host().kernel().node(focus).expect("a live node");
    node.props.str(PropId::TestId).unwrap_or("?").to_string()
}

/// A mouse's click at the middle of `name`.
fn click(p: &mut Presenter<()>, name: &str) {
    let (x, y, w, h) = p.rect_of(id(p, name)).unwrap();
    p.press_at(x + w / 2.0, y + h / 2.0, p.host.now());
    settle(p);
}

/// Enter, with the focus on `name`.
fn enter(p: &mut Presenter<()>, name: &str) {
    let target = id(p, name);
    assert!(p.set_focus(Some(target), p.host.now()).is_none());
    p.key_down("Enter", p.host.now());
    settle(p);
}

/// A location that arrives with no press: the root's `navigate`, as a
/// timer's push or a deep link.
fn arrive_at(p: &mut Presenter<()>, location: &str) {
    let (root, now) = (id(p, "nav"), p.host.now());
    let error = p
        .host
        .dispatch_at(root, Event::Navigate(location.into()), now);
    assert!(error.or(p.after_commit()).is_none());
    settle(p);
}

#[test]
fn a_push_moves_the_focus_to_the_routes_heading_and_back_returns_it_to_the_pressed_control() {
    let mut p = boot();
    assert_eq!(focused(&p), "nothing", "the first sight is no arrival");
    // A pointer's press: the heading takes the focus and shows no ring,
    // and Back gives it back to the button that was pressed.
    click(&mut p, "open");
    assert_eq!(focused(&p), "detail-title");
    assert_eq!(p.brush.ringless, Some(id(&p, "detail-title")));
    click(&mut p, "back");
    assert_eq!(focused(&p), "open");
    assert_eq!(p.brush.ringless, Some(id(&p, "open")));
    // A key's press: the same, and the focus it moves shows its ring.
    enter(&mut p, "open");
    assert_eq!(focused(&p), "detail-title");
    enter(&mut p, "back");
    assert_eq!(focused(&p), "open");
    assert_ne!(p.brush.ringless, Some(id(&p, "open")));
    // System Back is no press: with nothing focused where the route was,
    // the focus still returns to the control that pushed it.
    click(&mut p, "open");
    assert!(p.system_back(p.host.now()));
    settle(&mut p);
    assert_eq!(focused(&p), "open");
}

#[test]
fn a_route_with_no_heading_takes_the_focus_itself() {
    let mut p = boot();
    click(&mut p, "open-bare");
    assert_eq!(focused(&p), "route-bare");
    click(&mut p, "bare-back");
    assert_eq!(focused(&p), "open-bare");
}

#[test]
fn a_focus_the_commit_set_is_left_alone() {
    let mut p = boot();
    click(&mut p, "open-form");
    assert_eq!(focused(&p), "field", "the route's `autofocus` field has it");
}

#[test]
fn a_route_that_arrives_with_no_press_takes_the_focus_only_when_nothing_has_it() {
    // () has the focus: the route's heading takes it. Back from a
    // route no press pushed has no control to return to: the route beneath.
    let mut p = boot();
    arrive_at(&mut p, "/detail");
    assert_eq!(focused(&p), "detail-title");
    assert!(p.system_back(p.host.now()));
    settle(&mut p);
    assert_eq!(focused(&p), "route-home");
    // A person is typing beside the routes: the focus stays in the field.
    let mut p = boot();
    click(&mut p, "aside");
    assert_eq!(focused(&p), "aside");
    arrive_at(&mut p, "/detail");
    assert_eq!(focused(&p), "aside");
}

#[test]
fn a_press_explains_an_arrival_for_two_seconds() {
    // The pressed control still has the focus as a route arrives later (an
    // answer its press asked for): the focus follows the route.
    let mut p = boot();
    let open = id(&p, "open");
    assert!(p.set_focus(Some(open), 0.).is_none());
    p.note_press(open, false);
    arrive_at(&mut p, "/detail");
    assert_eq!(focused(&p), "detail-title");
    // Past two seconds the press explains nothing, and the focus is where
    // a person left it.
    let mut p = boot();
    let open = id(&p, "open");
    assert!(p.set_focus(Some(open), 0.).is_none());
    p.note_press(open, false);
    let pressed = p.route_focus.pressed.as_mut().unwrap();
    pressed.at = pressed.at.checked_sub(RECENT).unwrap();
    arrive_at(&mut p, "/bare");
    assert_eq!(focused(&p), "nothing", "its route went inert under it");
    assert!(p.route_focus.pressed.is_none(), "and the press is spent");
}

#[test]
fn a_replace_keeps_where_its_route_came_from() {
    let mut p = boot();
    click(&mut p, "open");
    click(&mut p, "swap");
    assert_eq!(focused(&p), "route-bare");
    click(&mut p, "bare-back");
    assert_eq!(focused(&p), "open");
}

#[test]
fn another_tabs_stack_is_no_arrival() {
    let mut p = boot();
    click(&mut p, "tab-other");
    assert_eq!(focused(&p), "tab-other", "the focus stays on its tab");
    click(&mut p, "tab-home");
    assert_eq!(focused(&p), "tab-home");
}

#[test]
fn a_long_mouse_press_notes_its_activation_before_the_route_arrives() {
    for scale in ["", "-exact-press-scale=0.9"] {
        let source = APP.replace("testId=\"open\"", &format!("testId=\"open\" {scale}"));
        let mut p = boot_source(&source);
        let open = id(&p, "open");
        assert!(p.set_focus(Some(open), 0.).is_none());
        p.note_press(open, false);
        let (x, y, w, h) = p.rect_of(open).unwrap();
        let (x, y) = (x + w / 2., y + h / 2.);
        p.pointer_down(x, y, 0.).unwrap();
        // Age the wall-clock record without sleeping or changing the app clock.
        let pressed = p.route_focus.pressed.as_mut().unwrap();
        pressed.at = pressed.at.checked_sub(RECENT).unwrap();
        p.pointer_up(x, y, 0.).unwrap();
        settle(&mut p);
        assert_eq!(focused(&p), "detail-title", "{scale}");
        assert_eq!(p.brush.ringless, Some(id(&p, "detail-title")));
        click(&mut p, "back");
        assert_eq!(
            focused(&p),
            "open",
            "{scale}: the route remembers its origin"
        );
    }
}

#[test]
fn mouse_field_click_keeps_its_focus_when_a_route_arrives() {
    for feedback in ["", "-exact-press-scale=0.9"] {
        for during_press in [false, true] {
            for prior_focus in [None, Some("open"), Some("aside")] {
                let action = if during_press {
                    "open(\"/detail\")"
                } else {
                    "typed(note)"
                };
                let source = APP.replace(
                    "input testId=\"aside\"",
                    &format!("input testId=\"aside\" press={action} {feedback}"),
                );
                let mut p = boot_source(&source);
                let prior = prior_focus.map(|name| id(&p, name));
                p.set_focus(prior, 0.);
                let field = id(&p, "aside");
                let (x, y, w, h) = p.rect_of(field).unwrap();
                let (x, y) = (x + w / 2., y + h / 2.);
                p.pointer_down(x, y, 0.).unwrap();
                assert_eq!(focused(&p), "aside");
                let released = Instant::now();
                p.pointer_up(x, y, 0.).unwrap();
                let recorded = (!during_press).then(|| p.route_focus.pressed.unwrap());
                if !during_press {
                    arrive_at(&mut p, "/detail");
                }
                assert_eq!(
                    focused(&p),
                    if prior_focus == Some("aside") {
                        "detail-title"
                    } else {
                        "aside"
                    },
                    "{feedback}: press={during_press}, prior={prior_focus:?}"
                );
                if let Some(press) = recorded {
                    assert_eq!(press.focus, prior, "activation remembers focus before down");
                    assert!(press.at >= released, "activation refreshes the timestamp");
                }
            }
        }
    }
}

#[test]
fn mouse_field_link_keeps_its_focus_when_the_click_follows() {
    let source = APP.replace(
        "      input testId=\"aside\" value=note input=typed height=30",
        "      link href=\"/detail\"\n        input testId=\"aside\" value=note input=typed height=30",
    );
    let mut p = boot_source(&source);
    let field = id(&p, "aside");
    let (x, y, w, h) = p.rect_of(field).unwrap();
    let (x, y) = (x + w / 2., y + h / 2.);
    p.pointer_down(x, y, 0.).unwrap();
    p.pointer_up(x, y, 0.).unwrap();
    assert_eq!(
        p.host.shown_stack().unwrap().last(),
        Some(&id(&p, "route-detail"))
    );
    assert_eq!(focused(&p), "aside");
}

#[test]
fn unactivated_mouse_field_contact_does_not_explain_a_route() {
    for feedback in ["", "-exact-press-scale=0.9"] {
        for already_focused in [true, false] {
            for end in ["cancel", "lost", "pan", "outside"] {
                let pan = if end == "pan" {
                    "pan=drag touch-action=\"none\""
                } else {
                    ""
                };
                let source = APP.replace(
                    "      input testId=\"aside\" value=note input=typed height=30",
                    &format!("      button testId=\"field-parent\" press=typed(note) {feedback}\n        input testId=\"aside\" value=note input=typed height=30 {pan}"),
                ).replace("  view\n", "  action drag(dx: number, dy: number)\n    note = note\n  view\n");
                let mut p = boot_source(&source);
                let field = id(&p, "aside");
                if already_focused {
                    p.set_focus(Some(field), 0.);
                }
                let (x, y, w, h) = p.rect_of(field).unwrap();
                let (x, y) = (x + w / 2., y + h / 2.);
                p.pointer_down(x, y, 0.).unwrap();
                assert_eq!(focused(&p), "aside");
                match end {
                    "cancel" => p.pointer_cancel(0.).unwrap(),
                    "lost" => p.pointer_lost(0.).unwrap(),
                    "pan" => {
                        p.pointer_move(x + 40., y, 0.).unwrap();
                        p.pointer_up(x + 40., y, 0.).unwrap();
                    }
                    _ => {
                        p.pointer_up(450., y, 0.).unwrap();
                    }
                }
                let recorded = p.route_focus.pressed;
                arrive_at(&mut p, "/detail");
                assert_eq!(
                    focused(&p),
                    "aside",
                    "{feedback}: focused={already_focused}, {end}"
                );
                assert!(recorded.is_none(), "no activation record");
                p.set_focus(None, 0.);
                assert!(p.system_back(0.));
                settle(&mut p);
                assert_eq!(
                    focused(&p),
                    "route-home",
                    "an unactivated ancestor is no Back origin"
                );
            }
        }
    }
}

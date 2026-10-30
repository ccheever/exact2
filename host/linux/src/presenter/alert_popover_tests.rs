//! A non-modal popover follows HTML: opening it leaves the focus where it
//! was unless something inside it autofocuses, and the page stays live: a
//! press outside an auto popover closes it and reaches what it lands on, a
//! press on its invoker closes it, and a manual popover stays open. Modal
//! dialogs and alerts keep the inert page and move the focus in. (`role="dialog"`
//! keeps the popover off the menu-shaped surface, as `alert_input_tests` do.)
use super::*;
use exact_runner::Value;

const APP: &str = r#"component App
  state probes = 0
  state saves = 0
  action probe
    probes = probes + 1
  action save
    saves = saves + 1
  view
    column width="100%" height="100%"
      button "Probe" testId="probe" press=probe width=120 height=30
      button "Open" testId="open" popovertarget="menu" width=120 height=30
      column id="menu" testId="menu" popover="auto" role="dialog" width=200
        button "Save" testId="save" press=save height=30
        button "Other" testId="other" height=30
"#;

fn boot(source: &str) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (480., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_overlays();
    p.frame();
    p
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn tap(p: &mut Presenter<()>, name: &str) {
    p.tap(id(p, name)).unwrap();
    p.run_commands(|| ());
    p.frame();
}

/// A person's click at the middle of `name`'s box.
fn click(p: &mut Presenter<()>, name: &str, at: f64) {
    let (x, y, w, h) = p.rect_of(id(p, name)).expect("drawn");
    let (x, y) = (x + w / 2., y + h / 2.);
    p.pointer_down(x, y, at).unwrap();
    p.pointer_up(x, y, at + 50.).unwrap();
    p.run_commands(|| ());
    p.frame();
}

fn count(p: &Presenter<()>, slot: &str) -> Option<Value> {
    p.host.runner().slot(slot).cloned()
}

#[test]
fn an_auto_popover_leaves_the_focus_on_its_invoker_and_a_tap_outside_reaches_the_page() {
    let mut p = boot(APP);
    tap(&mut p, "open");
    assert!(p.alert.is_some(), "the popover is open");
    assert_eq!(
        p.focus,
        Some(id(&p, "open")),
        "the focus stays on the invoker"
    );
    tap(&mut p, "probe");
    assert!(p.alert.is_none(), "the tap outside light-dismisses it");
    assert_eq!(
        count(&p, "probes"),
        Some(Value::Number(1.)),
        "and presses the page"
    );
    assert_eq!(p.focus, Some(id(&p, "probe")));
}

#[test]
fn a_click_outside_an_auto_popover_closes_it_and_presses_what_it_lands_on() {
    let mut p = boot(APP);
    click(&mut p, "open", 0.);
    assert!(p.alert.is_some());
    click(&mut p, "probe", 200.);
    assert!(p.alert.is_none());
    assert_eq!(count(&p, "probes"), Some(Value::Number(1.)));
}

#[test]
fn a_click_on_the_invoker_closes_its_open_popover_once() {
    let mut p = boot(APP);
    click(&mut p, "open", 0.);
    assert!(p.alert.is_some());
    click(&mut p, "open", 200.);
    assert!(
        p.alert.is_none(),
        "the invoker's toggle closes it, not reopens it"
    );
    click(&mut p, "open", 400.);
    assert!(p.alert.is_some());
}

#[test]
fn a_popover_focuses_what_autofocuses_inside_and_escape_returns_the_focus() {
    let mut p = boot(&APP.replace("testId=\"other\"", "testId=\"other\" autofocus=true"));
    tap(&mut p, "open");
    assert_eq!(p.focus, Some(id(&p, "other")));
    p.key_down("Escape", 300.);
    p.frame();
    assert!(p.alert.is_none());
    assert_eq!(p.focus, Some(id(&p, "open")));
}

#[test]
fn a_manual_popover_stays_open_under_a_live_page() {
    let mut p = boot(&APP.replace("popover=\"auto\"", "popover=\"manual\""));
    tap(&mut p, "open");
    assert_eq!(p.focus, Some(id(&p, "open")));
    tap(&mut p, "probe");
    assert!(p.alert.is_some(), "a manual popover has no light dismiss");
    assert_eq!(count(&p, "probes"), Some(Value::Number(1.)));
    assert_eq!(p.focus, Some(id(&p, "probe")));
    tap(&mut p, "save");
    assert_eq!(count(&p, "saves"), Some(Value::Number(1.)));
}

/// One test per kind: only the popover leaves the page live.
#[test]
fn each_kind_keeps_its_own_focus_and_inertness() {
    let dialog = APP
        .replace(
            "popovertarget=\"menu\"",
            "commandfor=\"menu\" command=\"show-modal\"",
        )
        .replace(
            "column id=\"menu\" testId=\"menu\" popover=\"auto\" role=\"dialog\"",
            "dialog id=\"menu\" testId=\"menu\"",
        );
    // An alert's rows are actions that close it and one cancel.
    let alert = dialog
        .replace(
            "dialog id=\"menu\"",
            "dialog role=\"alertdialog\" aria-label=\"Prompt\" id=\"menu\"",
        )
        .replace(
            "press=save",
            "press=save commandfor=\"menu\" command=\"close\"",
        )
        .replace(
            "testId=\"other\"",
            "testId=\"other\" commandfor=\"menu\" command=\"close\"",
        );
    for (kind, source, inert) in [
        ("popover", APP.to_owned(), false),
        ("dialog", dialog, true),
        ("alert", alert, true),
    ] {
        let mut p = boot(&source);
        tap(&mut p, "open");
        assert!(p.alert.is_some(), "{kind}");
        let root = p.alert_root();
        let k = p.host.kernel();
        let inside = std::iter::successors(p.focus, |f| k.node(*f).and_then(|n| n.parent))
            .any(|f| Some(f) == root);
        assert_eq!(
            inside, inert,
            "{kind}: the focus moves in only for a modal box"
        );
        assert_eq!(p.alert_allows(id(&p, "probe")), !inert, "{kind}");
        let _ = p.tap(id(&p, "probe"));
        p.run_commands(|| ());
        p.frame();
        let pressed = if inert { 0. } else { 1. };
        assert_eq!(count(&p, "probes"), Some(Value::Number(pressed)), "{kind}");
    }
}

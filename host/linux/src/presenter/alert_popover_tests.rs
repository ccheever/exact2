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

/// The page under a popover stays in the accessibility tree, with the
/// popover's own nodes; under a modal dialog the box is the whole tree.
#[test]
fn the_accessibility_tree_keeps_the_page_under_a_popover() {
    let mut p = boot(APP);
    tap(&mut p, "open");
    let views: Vec<ViewId> = p.access_tree().0.iter().map(|n| n.view).collect();
    for name in ["probe", "open", "save"] {
        assert!(views.contains(&id(&p, name)), "{name} in {views:?}");
    }
}

/// Structural reads see an open popover's subtree, as the web's do: the
/// presenter reports the box to the runner's top layer, and a closed one
/// is hidden again.
#[test]
fn reads_see_an_open_popovers_subtree() {
    let source = APP
        .replace("  state saves = 0", "  state saves = 0\n  state marked = []\n  action mark\n    marked = querySelectorAll(\"[data-mood~=\\\"accent\\\"]\")")
        .replace("testId=\"save\"", "testId=\"save\" id=\"save\" data-mood=\"accent\"");
    let mut p = boot(&source);
    let marked = |p: &mut Presenter<()>| {
        p.host.runner_mut().act("mark", vec![]).unwrap();
        p.host.runner().slot("marked").cloned()
    };
    assert_eq!(marked(&mut p), Some(Value::list(vec![])));
    tap(&mut p, "open");
    assert_eq!(marked(&mut p), Some(Value::list(vec![Value::str("save")])));
    p.key_down("Escape", 300.);
    p.frame();
    assert_eq!(marked(&mut p), Some(Value::list(vec![])));
}

/// What the runner's `frame()` reads for `name`.
fn read_frame(p: &mut Presenter<()>, name: &str) -> Option<Value> {
    p.host.runner_mut().act("measure", vec![]).unwrap();
    let _ = name;
    p.host.runner().slot("box").cloned()
}

const MEASURED: &str = "  state saves = 0\n  state box = []\n  action measure\n    box = [frame(\"menu\").x, frame(\"menu\").y, frame(\"menu\").width, frame(\"menu\").height]";

fn painted(p: &mut Presenter<()>, name: &str) -> Value {
    let (x, y, w, h) = p.rect_of(id(p, name)).expect("drawn");
    Value::list([x, y, w, h].map(|v| Value::Number(f64::from(v))).to_vec())
}

/// The popover's box the reads see follows its placement: after its
/// invoker moves, and once (not twice) through its own transform.
#[test]
fn reads_see_an_open_popovers_box_where_it_is_drawn() {
    let source = APP
        .replace("  state saves = 0", MEASURED)
        .replace(
            "testId=\"open\" popovertarget",
            "testId=\"open\" position=\"absolute\" right=20 top=100 popovertarget",
        )
        .replace(
            "role=\"dialog\" width=200",
            "role=\"dialog\" width=200 transform=\"translateX(10px)\"",
        );
    let mut p = boot(&source);
    tap(&mut p, "open");
    let drawn = painted(&mut p, "menu");
    assert_eq!(read_frame(&mut p, "menu"), Some(drawn));
    assert!(p.resize(600., 400.).is_none());
    p.frame();
    let drawn = painted(&mut p, "menu");
    assert_eq!(
        read_frame(&mut p, "menu"),
        Some(drawn),
        "after the invoker moved"
    );
}

/// Any button that targets the open popover toggles it, not only the one
/// that opened it: its press is no light dismiss (HTML).
#[test]
fn a_second_invoker_closes_the_open_popover_once() {
    let source = APP.replace(
        "      button \"Probe\"",
        "      button \"Again\" testId=\"again\" popovertarget=\"menu\" width=120 height=30\n      button \"Probe\"",
    );
    let mut p = boot(&source);
    click(&mut p, "open", 0.);
    assert!(p.alert.is_some());
    click(&mut p, "again", 200.);
    assert!(p.alert.is_none(), "the second invoker's toggle closes it");
}

/// The agent's tap on a page control the popover covers lands on the
/// popover, as a person's would: nothing under it is pressed.
#[test]
fn the_agents_tap_on_a_covered_page_control_presses_nothing() {
    let source = APP.replace(
        "      column id=\"menu\"",
        "      button \"Under\" testId=\"under\" press=probe position=\"absolute\" left=0 top=64 width=200 height=40\n      column id=\"menu\"",
    );
    let mut p = boot(&source);
    tap(&mut p, "open");
    let menu = p.rect_of(id(&p, "menu")).unwrap();
    let under = p.rect_of(id(&p, "under")).unwrap();
    assert!(
        under.1 >= menu.1 && under.1 + under.3 <= menu.1 + menu.3,
        "{menu:?} covers {under:?}"
    );
    let _ = p.tap(id(&p, "under"));
    p.run_commands(|| ());
    p.frame();
    assert_eq!(count(&p, "probes"), Some(Value::Number(0.)));
    assert!(p.alert.is_some(), "the press landed on the popover");
}

/// Tab goes from the invoker into its popover and from the popover's last
/// stop to what follows the invoker (HTML's popover focus scope).
#[test]
fn tab_enters_the_popover_after_its_invoker() {
    let source = APP.replace(
        "      column id=\"menu\"",
        "      button \"After\" testId=\"after\" width=120 height=30\n      column id=\"menu\"",
    );
    let mut p = boot(&source);
    tap(&mut p, "open");
    for next in ["save", "other", "after"] {
        p.key_down("Tab", 0.);
        assert_eq!(p.focus, Some(id(&p, next)), "{next}");
    }
}

/// A `dialog` shown as a popover runs the dialog focusing steps: its first
/// stop takes the focus (HTML).
#[test]
fn a_dialog_popover_focuses_its_first_stop() {
    let source = APP.replace(
        "column id=\"menu\" testId=\"menu\" popover=\"auto\" role=\"dialog\"",
        "dialog id=\"menu\" testId=\"menu\" popover=\"auto\"",
    );
    let mut p = boot(&source);
    tap(&mut p, "open");
    assert!(p.alert.is_some());
    assert_eq!(p.focus, Some(id(&p, "save")));
}

/// A manual popover keeps no focus to give back: hidden with the focus
/// inside, the focus is not returned to its invoker (HTML).
#[test]
fn a_manual_popover_gives_no_focus_back() {
    let source = APP
        .replace("popover=\"auto\"", "popover=\"manual\"")
        .replace(
            "testId=\"other\"",
            "testId=\"other\" popovertarget=\"menu\" popovertargetaction=\"hide\" autofocus=true",
        );
    let mut p = boot(&source);
    tap(&mut p, "open");
    assert_eq!(p.focus, Some(id(&p, "other")));
    tap(&mut p, "other");
    assert!(p.alert.is_none());
    assert_ne!(p.focus, Some(id(&p, "open")));
}

/// Back under a manual popover is the page's: nothing closes it, so the
/// app does not take Back with no route to go back to.
#[test]
fn back_under_a_manual_popover_is_the_pages() {
    let mut p = boot(&APP.replace("popover=\"auto\"", "popover=\"manual\""));
    tap(&mut p, "open");
    assert!(!p.system_back(0.), "nothing to go back to");
    assert!(p.alert.is_some());
}

/// A page key the popover doesn't handle reaches the page's shortcuts.
#[test]
fn a_page_shortcut_works_under_a_popover() {
    let source = APP.replace(
        "testId=\"probe\" press=probe",
        "testId=\"probe\" press=probe aria-keyshortcuts=\"Alt+P\"",
    );
    let mut p = boot(&source);
    tap(&mut p, "open");
    p.hardware_key("AltLeft", "Alt", true, false);
    p.hardware_key("KeyP", "p", true, false);
    p.hardware_key("KeyP", "p", false, false);
    p.hardware_key("AltLeft", "Alt", false, false);
    p.run_commands(|| ());
    assert_eq!(count(&p, "probes"), Some(Value::Number(1.)));
}

/// A long press on the live page under a manual popover opens its context
/// popover there.
#[test]
fn a_long_press_works_under_a_manual_popover() {
    let source = APP
        .replace("popover=\"auto\"", "popover=\"manual\"")
        .replace(
            "testId=\"probe\" press=probe",
            "testId=\"probe\" press=probe contextPopover=\"menu\"",
        );
    let mut p = boot(&source);
    tap(&mut p, "open");
    let (x, y, ..) = p.rect_of(id(&p, "probe")).unwrap();
    assert!(p.long_press(x + 5., y + 5., 500.));
}

/// A wheel over a popover that doesn't scroll goes on to the page.
#[test]
fn a_wheel_over_a_popover_scrolls_the_page() {
    let source = APP
        .replace(
            "column width=\"100%\" height=\"100%\"",
            "column width=\"100%\"",
        )
        .replace(
            "        button \"Other\" testId=\"other\" height=30\n",
            "        button \"Other\" testId=\"other\" height=30\n      box height=2000\n",
        );
    let mut p = boot(&source);
    p.wheel_at(400., 300., 0., 100.);
    p.frame();
    assert_eq!(p.page().1, 100., "the page scrolls without a popover");
    p.wheel_at(400., 300., 0., -100.);
    p.frame();
    tap(&mut p, "open");
    let (x, y, w, h) = p.rect_of(id(&p, "menu")).unwrap();
    p.wheel_at(x + w / 2., y + h / 2., 0., 100.);
    p.frame();
    assert!(p.page().1 > 0., "the page scrolled: {:?}", p.page());
}

/// A popover authored inside a scroller is drawn in the top layer: the
/// scroller's clip does not drop it from the accessibility tree.
#[test]
fn a_popover_inside_a_scroller_stays_in_the_accessibility_tree() {
    let source = APP.replace(
        "      column id=\"menu\" testId=\"menu\"",
        "      scroll testId=\"clip\" height=10 width=40\n        box height=100\n        column id=\"menu\" testId=\"menu\"",
    )
    .replace("        button \"Save\"", "          button \"Save\"")
    .replace("        button \"Other\"", "          button \"Other\"");
    let mut p = boot(&source);
    tap(&mut p, "open");
    let save = id(&p, "save");
    let node = p.access_tree().0.into_iter().find(|n| n.view == save);
    assert!(
        node.is_some_and(|n| n.rect[2] > 0. && n.rect[3] > 0.),
        "save is in the tree with its box"
    );
}

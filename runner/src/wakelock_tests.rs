//! The rule through a runner (LLP 1125 D11): held while displayed, released
//! under a `display: none` ancestor, on unmount and at `none`; a computed
//! word outside the two is journaled and holds nothing.

use crate::{DataError, DataSource, Event, Runner};
use exact_kernel::Kernel;
use exact_plan::Value;

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SOURCE: &str = r#"component App
  state shown = true
  state mounted = true
  state word = "screen"
  action hide
    shown = not shown
  action unmount
    mounted = not mounted
  action set(w: string)
    word = w
  view
    column
      button press=hide testId="hide"
        text "hide"
      button press=unmount testId="unmount"
        text "unmount"
      button press=set("none") testId="none"
        text "none"
      button press=set("dim") testId="dim"
        text "dim"
      button press=set("screen") testId="screen"
        text "screen"
      column testId="panel" display=(shown ? "flex" : "none")
        column
          if mounted
            column testId="player" wakelock=word
              text "playing"
"#;

fn boot() -> Runner<NoData> {
    let plan = contract::compile(SOURCE).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn press(r: &mut Runner<NoData>, id: &str) {
    let k = r.kernel();
    let view = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}

fn player(r: &Runner<NoData>) -> Option<u32> {
    let k = r.kernel();
    k.find_by_test_id("player")
        .first()
        .and_then(|key| k.node_by_key(*key))
        .map(|n| n.id)
}

fn state(r: &Runner<NoData>) -> String {
    let mut s = String::new();
    super::state(r.kernel(), &mut s);
    s
}

#[test]
fn held_while_displayed_and_released_when_hidden_or_unmounted() {
    let mut r = boot();
    let id = player(&r).unwrap();
    assert_eq!(super::holders(r.kernel()), vec![id]);
    assert_eq!(
        state(&r),
        format!(",\"wakelock\":{{\"held\":true,\"by\":[{id}]}}")
    );
    assert!(
        crate::agent::state(&r).contains(&format!("\"wakelock\":{{\"held\":true,\"by\":[{id}]}}"))
    );
    // An ancestor's `display: none` releases it; the node stays mounted.
    press(&mut r, "hide");
    assert_eq!(player(&r), Some(id));
    assert!(super::holders(r.kernel()).is_empty());
    assert_eq!(state(&r), ",\"wakelock\":{\"held\":false,\"by\":[]}");
    press(&mut r, "hide");
    assert_eq!(super::holders(r.kernel()), vec![id]);
    // Unmounting releases it.
    press(&mut r, "unmount");
    assert_eq!(player(&r), None);
    assert!(super::holders(r.kernel()).is_empty());
    press(&mut r, "unmount");
    assert_eq!(super::holders(r.kernel()).len(), 1);
}

#[test]
fn none_and_a_computed_word_outside_the_two_hold_nothing() {
    let mut r = boot();
    press(&mut r, "none");
    assert!(super::holders(r.kernel()).is_empty());
    press(&mut r, "screen");
    assert_eq!(super::holders(r.kernel()).len(), 1);
    press(&mut r, "dim");
    assert!(super::holders(r.kernel()).is_empty());
    let id = player(&r).unwrap();
    assert!(
        r.journal()
            .any(|l| l.contains(&format!("view {id}: invalid wakelock value \"dim\"; unset"))),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
    press(&mut r, "screen");
    assert_eq!(super::holders(r.kernel()), vec![id]);
}

#[test]
fn a_plan_without_the_attribute_holds_nothing() {
    let plan = contract::compile("component App\n  view\n    text \"hi\"\n").unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(state(&r), ",\"wakelock\":{\"held\":false,\"by\":[]}");
}

/// The runner journals a bad word each time the value changes to it, as
/// the JS target's wakelock.js does (host/web/tests/wakelock.test.mjs).
#[test]
fn a_bad_word_is_journaled_each_time_it_comes_back() {
    let mut r = boot();
    for word in ["dim", "screen", "dim", "dim"] {
        press(&mut r, word);
    }
    let lines = r
        .journal()
        .filter(|l| l.contains("invalid wakelock value \"dim\""))
        .count();
    assert_eq!(lines, 2);
}

fn boot_source(source: &str) -> Runner<NoData> {
    let plan = contract::compile(source).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

const TABS: &str = r#"routes nav
  tab first "/"
  tab second "/second"

component App
  action pick(name: string)
    nav = select(nav, name)
  action back
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      for t in nav.tabs key=t.name
        column role="tabpanel" id=`panel-${t.name}`
          for e in t.stack key=e.id
            column navigationKey=`${e.id}` testId=`route-${e.name}`
              if e.name == "second"
                text "Player" testId="player" wakelock="screen"
      row role="tablist"
        button role="tab" aria-controls="panel-first" press=pick("first") testId="tab-first"
          text "First"
        button role="tab" aria-controls="panel-second" press=pick("second") testId="tab-second"
          text "Second"
"#;

/// A tab not yet selected is not built (LLP 1075.003 §3.7); one selected
/// once stays mounted, hidden by every host when another is selected: it
/// holds nothing then.
#[test]
fn a_tab_not_selected_holds_nothing() {
    let mut r = boot_source(TABS);
    assert_eq!(player(&r), None, "the second tab waits for its selection");
    assert!(super::holders(r.kernel()).is_empty());
    press(&mut r, "tab-second");
    let id = player(&r).expect("the second tab is built");
    assert_eq!(super::holders(r.kernel()), vec![id]);
    press(&mut r, "tab-first");
    assert_eq!(player(&r), Some(id));
    assert!(super::holders(r.kernel()).is_empty());
}

const STACK: &str = r#"routes nav
  home "/"
    item "/item"
    add "/add"

component App
  action open
    nav = push(nav, "/item")
  action openAdd
    nav = push(nav, "/add")
  action back
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        if e.name == "home"
          column navigationKey=`${e.id}` position="absolute" inset=0
            text "Player" testId="player" wakelock="screen"
            button press=open testId="open"
              text "Open"
            button press=openAdd testId="add"
              text "Add"
        if e.name == "item"
          column navigationKey=`${e.id}` position="absolute" inset=0
            button press=back testId="back"
              text "Back"
            button press=openAdd testId="item-add"
              text "Add"
        if e.name == "add"
          column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0
            button press=back testId="close"
              text "Close"
"#;

/// A screen a stack keeps under its top holds nothing; the one a sheet
/// covers is still shown, and holds.
#[test]
fn a_covered_screen_holds_nothing_but_one_under_a_sheet_does() {
    let mut r = boot_source(STACK);
    let id = player(&r).unwrap();
    assert_eq!(super::holders(r.kernel()), vec![id]);
    press(&mut r, "open");
    assert_eq!(player(&r), Some(id));
    assert!(
        super::holders(r.kernel()).is_empty(),
        "a pushed route covers it"
    );
    press(&mut r, "back");
    assert_eq!(super::holders(r.kernel()), vec![id]);
    press(&mut r, "add");
    assert_eq!(
        super::holders(r.kernel()),
        vec![id],
        "a sheet leaves it shown"
    );
}

fn view(r: &Runner<NoData>, id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id
}

/// A split: the list a host shows beside the detail's pane holds, and so it
/// does under a sheet over that detail, as the web shows both (navigation.js
/// `covered`). With no pane reported the list is covered, under a sheet too.
#[test]
fn a_splits_list_beside_its_detail_holds_under_a_sheet_too() {
    use exact_kernel::{BlockAxis, HostBlock};
    let mut r = boot_source(STACK);
    let id = player(&r).unwrap();
    press(&mut r, "open");
    assert!(super::holders(r.kernel()).is_empty(), "no pane: covered");
    press(&mut r, "item-add");
    assert!(
        super::holders(r.kernel()).is_empty(),
        "no pane: covered under a sheet"
    );
    press(&mut r, "close");
    let detail = r.kernel().node(view(&r, "back")).unwrap().parent.unwrap();
    let pane = HostBlock {
        width: BlockAxis::Is(500.0),
        height: BlockAxis::Is(400.0),
    };
    r.kernel_mut().set_host_block(detail, Some(pane)).unwrap();
    assert_eq!(super::holders(r.kernel()), vec![id], "beside the pane");
    press(&mut r, "item-add");
    assert_eq!(
        super::holders(r.kernel()),
        vec![id],
        "beside the pane, under a sheet"
    );
}

/// A box its host lays out as `display: none` (an unopened tab, a closed
/// top-layer box) holds nothing.
#[test]
fn a_whole_host_cover_holds_nothing() {
    let mut r = boot();
    let id = player(&r).unwrap();
    let parent = r.kernel().node(id).unwrap().parent.unwrap();
    r.kernel_mut()
        .set_host_cover(parent, Some(exact_kernel::HostCover::Whole))
        .unwrap();
    assert!(super::holders(r.kernel()).is_empty());
    r.kernel_mut().set_host_cover(parent, None).unwrap();
    assert_eq!(super::holders(r.kernel()), vec![id]);
}

//! D3's rules through a runner (LLP 1133.029): the topmost sheet that holds
//! wins, the nearest holder wins, unrelated holders intersect, a conflict
//! holds none, a hidden or boxed holder counts for nothing.

use super::{Lock, Session, ALL};
use crate::{DataError, DataSource, Event, Runner};
use exact_kernel::Kernel;
use exact_plan::Value;

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SOURCE: &str = r#"routes nav
  home "/"
    item "/item"
    a "/a"
    b "/b"

component App
  state rootLock = "portrait"
  state childLock = "none"
  state sideLock = "none"
  state aLock = "landscape"
  state bLock = "none"
  action root(w: string)
    rootLock = w
  action child(w: string)
    childLock = w
  action side(w: string)
    sideLock = w
  action sheetA(w: string)
    aLock = w
  action sheetB(w: string)
    bLock = w
  action go(to: string)
    nav = push(nav, to)
  action back
    nav = back(nav)
  view
    column testId="root" orientationlock=rootLock
      main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
        for e in stack(nav) key=e.id
          if e.name == "home"
            column navigationKey=`${e.id}` position="absolute" inset=0
              column testId="child" orientationlock=childLock
                text "child"
              column testId="side" orientationlock=sideLock
                text "side"
              button press=root("none") testId="root-none"
                text "root none"
              button press=child("landscape") testId="child-landscape"
                text "child landscape"
              button press=child("any") testId="child-any"
                text "child any"
              button press=child("current") testId="child-current"
                text "child current"
              button press=side("landscape") testId="side-landscape"
                text "side landscape"
              button press=side("portrait-primary") testId="side-portrait-primary"
                text "side portrait-primary"
              button press=go("/item") testId="open-item"
                text "item"
              button press=go("/a") testId="open-a"
                text "a"
              dialog id="d"
                Boxed()
          if e.name == "item"
            column navigationKey=`${e.id}` position="absolute" inset=0
              button press=back testId="back"
                text "back"
          if e.name == "a"
            column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0
              column testId="a" orientationlock=aLock
                button press=sheetA("none") testId="a-none"
                  text "a none"
                button press=go("/b") testId="open-b"
                  text "b"
          if e.name == "b"
            column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0
              column testId="b" orientationlock=bLock
                button press=sheetB("portrait") testId="b-portrait"
                  text "b portrait"
                button press=sheetB("none") testId="b-none"
                  text "b none"
                button press=back testId="close-b"
                  text "close"

component Boxed
  view
    text "boxed" testId="boxed" orientationlock="landscape"
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

fn view(r: &Runner<NoData>, id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id
}

fn press(r: &mut Runner<NoData>, id: &str) {
    let v = view(r, id);
    r.dispatch(v, Event::Press).unwrap();
}

/// The counted holders by testId, and the lock's types.
fn held(r: &mut Runner<NoData>) -> (Vec<(u32, String)>, u8) {
    match r.orientation_session() {
        Session::Held(Lock { by, types, .. }) => (by, types),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_nearest_holder_wins_and_any_frees_a_subtree() {
    let mut r = boot();
    let (root, child) = (view(&r, "root"), view(&r, "child"));
    assert_eq!(held(&mut r), (vec![(root, "portrait".into())], 0b0011));
    press(&mut r, "child-landscape");
    assert_eq!(held(&mut r), (vec![(child, "landscape".into())], 0b1100));
    press(&mut r, "child-any");
    assert_eq!(held(&mut r), (vec![(child, "any".into())], ALL));
    press(&mut r, "root-none");
    press(&mut r, "child-current");
    match r.orientation_session() {
        Session::Held(lock) => assert!(lock.current && !lock.natural && lock.types == ALL),
        other => panic!("{other:?}"),
    }
}

#[test]
fn unrelated_holders_intersect_and_a_conflict_holds_none() {
    let mut r = boot();
    let (root, side) = (view(&r, "root"), view(&r, "side"));
    press(&mut r, "side-portrait-primary");
    // The root is the side's ancestor: the side replaces it.
    assert_eq!(
        held(&mut r),
        (vec![(side, "portrait-primary".into())], 0b0001)
    );
    press(&mut r, "child-landscape");
    let child = view(&r, "child");
    let conflict = vec![
        (child, "landscape".to_owned()),
        (side, "portrait-primary".to_owned()),
    ];
    assert_eq!(r.orientation_session(), Session::Conflict(conflict.clone()));
    let line = format!(
        "orientationlock: conflict: view {child} \"landscape\", view {side} \"portrait-primary\"; none held"
    );
    // Journaled once, however often it is read.
    r.orientation_session();
    assert_eq!(r.journal().filter(|l| l.contains(&line)).count(), 1);
    let state = crate::agent::state(&r);
    assert!(
        state.contains(&format!(
            "\"orientationlock\":{{\"held\":false,\"by\":[{child},{side}],\"words\":[\"landscape\",\"portrait-primary\"],\"lock\":null,\"conflict\":[[{child},\"landscape\"],[{side},\"portrait-primary\"]],\"applied\":\"unavailable\"}}"
        )),
        "{state}"
    );
    press(&mut r, "side-landscape");
    assert_eq!(
        held(&mut r),
        (
            vec![(child, "landscape".into()), (side, "landscape".into())],
            0b1100
        )
    );
    assert_ne!(root, child);
}

#[test]
fn the_topmost_sheet_that_holds_wins() {
    let mut r = boot();
    press(&mut r, "child-landscape");
    press(&mut r, "open-a");
    let a = view(&r, "a");
    // The page beneath stays displayed, and the sheet's lock wins over it.
    assert_eq!(held(&mut r), (vec![(a, "landscape".into())], 0b1100));
    // A sheet that holds nothing over one that does leaves its lock.
    press(&mut r, "open-b");
    assert_eq!(held(&mut r), (vec![(a, "landscape".into())], 0b1100));
    press(&mut r, "b-portrait");
    let b = view(&r, "b");
    assert_eq!(held(&mut r), (vec![(b, "portrait".into())], 0b0011));
    // Two sheets that hold nothing over the page: the page beneath both
    // is displayed, and its lock is in force (LLP 1133.029 D3).
    press(&mut r, "b-none");
    press(&mut r, "a-none");
    let child = view(&r, "child");
    assert_eq!(held(&mut r), (vec![(child, "landscape".into())], 0b1100));
    press(&mut r, "close-b");
    assert_eq!(held(&mut r), (vec![(child, "landscape".into())], 0b1100));
}

#[test]
fn a_hidden_screen_or_a_boxed_lock_holds_nothing() {
    let mut r = boot();
    press(&mut r, "root-none");
    press(&mut r, "child-landscape");
    press(&mut r, "open-item");
    assert_eq!(
        r.orientation_session(),
        Session::None,
        "a pushed screen hides it"
    );
    press(&mut r, "back");
    let child = view(&r, "child");
    assert_eq!(held(&mut r), (vec![(child, "landscape".into())], 0b1100));
    let boxed = view(&r, "boxed");
    let line =
        format!("orientationlock: view {boxed} is inside a dialog or popover; it holds nothing");
    r.orientation_session();
    assert_eq!(r.journal().filter(|l| l.contains(&line)).count(), 1);
}

#[test]
fn a_computed_word_outside_the_table_is_journaled_and_holds_nothing() {
    let src = "component App\n  state w = \"sideways\"\n  view\n    column testId=\"x\" orientationlock=w\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.orientation_session(), Session::None);
    let x = view(&r, "x");
    assert!(
        r.journal().any(|l| l.contains(&format!(
            "view {x}: invalid orientationlock value \"sideways\"; unset"
        ))),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
}

#[test]
fn the_host_readback_and_its_conflict_reach_the_state() {
    let mut r = boot();
    let root = view(&r, "root");
    let lock = r.orientation_session().lock().unwrap().json();
    assert_eq!(
        lock,
        format!("{{\"types\":3,\"orientations\":[\"portrait-primary\",\"portrait-secondary\"],\"natural\":false,\"current\":false,\"by\":[[{root},\"portrait\"]]}}")
    );
    r.set_orientation_applied("{\"asked\":[\"portrait-primary\"]}".into(), false);
    let state = crate::agent::state(&r);
    assert!(
        state.contains(&format!("\"orientationlock\":{{\"held\":true,\"by\":[{root}],\"words\":[\"portrait\"],\"lock\":{lock},\"conflict\":null,\"applied\":{{\"asked\":[\"portrait-primary\"]}}}}")),
        "{state}"
    );
    r.set_orientation_applied("{}".into(), true);
    let state = crate::agent::state(&r);
    assert!(state.contains("\"held\":false"), "{state}");
    assert!(
        state.contains(&format!("\"conflict\":[[{root},\"portrait\"]]")),
        "{state}"
    );
}

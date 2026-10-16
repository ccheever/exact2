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

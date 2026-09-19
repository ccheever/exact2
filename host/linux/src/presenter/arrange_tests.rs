use super::*;
use exact_runner::{DataError, Value as AppValue};

struct Rows(Vec<AppValue>);
impl DataSource for Rows {
    fn query(&mut self, name: &str, args: &[AppValue]) -> Result<AppValue, DataError> {
        if name == "remove" {
            self.0.retain(|v| v.as_str() != args[0].as_str());
        }
        Ok(AppValue::list(self.0.clone()))
    }
}
const APP: &str = r#"component App
  state grips = true
  state width = 300
  state count = 0
  resource initial = rows() as shape list<string>
  mutation changed as shape list<string>
  derive rows = match changed { case some(v) => v, case none => initial }
  action drop(item: string, before: option<string>) writes grips, count
    grips = false
    count = count + 1
  action hide writes grips
    grips = false
  action show writes grips
    grips = true
  action shrink writes width
    width = 200
  action remove writes changed
    send changed = remove("0")
  view
    column width="100%" height="100%"
      list testId="list" id="items" width=width height=180 virtualized=true reorderdrop=drop
        each item in rows key=item
          box height=40 testId=`row-${item}`
            when grips
              box height=40 width="100%" reorderFor="items" testId=`grip-${item}`
            else
              text item
      button testId="hide" press=hide
        text "hide"
      button testId="show" press=show
        text "show"
      button testId="shrink" press=shrink
        text "shrink"
      button testId="remove" press=remove
        text "remove"
"#;
fn boot() -> Presenter<Rows> {
    let (mut p, e) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        Rows((0..100).map(|i| AppValue::str(&i.to_string())).collect()),
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(e.is_none(), "{e:?}");
    for _ in 0..3 {
        p.frame();
    }
    p
}
fn id(p: &Presenter<Rows>, name: &str) -> ViewId {
    let k = p.host.kernel().find_by_test_id(name)[0];
    p.host.kernel().node_by_key(k).unwrap().id
}
fn start(p: &mut Presenter<Rows>) -> (NodeKey, ReorderToken, HoldToken) {
    p.pointer_down(10., 10., 0.).unwrap();
    assert!(p.pointer_move(10., 20., 10.).unwrap());
    let s = p.arrange.as_ref().unwrap();
    (s.binding.wrapper, s.token, s.hold)
}
#[test]
fn terminal_retains_offscreen_source_after_grip_death_and_second_feedback_then_finishes() {
    let mut p = boot();
    let (source, _, _) = start(&mut p);
    p.pointer_move(10., 170., 20.).unwrap();
    assert!(p.pointer_up(10., 170., 20.).unwrap());
    assert!(p.host.kernel().find_by_test_id("grip-0").is_empty());
    let list = id(&p, "list");
    p.scroll.insert(list, (0., 1800.));
    p.collection_scrolled(list);
    p.queue_collections();
    assert!(p.refine_collections().is_none());
    p.queue_collections();
    assert!(p.refine_collections().is_none());
    assert!(
        p.host.kernel().node_by_key(source).is_some(),
        "dead grip must not lose retained source key"
    );
    assert!(p.host.collections()[0].rows.len() < 32);
    p.clock(5000.);
    p.frame();
    assert!(p.arrange.is_none());
    assert!(p.collection_interaction().is_none());
    assert!(p.host.kernel().node_by_key(source).is_none());
}
#[test]
fn deletion_cancels_without_action_and_old_common_token_cannot_end_new_source() {
    let mut p = boot();
    let (source, token, hold) = start(&mut p);
    p.pointer_move(10., 70., 20.).unwrap();
    p.tap(id(&p, "remove")).unwrap();
    assert!(p.host.kernel().node_by_key(source).is_none());
    assert!(!p.host.has_hold(hold));
    assert!(!p.pointer_up(f32::NAN, 0., f64::NAN).unwrap());
    assert_eq!(p.host.runner().slot("count"), Some(&AppValue::Number(0.)));
    p.frame();
    p.pointer_down(10., 10., 30.).unwrap();
    assert!(p.pointer_move(10., 20., 40.).unwrap());
    let current = p.arrange.as_ref().unwrap().hold;
    assert_ne!(current, hold);
    let now = p.host.now();
    p.host.arrange_finish(token).unwrap();
    assert_eq!(p.host.now(), now);
    assert!(p.host.has_hold(current));
}
#[test]
fn width_reflow_cancels_without_action_and_same_grip_pin_transfer_is_not_revived() {
    let mut p = boot();
    let (_, token, _) = start(&mut p);
    p.pointer_move(10., 70., 20.).unwrap();
    p.tap(id(&p, "shrink")).unwrap();
    assert!(!p.host.runner().has_reorder(token));
    assert_eq!(p.host.runner().slot("count"), Some(&AppValue::Number(0.)));
    let grip = id(&p, "grip-0");
    let other = id(&p, "show");
    p.set_collection_interaction(Some(other));
    p.refine_collections();
    p.set_collection_interaction(Some(grip));
    p.refine_collections();
    p.clock(5000.);
    p.frame();
    assert_eq!(
        p.collection_interaction(),
        Some(grip),
        "old terminal cleanup cannot clear transferred-back pin"
    );
}
#[test]
fn stale_callback_precedes_invalid_clock_and_position_after_runtime_replacement() {
    let mut p = boot();
    let (_, old, hold) = start(&mut p);
    let replacement = boot();
    p.host = replacement.host;
    p.contact = None;
    p.arrange = None;
    p.brush.arrange_lift = None;
    let now = p.host.now();
    assert!(!p.host.has_hold(hold));
    assert!(!p.host.runner().has_reorder(old));
    assert!(!p.pointer_move(f32::NAN, 0., f64::NAN).unwrap());
    p.host.arrange_finish(old).unwrap();
    assert_eq!(p.host.now(), now);
}
#[test]
fn a_held_source_and_settled_preview_have_no_idle_animation_demand() {
    let mut p = boot();
    start(&mut p);
    p.pointer_move(10., 90., 20.).unwrap();
    p.tick(5000.);
    assert!(!p.needs_animation_frame());
    assert!(p.contact.is_some());
    assert!(p.host.has_hold(p.arrange.as_ref().unwrap().hold));
}
#[test]
fn nested_ancestor_scroll_changes_mapping_and_cancels_before_any_late_action() {
    let mut p = boot();
    let (_, token, _) = start(&mut p);
    p.pointer_move(10., 70., 20.).unwrap();
    p.page.1 = 12.;
    p.queue_collections();
    p.retire_pointer();
    assert!(!p.host.runner().has_reorder(token));
    assert!(p.contact.is_none());
    assert!(!p.pointer_up(10., 90., 30.).unwrap());
    assert_eq!(p.host.runner().slot("count"), Some(&AppValue::Number(0.)));
}

#[test]
fn an_arrange_sample_publishes_concurrent_height_layout_then_cancels_changed_port() {
    let source=APP.replace("  state grips = true", "  state panelHeight = 180\n  action grow writes panelHeight\n    panelHeight = 320\n  state grips = true")
        .replace("    column width=\"100%\" height=\"100%\"", "    column testId=\"panel\" width=400 height=panelHeight box-sizing=\"border-box\" transition=\"height spring(300,30,1)\" press=grow")
        .replace("width=width height=180", "width=width height=\"100%\" flex-shrink=0");
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(&source).unwrap().encode(),
        Rows((0..100).map(|i| AppValue::str(&i.to_string())).collect()),
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    for _ in 0..3 {
        p.frame();
    }
    let panel = id(&p, "panel");
    p.host.set_height_owner(Some(panel)).unwrap();
    let (_, token, hold) = start(&mut p);
    p.tap(panel).unwrap();
    assert_eq!(p.host.kernel().node(panel).unwrap().frame.height, 180.);
    assert!(!p.pointer_move(10., 50., 100.).unwrap());
    assert!(
        p.host.kernel().node(panel).unwrap().frame.height > 180.,
        "hold update must drain/project unrelated Height"
    );
    assert!(!p.host.runner().has_reorder(token));
    assert!(!p.host.has_hold(hold));
    assert_eq!(p.host.runner().slot("count"), Some(&AppValue::Number(0.)));
}

#[test]
fn actual_scroll_ahead_of_feedback_retires_before_invalid_callback_validation() {
    let mut p = boot();
    let (_, token, hold) = start(&mut p);
    let list = id(&p, "list");
    let now = p.host.now();
    p.scroll.insert(list, (0., 35.)); // Native scroll landed; its bounded feedback turn is still queued.
    assert_eq!(p.pointer_move(f32::NAN, 0., f64::NAN), Ok(false));
    assert_eq!(p.host.now(), now);
    assert!(!p.host.has_hold(hold));
    assert!(!p.host.runner().has_reorder(token));
    assert_eq!(p.host.runner().slot("count"), Some(&AppValue::Number(0.)));
}

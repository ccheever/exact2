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
  action drop(item: string, before: option<string>)
    grips = false
    count = count + 1
  action hide
    grips = false
  action show
    grips = true
  action shrink
    width = 200
  action remove
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
    p.brush.lift.arrange = None;
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
    let source=APP.replace("  state grips = true", "  state panelHeight = 180\n  action grow\n    panelHeight = 320\n  state grips = true")
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

// Dropping across lists (LLP 1094): a board of three grouped lists in a
// horizontal scroll, `c` empty. Drops on `a` and `b` move at once; a drop on
// `c` waits for the board's timer (an answer that comes later) unless the
// board is stalled, when it never comes.
struct Board(Vec<(String, String)>);
impl Board {
    fn value(&self) -> AppValue {
        AppValue::list(
            self.0
                .iter()
                .map(|(id, col)| AppValue::record(vec![AppValue::str(id), AppValue::str(col)]))
                .collect(),
        )
    }
}
impl DataSource for Board {
    fn query(&mut self, name: &str, args: &[AppValue]) -> Result<AppValue, DataError> {
        match name {
            "columns" => Ok(AppValue::list(["a", "b", "c"].map(AppValue::str).to_vec())),
            "move" => {
                let (item, col, at) = (
                    args[0].as_str().unwrap(),
                    args[1].as_str().unwrap(),
                    args[2].as_str().unwrap(),
                );
                let from = self.0.iter().position(|(id, _)| id == item).unwrap();
                let card = self.0.remove(from);
                let at = self
                    .0
                    .iter()
                    .position(|(id, _)| id == at)
                    .unwrap_or_else(|| {
                        self.0
                            .iter()
                            .rposition(|(_, c)| c == col)
                            .map_or(self.0.len(), |i| i + 1)
                    });
                self.0.insert(at, (card.0, col.to_owned()));
                Ok(self.value())
            }
            _ => Ok(self.value()),
        }
    }
}
const BOARD: &str = r#"shape Card
  id: string
  col: string
component App
  state log = ""
  state from = ""
  state stalled = false
  state queued = ""
  state queuedCol = ""
  state queuedAt = ""
  resource columns = columns() as shape list<string>
  resource initial = cards() as shape list<Card>
  mutation changed as shape list<Card>
  derive cards = match changed { case some(v) => v, case none => initial }
  action dropCard(col: string, item: string, before: option<string>, e: ReorderEvent)
    let at = match before { case some(k) => k, case none => "" }
    log = `${item}>${col}@${at}`
    from = e.from
    if col == "c"
      queued = item
      queuedCol = col
      queuedAt = at
    else
      send changed = move(item, col, at)
  action flush
    if queued != "" and not stalled
      send changed = move(queued, queuedCol, queuedAt)
      queued = ""
  action stall
    stalled = true
  task answers mount
    every(200, flush)
  view
    column width="100%" height="100%"
      scroll testId="board" overflow-x="auto" overflow-y="hidden" width=300 height=200
        row
          each col in columns key=col
            list id=`col-${col}` testId=`list-${col}` virtualized=true reorderGroup="cards" reorderdrop=dropCard(col) width=120 height=180 flex-shrink=0
              each card in filter(cards, k => k.col == col) key=card.id
                box height=40 width=120 reorderFor=`col-${col}` testId=`grip-${card.id}`
                  text card.id testId=`label-${card.id}`
                  text "!" visibility="hidden" testId=`badge-${card.id}`
      button testId="stall" press=stall
        text "stall"
"#;
fn board() -> Presenter<Board> {
    let cards = [
        ("a1", "a"),
        ("a2", "a"),
        ("a3", "a"),
        ("b1", "b"),
        ("b2", "b"),
    ];
    let (mut p, e) = Presenter::boot_with(
        &contract::compile(BOARD).unwrap().encode(),
        Board(
            cards
                .iter()
                .map(|(i, c)| (i.to_string(), c.to_string()))
                .collect(),
        ),
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
fn centre<D: DataSource>(p: &mut Presenter<D>, name: &str) -> (f32, f32) {
    let id = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id(name)[0])
        .unwrap()
        .id;
    let r = p.rect_of(id).unwrap();
    (r.0 + r.2 / 2., r.1 + r.3 / 2.)
}
fn slot<D: DataSource>(p: &Presenter<D>, name: &str) -> String {
    p.host
        .runner()
        .slot(name)
        .and_then(AppValue::as_str)
        .unwrap()
        .to_owned()
}
/// Lift `card` and carry it to `to`, a point; the ghost's centre follows.
fn carry(p: &mut Presenter<Board>, card: &str, to: (f32, f32), now: f64) {
    let (x, y) = centre(p, &format!("grip-{card}"));
    p.pointer_down(x, y, now).unwrap();
    p.pointer_move(x + 12., y, now + 10.).unwrap();
    assert!(p.group.is_some(), "a grouped row lifts in any direction");
    p.pointer_move(to.0, to.1, now + 20.).unwrap();
}
fn phase(p: &Presenter<Board>) -> Option<exact_runner::ReorderPhase> {
    let s = p.group.as_ref()?;
    p.host.runner().reorder_frame(s.token).map(|f| f.phase)
}

#[test]
fn a_cross_list_drag_drops_on_the_target_and_the_ghost_lands_then_goes() {
    let mut p = board();
    let b2 = centre(&mut p, "grip-b2");
    carry(&mut p, "a1", (b2.0, b2.1 - 10.), 0.);
    let ghost = p.brush.lift.ghost.expect("a ghost stands for the row");
    assert_eq!(ghost.scale, 1.03);
    let row = p.host.kernel().node_by_key(ghost.wrapper).unwrap();
    assert_eq!(
        row.style.visibility,
        exact_kernel::Visibility::Hidden,
        "the row hides"
    );
    let row_id = row.id;
    // b6 review B4: the ghost shows what the row inherits hidden from its
    // wrapper, not what its own nodes set hidden (the web's clone).
    let named = |p: &Presenter<Board>, name: &str| {
        let k = p.host.kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let kernel = p.host.kernel();
    assert!(crate::paint::revealed(
        kernel,
        named(&p, "label-a1"),
        row_id
    ));
    assert!(!crate::paint::revealed(
        kernel,
        named(&p, "badge-a1"),
        row_id
    ));
    // The ghost paints, and is not hit.
    p.frame();
    assert!(p.boxes().iter().all(|b| b.id != row_id || !b.pointer_hit));
    assert!(p.pointer_up(b2.0, b2.1 - 10., 30.).unwrap());
    assert_eq!(slot(&p, "log"), "a1>b@b2");
    assert_eq!(slot(&p, "from"), "col-a");
    // Landed: the ghost springs onto the row in b, then goes.
    assert!(p.brush.lift.ghost.is_some());
    assert!(p.needs_animation_frame());
    p.clock(2000.);
    p.frame();
    assert!(p.group.is_none() && p.brush.lift.ghost.is_none());
    assert!(p.collection_interaction().is_none());
    let grip = p.host.kernel().find_by_test_id("grip-a1")[0];
    let wrapper = p.host.kernel().node_by_key(grip).unwrap().parent.unwrap();
    assert_eq!(
        p.host.kernel().node(wrapper).unwrap().style.visibility,
        exact_kernel::Visibility::Visible
    );
}

#[test]
fn a_drag_at_the_board_edge_scrolls_the_board() {
    let mut p = board();
    let board = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("board")[0])
        .unwrap()
        .id;
    let r = p.rect_of(board).unwrap();
    // Into b, then to the board's right edge band.
    carry(&mut p, "a2", (r.0 + r.2 - 70., r.1 + 30.), 0.);
    p.pointer_move(r.0 + r.2 - 8., r.1 + 30., 30.).unwrap();
    assert!(p.needs_animation_frame(), "an edge band pumps");
    let before = p.scroll_of(board).0;
    for t in [50., 66., 82., 98., 114.] {
        p.tick(t);
    }
    assert!(p.scroll_of(board).0 > before, "the board scrolls toward c");
    p.pointer_up(r.0 + r.2 - 8., r.1 + 30., 120.).unwrap();
}

#[test]
fn a_held_drop_waits_ignores_escape_then_lands_when_the_answer_shows() {
    let mut p = board();
    let c = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("list-c")[0])
        .unwrap()
        .id;
    let r = p.rect_of(c).unwrap();
    let board = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("board")[0])
        .unwrap()
        .id;
    // Bring c into view first.
    p.scroll.insert(board, (100., 0.));
    p.publish_scroll();
    p.frame();
    let r = (r.0 - 100., r.1, r.2, r.3);
    carry(&mut p, "b1", (r.0 + 30., r.1 + 20.), 0.);
    assert!(p.pointer_up(r.0 + 30., r.1 + 20., 30.).unwrap());
    assert_eq!(slot(&p, "log"), "b1>c@");
    assert_eq!(phase(&p), Some(exact_runner::ReorderPhase::Holding));
    // Escape while holding: the send is out.
    p.pointer_lost(40.).unwrap();
    assert_eq!(phase(&p), Some(exact_runner::ReorderPhase::Holding));
    assert!(p.brush.lift.ghost.is_some(), "the ghost stays");
    // The board's timer answers at 200 ms: landed.
    p.clock(200.);
    p.clock(3000.);
    assert!(p.group.is_none() && p.brush.lift.ghost.is_none());
    let grip = p.host.kernel().find_by_test_id("grip-b1")[0];
    let wrapper = p.host.kernel().node_by_key(grip).unwrap().parent.unwrap();
    let list = p.host.kernel().node(wrapper).unwrap().parent.unwrap();
    assert_eq!(list, c, "the row landed in c");
}

#[test]
fn a_new_lift_waits_out_a_hold_but_ends_a_landing() {
    // LLP 1102 §3.18: D8's hold refuses a second drag; the landing after it does not.
    let mut p = board();
    let c = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("list-c")[0])
        .unwrap()
        .id;
    let r = p.rect_of(c).unwrap();
    let board = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("board")[0])
        .unwrap()
        .id;
    p.scroll.insert(board, (100., 0.));
    p.publish_scroll();
    p.frame();
    let r = (r.0 - 100., r.1, r.2, r.3);
    carry(&mut p, "b1", (r.0 + 30., r.1 + 20.), 0.);
    assert!(p.pointer_up(r.0 + 30., r.1 + 20., 30.).unwrap());
    assert_eq!(phase(&p), Some(exact_runner::ReorderPhase::Holding));
    let first = p.group.as_ref().unwrap().token;
    // While the drop holds, a press on another grip lifts nothing.
    let (x, y) = centre(&mut p, "grip-a1");
    p.pointer_down(x, y, 40.).unwrap();
    p.pointer_move(x + 12., y, 50.).unwrap();
    assert_eq!(
        p.group.as_ref().map(|s| s.token),
        Some(first),
        "the hold refuses"
    );
    p.pointer_up(x + 12., y, 60.).unwrap();
    // The board's timer answers at 200 ms: the move shows and the ghost lands.
    p.clock(200.);
    assert!(p.brush.lift.ghost.is_some(), "the ghost is springing home");
    // A new lift now ends that landing and begins its own session.
    let (x, y) = centre(&mut p, "grip-a1");
    p.pointer_down(x, y, 210.).unwrap();
    p.pointer_move(x + 12., y, 220.).unwrap();
    let second = p.group.as_ref().map(|s| s.token);
    assert!(
        second.is_some() && second != Some(first),
        "a new session: {second:?}"
    );
    assert!(
        p.host.runner().reorder_frame(first).is_none(),
        "the first finished"
    );
}

#[test]
fn a_hold_that_never_answers_times_out_and_the_ghost_returns_home() {
    let mut p = board();
    p.tap(
        p.host
            .kernel()
            .node_by_key(p.host.kernel().find_by_test_id("stall")[0])
            .unwrap()
            .id,
    )
    .unwrap();
    let board = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("board")[0])
        .unwrap()
        .id;
    let c = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("list-c")[0])
        .unwrap()
        .id;
    p.scroll.insert(board, (100., 0.));
    p.publish_scroll();
    p.frame();
    let r = p.rect_of(c).unwrap();
    let now = p.host.now();
    carry(&mut p, "b2", (r.0 + 30., r.1 + 20.), now);
    assert!(p.pointer_up(r.0 + 30., r.1 + 20., now + 30.).unwrap());
    assert_eq!(phase(&p), Some(exact_runner::ReorderPhase::Holding));
    let due = p.host.runner().timer_due_ms().unwrap();
    assert!(due <= now + 30. + 1000.);
    p.clock(now + 30. + 1000.);
    let s = p.group.as_ref().expect("the ghost returns");
    let f = p.host.runner().reorder_frame(s.token).unwrap();
    assert_eq!(f.ending, Some(exact_runner::ReorderEnding::Timeout));
    p.clock(now + 5000.);
    assert!(p.group.is_none() && p.brush.lift.ghost.is_none());
    assert_eq!(p.host.runner().reorder_json(), "null");
}

#[test]
fn keys_lift_step_across_lists_and_drop_with_focus_following_the_row() {
    let mut p = board();
    let grip = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("grip-a2")[0])
        .unwrap()
        .id;
    assert!(p.focusable(grip), "a grouped grip takes the focus");
    p.type_key(grip, "Space", " ", true, false).unwrap();
    assert!(
        p.group.is_some() && p.brush.lift.ghost.is_none(),
        "keys draw no ghost"
    );
    p.key_down("ArrowRight", p.host.now());
    assert!(p
        .host
        .runner()
        .reorder_json()
        .contains("\"to\":\"col-b\",\"before\":\"b2\""));
    p.key_down("ArrowDown", p.host.now());
    p.key_down("Enter", p.host.now());
    assert_eq!(slot(&p, "log"), "a2>b@");
    assert!(p.group.is_none());
    let moved = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("grip-a2")[0])
        .unwrap()
        .id;
    assert_eq!(p.focus, Some(moved), "focus follows the row that landed");
}

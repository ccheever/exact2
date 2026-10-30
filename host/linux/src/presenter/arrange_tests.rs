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
// Each row's grip spans its row, as the pointer coordinates below assume:
// the row writes its inline ends, so a cells list's inset (LLP 1138, tested
// in contract/cli/tests/it/list_rows.rs) leaves it alone.
const APP: &str = r#"component App
  state grips = true
  state width = 300
  state count = 0
  resource initial = rows() as shape string[]
  mutation changed as shape string[]
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
        for item in rows key=item
          box height=40 padding-left=0 padding-right=0 testId=`row-${item}`
            if grips
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
    boot_source(APP)
}
fn boot_source(source: &str) -> Presenter<Rows> {
    let (mut p, e) = Presenter::boot_with(
        &crate::test_plan(source),
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
fn id<D: DataSource>(p: &Presenter<D>, name: &str) -> ViewId {
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
fn padded_reorder_uses_row_coordinates_before_and_after_native_scrolling() {
    for (top, bottom, scroll) in [(0, 0, 0.), (48, 80, 0.), (48, 80, 64.), (0, 80, 64.)] {
        let source = APP
            .replace("  state grips = true", "  state dropped = \"\"\n  state grips = true")
            .replace("    count = count + 1", "    count = count + 1\n    dropped = match before { case some(k) => k, case none => \"end\" }")
            .replace("width=width height=180", &format!("width=width height=300 box-sizing=\"border-box\" border-top-width=3 border-left-width=2 padding-top={top} padding-bottom={bottom}"));
        let mut p = boot_source(&source);
        let list = id(&p, "list");
        p.scroll.insert(list, (0., scroll));
        p.collection_scrolled(list);
        p.queue_collections();
        assert!(p.refine_collections().is_none());
        let key = p.host.kernel().node(list).unwrap().key;
        let facts = p.host.runner().reorder_geometry(key).unwrap();
        assert_eq!(facts.scroll_top, scroll as f64 - top as f64);
        assert!(
            p.reorder_facts_current(&facts),
            "padding {top}/{bottom}, scroll {scroll}"
        );
        let (x, y) = centre(&mut p, "grip-2");
        p.pointer_down(x, y, 0.).unwrap();
        assert!(p.pointer_move(x, y + 10., 10.).unwrap());
        assert!(p.arrange.is_some(), "padded row lifts");
        assert!(p.pointer_move(x, y + 55., 20.).unwrap());
        assert!(p.pointer_up(x, y + 55., 30.).unwrap());
        assert_eq!(
            slot(&p, "dropped"),
            "4",
            "padding {top}/{bottom}, scroll {scroll}"
        );
        assert_eq!(p.host.runner().slot("count"), Some(&AppValue::Number(1.)));
        p.clock(5000.);
        p.frame();
        assert!(p.arrange.is_none());
        assert!(p.collection_interaction().is_none());
    }
}

#[test]
fn pointer_above_or_beside_port_selects_its_gap_instead_of_the_row_centre() {
    let source = APP
        .replace("  state count = 0", "  state dropped = \"\"\n  state count = 0")
        .replace("    count = count + 1", "    count = count + 1\n    dropped = `${item}>${match before { case some(k) => k, case none => \"end\" }}`")
        .replace("width=width height=180", "width=width height=300 margin-top=60");
    for (x, offset, before) in [(10., -48., "0"), (-100., 65., "2"), (500., 65., "2")] {
        let mut p = boot_source(&source);
        let list = id(&p, "list");
        let port = p.rect_of(list).unwrap();
        let grip = p.rect_of(id(&p, "grip-4")).unwrap();
        let y = grip.1 + grip.3 - 1.;
        p.pointer_down(10., y, 0.).unwrap();
        assert!(p.pointer_move(10., y + 10., 10.).unwrap());
        assert!(p.pointer_move(x, port.1 + offset, 20.).unwrap());
        assert!(p.pointer_up(x, port.1 + offset, 28.).unwrap());
        assert_eq!(slot(&p, "dropped"), format!("4>{before}"));
        assert_eq!(p.scroll_of(list).1, 0.);
        assert_eq!(p.host.runner().slot("count"), Some(&AppValue::Number(1.)));
    }
}

#[test]
fn edge_scroll_follows_agent_event_time_and_ignores_older_frames() {
    for (x, y) in [(10., 31.), (-100., 31.), (500., -100.)] {
        let mut p = boot();
        let list = id(&p, "list");
        p.scroll.insert(list, (0., 320.));
        p.publish_scroll();
        p.collection_scrolled(list);
        let point = centre(&mut p, "grip-10");
        p.pointer_down(point.0, point.1, 0.).unwrap();
        assert!(p.pointer_move_at(point.0, point.1 + 10., 0., 16.).unwrap());
        p.pointer_move_at(x, y, 0., 32.).unwrap();
        assert_eq!(p.scroll_of(list).1, 320., "first edge step is zero");
        assert!(p.arrange_edge().is_some());
        for at in [48., 64., 80., 96.] {
            p.pointer_move_at(x, y, 0., at).unwrap();
            let expected = 320. - 720. * (at - 32.) / 1000.;
            assert!((p.scroll_of(list).1 as f64 - expected).abs() < 0.001);
            assert_eq!(p.host.now(), 0., "moves leave the runner clock frozen");
        }
        let before = p.scroll_of(list).1;
        p.tick(0.);
        p.tick(80.);
        assert_eq!(p.scroll_of(list).1, before);
        assert_eq!(p.contact_event_ms(), Some(96.));
        p.pointer_move_at(x, y, 80., 112.).unwrap();
        assert!((p.scroll_of(list).1 - before + 720. * 0.016).abs() < 0.001);
        let before = p.scroll_of(list).1;
        p.tick(112.);
        assert_eq!(p.scroll_of(list).1, before, "same interval counts once");
        p.tick(128.);
        assert!((p.scroll_of(list).1 - before + 720. * 0.016).abs() < 0.001);
        p.tick(1000.);
        assert!((p.scroll_of(list).1 - before + 720. * 0.048).abs() < 0.001);

        p.pointer_move_at(x, 90., 1000., 1016.).unwrap();
        assert!(p.arrange_edge().is_none());
        let before = p.scroll_of(list).1;
        p.pointer_move_at(x, y, 1000., 2000.).unwrap();
        assert_eq!(
            p.scroll_of(list).1,
            before,
            "restart establishes a new origin"
        );
        p.pointer_move_at(x, y, 1000., 2016.).unwrap();
        assert!((p.scroll_of(list).1 - before + 720. * 0.016).abs() < 0.001);
        p.pointer_cancel(1000.).unwrap();
        let before = p.scroll_of(list).1;
        p.tick(5000.);
        assert_eq!(
            p.scroll_of(list).1,
            before,
            "contact end stops edge scrolling"
        );
    }
}

#[test]
fn a_frame_after_an_agent_hold_steps_the_edge_once_and_frames_go_on() {
    let mut p = boot();
    let list = id(&p, "list");
    p.scroll.insert(list, (0., 320.));
    p.publish_scroll();
    p.collection_scrolled(list);
    let point = centre(&mut p, "grip-10");
    p.pointer_down(point.0, point.1, 0.).unwrap();
    assert!(p.pointer_move_at(point.0, point.1 + 10., 0., 16.).unwrap());
    for at in [32., 48.] {
        p.pointer_move_at(10., 31., 0., at).unwrap();
    }
    // The agent's `hold 250` seeks the runner's clock and moves the contact's
    // event clock on by as much, ahead of the edge's last step at 48.
    p.delay_contact_event(250.);
    let before = p.scroll_of(list).1;
    p.tick(250.);
    assert!((before - p.scroll_of(list).1 - 720. * 0.032).abs() < 0.001);
    p.tick(266.);
    assert!((before - p.scroll_of(list).1 - 720. * 0.048).abs() < 0.001);
    assert_eq!(
        p.contact_event_ms(),
        Some(298.),
        "a frame's sample keeps it"
    );
    p.pointer_cancel(266.).unwrap();
}

#[test]
fn a_live_return_to_the_band_starts_the_edge_clock_again() {
    let mut p = boot();
    let list = id(&p, "list");
    p.scroll.insert(list, (0., 320.));
    p.publish_scroll();
    p.collection_scrolled(list);
    let point = centre(&mut p, "grip-10");
    p.pointer_down(point.0, point.1, 0.).unwrap();
    assert!(p.pointer_move(point.0, point.1 + 10., 10.).unwrap());
    p.pointer_move(10., 31., 20.).unwrap();
    assert_eq!(p.scroll_of(list).1, 320., "a live move does not scroll");
    p.tick(20.);
    assert_eq!(
        p.scroll_of(list).1,
        320.,
        "the first frame starts the clock"
    );
    p.tick(36.);
    let before = p.scroll_of(list).1;
    assert!((320. - before - 720. * 0.016).abs() < 0.001);
    // Out of the band and back with no frame between them.
    p.pointer_move(10., 90., 40.).unwrap();
    p.pointer_move(10., 31., 500.).unwrap();
    p.tick(500.);
    assert_eq!(p.scroll_of(list).1, before);
    p.tick(516.);
    assert!((before - p.scroll_of(list).1 - 720. * 0.016).abs() < 0.001);
    p.pointer_cancel(516.).unwrap();
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
        .replace("    column width=\"100%\" height=\"100%\"", "    column testId=\"panel\" width=400 height=panelHeight box-sizing=\"border-box\" transition=\"height -exact-spring(300,30,1)\" press=grow")
        .replace("width=width height=180", "width=width height=\"100%\" flex-shrink=0");
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
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
fn padded_reorder_rejects_unreported_scroll_and_padding() {
    for side in [
        exact_kernel::StyleId::PaddingTop,
        exact_kernel::StyleId::PaddingBottom,
    ] {
        let source = APP.replace(
            "width=width height=180",
            "width=width height=300 box-sizing=\"border-box\" padding-top=48 padding-bottom=80",
        );
        let mut p = boot_source(&source);
        let list = id(&p, "list");
        let key = p.host.kernel().node(list).unwrap().key;
        let facts = p.host.runner().reorder_geometry(key).unwrap();
        assert!(p.reorder_facts_current(&facts));
        p.scroll.insert(list, (0., 1.));
        assert!(
            !p.reorder_facts_current(&facts),
            "native scroll must be reported first"
        );
        p.scroll.insert(list, (0., 0.));
        let mut style = exact_kernel::StyleProps {
            padding_top: exact_kernel::Dimension::Points(64.),
            padding_bottom: exact_kernel::Dimension::Points(100.),
            ..Default::default()
        };
        style.mask.set(side);
        p.host.apply_test_ops(&[exact_kernel::Op::SetStyle {
            id: list,
            patch: Box::new(style),
        }]);
        assert!(
            !p.reorder_facts_current(&facts),
            "padding must be reported, including trailing-only changes"
        );
        p.queue_collections();
        assert!(p.refine_collections().is_none());
        let fresh = p.host.runner().reorder_geometry(key).unwrap();
        assert!(p.reorder_facts_current(&fresh));
        assert!(
            !p.reorder_facts_current(&facts),
            "old witness stays stale after feedback"
        );
    }
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
  resource columns = columns() as shape string[]
  resource initial = cards() as shape Card[]
  mutation changed as shape Card[]
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
    if queued != "" && ! stalled
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
          for col in columns key=col
            list id=`col-${col}` testId=`list-${col}` virtualized=true reorderGroup="cards" reorderdrop=dropCard(col) width=120 height=180 flex-shrink=0
              for card in cards.filter(k => k.col == col) key=card.id
                box height=40 width=120 reorderFor=`col-${col}` testId=`grip-${card.id}`
                  text card.id testId=`label-${card.id}`
                  text "!" visibility="hidden" testId=`badge-${card.id}`
      button testId="stall" press=stall
        text "stall"
"#;
fn board() -> Presenter<Board> {
    board_source(BOARD)
}
fn board_source(source: &str) -> Presenter<Board> {
    let cards = [
        ("a1", "a"),
        ("a2", "a"),
        ("a3", "a"),
        ("b1", "b"),
        ("b2", "b"),
    ];
    let (mut p, e) = Presenter::boot_with(
        &crate::test_plan(source),
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
fn padded_group_reorder_maps_between_different_origins_and_an_empty_target() {
    for (target, expected) in [("b", "a1>b@b2"), ("c", "a1>c@")] {
        let source = BOARD.replace("width=120 height=180 flex-shrink=0", "width=120 height=180 flex-shrink=0 border-top-width=3 padding-top=(col == \"a\" ? 48 : 80) padding-bottom=36");
        let mut p = board_source(&source);
        let point = if target == "b" {
            let b2 = centre(&mut p, "grip-b2");
            (b2.0, b2.1 - 10.)
        } else {
            (270., 100.) // The visible part of empty c, whose row origin is 83.
        };
        carry(&mut p, "a1", point, 0.);
        assert!(p.pointer_up(point.0, point.1, 30.).unwrap());
        assert_eq!(slot(&p, "log"), expected);
        assert_eq!(slot(&p, "from"), "col-a");
        p.clock(200.);
        p.clock(3000.);
        p.frame();
        assert!(p.group.is_none() && p.brush.lift.ghost.is_none());
        assert!(p.collection_interaction().is_none());
    }
}

#[test]
fn padded_group_drop_refuses_a_target_scrolled_ahead_of_feedback() {
    let source = BOARD.replace(
        "width=120 height=180 flex-shrink=0",
        "width=120 height=180 flex-shrink=0 box-sizing=\"border-box\" padding-top=48 padding-bottom=80",
    );
    let mut p = board_source(&source);
    let b2 = centre(&mut p, "grip-b2");
    carry(&mut p, "a1", (b2.0, b2.1 - 10.), 0.);
    let list = id(&p, "list-b");
    p.scroll.insert(list, (0., 1.));
    assert!(
        !p.end_group(true, 30.).unwrap(),
        "an old target cannot certify a pointer drop"
    );
    assert_eq!(slot(&p, "log"), "");
    p.clock(2000.);
    p.frame();
    assert!(p.group.is_none() && p.brush.lift.ghost.is_none());
    assert!(p.collection_interaction().is_none());
}

#[test]
fn padded_group_drop_refuses_geometry_changed_by_a_due_timer() {
    let source = BOARD
        .replace(
            "  state log = \"\"",
            "  state padding = 48\n  state log = \"\"",
        )
        .replace("  action flush\n", "  action flush\n    padding = 96\n")
        .replace(
            "width=120 height=180 flex-shrink=0",
            "width=120 height=180 flex-shrink=0 padding-top=padding padding-bottom=80",
        );
    let mut p = board_source(&source);
    let b2 = centre(&mut p, "grip-b2");
    carry(&mut p, "a1", (b2.0, b2.1 - 10.), 0.);
    assert!(
        !p.end_group(true, 200.).unwrap(),
        "due padding changes must precede drop certification"
    );
    assert_eq!(
        p.host.runner().slot("padding"),
        Some(&AppValue::Number(96.))
    );
    assert_eq!(slot(&p, "log"), "");
    p.clock(2000.);
    p.frame();
    assert!(p.group.is_none() && p.brush.lift.ghost.is_none());
    assert!(p.collection_interaction().is_none());
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
fn a_contact_at_the_edge_scrolls_though_the_ghost_centre_has_passed_it() {
    // D7 as amended (2026-10-07): a row held by its left end has its centre
    // 54 points right of the contact; with the contact in the board's right
    // band, the centre is past the board, and the board still scrolls (bench
    // t4: a tall card held by its top never autoscrolled to Done).
    let mut p = board();
    let board = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("board")[0])
        .unwrap()
        .id;
    let r = p.rect_of(board).unwrap();
    let grip = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("grip-a2")[0])
        .unwrap()
        .id;
    let g = p.rect_of(grip).unwrap();
    let (x, y) = (g.0 + 6., g.1 + g.3 / 2.);
    p.pointer_down(x, y, 0.).unwrap();
    p.pointer_move(x + 12., y, 10.).unwrap();
    assert!(p.group.is_some(), "the row lifts");
    let edge = r.0 + r.2 - 4.;
    p.pointer_move(edge, y, 20.).unwrap();
    let ghost = p.brush.lift.ghost.expect("a ghost");
    assert!(
        ghost.at.0 + g.2 / 2. > r.0 + r.2,
        "the ghost's centre is past the board's right edge"
    );
    assert!(p.needs_animation_frame(), "the contact's band pumps");
    let before = p.scroll_of(board).0;
    for t in [50., 66., 82., 98., 114.] {
        p.tick(t);
    }
    assert!(
        p.scroll_of(board).0 > before,
        "the board scrolls toward the edge"
    );
    p.pointer_up(edge, y, 120.).unwrap();
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
    assert!(
        p.host
            .runner()
            .journal()
            .any(|l| l.contains("reorder: a drag refused: the last drop is held")),
        "and says so (LLP 1102 §3.17)"
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
    for padding in ["", " padding-top=48 padding-bottom=80"] {
        let source = BOARD.replace(
            "width=120 height=180 flex-shrink=0",
            &format!("width=120 height=180 flex-shrink=0{padding}"),
        );
        let mut p = board_source(&source);
        let grip = id(&p, "grip-a2");
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
        let moved = id(&p, "grip-a2");
        assert_eq!(p.focus, Some(moved), "focus follows the row that landed");
    }
}

#[test]
#[cfg(not(windows))]
fn scroll_end_waits_for_owned_edge_scrolling_to_stop() {
    let source = APP
        .replace(
            "  state grips = true",
            "  state ends = 0\n  action ended\n    ends = ends + 1\n  state grips = true",
        )
        .replace("reorderdrop=drop", "reorderdrop=drop scrollend=ended");
    let mut p = boot_source(&source);
    start(&mut p);
    p.pointer_move(10., 175., 20.).unwrap();
    // The first frame in the band starts the edge's clock; the next scrolls.
    p.tick(84.);
    p.tick(100.);
    assert!(p.scroll_of(id(&p, "list")).1 > 0.);
    assert_eq!(p.host.runner().slot("ends"), Some(&AppValue::Number(0.)));
    p.pointer_move(10., 90., 110.).unwrap();
    p.tick(110.);
    assert_eq!(p.host.runner().slot("ends"), Some(&AppValue::Number(1.)));
    p.tick(210.);
    assert_eq!(p.host.runner().slot("ends"), Some(&AppValue::Number(1.)));
}

#[test]
#[cfg(not(windows))]
fn scroll_end_at_the_arrange_boundary_completes_once() {
    let source = APP
        .replace(
            "  state grips = true",
            "  state ends = 0\n  action ended\n    ends = ends + 1\n  state grips = true",
        )
        .replace("reorderdrop=drop", "reorderdrop=drop scrollend=ended");
    let mut p = boot_source(&source);
    start(&mut p);
    p.pointer_move(10., 175., 20.).unwrap();
    let list = id(&p, "list");
    for tick in 1..=400 {
        p.tick(tick as f64 * 50.);
        if p.arrange_edge().is_none() {
            break;
        }
    }
    assert_eq!(p.scroll_of(list).1, p.collection_scroll_limits()[&list]);
    assert_eq!(p.host.runner().slot("ends"), Some(&AppValue::Number(1.)));
}

#[test]
#[cfg(not(windows))]
fn scroll_end_at_the_group_boundary_completes_once() {
    let source = BOARD
        .replace(
            "  state log = \"\"",
            "  state ends = 0\n  action ended\n    ends = ends + 1\n  state log = \"\"",
        )
        .replace(
            "scroll testId=\"board\"",
            "scroll scrollend=ended testId=\"board\"",
        )
        .replace("every(200, flush)", "after(100000, flush)");
    let mut p = board_source(&source);
    let board = id(&p, "board");
    let r = p.rect_of(board).unwrap();
    carry(&mut p, "a2", (r.0 + r.2 - 70., r.1 + 30.), 0.);
    p.pointer_move(r.0 + r.2 - 8., r.1 + 30., 30.).unwrap();
    for tick in 1..=20 {
        p.tick(tick as f64 * 50.);
    }
    assert_eq!(p.scroll_of(board).0, 60.);
    assert_eq!(p.group_edge(), None);
    assert_eq!(p.host.runner().slot("ends"), Some(&AppValue::Number(1.)));
}

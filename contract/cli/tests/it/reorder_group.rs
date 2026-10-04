//! Dropping across lists (LLP 1094): a board of three grouped lists, one
//! empty, driven through the runner's session with no host: the previews
//! on both lists, the drop on the target with its `ReorderEvent`, the hold
//! and its endings, the keyboard's steps, and the compiler's refusals.
use exact_kernel::{Kernel, NodeKey};
use exact_runner::{
    Answer, CollectionFeedback, DataError, DataSource, Outcome, ReorderEnding, ReorderPhase,
    ReorderProgress, ReorderStep, ReorderToken, Request, Response, RowMeasurement, Runner, Value,
};

const BOARD: &str = r#"shape Card
  id: string
  col: string
component App
  state log = ""
  state from = ""
  state to = ""
  state drops = 0
  resource columns = columns() as shape list<string>
  resource initial = cards() as shape list<Card>
  mutation changed as shape list<Card>
  derive cards = match changed { case some(v) => v, case none => initial }
  action dropCard(col: string, item: string, before: option<string>, e: ReorderEvent)
    let at = match before { case some(k) => k, case none => "" }
    log = `${item}>${col}@${at}`
    from = e.from
    to = e.to
    drops = drops + 1
    send changed = move(item, col, at)
  view
    row
      each col in columns key=col
        list id=`col-${col}` testId=`list-${col}` virtualized=true reorderGroup="cards" reorderdrop=dropCard(col) height=100 width=100
          each card in filter(cards, k => k.col == col) key=card.id
            column reorderFor=`col-${col}` testId=`grip-${card.id}` height=20
              text card.id
"#;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// The move answers in the drop's own commit.
    Now,
    /// It answers when the test fulfils it, as a data module's would.
    Later,
    /// It answers by putting the card first in the column.
    Elsewhere,
    /// It answers by deleting the card.
    Delete,
}

struct Board {
    cards: Vec<(String, String)>,
    mode: Mode,
}

impl Board {
    fn new(mode: Mode) -> Board {
        let cards = [
            ("a1", "a"),
            ("a2", "a"),
            ("a3", "a"),
            ("a4", "a"),
            ("b1", "b"),
            ("b2", "b"),
            ("b3", "b"),
        ];
        Board {
            cards: cards
                .iter()
                .map(|(id, col)| (id.to_string(), col.to_string()))
                .collect(),
            mode,
        }
    }
    fn value(&self) -> Value {
        Value::list(
            self.cards
                .iter()
                .map(|(id, col)| Value::record(vec![Value::str(id), Value::str(col)]))
                .collect(),
        )
    }
    fn apply(&mut self, args: &[Value]) {
        let (item, col, at) = (
            args[0].as_str().unwrap(),
            args[1].as_str().unwrap(),
            args[2].as_str().unwrap(),
        );
        let from = self.cards.iter().position(|(id, _)| id == item).unwrap();
        let card = self.cards.remove(from);
        if self.mode == Mode::Delete {
            return;
        }
        let at = match self.mode {
            Mode::Elsewhere => self.cards.iter().position(|(_, c)| c == col),
            _ => self.cards.iter().position(|(id, _)| id == at),
        };
        let at = at.unwrap_or_else(|| {
            self.cards
                .iter()
                .rposition(|(_, c)| c == col)
                .map_or(self.cards.len(), |i| i + 1)
        });
        self.cards.insert(at, (card.0, col.to_string()));
    }
}

impl DataSource for Board {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == "columns" {
            return Ok(Value::list(["a", "b", "c"].map(Value::str).to_vec()));
        }
        if source == "move" {
            self.apply(args);
        }
        Ok(self.value())
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if source == "move" && self.mode != Mode::Now {
            return Ok(Answer::Later(Request::post_json(
                "https://board.test/move",
                "{}",
            )));
        }
        self.query(source, args).map(Answer::Now)
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        self.apply(args);
        Ok(Answer::Now(self.value()))
    }
}

fn boot(mode: Mode) -> Runner<Board> {
    boot_with(Board::new(mode))
}

fn boot_with(board: Board) -> Runner<Board> {
    let mut r = Runner::boot(
        contract::compile(BOARD).unwrap_or_else(|e| panic!("{e}")),
        board,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    report_all(&mut r, None);
    r
}

fn key<D: DataSource>(r: &Runner<D>, test_id: &str) -> NodeKey {
    r.kernel().find_by_test_id(test_id)[0]
}

/// Each list's report: a 100-tall port, every mounted row 20 tall, the
/// grip's pin on the list that holds it.
fn report<D: DataSource>(r: &mut Runner<D>, list: &str, pin: Option<NodeKey>, top: f64) {
    let view = r.kernel().node_by_key(key(r, list)).unwrap().id;
    let c = r
        .collections()
        .into_iter()
        .find(|c| c.view == view)
        .unwrap();
    let pin = pin
        .and_then(|p| r.kernel().node_by_key(p).map(|n| n.id))
        .filter(|id| c.rows.iter().any(|row| row.root == *id));
    r.collection_feedback(CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset: top,
        port_cross: 100.,
        port_main: 100.,
        cross: 100.,
        measurements: c
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                size: 20.,
            })
            .collect(),
        focus_view: None,
        interaction_view: pin,
    })
    .unwrap();
}

fn report_all<D: DataSource>(r: &mut Runner<D>, pin: Option<NodeKey>) {
    for _ in 0..3 {
        for list in ["list-a", "list-b", "list-c"] {
            report(r, list, pin, 0.);
        }
    }
}

/// Each mounted row's key and translate, in index order.
fn offsets<D: DataSource>(r: &Runner<D>, list: &str) -> Vec<(String, f32)> {
    let view = r.kernel().node_by_key(key(r, list)).unwrap().id;
    let c = r
        .collections()
        .into_iter()
        .find(|c| c.view == view)
        .unwrap();
    c.rows
        .iter()
        .map(|row| {
            let wrapper = r.kernel().node(row.view).unwrap();
            let root = r.kernel().node(row.root).unwrap();
            let id = root
                .props
                .str(exact_kernel::PropId::TestId)
                .unwrap()
                .trim_start_matches("grip-")
                .to_owned();
            (id, wrapper.style.translate.y)
        })
        .collect()
}

fn hidden<D: DataSource>(r: &Runner<D>, test_id: &str) -> bool {
    let grip = r.kernel().node_by_key(key(r, test_id)).unwrap();
    let wrapper = r.kernel().node(grip.parent.unwrap()).unwrap();
    wrapper.style.visibility == exact_kernel::Visibility::Hidden
}

fn text<D: DataSource>(r: &Runner<D>, slot: &str) -> String {
    r.slot(slot).and_then(Value::as_str).unwrap().to_owned()
}

/// Lift `card` with a ghost: the session's token.
fn lift<D: DataSource>(r: &mut Runner<D>, card: &str, ghost: bool) -> ReorderToken {
    let grip = key(r, &format!("grip-{card}"));
    report_all(r, Some(grip));
    let b = r.reorder_binding(grip).unwrap();
    let g = r.reorder_geometry(b.list).unwrap();
    r.begin_group_reorder(b, g, ghost).unwrap().unwrap().token
}

fn into<D: DataSource>(
    r: &mut Runner<D>,
    token: ReorderToken,
    list: &str,
    y: f64,
) -> ReorderProgress {
    let target = key(r, list);
    let g = r.reorder_geometry(target).unwrap();
    r.preview_reorder_into(token, target, g, y).unwrap()
}

fn drop_on<D: DataSource>(r: &mut Runner<D>, token: ReorderToken, list: &str) -> bool {
    let target = key(r, list);
    let g = r.reorder_geometry(target).unwrap();
    r.drop_reorder(token, g).unwrap().is_some()
}

#[test]
fn a_grouped_list_is_accepted_and_each_refusal_is_whole() {
    contract::compile(BOARD).unwrap();
    let refused = |from: &str, to: &str| {
        let e = contract::compile(&BOARD.replace(from, to)).unwrap_err();
        (e.id.clone(), e.message.clone())
    };
    let group = "`reorderGroup` joins lists that take a drop: give this `list virtualized=true` a `reorderdrop` and an `id`";
    assert_eq!(
        refused(" reorderdrop=dropCard(col)", ""),
        ("lower-reorder-group".into(), group.into())
    );
    assert_eq!(
        refused("list id=`col-${col}` ", "list "),
        ("lower-reorder-group".into(), group.into())
    );
    assert_eq!(
        refused(
            "virtualized=true reorderGroup=\"cards\" reorderdrop=dropCard(col)",
            "reorderGroup=\"cards\""
        ),
        ("lower-reorder-group".into(), group.into())
    );
    let (id, message) = refused("key=card.id", "key=length(card.id)");
    assert_eq!(id, "lower-reorder-group");
    assert_eq!(message, "a grouped list's rows move between lists by their keys, so its `each` is keyed by a string, not a number: key it by the item's string id");
    // A row list keeps its own reorder refusal (LLP 1094 §7).
    let (id, _) = refused(
        "height=100 width=100",
        "display=\"flex\" height=100 width=100",
    );
    assert_eq!(id, "lower-collection-reorder");
}

#[test]
fn reorder_event_is_typed_and_optional() {
    // Without the record, the two keys as before.
    contract::compile(
        &BOARD
            .replace(", e: ReorderEvent)", ")")
            .replace("    from = e.from\n    to = e.to\n", ""),
    )
    .unwrap();
    let e = contract::compile(
        &BOARD
            .replace("e: ReorderEvent", "e: number")
            .replace("    from = e.from\n    to = e.to\n", ""),
    )
    .unwrap_err();
    assert!(e.message.contains("for its `ReorderEvent`"), "{e}");
}

#[test]
fn a_drag_into_another_list_previews_both_and_drops_once_on_the_target() {
    let mut r = boot(Mode::Now);
    let token = lift(&mut r, "a1", true);
    assert!(hidden(&r, "grip-a1"), "a ghost stands for the row");
    // Into b, before b2: one commit offsets both lists.
    let ReorderProgress::Accepted {
        receipt: Some(receipt),
    } = into(&mut r, token, "list-b", 25.)
    else {
        panic!("the gap in b is certified");
    };
    let touched: Vec<_> = receipt.touched.clone();
    assert!(!touched.is_empty());
    assert_eq!(
        offsets(&r, "list-a"),
        [
            ("a1".into(), 0.),
            ("a2".into(), -20.),
            ("a3".into(), -20.),
            ("a4".into(), -20.)
        ],
        "Outgoing: the row keeps its slot, the rows after it close the gap"
    );
    assert_eq!(
        offsets(&r, "list-b"),
        [("b1".into(), 0.), ("b2".into(), 20.), ("b3".into(), 20.)],
        "Incoming: every row at or after the gap moves by the row's height"
    );
    let frame = r.reorder_frame(token).unwrap();
    assert_eq!(frame.target, Some(key(&r, "list-b")));
    assert_eq!(frame.phase, ReorderPhase::Active);
    assert!(r
        .reorder_json()
        .contains("\"to\":\"col-b\",\"before\":\"b2\""));
    // The drop fires once, on b, with both lists named.
    assert!(drop_on(&mut r, token, "list-b"));
    assert_eq!(text(&r, "log"), "a1>b@b2");
    assert_eq!(
        (text(&r, "from"), text(&r, "to")),
        ("col-a".into(), "col-b".into())
    );
    assert_eq!(r.slot("drops"), Some(&Value::Number(1.)));
    // The move showed in the drop's own commit: landed, offsets back to 0
    // at once, the row hidden where it landed until the ghost arrives.
    let frame = r.reorder_frame(token).unwrap();
    assert_eq!(frame.phase, ReorderPhase::Settling);
    assert_eq!(frame.ending, Some(ReorderEnding::Landed));
    assert_eq!(
        offsets(&r, "list-b"),
        [
            ("b1".into(), 0.),
            ("a1".into(), 0.),
            ("b2".into(), 0.),
            ("b3".into(), 0.)
        ]
    );
    assert!(offsets(&r, "list-a").iter().all(|(_, y)| *y == 0.));
    assert_eq!(
        frame.row,
        r.kernel()
            .node_by_key(key(&r, "grip-a1"))
            .and_then(|n| n.parent)
            .map(|p| r.kernel().node(p).unwrap().key)
    );
    assert!(hidden(&r, "grip-a1"));
    r.finish_reorder(token).unwrap().unwrap();
    assert!(!hidden(&r, "grip-a1"));
    assert_eq!(r.reorder_json(), "null");
}

#[test]
fn leaving_returning_and_moving_between_foreign_lists() {
    let mut r = boot(Mode::Now);
    let token = lift(&mut r, "a2", true);
    assert!(matches!(
        into(&mut r, token, "list-b", 5.),
        ReorderProgress::Accepted { .. }
    ));
    // b to c (empty): b's gap closes, c certifies gap 0, the end.
    assert!(matches!(
        into(&mut r, token, "list-c", 50.),
        ReorderProgress::Accepted { .. }
    ));
    assert!(offsets(&r, "list-b").iter().all(|(_, y)| *y == 0.));
    assert!(r
        .reorder_json()
        .contains("\"to\":\"col-c\",\"before\":null"));
    // Home: Outgoing ends and today's preview places the gap.
    assert!(matches!(
        into(&mut r, token, "list-a", 65.),
        ReorderProgress::Accepted { .. }
    ));
    assert_eq!(
        offsets(&r, "list-a"),
        [
            ("a1".into(), 0.),
            ("a2".into(), 20.),
            ("a3".into(), -20.),
            ("a4".into(), 0.)
        ],
        "within one list the preview is today's"
    );
    assert_eq!(
        r.reorder_frame(token).unwrap().target,
        Some(key(&r, "list-a"))
    );
    // An empty list takes a drop at its end.
    assert!(matches!(
        into(&mut r, token, "list-c", 10.),
        ReorderProgress::Accepted { .. }
    ));
    assert!(drop_on(&mut r, token, "list-c"));
    assert_eq!(text(&r, "log"), "a2>c@");
    assert_eq!(offsets(&r, "list-c"), [("a2".into(), 0.)]);
}

#[test]
fn a_list_that_already_holds_the_row_is_never_a_target() {
    // A second card keyed `a1` in b: b holds the dragged row's key.
    let mut board = Board::new(Mode::Now);
    board.cards.push(("a1".into(), "b".into()));
    let mut r = boot_with(board);
    let token = lift(&mut r, "a1", true);
    let start = r.journal().count();
    assert!(matches!(
        into(&mut r, token, "list-c", 10.),
        ReorderProgress::Accepted { .. }
    ));
    assert!(matches!(
        into(&mut r, token, "list-b", 10.),
        ReorderProgress::Accepted { receipt: None }
    ));
    assert!(matches!(
        into(&mut r, token, "list-b", 30.),
        ReorderProgress::Accepted { receipt: None }
    ));
    assert_eq!(
        r.reorder_frame(token).unwrap().target,
        Some(key(&r, "list-c")),
        "the sticky target stays"
    );
    let said: Vec<_> = r
        .journal()
        .skip(start)
        .filter(|l| l.contains("already holds"))
        .collect();
    assert_eq!(said.len(), 1, "once a drag: {said:?}");
}

#[test]
fn a_held_drop_lands_when_its_answer_shows_the_move() {
    let mut r = boot(Mode::Later);
    let token = lift(&mut r, "a1", true);
    assert!(matches!(
        into(&mut r, token, "list-b", 45.),
        ReorderProgress::Accepted { .. }
    ));
    assert!(drop_on(&mut r, token, "list-b"));
    assert_eq!(text(&r, "log"), "a1>b@b3");
    let frame = r.reorder_frame(token).unwrap();
    assert_eq!(frame.phase, ReorderPhase::Holding);
    assert_eq!(frame.ending, None);
    assert_eq!(
        r.timer_due_ms(),
        Some(r.now_ms() + 1000.),
        "the deadline wakes the host"
    );
    // The previews stay; Escape does nothing; a second drop and a new lift are refused.
    assert_eq!(offsets(&r, "list-b")[2], ("b3".into(), 20.));
    assert!(r.cancel_reorder(token).unwrap().is_none());
    assert!(!drop_on(&mut r, token, "list-b"));
    assert!(r.finish_reorder(token).unwrap().is_none());
    let grip = key(&r, "grip-b1");
    let b = r.reorder_binding(grip);
    if let Some(b) = b {
        let g = r.reorder_geometry(b.list).unwrap();
        assert!(
            r.begin_group_reorder(b, g, true).unwrap().is_none(),
            "no lift while holding"
        );
    }
    // The answer: the move shows, before b3 as dropped.
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(ticket, ok()).unwrap();
    let frame = r.reorder_frame(token).unwrap();
    assert_eq!(
        (frame.phase, frame.ending),
        (ReorderPhase::Settling, Some(ReorderEnding::Landed))
    );
    assert_eq!(
        offsets(&r, "list-b"),
        [
            ("b1".into(), 0.),
            ("b2".into(), 0.),
            ("a1".into(), 0.),
            ("b3".into(), 0.)
        ]
    );
    assert_eq!(r.timer_due_ms(), None);
    r.finish_reorder(token).unwrap().unwrap();
}

#[test]
fn a_hold_ends_where_the_action_put_the_row_or_where_it_went() {
    for (mode, ending, at) in [
        (Mode::Elsewhere, ReorderEnding::Landed, Some(0)),
        (Mode::Delete, ReorderEnding::Gone, None),
    ] {
        let mut r = boot(mode);
        let token = lift(&mut r, "a1", true);
        assert!(matches!(
            into(&mut r, token, "list-b", 45.),
            ReorderProgress::Accepted { .. }
        ));
        assert!(drop_on(&mut r, token, "list-b"));
        assert_eq!(r.reorder_frame(token).unwrap().phase, ReorderPhase::Holding);
        let ticket = r.take_requests()[0].ticket;
        r.fulfill(ticket, ok()).unwrap();
        let frame = r.reorder_frame(token).unwrap();
        assert_eq!(frame.ending, Some(ending));
        assert_eq!(frame.row.is_some(), at.is_some());
        if at.is_some() {
            assert_eq!(offsets(&r, "list-b")[0], ("a1".into(), 0.));
        }
    }
}

#[test]
fn a_hold_that_never_answers_times_out_on_the_session_clock() {
    let mut r = boot(Mode::Later);
    let token = lift(&mut r, "a3", true);
    assert!(matches!(
        into(&mut r, token, "list-c", 10.),
        ReorderProgress::Accepted { .. }
    ));
    assert!(drop_on(&mut r, token, "list-c"));
    let due = r.timer_due_ms().unwrap();
    // No plan timer: the deadline alone is due, and `clock settle` reaches it.
    r.advance(due - 1.).unwrap();
    assert_eq!(r.reorder_frame(token).unwrap().phase, ReorderPhase::Holding);
    let receipts = r.advance(due).unwrap();
    assert_eq!(receipts.len(), 1, "one commit carries the ending");
    let frame = r.reorder_frame(token).unwrap();
    assert_eq!(
        (frame.phase, frame.ending),
        (ReorderPhase::Settling, Some(ReorderEnding::Timeout))
    );
    assert!(r
        .journal()
        .any(|l| l.contains("reorderdrop: the move did not show within 1 s")));
    assert!(r.reorder_json().contains("\"ending\":\"timeout\""));
    // Home: the row is still in a, where the ghost springs.
    assert_eq!(
        frame.row,
        r.kernel()
            .node_by_key(key(&r, "grip-a3"))
            .and_then(|n| n.parent)
            .map(|p| r.kernel().node(p).unwrap().key)
    );
    assert!(offsets(&r, "list-a").iter().all(|(_, y)| *y == 0.));
    assert_eq!(r.timer_due_ms(), None);
}

#[test]
fn a_cancel_closes_both_previews_and_runs_nothing() {
    let mut r = boot(Mode::Now);
    let token = lift(&mut r, "a1", true);
    assert!(matches!(
        into(&mut r, token, "list-b", 25.),
        ReorderProgress::Accepted { .. }
    ));
    r.cancel_reorder(token).unwrap().unwrap();
    assert_eq!(
        r.reorder_frame(token).unwrap().phase,
        ReorderPhase::Cancelling
    );
    assert!(offsets(&r, "list-a")
        .iter()
        .chain(&offsets(&r, "list-b"))
        .all(|(_, y)| *y == 0.));
    assert_eq!(r.slot("drops"), Some(&Value::Number(0.)));
    assert!(hidden(&r, "grip-a1"), "the ghost springs home first");
    r.finish_reorder(token).unwrap().unwrap();
    assert!(!hidden(&r, "grip-a1"));
}

#[test]
fn a_sample_outside_every_list_leaves_the_certified_gap() {
    let mut r = boot(Mode::Now);
    let token = lift(&mut r, "a1", true);
    assert!(matches!(
        into(&mut r, token, "list-b", 25.),
        ReorderProgress::Accepted { .. }
    ));
    // Over a header or a gutter the host sends nothing: b's gap stands,
    // and so does the drop on it.
    assert!(drop_on(&mut r, token, "list-b"));
    assert_eq!(text(&r, "log"), "a1>b@b2");
    // A sample against another list's stale geometry changes nothing.
    let mut r = boot(Mode::Now);
    let token = lift(&mut r, "a1", true);
    assert!(matches!(
        into(&mut r, token, "list-b", 25.),
        ReorderProgress::Accepted { .. }
    ));
    let c = key(&r, "list-c");
    let mut g = r.reorder_geometry(c).unwrap();
    g.scroll_sequence += 1;
    assert!(matches!(
        r.preview_reorder_into(token, c, g, 10.).unwrap(),
        ReorderProgress::Stale
    ));
    assert_eq!(
        r.reorder_frame(token).unwrap().target,
        Some(key(&r, "list-b"))
    );
}

#[test]
fn keys_and_custom_actions_step_the_gap_across_lists_and_clamp() {
    let mut r = boot(Mode::Now);
    let token = lift(&mut r, "a2", false);
    assert!(
        !hidden(&r, "grip-a2"),
        "without a ghost the row shows in its own list"
    );
    // Up twice: clamped at the top of a.
    for _ in 0..2 {
        r.reorder_step(token, ReorderStep::Earlier).unwrap();
    }
    assert!(r
        .reorder_json()
        .contains("\"to\":\"col-a\",\"before\":\"a1\""));
    // Right: b, same index (0); right again: c, clamped to its end.
    r.reorder_step(token, ReorderStep::NextList).unwrap();
    assert!(r
        .reorder_json()
        .contains("\"to\":\"col-b\",\"before\":\"b1\""));
    assert!(
        hidden(&r, "grip-a2"),
        "another list is the target: the row hides"
    );
    r.reorder_step(token, ReorderStep::Later).unwrap();
    r.reorder_step(token, ReorderStep::NextList).unwrap();
    assert!(r
        .reorder_json()
        .contains("\"to\":\"col-c\",\"before\":null"));
    // Right of the last list: nothing changes.
    assert!(r
        .reorder_step(token, ReorderStep::NextList)
        .unwrap()
        .is_empty());
    r.reorder_step(token, ReorderStep::PreviousList).unwrap();
    assert!(
        r.reorder_json()
            .contains("\"to\":\"col-b\",\"before\":\"b1\""),
        "index 0, from c's end clamped"
    );
    // The drop needs no measured sample.
    let b = key(&r, "list-b");
    let g = r.reorder_geometry(b).unwrap();
    assert!(r.drop_reorder(token, g).unwrap().is_some());
    assert_eq!(text(&r, "log"), "a2>b@b1");
    assert_eq!(
        r.reorder_frame(token).unwrap().ending,
        Some(ReorderEnding::Landed)
    );
    assert!(
        !hidden(&r, "grip-a2"),
        "without a ghost it shows where it landed"
    );
}

#[test]
fn within_one_grouped_list_the_preview_is_todays() {
    let mut r = boot(Mode::Now);
    let token = lift(&mut r, "a1", true);
    for y in [70., 15., 70.] {
        let g = r.reorder_geometry(key(&r, "list-a")).unwrap();
        assert!(matches!(
            r.preview_reorder(token, g, y).unwrap(),
            ReorderProgress::Accepted { .. }
        ));
    }
    assert_eq!(
        offsets(&r, "list-a"),
        [
            ("a1".into(), 60.),
            ("a2".into(), -20.),
            ("a3".into(), -20.),
            ("a4".into(), -20.)
        ]
    );
    assert!(drop_on(&mut r, token, "list-a"));
    assert_eq!(
        (text(&r, "log"), text(&r, "from"), text(&r, "to")),
        ("a1>a@".into(), "col-a".into(), "col-a".into())
    );
    assert_eq!(
        r.reorder_frame(token).unwrap().ending,
        Some(ReorderEnding::Landed)
    );
    assert!(offsets(&r, "list-a").iter().all(|(_, y)| *y == 0.));
}

fn ok() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: b"{}".to_vec(),
    })
}

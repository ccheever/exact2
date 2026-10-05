//! Dropping across lists on the Apple host (LLP 1094 stage 4): a board of
//! three grouped lists, one empty, through the grouped calls Swift makes.
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{
    Answer, CollectionFeedback, DataError, Outcome, Request, Response, RowMeasurement,
    Value as DataValue,
};

struct Board {
    cards: Vec<(String, String)>,
    later: bool,
}

impl Board {
    fn value(&self) -> DataValue {
        DataValue::list(
            self.cards
                .iter()
                .map(|(id, col)| DataValue::record(vec![DataValue::str(id), DataValue::str(col)]))
                .collect(),
        )
    }
    fn apply(&mut self, args: &[DataValue]) {
        let (item, col, at) = (
            args[0].as_str().unwrap(),
            args[1].as_str().unwrap(),
            args[2].as_str().unwrap(),
        );
        let from = self.cards.iter().position(|(id, _)| id == item).unwrap();
        let card = self.cards.remove(from);
        let at = self
            .cards
            .iter()
            .position(|(id, _)| id == at)
            .unwrap_or_else(|| {
                self.cards
                    .iter()
                    .rposition(|(_, c)| c == col)
                    .map_or(self.cards.len(), |i| i + 1)
            });
        self.cards.insert(at, (card.0, col.into()));
    }
}

impl DataSource for Board {
    fn query(&mut self, source: &str, args: &[DataValue]) -> Result<DataValue, DataError> {
        if source == "columns" {
            return Ok(DataValue::list(
                ["a", "b", "c"].map(DataValue::str).to_vec(),
            ));
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
        args: &[DataValue],
    ) -> Result<Answer, DataError> {
        if source == "move" && self.later {
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
        args: &[DataValue],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        self.apply(args);
        Ok(Answer::Now(self.value()))
    }
}

const BOARD: &str = r#"shape Card
  id: string
  col: string
component App
  state log = ""
  resource columns = columns() as shape list<string>
  resource initial = cards() as shape list<Card>
  mutation changed as shape list<Card>
  derive cards = match changed { case some(v) => v, case none => initial }
  action dropCard(col: string, item: string, before: option<string>, e: ReorderEvent)
    let at = match before { case some(k) => k, case none => "" }
    log = `${item}>${col}@${at} ${e.from}>${e.to}`
    send changed = move(item, col, at)
  view
    row
      each col in columns key=col
        list id=`col-${col}` testId=`list-${col}` virtualized=true reorderGroup="cards" reorderdrop=dropCard(col) height=100 width=100
          each card in filter(cards, k => k.col == col) key=card.id
            column reorderFor=`col-${col}` testId=`grip-${card.id}` height=20
              text card.id
"#;

fn boot(later: bool) -> Host<Board> {
    let cards = [
        ("a1", "a"),
        ("a2", "a"),
        ("a3", "a"),
        ("b1", "b"),
        ("b2", "b"),
    ];
    let board = Board {
        cards: cards
            .iter()
            .map(|(i, c)| (i.to_string(), c.to_string()))
            .collect(),
        later,
    };
    let plan = contract::compile(BOARD).unwrap().encode();
    let (mut h, _) = Host::boot(
        &plan,
        board,
        Box::new(MonospaceMeasurer::default()),
        320.,
        400.,
    )
    .unwrap();
    report(&mut h, None, 0.);
    h
}

fn view(h: &Host<Board>, test_id: &str) -> ViewId {
    let key = h.runner.kernel().find_by_test_id(test_id)[0];
    h.runner.kernel().node_by_key(key).unwrap().id
}

/// Each list's port, three times, the grip pinned in its own list, the
/// target list `b` at `b_top`.
fn report(h: &mut Host<Board>, pin: Option<ViewId>, b_top: f64) {
    let b = view(h, "list-b");
    for _ in 0..3 {
        for c in h.runner.collections() {
            let pinned = pin.filter(|p| c.rows.iter().any(|r| r.root == *p));
            let f = CollectionFeedback {
                view: c.view,
                revision: c.revision,
                scroll_sequence: c.scroll_sequence + 1,
                offset: if c.view == b { b_top } else { 0. },
                port_cross: 100.,
                port_main: 100.,
                cross: 100.,
                focus_view: None,
                interaction_view: pinned,
                measurements: c
                    .rows
                    .iter()
                    .map(|r| RowMeasurement {
                        view: r.view,
                        epoch: r.epoch,
                        size: 20.,
                    })
                    .collect(),
            };
            let reply = h.collection_feedback(&f.encode().unwrap(), 0.);
            assert!(reply.contains("\"error\":null"), "{reply}");
        }
    }
}

/// The batch's last grouped `reorder` op: a field's raw JSON text.
fn field(batch: &str, name: &str) -> String {
    let op = batch.rsplit("\"group\":true").next().unwrap();
    let raw = op.split(&format!("\"{name}\":")).nth(1).unwrap();
    raw.split([',', '}'])
        .next()
        .unwrap()
        .trim_matches('"')
        .to_owned()
}

fn lift(h: &mut Host<Board>, card: &str, ghost: bool) -> u64 {
    let grip = view(h, &format!("grip-{card}"));
    report(h, Some(grip), 0.);
    let reply = h.reorder_group_begin(grip, 0., ghost, 100.);
    assert_eq!(field(&reply, "phase"), "active", "{reply}");
    field(&reply, "token").parse().unwrap()
}

fn hidden(h: &Host<Board>, card: &str) -> bool {
    let grip = view(h, &format!("grip-{card}"));
    let wrapper = h.runner.kernel().node(grip).unwrap().parent.unwrap();
    h.runner.kernel().node(wrapper).unwrap().style.visibility == exact_kernel::Visibility::Hidden
}

fn log(h: &Host<Board>) -> String {
    h.runner
        .slot("log")
        .and_then(DataValue::as_str)
        .unwrap()
        .to_owned()
}

#[test]
fn a_ghosted_card_moves_to_another_list_and_lands() {
    let mut h = boot(false);
    let token = lift(&mut h, "a1", true);
    assert!(hidden(&h, "a1"), "the ghost stands for the row");
    let b = view(&h, "list-b");
    let reply = h.reorder_move_into(token, b, 25., 0., true, 120.);
    assert_eq!(field(&reply, "target"), b.to_string(), "{reply}");
    // Outside every port: nothing moves; the gap in b stands.
    let reply = h.reorder_move_into(token, b, 90., 0., false, 130.);
    assert_eq!(field(&reply, "target"), b.to_string());
    let reply = h.reorder_group_end(token, true, 140.);
    assert_eq!(log(&h), "a1>b@b2 col-a>col-b");
    assert_eq!(field(&reply, "phase"), "settling", "{reply}");
    assert_eq!(field(&reply, "ending"), "landed");
    assert_eq!(field(&reply, "dispatched"), "true");
    // The row is hidden where it landed, until the ghost arrives there.
    let landed = h
        .runner
        .kernel()
        .node(view(&h, "grip-a1"))
        .unwrap()
        .parent
        .unwrap();
    assert_eq!(field(&reply, "row"), landed.to_string());
    assert!(hidden(&h, "a1"));
    // A new lift waits for the ghost.
    let b1 = view(&h, "grip-b1");
    assert_eq!(
        field(&h.reorder_group_begin(b1, 0., true, 150.), "phase"),
        "refused"
    );
    let reply = h.reorder_group_finish(token, 400.);
    assert_eq!(field(&reply, "phase"), "finished", "{reply}");
    assert!(!hidden(&h, "a1"), "the ghost is gone and the row shows");
    assert!(h.group.is_none());
}

#[test]
fn a_scrolled_target_certifies_only_at_its_reported_offset() {
    let mut h = boot(false);
    let grip = view(&h, "grip-a2");
    report(&mut h, Some(grip), 0.);
    let token: u64 = field(&h.reorder_group_begin(grip, 0., true, 100.), "token")
        .parse()
        .unwrap();
    let b = view(&h, "list-b");
    // A scrollTop the runner has not accepted is stale: no target yet.
    let reply = h.reorder_move_into(token, b, 25., 7., true, 110.);
    assert_eq!(field(&reply, "target"), view(&h, "list-a").to_string());
    // An autoscrolled board reaches the empty list c: gap 0 certified.
    let c = view(&h, "list-c");
    let reply = h.reorder_move_into(token, c, 10., 0., true, 120.);
    assert_eq!(field(&reply, "target"), c.to_string());
    h.reorder_group_end(token, true, 130.);
    assert_eq!(log(&h), "a2>c@ col-a>col-c");
}

#[test]
fn a_held_drop_keeps_the_ghost_until_its_answer_or_its_deadline() {
    let mut h = boot(true);
    let token = lift(&mut h, "a1", true);
    let b = view(&h, "list-b");
    h.reorder_move_into(token, b, 25., 0., true, 120.);
    let reply = h.reorder_group_end(token, true, 140.);
    assert_eq!(field(&reply, "phase"), "holding", "{reply}");
    // Escape does nothing while holding; finish is refused.
    assert_eq!(
        field(&h.reorder_group_end(token, false, 150.), "phase"),
        "holding"
    );
    assert_eq!(
        field(&h.reorder_group_finish(token, 160.), "phase"),
        "holding"
    );
    // The answer lands the move: a later batch says so.
    let ticket = h.take_requests()[0].ticket;
    let ok = Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: b"{}".to_vec(),
    });
    let batch = h.fulfill_all(vec![(ticket, ok, None)], 300.);
    assert_eq!(field(&batch, "phase"), "settling", "{batch}");
    assert_eq!(field(&batch, "ending"), "landed");
    h.reorder_group_finish(token, 600.);

    // One that never answers ends at its deadline, on the session clock.
    let mut h = boot(true);
    let token = lift(&mut h, "a2", true);
    let c = view(&h, "list-c");
    h.reorder_move_into(token, c, 10., 0., true, 120.);
    let reply = h.reorder_group_end(token, true, 140.);
    assert!(
        reply.contains("\"timers\":true") || h.runner.timer_due_ms().is_some(),
        "{reply}"
    );
    let due = h.runner.timer_due_ms().unwrap();
    let batch = h.advance(due);
    assert_eq!(field(&batch, "ending"), "timeout", "{batch}");
    // Home: the ghost lands on the row still in a.
    let home = h
        .runner
        .kernel()
        .node(view(&h, "grip-a2"))
        .unwrap()
        .parent
        .unwrap();
    assert_eq!(field(&batch, "row"), home.to_string());
    assert!(hidden(&h, "a2"));
    h.reorder_group_finish(token, due + 400.);
    assert!(!hidden(&h, "a2"));
}

#[test]
fn keys_step_without_a_ghost_and_a_cancel_runs_nothing() {
    let mut h = boot(false);
    let token = lift(&mut h, "a2", false);
    assert!(!hidden(&h, "a2"));
    let reply = h.reorder_group_step(token, 4, 110.);
    assert_eq!(field(&reply, "target"), view(&h, "list-b").to_string());
    assert!(!hidden(&h, "a2"), "the focused grip's row stays shown");
    h.reorder_group_step(token, 2, 120.);
    let reply = h.reorder_group_end(token, true, 130.);
    assert_eq!(field(&reply, "ending"), "landed", "{reply}");
    assert_eq!(log(&h), "a2>b@ col-a>col-b", "clamped to b's end");
    assert!(!hidden(&h, "a2"));
    h.reorder_group_finish(token, 140.);

    let token = lift(&mut h, "a1", true);
    h.reorder_move_into(token, view(&h, "list-c"), 10., 0., true, 200.);
    let reply = h.reorder_group_end(token, false, 210.);
    assert_eq!(field(&reply, "phase"), "cancelling", "{reply}");
    assert_eq!(field(&reply, "dispatched"), "false");
    h.reorder_group_finish(token, 500.);
    assert!(!hidden(&h, "a1"));
    assert_eq!(log(&h), "a2>b@ col-a>col-b", "no action ran");
    assert!(h
        .reorder_group_step(token, 9, 600.)
        .contains("invalid reorder step"));
}

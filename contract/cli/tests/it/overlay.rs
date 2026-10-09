//! Optimistic writes: a send to a mutation that declares `refreshes` shows
//! through its source's overlay over the resource's answer, from the send
//! until an answer asked after it landed includes it, and no answer is
//! asked at the send.

use exact_kernel::{Kernel, PropId};
use exact_runner::{
    Answer, DataError, DataSource, Message, Outcome, Overlaid, Request, Response, Runner, Store,
    Value, Write,
};

const SRC: &str = r#"
shape Item
  text: string

shape Time
  epochAtZero: number

component App
  state other = 0
  state folder = "inbox"
  mutation added as shape Item refreshes items
  mutation queued as shape Item queue refreshes items
  mutation plain as shape Item
  mutation flag as shape Item
  mutation stamped as shape Item refreshes time
  mutation filed as shape Item refreshes files
  resource items = items() as shape list<Item>
  resource time = exactTime() as shape Time
  resource files = files(folder) as shape list<Item>
  derive shown = join(map(items, i => i.text), ",")
  derive itemsFailed = failed(items)
  derive itemsPending = pending(items)
  derive itemsCode = match failure(items) { case some(f) => f.code, case none => "ok" }

  action add(t: string)
    send added = add(t)
  action enqueue(t: string)
    send queued = add(t)
  action tap
    send plain = add("x")
  action clear
    added = none
  action addNow(t: string)
    send added = addNow(t)
  action streamQueued(t: string)
    send queued = streamAdd(t)
  action addAndRefuse(t: string)
    send added = add(t)
    send plain = add("no")
  action flagNow(t: string)
    send flag = addNow(t)
  action stamp(t: string)
    send stamped = add(t)
  action open(f: string)
    folder = f
  action file(t: string)
    send filed = add(t)
  action bump
    other = other + 1
  action reload
    refresh items

  task watch key=(match added { case some(a) => a.text == "boom", case none => false }) ? 0 / 0 : 1
    after(100000, bump)
  task watchFlag key=(match flag { case some(a) => a.text == "boom", case none => false }) ? 0 / 0 : 1
    after(100000, bump)
  task watchItems key=(shown == "a,bad" ? 0 / 0 : 1)
    after(100000, bump)

  view
    column
      text shown testId="items"
"#;

/// A list on a server, read by `items` and written by `add`. Its overlay
/// lays each write's text over the answer: a pending one always; a landed
/// one unless the answer has it already, keeping an answered one the answer
/// does not include yet (a server that is not read-after-write consistent).
#[derive(Default)]
struct Board {
    server: Vec<String>,
    /// `items` answers later, by request; else now.
    live: bool,
    /// The next `items` reply answers this list instead of the server's.
    stale: Option<Vec<String>>,
    /// The next `add` reply fails to parse.
    fail_add: bool,
    /// `items` refuses when asked now.
    refuse_items: bool,
    /// No overlay at all: the answer shows.
    no_overlay: bool,
    /// The overlay answers outside the resource's shape, keeping every
    /// answered write.
    bad_overlay: bool,
    /// The next `items` asked now answers outside its shape.
    bad_items: bool,
    /// The source is still loading: sends wait for it.
    loading: bool,
    items_asks: usize,
    overlays: Vec<Vec<(u64, Option<String>, bool)>>,
    /// The arguments each overlay of `files` was called with.
    files_overlaid: Vec<Vec<Value>>,
}

fn list(texts: &[String]) -> Value {
    Value::list(
        texts
            .iter()
            .map(|t| Value::record(vec![Value::str(t)]))
            .collect(),
    )
}

fn texts(v: &Value) -> Vec<String> {
    match v {
        Value::List(items) => items
            .iter()
            .map(|i| match i {
                Value::Record(f) => f[0].as_str().unwrap_or("").to_string(),
                _ => String::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn ok() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: b"{}".to_vec(),
    })
}

impl DataSource for Board {
    fn ready(&self) -> bool {
        !self.loading
    }
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "items" | "files" => Ok(list(&self.server)),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        match source {
            "items" => {
                self.items_asks += 1;
                if self.refuse_items {
                    return Err(DataError::Unavailable("the list is down".into()));
                }
                if std::mem::take(&mut self.bad_items) {
                    return Ok(Answer::Now(Value::str("not a list")));
                }
                if self.live {
                    Ok(Answer::Later(Request::get("https://board.test/items")))
                } else {
                    Ok(Answer::Now(list(&self.server)))
                }
            }
            "files" if self.live => Ok(Answer::Later(Request::get("https://board.test/files"))),
            "files" => Ok(Answer::Now(list(&self.server))),
            "addNow" => {
                let t = args[0].as_str().unwrap_or("").to_string();
                self.server.push(t.clone());
                Ok(Answer::Now(Value::record(vec![Value::str(&t)])))
            }
            "streamAdd" => Ok(Answer::stream(Request::get("https://board.test/stream"))),
            "add" if args[0].as_str() == Some("no") => {
                Err(DataError::Unavailable("refused at once".into()))
            }
            "add" => Ok(Answer::Later(Request::post_json(
                "https://board.test/add",
                &format!("{:?}", args[0].as_str()),
            ))),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        match source {
            "items" => Ok(Answer::Now(list(
                &self.stale.take().unwrap_or_else(|| self.server.clone()),
            ))),
            "streamAdd" => Ok(Answer::Now(Value::record(vec![Value::str(
                args[0].as_str().unwrap_or(""),
            )]))),
            "add" => {
                if std::mem::take(&mut self.fail_add) {
                    return Err(DataError::Unavailable("the write failed".into()));
                }
                let t = args[0].as_str().unwrap_or("").to_string();
                self.server.push(t.clone());
                Ok(Answer::Now(Value::record(vec![Value::str(&t)])))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn overlay(
        &mut self,
        source: &str,
        args: &[Value],
        answer: &Value,
        writes: &[Write<'_>],
    ) -> Result<Option<Overlaid>, DataError> {
        if source == "files" {
            self.files_overlaid.push(args.to_vec());
            return Ok(None);
        }
        assert_eq!(source, "items");
        self.overlays.push(
            writes
                .iter()
                .map(|w| (w.id, w.reply.map(|_| w.mutation.to_string()), w.answered))
                .collect(),
        );
        if self.no_overlay {
            return Ok(None);
        }
        if self.bad_overlay {
            return Ok(Some(Overlaid {
                value: Value::str("not a list"),
                keep: writes.iter().filter(|w| w.answered).map(|w| w.id).collect(),
            }));
        }
        let mut shown = texts(answer);
        let mut keep = Vec::new();
        for w in writes {
            let t = w.args[0].as_str().unwrap_or("").to_string();
            let included = shown.contains(&t);
            match (w.reply, w.answered) {
                (None, _) => shown.push(t),
                (Some(_), true) if !included => {
                    keep.push(w.id);
                    shown.push(t);
                }
                (Some(_), false) if !included => shown.push(t),
                _ => {}
            }
        }
        Ok(Some(Overlaid {
            value: list(&shown),
            keep,
        }))
    }
}

fn boot(board: Board) -> Runner<Board> {
    // The bake answers the server's list as it starts.
    let seed = Board {
        server: board.server.clone(),
        ..Board::default()
    };
    let baked = contract::bake(contract::compile(SRC).unwrap(), seed).unwrap();
    let mut r = Runner::boot(
        baked,
        board,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // The bake's answer is a first frame: the source is asked at launch, and
    // that first load answered.
    r.data_ready().unwrap();
    for q in r.take_requests() {
        r.fulfill(q.ticket, ok()).unwrap();
    }
    r
}

fn live() -> Runner<Board> {
    boot(Board {
        live: true,
        server: vec!["a".into()],
        ..Board::default()
    })
}

fn shown(r: &Runner<Board>) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id("items")[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}

fn requests(r: &mut Runner<Board>) -> Vec<(String, u64)> {
    r.take_requests()
        .into_iter()
        .map(|q| (q.target, q.ticket))
        .collect()
}

fn ticket(reqs: &[(String, u64)], target: &str) -> u64 {
    reqs.iter().find(|(t, _)| t == target).unwrap().1
}

#[test]
fn a_send_shows_its_write_and_asks_nothing_until_it_lands() {
    let mut r = live();
    assert_eq!(shown(&r), "a");
    let asks = r.data().items_asks;
    r.act("add", vec![Value::str("b")]).unwrap();
    assert_eq!(shown(&r), "a,b", "the write shows in the sending commit");
    assert_eq!(r.data().items_asks, asks, "no answer is asked at the send");
    let reqs = requests(&mut r);
    assert_eq!(reqs.len(), 1, "{reqs:?}");
    assert!(r.journal().any(|l| l.contains("overlay items (1 writes)")));
    // The reply lands: the list is asked again, and the write shows until
    // that answer lands, with no flicker back to the old answer.
    r.fulfill(ticket(&reqs, "added"), ok()).unwrap();
    assert_eq!(shown(&r), "a,b");
    let reqs = requests(&mut r);
    assert_eq!(r.data().items_asks, asks + 1);
    r.fulfill(ticket(&reqs, "items"), ok()).unwrap();
    assert_eq!(shown(&r), "a,b", "the answer has it now");
    assert!(r.writes().is_empty(), "the answer retired the write");
}

#[test]
fn a_read_that_lands_while_a_write_is_pending_shows_the_write() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let write = ticket(&requests(&mut r), "added");
    // Something else changes on the server and the list is read again.
    r.data().server.push("z".into());
    r.act("reload", vec![]).unwrap();
    let read = ticket(&requests(&mut r), "items");
    r.fulfill(read, ok()).unwrap();
    assert_eq!(shown(&r), "a,z,b");
    r.fulfill(write, ok()).unwrap();
    assert_eq!(shown(&r), "a,z,b");
}

#[test]
fn a_failed_write_disappears_in_the_commit_it_ends_and_the_list_is_read_again() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    assert_eq!(shown(&r), "a,b");
    let write = ticket(&requests(&mut r), "added");
    r.data().fail_add = true;
    r.fulfill(write, ok()).unwrap();
    assert_eq!(shown(&r), "a", "rolled back at once");
    assert!(r.writes().is_empty());
    assert_eq!(
        requests(&mut r)
            .iter()
            .filter(|(t, _)| t == "items")
            .count(),
        1,
        "the list is read again to reconcile"
    );
}

#[test]
fn a_failed_write_disappears_even_when_the_reconciling_read_is_refused() {
    let mut r = boot(Board {
        server: vec!["a".into()],
        ..Board::default()
    });
    // `items` answers now; `add` later.
    r.act("add", vec![Value::str("b")]).unwrap();
    assert_eq!(shown(&r), "a,b");
    let write = ticket(&requests(&mut r), "added");
    r.data().fail_add = true;
    r.data().refuse_items = true;
    r.fulfill(write, ok()).unwrap();
    assert_eq!(
        shown(&r),
        "a",
        "the refused read does not veto the rollback"
    );
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(true)));
}

#[test]
fn a_superseded_or_assigned_write_disappears() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    requests(&mut r);
    let asks = r.data().items_asks;
    r.act("add", vec![Value::str("c")]).unwrap();
    assert_eq!(shown(&r), "a,c", "the newer send wins");
    assert_eq!(r.writes().len(), 1);
    // The newer send still shows in the list: superseding asks nothing.
    assert_eq!(r.data().items_asks, asks, "no answer is asked at the send");
    assert!(requests(&mut r).iter().all(|(t, _)| t != "items"));
    r.act("clear", vec![]).unwrap();
    assert_eq!(shown(&r), "a", "an assignment to its slot ends it");
    assert!(r.writes().is_empty());
}

#[test]
fn a_read_asked_before_the_landing_does_not_retire_the_write() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let write = ticket(&requests(&mut r), "added");
    // A read starts before the write lands, and answers without it.
    r.act("reload", vec![]).unwrap();
    let early = ticket(&requests(&mut r), "items");
    r.data().stale = Some(vec!["a".into()]);
    r.fulfill(write, ok()).unwrap();
    // The landing asks again, replacing the early read: answer it stale.
    let late = ticket(&requests(&mut r), "items");
    assert_ne!(early, late);
    assert!(!r.holds(early));
    r.fulfill(late, ok()).unwrap();
    // Asked after the landing but without the write: the overlay keeps it.
    assert_eq!(shown(&r), "a,b");
    assert_eq!(r.writes().len(), 1, "kept until an answer includes it");
    let answered = r.data().overlays.last().unwrap()[0].2;
    assert!(answered);
    // A later read that includes it retires it.
    r.act("reload", vec![]).unwrap();
    let next = ticket(&requests(&mut r), "items");
    r.fulfill(next, ok()).unwrap();
    assert_eq!(shown(&r), "a,b");
    assert!(r.writes().is_empty());
}

#[test]
fn a_synchronous_source_retires_a_landed_write_in_one_commit() {
    let mut r = boot(Board {
        server: vec!["a".into()],
        ..Board::default()
    });
    r.act("add", vec![Value::str("b")]).unwrap();
    let write = ticket(&requests(&mut r), "added");
    r.fulfill(write, ok()).unwrap();
    assert_eq!(shown(&r), "a,b");
    assert!(r.writes().is_empty(), "retired in the landing's commit");
}

#[test]
fn queued_sends_show_while_they_wait_and_identical_sends_are_distinct() {
    let mut r = live();
    r.act("enqueue", vec![Value::str("b")]).unwrap();
    r.act("enqueue", vec![Value::str("b")]).unwrap();
    assert_eq!(shown(&r), "a,b,b");
    let ids: Vec<u64> = r.writes().iter().map(|w| w.0).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn no_overlay_is_called_for_a_resource_with_no_writes() {
    let mut r = live();
    r.act("reload", vec![]).unwrap();
    let read = ticket(&requests(&mut r), "items");
    r.fulfill(read, ok()).unwrap();
    r.act("tap", vec![]).unwrap();
    assert!(r.data().overlays.is_empty());
}

#[test]
fn without_an_overlay_the_answer_shows_and_answered_writes_retire() {
    let mut r = boot(Board {
        server: vec!["a".into()],
        no_overlay: true,
        ..Board::default()
    });
    r.act("add", vec![Value::str("b")]).unwrap();
    assert_eq!(shown(&r), "a");
    let write = ticket(&requests(&mut r), "added");
    r.fulfill(write, ok()).unwrap();
    assert_eq!(shown(&r), "a,b");
    assert!(r.writes().is_empty());
}

#[test]
fn an_overlay_outside_the_shape_shows_the_answer_and_never_refuses() {
    let mut r = live();
    r.data().bad_overlay = true;
    r.act("add", vec![Value::str("b")]).unwrap();
    assert_eq!(shown(&r), "a");
    assert!(r
        .journal()
        .any(|l| l.contains("overlay items: outside its shape, so its answer shows")));
}

#[test]
fn a_reload_does_not_carry_a_resource_a_write_shows_in() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let carried = r.carry();
    assert!(carried.resources.iter().all(|(name, ..)| name != "items"));
}

#[test]
fn the_agent_s_state_shows_the_overlay_and_lists_the_writes() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let state = exact_runner::agent::state(&r);
    assert!(state.contains(r#""writes":[{"id":"#), "{state}");
    assert!(
        state.contains(r#""mutation":"added","landed":false"#),
        "{state}"
    );
    assert_eq!(texts(r.shown_resource("items").unwrap()), ["a", "b"]);
    assert_eq!(
        texts(r.resource("items").unwrap()),
        ["a"],
        "the answer stays the answer"
    );
}

#[test]
fn a_reply_whose_commit_is_refused_ends_its_write() {
    let mut r = live();
    r.act("add", vec![Value::str("boom")]).unwrap();
    assert_eq!(shown(&r), "a,boom");
    let write = ticket(&requests(&mut r), "added");
    // Its landing makes the task's key no key: that commit is refused. The
    // reply is spent, so the write ends and the list is read again.
    r.fulfill(write, ok()).unwrap();
    assert!(r.journal().any(|l| l.contains("refused: TaskKey")));
    assert!(r.writes().is_empty());
    assert_eq!(shown(&r), "a");
    assert!(requests(&mut r).iter().any(|(t, _)| t == "items"));
}

#[test]
fn a_queued_send_whose_ask_is_refused_ends_its_write() {
    let mut r = live();
    r.act("enqueue", vec![Value::str("b")]).unwrap();
    r.act("enqueue", vec![Value::str("no")]).unwrap();
    assert_eq!(shown(&r), "a,b,no");
    let first = ticket(&requests(&mut r), "queued");
    r.fulfill(first, ok()).unwrap();
    // The waiting send's turn: its ask is refused, and it ends in a commit
    // the host applies.
    let advanced = r.advance(0.0);
    assert!(advanced.is_ok_and(|receipts| !receipts.is_empty()));
    assert!(r.journal().any(|l| l.contains("queued send refused")));
    assert!(!shown(&r).contains("no"), "{}", shown(&r));
}

#[test]
fn a_refused_action_puts_back_the_writes_it_ended() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let first = ticket(&requests(&mut r), "added");
    // A send that would supersede it, in an action another send refuses.
    assert!(r.act("addAndRefuse", vec![Value::str("c")]).is_err());
    assert_eq!(shown(&r), "a,b", "the first write still shows");
    assert_eq!(r.writes().len(), 1);
    assert!(r.holds(first));
    r.fulfill(first, ok()).unwrap();
    let read = ticket(&requests(&mut r), "items");
    r.fulfill(read, ok()).unwrap();
    assert_eq!(shown(&r), "a,b");
    assert!(r.writes().is_empty());
}

#[test]
fn a_replacement_runner_asks_a_resource_a_write_showed_in() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let carried = r.carry();
    let asks = r.data().items_asks;
    let plan = contract::bake(
        contract::compile(SRC).unwrap(),
        Board {
            server: vec!["a".into()],
            ..Board::default()
        },
    )
    .unwrap();
    let mut next = Runner::boot_carrying(
        plan,
        Board {
            live: true,
            server: vec!["a".into(), "b".into()],
            items_asks: asks,
            ..Board::default()
        },
        Kernel::with_monospace(),
        &carried,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(next.writes().is_empty(), "writes are not carried");
    assert!(
        requests(&mut next).iter().any(|(t, _)| t == "items"),
        "the list is asked again"
    );
}

fn message() -> Outcome {
    Outcome::Message(Message {
        event: String::new(),
        id: String::new(),
        data: "{}".into(),
        coalesced: 0,
    })
}

#[test]
fn a_stream_s_later_messages_land_nothing_more() {
    let mut r = live();
    r.act("streamQueued", vec![Value::str("b")]).unwrap();
    r.act("streamQueued", vec![Value::str("c")]).unwrap();
    let stream = ticket(&requests(&mut r), "queued");
    r.fulfill(stream, message()).unwrap();
    r.fulfill(stream, message()).unwrap();
    // The first message landed the first write; the second, still waiting,
    // is pending.
    let writes = r.writes();
    assert_eq!(writes.len(), 2, "{writes:?}");
    assert!(writes[0].2 && !writes[1].2, "{writes:?}");
}

#[test]
fn a_send_answered_at_once_lets_go_of_the_one_it_replaces() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let older = ticket(&requests(&mut r), "added");
    r.act("addNow", vec![Value::str("c")]).unwrap();
    assert!(!r.holds(older), "the older reply is not wanted");
}

#[test]
fn a_reconciling_answer_outside_its_shape_fails_the_list_and_publishes() {
    let mut r = boot(Board {
        server: vec!["a".into()],
        ..Board::default()
    });
    r.act("add", vec![Value::str("b")]).unwrap();
    let write = ticket(&requests(&mut r), "added");
    r.data().fail_add = true;
    r.data().bad_items = true;
    r.fulfill(write, ok()).unwrap();
    assert_eq!(shown(&r), "a");
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(true)));
    assert_eq!(r.derive("itemsCode"), Some(&Value::str("shape")));
}

#[test]
fn a_send_made_before_the_source_is_ready_shows_and_is_sent_when_it_is() {
    let mut r = live();
    r.data().loading = true;
    r.act("add", vec![Value::str("b")]).unwrap();
    assert!(requests(&mut r).is_empty(), "it waits for the source");
    assert_eq!(r.writes().len(), 1);
    r.data().loading = false;
    r.data_ready().unwrap();
    let write = ticket(&requests(&mut r), "added");
    assert_eq!(shown(&r), "a,b");
    r.fulfill(write, ok()).unwrap();
    let read = ticket(&requests(&mut r), "items");
    r.fulfill(read, ok()).unwrap();
    assert_eq!(shown(&r), "a,b");
    assert!(r.writes().is_empty());
}

#[test]
fn a_later_send_lets_go_of_the_one_it_supersedes() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let older = ticket(&requests(&mut r), "added");
    r.act("add", vec![Value::str("c")]).unwrap();
    assert!(!r.holds(older));
    // Its late reply is dropped: it lands nothing.
    r.fulfill(older, ok()).unwrap();
    assert_eq!(r.writes().len(), 1);
    assert_eq!(shown(&r), "a,c");
}

#[test]
fn a_landed_write_shown_in_no_resource_of_the_source_s_retires() {
    let mut r = live();
    r.act("stamp", vec![Value::str("b")]).unwrap();
    let write = ticket(&requests(&mut r), "stamped");
    r.fulfill(write, ok()).unwrap();
    assert!(r.writes().is_empty(), "{:?}", r.writes());
}

#[test]
fn a_refused_data_ready_publishes_the_writes_it_ended() {
    let mut r = live();
    r.data().loading = true;
    r.act("add", vec![Value::str("no")]).unwrap();
    r.act("flagNow", vec![Value::str("boom")]).unwrap();
    assert_eq!(shown(&r), "a,no");
    r.data().loading = false;
    r.data().refuse_items = true;
    // Its source refuses the write, and the flag it lands makes the
    // task's key no key: the commit is refused, said in the journal. The
    // refused write stays ended, and the list it showed in is read again in
    // a commit of its own, which marks the list failed rather than refusing
    // and is the host's to apply.
    let receipt = r.data_ready().unwrap().expect("a receipt");
    let items = r.kernel().find_by_test_id("items")[0];
    assert!(
        receipt.touched.contains(&items),
        "the host hears the list change"
    );
    assert!(r.journal().any(|l| l.contains("refused: TaskKey")));
    assert!(r.writes().is_empty());
    assert_eq!(shown(&r), "a");
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(true)));
}

#[test]
fn a_send_made_once_the_source_is_ready_replaces_one_still_waiting_for_it() {
    let mut r = live();
    r.data().loading = true;
    r.act("add", vec![Value::str("b")]).unwrap();
    r.data().loading = false;
    // Ready, before the host says so: this send goes now.
    r.act("add", vec![Value::str("c")]).unwrap();
    let write = ticket(&requests(&mut r), "added");
    assert_eq!(shown(&r), "a,c");
    r.data_ready().unwrap();
    assert!(
        !requests(&mut r).iter().any(|(t, _)| t == "added"),
        "the older send never goes"
    );
    r.fulfill(write, ok()).unwrap();
    let read = ticket(&requests(&mut r), "items");
    r.fulfill(read, ok()).unwrap();
    assert_eq!(shown(&r), "a,c");
    assert!(r.writes().is_empty());
}

#[test]
fn an_overlay_over_the_bake_s_answer_is_given_the_bake_s_arguments() {
    let plan = contract::bake(contract::compile(SRC).unwrap(), Board::default()).unwrap();
    let mut r = Runner::boot(
        plan,
        Board {
            loading: true,
            ..Board::default()
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // The folder changes and a write is sent before the source can answer:
    // the bake's answer for the inbox shows until it can.
    r.act("open", vec![Value::str("sent")]).unwrap();
    r.act("file", vec![Value::str("x")]).unwrap();
    r.data().loading = false;
    r.data().live = true;
    r.data_ready().unwrap();
    let overlaid = &r.data().files_overlaid;
    assert!(!overlaid.is_empty());
    assert!(
        overlaid.iter().all(|a| a == &vec![Value::str("inbox")]),
        "{overlaid:?}"
    );
}

/// The bake's answer stands in for `items` until the source can answer.
fn loading() -> Runner<Board> {
    let plan = contract::bake(
        contract::compile(SRC).unwrap(),
        Board {
            server: vec!["a".into()],
            ..Board::default()
        },
    )
    .unwrap();
    Runner::boot(
        plan,
        Board {
            loading: true,
            live: true,
            server: vec!["a".into(), "fresh".into()],
            ..Board::default()
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn a_refused_data_ready_still_asks_what_stood_in_for_an_answer() {
    let mut r = loading();
    r.act("flagNow", vec![Value::str("boom")]).unwrap();
    r.data().loading = false;
    // The flag makes the task's key no key: the commit is refused, and the
    // one that drops the flag asks the list the bake stood in for.
    assert!(r.data_ready().unwrap().is_some());
    let read = ticket(&requests(&mut r), "items");
    r.fulfill(read, ok()).unwrap();
    // The server's list, the flag's own add included: a source's effects
    // stand when the commit that asked them is refused.
    assert_eq!(shown(&r), "a,fresh,boom");
}

#[test]
fn a_refused_read_at_data_ready_fails_its_resource_and_refuses_nothing() {
    let mut r = loading();
    r.data().loading = false;
    r.data().live = false;
    r.data().refuse_items = true;
    assert!(r.data_ready().is_ok());
    assert_eq!(shown(&r), "a");
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(true)));
    // Its own failure, in the activation's commit
    assert!(
        !r.journal().any(|l| l.contains("refused")),
        "nothing was refused"
    );
    // `refresh` asks it again.
    r.data().refuse_items = false;
    r.act("reload", vec![]).unwrap();
    assert_eq!(shown(&r), "a,fresh");
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(false)));
}

#[test]
fn a_placeholder_whose_read_is_refused_at_data_ready_fails_and_is_not_pending() {
    // Unbaked: a placeholder stands in until the source can answer.
    let mut r = Runner::boot(
        contract::compile(SRC).unwrap(),
        Board {
            loading: true,
            ..Board::default()
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.derive("itemsPending"), Some(&Value::Bool(true)));
    r.data().loading = false;
    r.data().refuse_items = true;
    assert!(r.data_ready().is_ok());
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(true)));
    assert_eq!(r.derive("itemsPending"), Some(&Value::Bool(false)));
}

#[test]
fn a_data_ready_its_own_gate_refuses_fails_what_stood_in() {
    let mut r = loading();
    r.data().loading = false;
    r.data().live = false;
    r.data().server = vec!["a".into(), "bad".into()];
    // The answer makes a task's key no key: activation is refused, and what
    // stood in fails, saying so, in a commit the host applies.
    assert!(r.data_ready().unwrap().is_some());
    assert!(r
        .journal()
        .any(|l| l.contains("TaskKey { task: \"watchItems\" }")));
    assert_eq!(shown(&r), "a");
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(true)));
    assert_eq!(r.derive("itemsPending"), Some(&Value::Bool(false)));
    // Later actions are not refused, and `refresh` asks again.
    r.act("bump", vec![]).unwrap();
    r.data().server = vec!["a".into(), "good".into()];
    r.act("reload", vec![]).unwrap();
    assert_eq!(shown(&r), "a,good");
}

#[test]
fn a_reconciling_answer_its_gate_refuses_fails_the_list_and_actions_stand() {
    let mut r = boot(Board {
        server: vec!["a".into()],
        ..Board::default()
    });
    r.act("add", vec![Value::str("x")]).unwrap();
    let write = ticket(&requests(&mut r), "added");
    r.data().fail_add = true;
    r.data().server = vec!["a".into(), "bad".into()];
    // The write fails, and the list read again makes a task's key no key:
    // that commit is refused, and the next fails the list, keeping its
    // answer without the write, for the host to apply.
    assert!(r.fulfill(write, ok()).is_ok());
    assert!(r
        .journal()
        .any(|l| l.contains("TaskKey { task: \"watchItems\" }")));
    assert_eq!(shown(&r), "a");
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(true)));
    assert!(r.writes().is_empty());
    // Later actions stand, and `refresh` asks again.
    r.act("bump", vec![]).unwrap();
    r.data().server = vec!["a".into(), "good".into()];
    r.act("reload", vec![]).unwrap();
    assert_eq!(shown(&r), "a,good");
}

#[test]
fn a_read_failed_after_its_gate_refused_lets_go_of_an_older_read_in_flight() {
    let mut r = live();
    r.act("reload", vec![]).unwrap();
    let older = ticket(&requests(&mut r), "items");
    r.act("add", vec![Value::str("x")]).unwrap();
    let write = ticket(&requests(&mut r), "added");
    // The write fails, the list answers now, and that answer makes a task's
    // key no key: the list fails, and the read before it is let go.
    r.data().live = false;
    r.data().fail_add = true;
    r.data().server = vec!["a".into(), "bad".into()];
    assert!(r.fulfill(write, ok()).is_ok());
    assert_eq!(r.derive("itemsFailed"), Some(&Value::Bool(true)));
    assert_eq!(r.derive("itemsPending"), Some(&Value::Bool(false)));
    assert!(!r.holds(older));
    assert_eq!(shown(&r), "a");
}

#[test]
fn an_overlay_outside_the_shape_keeps_nothing() {
    let mut r = live();
    r.act("add", vec![Value::str("b")]).unwrap();
    let write = ticket(&requests(&mut r), "added");
    r.fulfill(write, ok()).unwrap();
    let read = ticket(&requests(&mut r), "items");
    // The answer lags the write, and the overlay answers outside the shape:
    // it counts as none, so the answered write retires.
    r.data().stale = Some(vec!["a".into()]);
    r.data().bad_overlay = true;
    r.fulfill(read, ok()).unwrap();
    assert_eq!(shown(&r), "a");
    assert!(r.writes().is_empty());
}

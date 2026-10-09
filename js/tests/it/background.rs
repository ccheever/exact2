//! Storage that finishes after the answer (LLP 1097): an answer replies when
//! its value is ready, the storage it started and did not await finishes as
//! the module's background work under one round at a time, every operation
//! of the module runs in one queue in the order issued, and what fails is
//! journaled.
use super::storage::{args, call, plan, text, Root, GRANTS};
use exact_js::Module;
use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::failure::FailureCode;
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, FailureKind, Outcome, Placement, Request, Runner,
    Store, Work, BACKGROUND,
};
use std::collections::{HashMap, VecDeque};

/// What a native host does with background work after each call (LLP 1097
/// D5): each round's continuation runs on a worker and comes back, until no
/// round is due. The rounds it ran.
pub(crate) fn rounds(m: &mut Module, s: &Store) -> usize {
    let mut rounds = 0;
    let mut next = DataSource::background(m, s);
    while let Some(request) = next {
        assert_eq!(request.continuation, Some(BACKGROUND));
        assert!(
            DataSource::background(m, s).is_none(),
            "one round out at a time"
        );
        let Dispatch::Run(Work::Now(work)) = m.dispatch(BACKGROUND, s) else {
            panic!("a background round runs on the host's worker")
        };
        let outcome = std::thread::spawn(work).join().unwrap();
        rounds += 1;
        assert!(rounds < 1000, "background work did not end");
        next = match m.background_landed(s, outcome).unwrap() {
            Some(next) => Some(next),
            None => DataSource::background(m, s),
        };
    }
    rounds
}

/// Answers to their ends, as a native host runs them: a continuation is
/// dispatched (a waiting answer is held until a delivery releases it), a
/// fetch is answered by `fetch`, and background rounds run between. The
/// answers' texts, in the order they were given.
pub(crate) fn drive(
    m: &mut Module,
    s: &mut Store,
    answers: Vec<(Vec<Value>, Answer)>,
    fetch: impl Fn(&[Value], &Request) -> Outcome,
) -> Vec<String> {
    let mut pending: VecDeque<(Vec<Value>, Answer)> = answers.into();
    let mut held: HashMap<u64, Vec<Value>> = HashMap::new();
    let mut done = Vec::new();
    for _ in 0..1000 {
        rounds(m, s);
        for (token, dispatch) in m.release(s) {
            let a = held.remove(&token).expect("released work was held");
            let Dispatch::Run(Work::Now(work)) = crate::runnable(dispatch) else {
                panic!("released work runs")
            };
            let outcome = std::thread::spawn(work).join().unwrap();
            let next = m.parse(s, "work", &a, outcome).unwrap();
            pending.push_back((a, next));
        }
        let Some((a, answer)) = pending.pop_front() else {
            if held.is_empty() {
                break;
            }
            continue;
        };
        match answer {
            Answer::Now(v) => done.push(text(v)),
            Answer::Later(request) => match request.continuation {
                Some(token) => match crate::runnable(m.dispatch(token, s)) {
                    Dispatch::Run(Work::Now(work)) => {
                        let outcome = std::thread::spawn(work).join().unwrap();
                        let next = m.parse(s, "work", &a, outcome).unwrap();
                        pending.push_back((a, next));
                    }
                    Dispatch::Held => {
                        held.insert(token, a);
                    }
                    _ => panic!("native storage work runs or is held"),
                },
                None => {
                    let outcome = fetch(&a, &request);
                    let next = m.parse(s, "work", &a, outcome).unwrap();
                    pending.push_back((a, next));
                }
            },
        }
    }
    assert!(
        held.is_empty() && pending.is_empty(),
        "answers left waiting"
    );
    done
}

fn read_file(root: &Root, name: &str) -> String {
    std::fs::read_to_string(root.0.join("data").join(name)).unwrap_or_default()
}

/// The runner and a host's loop over it, counting what ran: storage
/// continuation rounds (a 204 from the store's wait) and every other.
struct Host {
    runner: Runner<Module>,
    held: HashMap<u64, u64>,
    work: VecDeque<(u64, Work)>,
    storage_rounds: usize,
}

impl Host {
    fn new(root: &Root) -> Host {
        let mut m = Module::new(super::storage::HBC.to_vec(), super::storage::APP, GRANTS);
        m.set_budget_ms(f64::INFINITY);
        m.configure_storage(
            root.0.join("data"),
            root.0.join("cache"),
            root.0.join("tmp"),
        )
        .unwrap();
        let plan = plan();
        m.bind(&plan);
        m.activate().unwrap();
        Host {
            runner: Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/")
                .unwrap(),
            held: HashMap::new(),
            work: VecDeque::new(),
            storage_rounds: 0,
        }
    }

    /// What the bridge does after every commit: dispatch what was asked,
    /// then run what a source released.
    fn emit(&mut self) {
        for request in self.runner.take_requests() {
            let token = request.request.continuation.expect("storage continuation");
            match crate::runnable(self.runner.dispatch_work(token)) {
                Dispatch::Run(w) => self.work.push_back((request.ticket, w)),
                Dispatch::Held => {
                    self.held.insert(token, request.ticket);
                }
                _ => panic!("native storage work runs or is held"),
            }
        }
        for (token, dispatch) in self.runner.release_work() {
            let Dispatch::Run(w) = crate::runnable(dispatch) else {
                panic!("released work runs")
            };
            if let Some(ticket) = self.held.remove(&token) {
                self.work.push_back((ticket, w));
            }
        }
    }

    /// Run everything the host holds to its end.
    fn settle(&mut self) {
        self.emit();
        for _ in 0..1000 {
            let Some((ticket, w)) = self.work.pop_front() else {
                return;
            };
            let Work::Now(w) = w else {
                panic!("native work runs now")
            };
            let outcome = std::thread::spawn(w).join().unwrap();
            if matches!(&outcome, Outcome::Response(r) if r.status == 204) {
                self.storage_rounds += 1;
            }
            self.runner.fulfill(ticket, outcome).unwrap();
            self.emit();
        }
        panic!("the host's work did not end");
    }

    fn invoke(&mut self, op: &str, value: &str) {
        self.runner
            .act("invoke", vec![Value::str(op), Value::str(value)])
            .unwrap();
    }

    fn result(&self) -> Option<String> {
        let key = *self.runner.kernel().find_by_test_id("result").first()?;
        self.runner
            .kernel()
            .node_by_key(key)?
            .props
            .str(PropId::Text)
            .map(str::to_owned)
    }

    fn journal(&self) -> Vec<String> {
        self.runner.journal().map(str::to_owned).collect()
    }
}

/// D3's first test: no spin. A background write, an answer's read queued
/// behind it, and an idle module: each storage continuation round the host
/// runs ends with exactly one delivery (two operations, two rounds), the
/// read waits with the module rather than on the store, and an idle module
/// hands out no round.
#[test]
fn background_rounds_equal_deliveries_and_an_idle_module_hands_out_none() {
    let root = Root::new();
    let mut host = Host::new(&root);
    host.settle();
    assert_eq!(host.storage_rounds, 0);
    // The write moves to the background; the read is queued behind it.
    host.invoke("save", "one");
    assert_eq!(
        host.result().as_deref(),
        Some("saved one"),
        "replied at once"
    );
    host.invoke("read-at", "song");
    host.emit();
    assert!(
        host.runner
            .in_flight()
            .iter()
            .any(|(name, _)| name == "background"),
        "the background round is a request in flight: {:?}",
        host.runner.in_flight()
    );
    assert_eq!(
        host.held.len(),
        1,
        "the read waits with the module, not on the store"
    );
    assert_eq!(host.work.len(), 1, "one waiter on the store: the round");
    host.settle();
    assert_eq!(host.result().as_deref(), Some("one"));
    assert_eq!(host.storage_rounds, 2, "rounds equal deliveries");
    // Idle: nothing is handed out, and nothing is pending.
    assert!(host.runner.take_requests().is_empty());
    assert!(!host.runner.has_pending());
    let journal = host.journal();
    assert!(
        journal
            .iter()
            .any(|l| l.contains("background: storage (1 waiting)")),
        "{journal:#?}"
    );
    assert!(
        journal
            .iter()
            .any(|l| l.contains("background: done (1 operations)")),
        "{journal:#?}"
    );
    assert_eq!(read_file(&root, "song"), "one");
}

/// D1: an answer that starts a write and does not await it replies in its
/// own turn; the write lands under the background round. One that awaits
/// its write replies after it.
#[test]
fn an_unawaited_write_lands_after_the_reply_and_an_awaited_one_before() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let a = args("save", "first");
    assert!(
        matches!(m.answer(&mut s, "work", &a).unwrap(), Answer::Now(_)),
        "replied in its own turn"
    );
    assert_eq!(read_file(&root, "song"), "", "not yet written");
    assert_eq!(m.background_state().unwrap().in_flight, 1);
    assert_eq!(rounds(&mut m, &s), 1);
    assert_eq!(read_file(&root, "song"), "first");
    assert_eq!(m.background_state().unwrap().done, 1);
    // `file` awaits its write and reads it back: it waits for both.
    let a = args("file", "awaited");
    let answer = m.answer(&mut s, "work", &a).unwrap();
    assert!(matches!(answer, Answer::Later(_)), "it waits for its write");
    let done = drive(&mut m, &mut s, vec![(a, answer)], |_, _| unreachable!());
    assert_eq!(done, ["awaited"]);
}

/// D3: storage keeps one order, the order issued. Two unawaited edits, then
/// a read, with no settle between: the read sees the second edit. Chained
/// on a promise, the second edit is issued only when the first lands, so the
/// read, issued before it, sees the first; the file ends as the second.
#[test]
fn two_unawaited_edits_then_a_read_see_the_second_edit_and_a_chain_its_first() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let mut answers = Vec::new();
    for (op, value) in [("save", "one"), ("save", "two"), ("read-at", "song")] {
        let a = args(op, value);
        answers.push((a.clone(), m.answer(&mut s, "work", &a).unwrap()));
    }
    let done = drive(&mut m, &mut s, answers, |_, _| unreachable!());
    assert_eq!(done, ["saved one", "saved two", "two"]);

    let mut answers = Vec::new();
    for (op, value) in [
        ("save-chained", "three"),
        ("save-chained", "four"),
        ("read-at", "song"),
    ] {
        let a = args(op, value);
        answers.push((a.clone(), m.answer(&mut s, "work", &a).unwrap()));
    }
    let done = drive(&mut m, &mut s, answers, |_, _| unreachable!());
    assert_eq!(done, ["saved three", "saved four", "three"]);
    assert_eq!(read_file(&root, "song"), "four");
}

/// D2's liveness: an answer awaiting a promise chained on background work
/// waits with the module, and replies after the work lands.
#[test]
fn an_answer_awaiting_background_work_waits_for_it() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let save = args("save", "one");
    let saved = m.answer(&mut s, "work", &save).unwrap();
    let after = args("await-saving", "");
    let waiting = m.answer(&mut s, "work", &after).unwrap();
    assert!(matches!(&waiting, Answer::Later(_)), "it waits");
    let done = drive(
        &mut m,
        &mut s,
        vec![(save, saved), (after, waiting)],
        |_, _| unreachable!(),
    );
    assert_eq!(done, ["saved one", "after "]);
}

/// D3's bound: with 256 operations waiting behind the one in flight, the
/// next call is refused at once with `full`, and the refusal is journaled.
#[test]
fn the_258th_operation_is_refused_full_and_journaled() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(call(&mut m, &mut s, "flood", ""), "full");
    let journal = DataSource::take_logs(&mut m);
    assert!(
        journal
            .iter()
            .any(|l| l == "storage refused: full (writeFile app:/data/flood)"),
        "{journal:#?}"
    );
    assert_eq!(
        read_file(&root, "flood"),
        "256",
        "the 257 accepted ran in order"
    );
}

/// D2: background work may not fetch or call the native module: each is
/// refused to the app's promise and journaled, and no answer claims a
/// request for it.
#[test]
fn background_work_cannot_fetch_or_call_native() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(call(&mut m, &mut s, "save-then-fetch", "a"), "saved a");
    assert!(call(&mut m, &mut s, "last-error", "").contains("fetch() called from background work"),);
    assert_eq!(call(&mut m, &mut s, "save-then-native", "b"), "saved b");
    assert!(call(&mut m, &mut s, "last-error", "")
        .contains("native.call() called from background work"));
    let journal = DataSource::take_logs(&mut m);
    for line in [
        "data: fetch() called from background work: it was never run. Fetch inside an answer",
        "data: native.call() called from background work: it was never run",
    ] {
        assert!(journal.iter().any(|l| l == line), "{line}: {journal:#?}");
    }
    assert_eq!(m.in_flight(), 0, "no answer claimed a request");
}

/// D8: a failing background write rejects the app's own promise and is
/// journaled, with `state.background` counting it; an unhandled rejection
/// is journaled through the engine's tracker.
#[test]
fn background_failures_and_unhandled_rejections_are_journaled() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(call(&mut m, &mut s, "save-bad", "x"), "saved x");
    assert_eq!(call(&mut m, &mut s, "last-error", ""), "ENOENT");
    let state = m.background_state().unwrap();
    assert_eq!((state.failed, state.queued, state.in_flight), (1, 0, 0));
    let line = state.last.expect("the last failure");
    assert!(
        line.starts_with("storage failed: writeFile app:/data/absent/file: ENOENT "),
        "{line}"
    );
    assert_eq!(call(&mut m, &mut s, "reject", "here"), "answered");
    let journal = DataSource::take_logs(&mut m);
    assert!(journal.contains(&line), "{journal:#?}");
    assert!(
        journal
            .iter()
            .any(|l| l == "data: unhandled rejection: lost here"),
        "{journal:#?}"
    );
}

/// D7 (Charlie, 2026-10-07): the host does not close a database for work
/// that failed (an app may keep or share a handle); a database still open
/// after a failure in the background work that opened it is journaled with
/// the fix.
#[test]
fn a_database_left_open_by_failed_background_work_is_journaled() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(call(&mut m, &mut s, "open-leak", "x"), "answered");
    rounds(&mut m, &s);
    let journal = DataSource::take_logs(&mut m);
    assert!(
        journal
            .iter()
            .any(|l| l == "data: unhandled rejection: leaked x"),
        "{journal:#?}"
    );
    assert!(
        journal.iter().any(|l| l.starts_with(
            "storage: app:/data/notes.db is still open after a failure in background work"
        ) && l.contains("finally { db.close() }")),
        "{journal:#?}"
    );
}

/// drums R10 and R11 together: a value given at once, its save chained in
/// the answer's checkpoint, lands; the next edit's answer does not wait for
/// it.
#[test]
fn a_chained_save_lands_and_the_next_edit_is_not_behind_it() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    for value in ["third", "fourth"] {
        let a = args("deferred", value);
        assert!(
            matches!(m.answer(&mut s, "work", &a).unwrap(), Answer::Now(_)),
            "the edit's answer is there at once"
        );
    }
    rounds(&mut m, &s);
    assert_eq!(read_file(&root, "deferred"), "fourth");
}

/// A Rust child for the mixed composer: it answers nothing here.
struct Quiet;

impl DataSource for Quiet {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn app_id(&self) -> &str {
        super::storage::APP
    }
}

/// D5's composers: every round, the first and the next, reaches the host's
/// requests through `Storage`, `Mixed` and `Placed` on `Main`, and the
/// module's journal through each `take_logs`.
#[test]
fn rounds_and_journal_lines_pass_through_the_composers() {
    let root = Root::new();
    let mut m = Module::new(super::storage::HBC.to_vec(), super::storage::APP, GRANTS);
    m.set_budget_ms(f64::INFINITY);
    let mixed = exact_data::Mixed::new(m.placed(Placement::Main), Quiet, &["work"], &[]).unwrap();
    let mut data = exact_data_host::Storage::new(mixed);
    data.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("tmp"),
    )
    .unwrap();
    let plan = plan();
    data.bind(&plan);
    data.activate().unwrap();
    let mut runner = Runner::boot(
        plan,
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    for value in ["one", "two"] {
        runner
            .act("invoke", vec![Value::str("save"), Value::str(value)])
            .unwrap();
    }
    let mut rounds = 0;
    for _ in 0..10 {
        let mut out = runner.take_requests();
        let Some(request) = out.pop() else { break };
        assert!(out.is_empty(), "one round at a time");
        assert_eq!(request.target, "background");
        let Dispatch::Run(Work::Now(work)) = runner.dispatch_work(BACKGROUND) else {
            panic!("the round's work passes through every composer")
        };
        let outcome = std::thread::spawn(work).join().unwrap();
        assert_eq!(runner.fulfill(request.ticket, outcome).unwrap(), None);
        rounds += 1;
    }
    assert_eq!(rounds, 2, "the second round came under a ticket of its own");
    assert!(!runner.has_pending());
    assert_eq!(read_file(&root, "song"), "two");
    assert_eq!(runner.background_state().unwrap().done, 2);
    runner
        .act("invoke", vec![Value::str("reject"), Value::str("x")])
        .unwrap();
    runner.take_requests();
    assert!(runner
        .journal()
        .any(|l| l.ends_with("data: unhandled rejection: lost x")));
}

/// D4.5: no answer waits for another to begin. One asked while another's
/// awaited write is in flight begins and replies at once; two that await
/// interleave at their awaits, in the order their operations were issued,
/// as two async calls do on the web (`write1`, `writeB`, `write2`).
#[test]
fn answers_begin_at_once_and_interleave_at_their_awaits() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let first = args("append", "A1,A2");
    let a = m.answer(&mut s, "work", &first).unwrap();
    assert!(matches!(a, Answer::Later(_)), "it awaits its write");
    let save = args("save", "now");
    assert!(
        matches!(m.answer(&mut s, "work", &save).unwrap(), Answer::Now(_)),
        "begun and replied while the other's write is in flight"
    );
    let second = args("append", "B");
    let b = m.answer(&mut s, "work", &second).unwrap();
    let done = drive(
        &mut m,
        &mut s,
        vec![(first, a), (second, b)],
        |_, _| unreachable!(),
    );
    assert_eq!(done, ["B", "A1,A2"]);
    assert_eq!(read_file(&root, "log"), "A1;B;A2;");
    assert_eq!(read_file(&root, "song"), "now");
}

/// D10: a dev restart or a reload unloads the module; what its answers left
/// is finished first, within a second.
#[test]
fn unloading_finishes_what_the_answers_left() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    for value in ["one", "two", "three"] {
        let a = args("save", value);
        assert!(matches!(
            m.answer(&mut s, "work", &a).unwrap(),
            Answer::Now(_)
        ));
    }
    m.unload();
    assert_eq!(read_file(&root, "song"), "three");
}

/// A `serial` mutation queued behind an unawaited save, superseded before
/// its own statement is head. The round that delivers the save issues the
/// chain; that round has to finish the chain, or quit's pump never sees a
/// background head again and drops it.
#[test]
fn a_let_go_chain_behind_a_background_write_finishes() {
    let root = Root::new();
    let mut host = Host::new(&root);
    host.invoke("save", "bg");
    host.invoke("serial", "a");
    host.invoke("serial", "b");
    // Quit pumps background rounds only. Resuming the waiting answer would
    // finish the chain from `resume`'s `finish_let_go`, which quit never runs.
    let mut later = Vec::new();
    for _ in 0..10 {
        let mut background = None;
        for request in host.runner.take_requests() {
            if request.target == "background" {
                background = Some(request);
            } else {
                later.push(request);
            }
        }
        let Some(request) = background else { break };
        let token = request
            .request
            .continuation
            .expect("background continuation");
        let Dispatch::Run(Work::Now(work)) = crate::runnable(host.runner.dispatch_work(token))
        else {
            panic!("a background round runs");
        };
        let outcome = std::thread::spawn(work).join().unwrap();
        host.runner.fulfill(request.ticket, outcome).unwrap();
    }
    assert_eq!(
        host.runner.background_operations(),
        0,
        "the let-go chain is still in flight after the background rounds"
    );
    for request in later {
        let Some(token) = request.request.continuation else {
            continue;
        };
        if let Dispatch::Run(Work::Now(work)) = crate::runnable(host.runner.dispatch_work(token)) {
            let outcome = std::thread::spawn(work).join().unwrap();
            host.runner.fulfill(request.ticket, outcome).unwrap();
        }
    }
    host.settle();
    assert_eq!(host.result().as_deref(), Some("b:2"));
    assert!(!host.runner.has_pending());
}

/// A storage continuation that fails (the 30s timeout, here passed in)
/// settles the head. The answer fails, the failure is journaled, and a
/// later storage call is issued instead of waiting on that promise.
#[test]
fn a_failed_storage_continuation_releases_the_queue() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let a = args("file", "first");
    assert!(matches!(
        m.answer(&mut s, "work", &a).unwrap(),
        Answer::Later(_)
    ));
    let err = m
        .parse(
            &mut s,
            "work",
            &a,
            Outcome::Failed {
                kind: FailureKind::Aborted,
                message: "storage continuation timed out".into(),
            },
        )
        .unwrap_err();
    // A storage call's failure the module let through (LLP 1109 D3).
    let DataError::Failed(FailureCode::Storage, message) = err else {
        panic!("{err:?}");
    };
    assert!(message.contains("timed out"), "{message}");
    let logs = DataSource::take_logs(&mut m);
    assert!(
        logs.iter()
            .any(|l| l.contains("storage failed:") && l.contains("timed out")),
        "{logs:?}"
    );
    let saved = args("save", "after");
    assert!(matches!(
        m.answer(&mut s, "work", &saved).unwrap(),
        Answer::Now(_)
    ));
    // The timed-out write's task can still complete. Its delivery is ignored
    // (`op.settled`); the save, issued after the head was cleared, lands in
    // a later round.
    assert!(rounds(&mut m, &s) >= 1, "the save was never delivered");
    assert_eq!(read_file(&root, "song"), "after");
    let state = m.background_state().unwrap();
    assert_eq!(state.queued + state.in_flight, 0);
}

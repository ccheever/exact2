//! A request with effects that the runner lets go runs to its end (LLP 1146
//! D2): the runner offers it to the source when the commit ends, keeps it
//! when the source takes it, and delivers its reply to `module_landed`. A
//! refused commit keeps nothing: what it let go is in flight again.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, InFlight, Outcome, Request, Response, Runner, Store,
};
use std::sync::{Arc, Mutex};

const SOURCE: &str = r##"shape Row
  n: number
component App
  state n = 0
  state refuse = false
  state strict = false
  resource row = row(n) as shape Row
  resource bad = bad(n, refuse) as shape Row
  resource reader = reader() as shape Row
  resource held = held() as shape Row
  resource picky = picky(strict) as shape Row
  mutation wrote as shape Row
  mutation posted as shape Row
  action bump
    n = n + 1
  action post
    send posted = post()
  action refusing
    refuse = true
    send posted = post()
  action landing
    refuse = true
    send posted = now()
  action write
    send wrote = write()
  action insist
    strict = true
  view
    text `row ${row.n} ${bad.n} ${reader.n} ${held.n} ${picky.n}`
"##;

/// What the runner offered (how many, and whether its commit was refused)
/// and which tokens came back.
#[derive(Default)]
struct Seen {
    offered: Vec<(usize, bool)>,
    landed: Vec<u64>,
}

/// `row` asks with a POST, `bad` refuses its commit once `refuse` is set,
/// `reader` reads the store and asks with a GET, `held` likewise with a held
/// GET, `picky` reads it and refuses its commit once `strict` is set and a
/// module turn has written it, and `write` writes the store now; a kept
/// reply's module turn writes the store too.
struct Source {
    keeps: bool,
    seen: Arc<Mutex<Seen>>,
}

fn row(n: f64) -> Value {
    Value::record(vec![Value::Number(n)])
}

impl DataSource for Source {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            // Each argument its own request, so newer ones replace it.
            "row" => Ok(Answer::Later(Request::post_json(
                &format!("https://example.test/row/{:?}", args[0]),
                "{}",
            ))),
            "bad" if args[1] == Value::Bool(true) => Err(DataError::UnknownSource("bad".into())),
            "bad" | "now" => Ok(Answer::Now(row(0.0))),
            "post" => Ok(Answer::Later(Request::post_json(
                "https://example.test/post",
                "{}",
            ))),
            "reader" => {
                let _ = store.get("k");
                Ok(Answer::Later(Request::get("https://example.test/reader")))
            }
            "picky" => {
                let landed = store.get("k") == Some("landed");
                if args[0] == Value::Bool(true) && landed {
                    return Err(DataError::UnknownSource("picky".into()));
                }
                Ok(Answer::Now(row(0.0)))
            }
            "held" => {
                let _ = store.get("k");
                Ok(Answer::Later(
                    Request::get("https://example.test/held").held(),
                ))
            }
            "write" => {
                store
                    .set("k", "written")
                    .map_err(|e| DataError::Unavailable(format!("{e:?}")))?;
                Ok(Answer::Now(row(0.0)))
            }
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(row(1.0)))
    }

    fn grants(&self) -> &str {
        "secret.keep k"
    }

    fn keep_sent(&mut self, _: &Store, let_go: &[InFlight<'_>], refused: bool) -> Vec<Option<u64>> {
        self.seen
            .lock()
            .unwrap()
            .offered
            .push((let_go.len(), refused));
        let_go.iter().map(|_| self.keeps.then_some(77)).collect()
    }

    fn module_landed(
        &mut self,
        store: &mut Store,
        token: u64,
        _: Outcome,
    ) -> Result<(), DataError> {
        self.seen.lock().unwrap().landed.push(token);
        store
            .set("k", "landed")
            .map_err(|e| DataError::Unavailable(format!("{e:?}")))
    }
}

fn boot(keeps: bool) -> (Runner<Source>, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let source = Source {
        keeps,
        seen: seen.clone(),
    };
    let plan = contract::compile(SOURCE).unwrap();
    let mut r = Runner::boot(
        plan,
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.listen(Arc::new(|| {}));
    r.data_ready().unwrap();
    (r, seen)
}

fn ok() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: Vec::new(),
        body: b"{}".to_vec(),
    })
}

/// The first requests: `row`'s POST and `reader`'s GET, in that order.
fn first(r: &mut Runner<Source>) -> (u64, u64) {
    let out = r.take_requests();
    let ticket = |end: &str| {
        (out.iter().find(|o| o.request.url.contains(end)))
            .unwrap_or_else(|| panic!("no request to {end}: {out:?}"))
            .ticket
    };
    (ticket("/row/"), ticket("/reader"))
}

fn asks_reader(r: &mut Runner<Source>) -> bool {
    (r.take_requests().iter()).any(|o| o.request.url.ends_with("/reader"))
}

/// Newer arguments let the first POST go; the source keeps it, the host
/// must too, and its reply goes to `module_landed` under the source's token.
#[test]
fn a_let_go_post_the_source_keeps_runs_to_its_end() {
    let (mut r, seen) = boot(true);
    let (post, _) = first(&mut r);
    r.act("bump", vec![]).unwrap();
    assert!(
        seen.lock().unwrap().offered.contains(&(1, false)),
        "offered as let go"
    );
    assert!(r.holds(post), "kept, so the host goes on running it");
    assert!(r.pending().contains(&("module work".to_string(), post)));
    let before = r.journal().count();
    assert!(r.fulfill(post, ok()).unwrap().is_some());
    assert_eq!(seen.lock().unwrap().landed, [77]);
    assert!(!r.holds(post));
    let journal: Vec<&str> = r.journal().skip(before).collect();
    assert!(
        journal.iter().any(|l| l.ends_with("store k")),
        "the module turn's write is journaled: {journal:?}"
    );
}

/// A request with effects is in flight from its send until its reply lands,
/// kept ones included, so the host keeps the app running until then, as the
/// agent's `background` op tells a quit; a GET never is one.
#[test]
fn a_request_with_effects_is_in_flight_until_it_lands_kept_or_not() {
    let (mut r, _) = boot(true);
    let background =
        |r: &exact_runner::Runner<_>| exact_runner::agent::handle(r, r#"{"op":"background"}"#);
    let (kept, reader) = first(&mut r);
    assert!(r.effects_in_flight(), "the POST is out");
    assert_eq!(background(&r), r#"{"mustFinish":true}"#);
    r.act("bump", vec![]).unwrap();
    let newer = (r.take_requests().into_iter())
        .find(|o| o.request.url.contains("/row/"))
        .expect("the newer POST")
        .ticket;
    r.fulfill(newer, ok()).unwrap();
    assert!(r.effects_in_flight(), "the kept POST is still out");
    r.fulfill(kept, ok()).unwrap();
    assert!(r.holds(reader), "the GET is still out");
    assert!(!r.effects_in_flight(), "a GET has no effects");
    assert_eq!(background(&r), r#"{"mustFinish":false}"#);
}

/// A source that keeps nothing (a Rust data source's default): the host
/// cancels the let-go POST, as before.
#[test]
fn a_let_go_post_the_source_does_not_keep_is_cancelled() {
    let (mut r, seen) = boot(false);
    let (post, _) = first(&mut r);
    r.act("bump", vec![]).unwrap();
    assert!(
        seen.lock().unwrap().offered.contains(&(1, false)),
        "offered as let go"
    );
    assert!(!r.holds(post));
    assert!(
        r.fulfill(post, ok()).unwrap().is_none(),
        "its reply is dropped"
    );
    assert!(seen.lock().unwrap().landed.is_empty());
}

/// A POST the host refused at admission never ran: let go, nothing of it is
/// offered, so nothing waits for a reply that will not come.
#[test]
fn a_let_go_post_the_host_refused_is_not_offered() {
    let (mut r, seen) = boot(true);
    let (post, _) = first(&mut r);
    r.refuse_request(post, "native executor admission limit reached", false);
    assert!(
        !r.effects_in_flight(),
        "it never ran, so nothing waits for it"
    );
    r.act("bump", vec![]).unwrap();
    assert!(!seen.lock().unwrap().offered.contains(&(1, false)));
    assert!(!r.holds(post));
}

/// A refused commit tells the source, and the POST in flight before it stays
/// in flight. (The refusal comes before the newer send would let that POST go.)
#[test]
fn a_refused_commit_tells_the_source_and_keeps_its_post_in_flight() {
    let (mut r, seen) = boot(true);
    let _ = r.take_requests();
    r.act("post", vec![]).unwrap();
    let post = r.take_requests()[0].ticket;
    assert!(r.act("refusing", vec![]).is_err());
    let offered = seen.lock().unwrap().offered.clone();
    assert!(
        offered.contains(&(0, true)),
        "told it was refused: {offered:?}"
    );
    assert!(r.pending().contains(&("posted".to_string(), post)));
    assert!(r.holds(post));
}

/// A send answered now lets the POST in flight go before settlement refuses
/// the commit: the let-go POST is not offered, and it is in flight again.
#[test]
fn a_refused_commit_keeps_nothing_it_let_go() {
    let (mut r, seen) = boot(true);
    let _ = r.take_requests();
    r.act("post", vec![]).unwrap();
    let post = r.take_requests()[0].ticket;
    assert!(r.act("landing", vec![]).is_err());
    let offered = seen.lock().unwrap().offered.clone();
    assert_eq!(offered.last(), Some(&(0, true)), "{offered:?}");
    assert!(r.pending().contains(&("posted".to_string(), post)));
    assert!(r.holds(post));
}

/// Kept requests the host refuses at admission settle as module work lands,
/// one a pump, each once, and the second still waiting asks for its own pump
/// (LLP 1146 §2.1): each continuation hears its failure, and the runner holds
/// the ticket no more.
#[test]
fn kept_requests_the_host_refuses_together_each_settle_once() {
    let (mut r, seen) = boot(true);
    let (first_post, _) = first(&mut r);
    r.act("bump", vec![]).unwrap();
    let second_post = (r.take_requests().into_iter())
        .find(|o| o.request.url.contains("/row/"))
        .expect("the second POST")
        .ticket;
    r.act("bump", vec![]).unwrap();
    let _ = r.take_requests();
    assert!(r.holds(first_post) && r.holds(second_post), "both kept");
    for ticket in [first_post, second_post] {
        r.refuse_request(ticket, "native executor admission limit reached", false);
    }
    for ticket in [first_post, second_post] {
        assert!(r.has_request_refusals(false), "a pump for each");
        let (taken, outcome) = r.take_request_refusal(false).expect("a refusal");
        assert_eq!(taken, ticket);
        r.fulfill(taken, outcome).unwrap();
    }
    assert!(!r.has_request_refusals(false));
    assert_eq!(seen.lock().unwrap().landed, [77, 77]);
    assert!(!r.holds(first_post) && !r.holds(second_post));
}

/// An ordered kept request the host refuses fences later ordered work and
/// settles only once earlier ordered work has (LLP 1041 §8.4), then once.
#[test]
fn an_ordered_kept_request_the_host_refuses_waits_behind_ordered_work() {
    let (mut r, seen) = boot(true);
    let (post, _) = first(&mut r);
    r.act("bump", vec![]).unwrap();
    let _ = r.take_requests();
    r.refuse_request(post, "native executor admission limit reached", true);
    assert!(r.has_ordered_request_refusals(), "later ordered work waits");
    assert!(!r.has_request_refusals(false));
    assert!(
        r.take_request_refusal(false).is_none(),
        "earlier ordered work first"
    );
    let (taken, outcome) = r.take_request_refusal(true).expect("once it allows");
    assert_eq!(taken, post);
    r.fulfill(taken, outcome).unwrap();
    assert_eq!(seen.lock().unwrap().landed, [77]);
    assert!(!r.holds(post) && !r.has_ordered_request_refusals());
    assert!(r.take_request_refusal(true).is_none());
}

/// A refused commit after a module turn leaves the turn's writes standing:
/// they were in the store before the commit's checkpoint, as a browser's
/// continuation writes outside every commit, and they are journaled.
#[test]
fn a_refused_commit_after_a_module_turn_keeps_its_writes() {
    let (mut r, seen) = boot(true);
    let (post, _) = first(&mut r);
    r.act("bump", vec![]).unwrap();
    r.act("insist", vec![]).unwrap();
    let _ = r.take_requests();
    assert!(r.fulfill(post, ok()).is_err(), "`picky` refuses the commit");
    assert_eq!(seen.lock().unwrap().landed, [77]);
    assert_eq!(r.store().get("k"), Some("landed"));
    let journal: Vec<&str> = r.journal().collect();
    assert!(
        journal.iter().any(|l| l.ends_with("store k")),
        "{journal:#?}"
    );
}

/// A kept reply's module turn writes the store: a reader whose request may
/// never land (`held`) is asked again at once, and one in flight that will
/// land (`reader`) waits for its reply (LLP 1146 §2.3).
#[test]
fn a_held_store_reader_is_asked_again_at_once_after_module_work() {
    let (mut r, _) = boot(true);
    let (post, _) = first(&mut r);
    r.act("bump", vec![]).unwrap();
    let _ = r.take_requests();
    r.fulfill(post, ok()).unwrap();
    let out = r.take_requests();
    assert!(
        out.iter().any(|o| o.request.url.ends_with("/held")),
        "asked again at once: {out:?}"
    );
    assert!(
        !out.iter().any(|o| o.request.url.ends_with("/reader")),
        "waits for its reply"
    );
}

/// A kept reply's module turn writes the store: `reader`, in flight, waits
/// for its reply. An ordinary write after it asks `reader` again at once,
/// as any other write does (LLP 1146 §2.3).
#[test]
fn module_work_s_wait_on_a_reader_lapses_at_the_next_write() {
    let (mut r, _) = boot(true);
    let (post, _) = first(&mut r);
    r.act("bump", vec![]).unwrap();
    let _ = r.take_requests();
    r.fulfill(post, ok()).unwrap();
    assert!(!asks_reader(&mut r), "it waits for its reply");
    r.act("write", vec![]).unwrap();
    assert!(asks_reader(&mut r), "asked again at once");
}

const PASSES: &str = r##"shape Row
  n: number
component App
  state go = false
  resource reader = reader(go) as shape Row
  resource writer = writer(go) as shape Row
  action start
    go = true
  view
    text `${reader.n} ${writer.n}`
"##;

/// Once `go` is set, `reader` asks with a GET until the store holds `k`,
/// which `writer`, asked after it, writes; what the source hears is still in
/// flight is recorded.
struct Passes {
    forgotten: Arc<Mutex<Vec<Vec<String>>>>,
    /// How many times it was told what it may keep (`keep_sent`).
    offered: Arc<Mutex<usize>>,
}

impl DataSource for Passes {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let go = args[0] == Value::Bool(true);
        match source {
            "reader" if !go || store.get("k").is_some() => Ok(Answer::Now(row(1.0))),
            "reader" => Ok(Answer::Later(Request::get("https://example.test/reader"))),
            "writer" if !go => Ok(Answer::Now(row(0.0))),
            "writer" => {
                store
                    .set("k", "written")
                    .map_err(|e| DataError::Unavailable(format!("{e:?}")))?;
                Ok(Answer::Now(row(2.0)))
            }
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(row(1.0)))
    }

    fn grants(&self) -> &str {
        "secret.keep k"
    }

    fn forgotten(&mut self, _: &Store, in_flight: &[InFlight<'_>]) {
        let sources = in_flight.iter().map(|f| f.source.to_string()).collect();
        self.forgotten.lock().unwrap().push(sources);
    }

    fn keep_sent(&mut self, _: &Store, let_go: &[InFlight<'_>], _: bool) -> Vec<Option<u64>> {
        *self.offered.lock().unwrap() += 1;
        vec![None; let_go.len()]
    }
}

/// A later pass of `start`'s commit answers `reader` now, so the GET an
/// earlier pass made is dropped before it is handed out: the source hears
/// that nothing of it is in flight, and drops what it parked for it.
#[test]
fn a_plain_request_a_later_pass_drops_is_forgotten() {
    let forgotten = Arc::new(Mutex::new(Vec::new()));
    let source = Passes {
        forgotten: forgotten.clone(),
        offered: Default::default(),
    };
    let plan = contract::compile(PASSES).unwrap();
    let mut r = Runner::boot(
        plan,
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.listen(Arc::new(|| {}));
    r.data_ready().unwrap();
    let _ = r.take_requests();
    r.act("start", vec![]).unwrap();
    assert!(r.take_requests().is_empty(), "the GET is never handed out");
    let heard = forgotten.lock().unwrap().clone();
    assert!(
        heard
            .last()
            .is_some_and(|f| !f.iter().any(|s| s == "reader")),
        "{heard:?}"
    );
}

/// The same at boot: with `go` already set, a later pass of the boot's
/// settlement drops the GET an earlier pass made, and the source hears so
/// before any commit.
#[test]
fn a_plain_request_the_boot_drops_is_forgotten() {
    let forgotten = Arc::new(Mutex::new(Vec::new()));
    let offered = Arc::new(Mutex::new(0));
    let source = Passes {
        forgotten: forgotten.clone(),
        offered: offered.clone(),
    };
    let plan = contract::compile(&PASSES.replace("state go = false", "state go = true")).unwrap();
    let mut r = Runner::boot(
        plan,
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(r.take_requests().is_empty(), "the GET is never handed out");
    let heard = forgotten.lock().unwrap().clone();
    assert!(
        heard
            .last()
            .is_some_and(|f| !f.iter().any(|s| s == "reader")),
        "{heard:?}"
    );
    // And it is told what it may keep, as at a commit's end, so a call it set aside is not left waiting.
    assert_eq!(*offered.lock().unwrap(), 1);
}

use crate::Mixed;
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Request, Store};
use serde_json::{json, Value as Json};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
struct Source {
    id: String,
    grants: String,
    revision: String,
    state: u8,
    ready: bool,
    calls: Rc<RefCell<Vec<String>>>,
}

impl Source {
    fn new(label: &str) -> Self {
        Self {
            id: "com.exact.test".into(),
            grants: format!("secret.keep {label}\nsecret.keep shared"),
            revision: label.into(),
            state: 1,
            ready: true,
            calls: Default::default(),
        }
    }
    fn record(&self, value: impl Into<String>) {
        self.calls.borrow_mut().push(value.into());
    }
}

impl DataSource for Source {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        self.record(format!("query:{source}"));
        assert!(self.ready);
        Ok(Value::Number(self.state.into()))
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.record(format!("answer:{source}"));
        if source == "unknownOwned" {
            return Err(DataError::UnknownSource(source.into()));
        }
        if source.contains("wait") {
            return Ok(Answer::Later(Request::continuation(u64::MAX)));
        }
        if source.contains("request") {
            let mut request = Request::get("https://example.test");
            if source == "nestedrequest" {
                request.grants = Some("secret.keep rust".into());
            }
            return Ok(Answer::Later(request));
        }
        if source.contains("secret") {
            let own = self.revision.clone();
            let other = if own == "js" { "rust" } else { "js" };
            assert!(store.get(other).is_none());
            assert!(store.set(other, "stolen").is_err());
            assert!(!store.names().contains(&other));
            assert!(!store
                .snapshot()
                .iter()
                .any(|(name, _)| name == other || name.starts_with(Store::KEPT)));
            store.set(&own, "written")?;
            return Ok(Answer::Now(Value::str(store.get("shared").unwrap_or(""))));
        }
        self.query(source, args).map(Answer::Now)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        self.record(format!("parse:{source}"));
        self.answer(store, source, args)
    }
    fn app_id(&self) -> &str {
        &self.id
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn revision(&self) -> Option<&str> {
        Some(&self.revision)
    }
    fn ready(&self) -> bool {
        self.ready
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.record("activate");
        self.ready = true;
        Ok(())
    }
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.record("validate");
        self.activate()
    }
    fn bind(&mut self, _: &Plan) {
        self.record("bind");
    }
    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        self.record(format!("storage:{data:?}:{cache:?}:{temporary:?}"));
        Ok(())
    }
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let message = format!("{}:{token}", self.revision);
        Some(Box::new(move || Outcome::Failed {
            kind: FailureKind::Aborted,
            message,
        }))
    }
    fn replacement(&self, _: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        let metadata: Json = serde_json::from_str(receipt).unwrap();
        if module.len() != 1 || module[0] == 255 {
            return Err(DataError::Unavailable("rejected child".into()));
        }
        let mut next = self.clone();
        next.state = module[0];
        next.revision = format!("{}:{}", self.revision, module[0]);
        next.ready = false;
        if metadata["identity"] == true {
            next.id = "com.exact.other".into();
        }
        if metadata["grants"] == true {
            next.grants.push_str("\nsecret.keep stolen");
        }
        Ok(next)
    }
}

fn pair() -> Mixed<Source, Source> {
    Mixed::new(
        Source::new("js"),
        Source::new("rust"),
        &[
            "js",
            "jswait",
            "jssecret",
            "jsrequest",
            "nestedrequest",
            "unknownOwned",
        ],
        &["rust", "rustwait", "rustsecret", "rustrequest"],
    )
    .unwrap()
}
fn receipt() -> Json {
    json!({"version":1,"kind":"mixed","javascriptBytes":1,"javascript":{"kind":"javascript"},"rust":{"kind":"rust"}})
}
fn failed() -> Outcome {
    Outcome::Failed {
        kind: FailureKind::Aborted,
        message: "test".into(),
    }
}
fn token(answer: Answer) -> u64 {
    match answer {
        Answer::Later(request) => request.continuation.unwrap(),
        _ => panic!("expected continuation"),
    }
}

#[test]
fn ownership_is_validated_without_probing_and_never_falls_back() {
    let js = Source::new("js");
    let rust = Source::new("rust");
    for (left, right) in [
        (vec!["same"], vec!["same"]),
        (vec!["x", "x"], vec![]),
        (vec![""], vec![]),
    ] {
        assert!(Mixed::new(js.clone(), rust.clone(), &left, &right).is_err());
    }
    let mut wrong = rust.clone();
    wrong.id.clear();
    assert!(Mixed::new(js.clone(), wrong, &["js"], &["rust"]).is_err());
    assert!(js.calls.borrow().is_empty() && rust.calls.borrow().is_empty());
    let mut mixed = Mixed::new(js.clone(), rust.clone(), &["unknownOwned"], &["rust"]).unwrap();
    assert!(mixed
        .answer(&mut Store::default(), "unknownOwned", &[])
        .is_err());
    assert!(mixed.query("absent", &[]).is_err());
    assert!(rust.calls.borrow().is_empty());
    assert_eq!(js.calls.borrow().as_slice(), &["answer:unknownOwned"]);
}

#[test]
fn an_unchanged_grant_ceiling_preserves_javascript_receipt_spelling() {
    let mut javascript = Source::new("js");
    javascript.grants = "secret.keep shared\nsecret.keep js\n".into();
    let mut rust = Source::new("rust");
    rust.grants = "secret.keep js\nsecret.keep shared".into();
    let mixed = Mixed::new(javascript.clone(), rust, &["js"], &["rust"]).unwrap();
    assert_eq!(mixed.grants(), javascript.grants);
}

#[test]
fn both_sides_get_storage_and_parse_uses_the_declared_owner() {
    let js = Source::new("js");
    let rust = Source::new("rust");
    let mut mixed = Mixed::new(js.clone(), rust.clone(), &["js"], &["rust"]).unwrap();
    mixed
        .configure_storage("data".into(), "cache".into(), "temporary".into())
        .unwrap();
    for source in ["js", "rust"] {
        mixed
            .parse(&mut Store::default(), source, &[], failed())
            .unwrap();
    }
    assert_eq!(js.calls.borrow()[0], rust.calls.borrow()[0]);
    assert!(js.calls.borrow().contains(&"parse:js".into()));
    assert!(rust.calls.borrow().contains(&"parse:rust".into()));
}

#[test]
fn colliding_full_width_continuations_route_once_to_their_owner() {
    let mut mixed = pair();
    let mut store = Store::default();
    let js = token(mixed.answer(&mut store, "jswait", &[]).unwrap());
    let rust = token(mixed.answer(&mut store, "rustwait", &[]).unwrap());
    assert_ne!(js, rust);
    assert_eq!(
        mixed.continuation(rust).unwrap()(),
        Outcome::Failed {
            kind: FailureKind::Aborted,
            message: format!("rust:{}", u64::MAX)
        }
    );
    assert_eq!(
        mixed.continuation(js).unwrap()(),
        Outcome::Failed {
            kind: FailureKind::Aborted,
            message: format!("js:{}", u64::MAX)
        }
    );
    assert!(mixed.continuation(js).is_none());
    // A composed token routes once at dispatch, then is gone.
    let browser = token(mixed.answer(&mut store, "jswait", &[]).unwrap());
    assert!(matches!(mixed.dispatch(browser, &store), Dispatch::Run(_)));
    assert!(matches!(mixed.dispatch(browser, &store), Dispatch::Missing));
}

#[test]
fn secret_union_preserves_per_side_restrictions_and_shared_values() {
    let mut mixed = pair();
    assert_eq!(
        mixed.grants(),
        "secret.keep js\nsecret.keep rust\nsecret.keep shared"
    );
    let mut store = Store::new(
        mixed.grants(),
        [
            ("js".into(), "old js".into()),
            ("rust".into(), "old rust".into()),
            ("shared".into(), "both".into()),
            ("exact.kept.private".into(), "runner".into()),
        ],
    );
    for source in ["jssecret", "rustsecret"] {
        assert_eq!(
            mixed.answer(&mut store, source, &[]).unwrap(),
            Answer::Now(Value::str("both"))
        );
        mixed.parse(&mut store, source, &[], failed()).unwrap();
    }
    assert_eq!(store.get("js"), Some("written"));
    assert_eq!(store.get("rust"), Some("written"));
    assert_eq!(store.take_writes().len(), 4);
    for source in ["jsrequest", "rustrequest"] {
        let Answer::Later(request) = mixed.answer(&mut store, source, &[]).unwrap() else {
            panic!()
        };
        assert!(!request.grants.unwrap().contains(if source == "jsrequest" {
            "secret.keep rust"
        } else {
            "secret.keep js"
        }));
    }
    assert!(mixed.answer(&mut store, "nestedrequest", &[]).is_err());
}

#[test]
fn replacement_is_paired_and_refusal_leaves_current_state_usable() {
    let mut mixed = pair();
    let original = mixed.revision().unwrap().to_owned();
    for boundary in [
        json!(-1),
        json!(0),
        json!(1.5),
        json!(2),
        json!(3),
        Json::Null,
    ] {
        let mut metadata = receipt();
        metadata["javascriptBytes"] = boundary;
        assert!(mixed
            .replacement(&[1], &metadata.to_string(), vec![2, 3])
            .is_err());
    }
    for bytes in [vec![255, 3], vec![2, 255], vec![]] {
        assert!(mixed
            .replacement(&[1], &receipt().to_string(), bytes)
            .is_err());
    }
    for metadata in [
        json!({"kind":"javascript"}),
        json!({"kind":"rust"}),
        json!({"version":2,"kind":"mixed"}),
    ] {
        assert!(mixed
            .replacement(&[1], &metadata.to_string(), vec![2, 3])
            .is_err());
    }
    for side in ["javascript", "rust"] {
        for changed in ["identity", "grants"] {
            let mut metadata = receipt();
            metadata[side][changed] = true.into();
            assert!(mixed
                .replacement(&[1], &metadata.to_string(), vec![2, 3])
                .is_err());
        }
    }
    assert_eq!(mixed.revision(), Some(original.as_str()));
    assert_eq!(mixed.query("rust", &[]).unwrap(), Value::Number(1.0));
    let mut next = mixed
        .replacement(&[1], &receipt().to_string(), vec![2, 3])
        .unwrap();
    assert!(!next.ready());
    next.activate_for_validation().unwrap();
    assert_eq!(next.query("js", &[]).unwrap(), Value::Number(2.0));
    assert_eq!(next.query("rust", &[]).unwrap(), Value::Number(3.0));
}

#[test]
fn javascript_only_update_explicitly_preserves_mutated_embedded_state() {
    let mut rust = Source::new("rust");
    rust.state = 73;
    let current = Mixed::new(Source::new("js"), rust, &["js"], &["rust"])
        .unwrap()
        .with_embedded_rust(|source| Ok(source.clone()));
    assert!(current
        .replacement(&[1], &receipt().to_string(), vec![2, 3])
        .is_err());
    assert!(current
        .replacement(&[1], r#"{"kind":"rust"}"#, vec![2])
        .is_err());
    let mut next = current
        .replacement(&[1], r#"{"kind":"javascript"}"#, vec![2])
        .unwrap();
    next.activate().unwrap();
    assert_eq!(next.query("rust", &[]).unwrap(), Value::Number(73.0));
}

#[test]
fn moving_an_operation_changes_owner_without_changing_the_shared_store() {
    let js = Source::new("js");
    let rust = Source::new("rust");
    let mut before = Mixed::new(js.clone(), rust.clone(), &["secret"], &[]).unwrap();
    let mut store = Store::new(before.grants(), [("shared".into(), "durable".into())]);
    let value = before.answer(&mut store, "secret", &[]).unwrap();
    let mut after = Mixed::new(js.clone(), rust.clone(), &[], &["secret"]).unwrap();
    assert_eq!(after.answer(&mut store, "secret", &[]).unwrap(), value);
    assert_eq!(store.get("shared"), Some("durable"));
    assert_eq!(js.calls.borrow().as_slice(), &["answer:secret"]);
    assert_eq!(rust.calls.borrow().as_slice(), &["answer:secret"]);
}

#[test]
fn secret_scope_restores_after_error_unwind_and_cannot_broaden_parent() {
    let mut store = Store::new(
        "secret.keep a\nsecret.keep b",
        [("a".into(), "A".into()), ("b".into(), "B".into())],
    );
    store.set("b", "pending B").unwrap();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.with_grants("secret.keep a", |scope| {
            assert!(scope.take_writes().is_empty());
            scope.with_grants("secret.keep b", |inner| assert!(inner.get("b").is_none()));
            panic!("child failed");
        })
    }));
    assert_eq!(store.get("b"), Some("pending B"));
    assert_eq!(store.take_writes().len(), 1);
    assert_eq!(store.granted(), &["a", "b"]);
}

// --- LLP 1027.002 D3: the ordered set ------------------------------------------

use crate::{envelope, Placed};
use exact_runner::{Dispatch, Placement, Reply, Work};
use std::sync::{Arc, Mutex};

/// A worker child's proxy, without a thread: it records at `answer`, and at
/// `dispatch` runs the call against a scoped snapshot and answers at once
/// with the turn's envelope — the shape `Placed` and the TypeScript worker
/// have, made deterministic.
struct Proxy {
    inner: Source,
    recorded: Vec<(u64, String, Vec<Value>)>,
    next: u64,
}

impl Proxy {
    fn new(inner: Source) -> Self {
        Self {
            inner,
            recorded: Vec::new(),
            next: 100,
        }
    }
}

impl DataSource for Proxy {
    fn placement(&self) -> Placement {
        Placement::Worker
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.inner.query(source, args)
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        self.next += 1;
        self.recorded
            .push((self.next, source.to_string(), args.to_vec()));
        Ok(Answer::Later(Request::continuation(self.next)))
    }
    fn parse(
        &mut self,
        store: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        envelope::apply(outcome, store, &mut Vec::new())
    }
    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        let Some(at) = self.recorded.iter().position(|(t, _, _)| *t == token) else {
            return Dispatch::Missing;
        };
        let (_, source, args) = self.recorded.remove(at);
        let grants = self.inner.grants().to_owned();
        let mut local = Store::new(&grants, envelope::snapshot(store, &grants));
        let result = self.inner.answer(&mut local, &source, &args);
        let outcome = envelope::encode(result, &mut local, Vec::new());
        Dispatch::Run(Work::Now(Box::new(move || outcome)))
    }
    fn app_id(&self) -> &str {
        self.inner.app_id()
    }
    fn grants(&self) -> &str {
        self.inner.grants()
    }
    fn revision(&self) -> Option<&str> {
        self.inner.revision()
    }
}

fn run(dispatch: Dispatch) -> Outcome {
    match dispatch {
        Dispatch::Run(Work::Now(work)) => work(),
        Dispatch::Run(Work::Later(work)) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |outcome| {
                let _ = tx.send(outcome);
            }));
            rx.recv_timeout(std::time::Duration::from_secs(5))
                .expect("a reply")
        }
        Dispatch::Held => panic!("held"),
        Dispatch::Host(_) => panic!("host"),
        Dispatch::Missing => panic!("missing"),
    }
}

fn value_of(answer: Result<Answer, DataError>) -> Value {
    match answer.unwrap() {
        Answer::Now(v) => v,
        _ => panic!("a value"),
    }
}

#[test]
fn a_worker_child_orders_every_member_that_shares_a_secret_and_commits_through_parse() {
    // Both children keep `shared`: one set, both members, one turn at a time.
    let js = Source::new("js");
    let rust = Source::new("rust");
    let mut mixed = Mixed::new(
        Proxy::new(js.clone()),
        rust.clone(),
        &["jssecret", "js"],
        &["rustsecret", "rust"],
    )
    .unwrap();
    assert_eq!(mixed.placement(), Placement::Worker);
    let mut store = Store::new(
        "secret.keep js\nsecret.keep rust\nsecret.keep shared",
        [("shared".to_string(), "before".to_string())],
    );
    // Nothing ran at `answer`: both calls are recorded, both answer later.
    let a = token(mixed.answer(&mut store, "jssecret", &[]).unwrap());
    let b = token(mixed.answer(&mut store, "rustsecret", &[]).unwrap());
    assert!(js.calls.borrow().is_empty());
    assert!(rust.calls.borrow().is_empty());
    // The first turn is dispatched; the second is held behind it.
    let first = mixed.dispatch(a, &store);
    assert!(matches!(mixed.dispatch(b, &store), Dispatch::Held));
    assert!(mixed.release(&store).is_empty());
    let outcome = run(first);
    assert!(envelope::is_turn(&outcome));
    // The worker child ran against a scoped snapshot; its write lands in
    // the live store only now, through `parse`, inside the transaction.
    assert_eq!(store.get("js"), None);
    let v = value_of(mixed.parse(&mut store, "jssecret", &[], outcome));
    assert_eq!(v.as_str(), Some("before"));
    assert_eq!(store.get("js"), Some("written"));
    // The turn ended: the held call is released, computed on `main`
    // against the store as committed now, and commits the same way.
    store.set("shared", "after").unwrap();
    let mut released = mixed.release(&store);
    assert_eq!(released.len(), 1);
    let (t, dispatch) = released.pop().unwrap();
    assert_eq!(t, b);
    assert_eq!(store.get("rust"), None);
    let outcome = run(dispatch);
    let v = value_of(mixed.parse(&mut store, "rustsecret", &[], outcome));
    assert_eq!(v.as_str(), Some("after"));
    assert_eq!(store.get("rust"), Some("written"));
    assert!(mixed.release(&store).is_empty());
    assert_eq!(rust.calls.borrow().as_slice(), ["answer:rustsecret"]);
}

#[test]
fn disjoint_secrets_leave_a_main_child_inline_and_a_refusal_discards_the_recorded_call() {
    let mut js = Source::new("js");
    js.grants = "secret.keep js".into();
    let mut rust = Source::new("rust");
    rust.grants = "secret.keep rust".into();
    let mut mixed = Mixed::new(Proxy::new(js), rust.clone(), &["js"], &["rust"]).unwrap();
    let mut store = Store::new("secret.keep js\nsecret.keep rust", []);
    // The main child shares no name with the worker: it answers now, inline.
    assert!(matches!(
        mixed.answer(&mut store, "rust", &[]).unwrap(),
        Answer::Now(_)
    ));
    // A recorded worker call whose transaction is refused is discarded: its
    // token dispatches nothing and nothing waits for it.
    let a = token(mixed.answer(&mut store, "js", &[]).unwrap());
    mixed.discard(a);
    assert!(matches!(mixed.dispatch(a, &store), Dispatch::Missing));
    assert!(mixed.release(&store).is_empty());
    let b = token(mixed.answer(&mut store, "js", &[]).unwrap());
    let outcome = run(mixed.dispatch(b, &store));
    assert!(matches!(
        mixed.parse(&mut store, "js", &[], outcome).unwrap(),
        Answer::Now(_)
    ));
}

/// A `Send` source for the real owner thread: reads `shared`, writes its
/// own name, and yields once through a storage request when asked to.
#[derive(Clone)]
struct Threaded {
    calls: Arc<Mutex<Vec<String>>>,
}

impl DataSource for Threaded {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Number(0.0))
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.calls.lock().unwrap().push(format!(
            "answer:{source}:{}",
            std::thread::current().name().unwrap_or("?")
        ));
        store.set("rust", "written")?;
        if source == "yield" {
            return Ok(Answer::Later(Request::storage(b"payload".to_vec())));
        }
        if source == "fail" {
            return Err(DataError::BadArguments("n".into()));
        }
        let seen = args
            .first()
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(Answer::Now(Value::str(&format!(
            "{}+{seen}",
            store.get("shared").unwrap_or("")
        ))))
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.calls.lock().unwrap().push(format!("parse:{source}"));
        let text = match outcome {
            Outcome::Storage(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            other => format!("{other:?}"),
        };
        store.set("rust", &format!("resumed:{text}"))?;
        Ok(Answer::Now(Value::str(&text)))
    }
    fn app_id(&self) -> &str {
        "com.exact.test"
    }
    fn grants(&self) -> &str {
        "secret.keep rust\nsecret.keep shared"
    }
}

#[test]
fn a_placed_source_runs_its_turns_on_its_own_thread_and_the_runner_commits_them() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut placed = Placed::new(
        Threaded {
            calls: calls.clone(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    assert_eq!(placed.placement(), Placement::Worker);
    let mut store = Store::new(
        "secret.keep rust\nsecret.keep shared",
        [("shared".to_string(), "s".to_string())],
    );
    let arg = Value::str("arg");
    let t = token(
        placed
            .answer(&mut store, "now", std::slice::from_ref(&arg))
            .unwrap(),
    );
    let outcome = run(placed.dispatch(t, &store));
    assert!(calls.lock().unwrap()[0].starts_with("answer:now:exact-owner"));
    assert_eq!(store.get("rust"), None);
    let v = value_of(placed.parse(&mut store, "now", std::slice::from_ref(&arg), outcome));
    assert_eq!(v.as_str(), Some("s+arg"));
    assert_eq!(store.get("rust"), Some("written"));

    // A yield: the turn's writes commit, the request goes to the host, and
    // the outcome resumes on the owner as a new turn with a fresh snapshot.
    let t = token(placed.answer(&mut store, "yield", &[]).unwrap());
    let outcome = run(placed.dispatch(t, &store));
    let request = match placed.parse(&mut store, "yield", &[], outcome).unwrap() {
        Answer::Later(request) => request,
        _ => panic!("a yield"),
    };
    assert_eq!(request.storage.as_deref(), Some(&b"payload"[..]));
    assert_eq!(placed.staged_keys(), 1);
    let t = token(
        placed
            .parse(&mut store, "yield", &[], Outcome::Storage(b"done".to_vec()))
            .unwrap(),
    );
    let outcome = run(placed.dispatch(t, &store));
    let v = value_of(placed.parse(&mut store, "yield", &[], outcome));
    assert_eq!(v.as_str(), Some("done"));
    assert_eq!(placed.staged_keys(), 0);
    assert_eq!(store.get("rust"), Some("resumed:done"));

    // An error keeps its kind; a dropped owner reports `Aborted` as data.
    let t = token(placed.answer(&mut store, "fail", &[]).unwrap());
    let outcome = run(placed.dispatch(t, &store));
    assert_eq!(
        placed.parse(&mut store, "fail", &[], outcome).unwrap_err(),
        DataError::BadArguments("n".into())
    );
    // Work already dispatched outlives its proxy: the owner finishes the
    // turn it holds, and the reply lands on a ticket nobody holds.
    let t = token(placed.answer(&mut store, "now", &[]).unwrap());
    let dispatch = placed.dispatch(t, &store);
    drop(placed);
    assert!(envelope::is_turn(&run(dispatch)));
    // A reply dropped unsent — an owner that died mid-turn — is `Aborted`.
    let (tx, rx) = std::sync::mpsc::channel();
    drop(Reply::new(move |outcome| {
        let _ = tx.send(outcome);
    }));
    assert!(matches!(
        rx.recv().unwrap(),
        Outcome::Failed {
            kind: FailureKind::Aborted,
            ..
        }
    ));
}

#[test]
fn a_built_worker_keeps_its_template_here_and_builds_its_instance_on_the_owner() {
    fn build(template: &Threaded) -> crate::placed::Obtain<Threaded> {
        let calls = template.calls.clone();
        Box::new(move || {
            calls.lock().unwrap().push(format!(
                "built:{}",
                std::thread::current().name().unwrap_or("?")
            ));
            Ok(Threaded { calls })
        })
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut placed = Placed::built(
        Threaded {
            calls: calls.clone(),
        },
        Placement::Worker,
        build,
    );
    placed.activate().unwrap();
    let mut store = Store::new("secret.keep rust\nsecret.keep shared", []);
    let t = token(placed.answer(&mut store, "now", &[]).unwrap());
    let outcome = run(placed.dispatch(t, &store));
    let v = value_of(placed.parse(&mut store, "now", &[], outcome));
    assert_eq!(v.as_str(), Some("+"));
    assert_eq!(calls.lock().unwrap()[0], "built:exact-owner");
    // The template stays: replacement and validation go through it.
    assert!(placed.query("x", &[]).is_ok());
    placed.activate_for_validation().unwrap();
    assert_eq!(placed.placement(), Placement::Main);
    assert!(matches!(
        placed.answer(&mut store, "now", &[]).unwrap(),
        Answer::Now(_)
    ));
}

#[test]
fn placed_terminal_turns_release_argument_keys_and_keep_identical_calls() {
    let mut placed = Placed::new(
        Threaded {
            calls: Default::default(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut store = Store::new("secret.keep rust\nsecret.keep shared", []);
    // Changing editor contents produces distinct, potentially large argument keys.
    for revision in 0..32 {
        let args = [Value::str(&format!(
            "{revision}:{}",
            "document".repeat(512)
        ))];
        for source in ["now", "fail", "discard", "malformed"] {
            let t = token(placed.answer(&mut store, source, &args).unwrap());
            assert_eq!(placed.staged_keys(), 1);
            if source == "discard" {
                placed.discard(t);
                assert!(matches!(placed.dispatch(t, &store), Dispatch::Missing));
            } else {
                let outcome = run(placed.dispatch(t, &store));
                let outcome = if source == "malformed" {
                    failed()
                } else {
                    outcome
                };
                let result = placed.parse(&mut store, source, &args, outcome);
                assert_eq!(result.is_ok(), source == "now");
            }
            assert_eq!(placed.staged_keys(), 0, "retained {source} argument key");
        }
    }
    let args = [Value::str("same")];
    let first = token(placed.answer(&mut store, "now", &args).unwrap());
    let second = token(placed.answer(&mut store, "now", &args).unwrap());
    let dropped = token(placed.answer(&mut store, "now", &args).unwrap());
    placed.discard(dropped);
    assert_eq!(placed.staged_keys(), 1);
    let outcome = run(placed.dispatch(first, &store));
    assert_eq!(
        value_of(placed.parse(&mut store, "now", &args, outcome)).as_str(),
        Some("+same")
    );
    assert_eq!(placed.staged_keys(), 1);
    let outcome = run(placed.dispatch(second, &store));
    assert_eq!(
        value_of(placed.parse(&mut store, "now", &args, outcome)).as_str(),
        Some("+same")
    );
    assert_eq!(placed.staged_keys(), 0);
}

#[test]
fn mixed_terminal_turns_release_argument_keys_and_keep_identical_calls() {
    let mut mixed = Mixed::new(
        Proxy::new(Source::new("js")),
        Source::new("rust"),
        &["js", "unknownOwned", "malformed", "discard"],
        &["rust"],
    )
    .unwrap();
    let mut store = Store::new("secret.keep js\nsecret.keep rust\nsecret.keep shared", []);
    for revision in 0..32 {
        let args = [Value::str(&format!(
            "{revision}:{}",
            "document".repeat(512)
        ))];
        for source in ["js", "rust", "unknownOwned", "discard", "malformed"] {
            let t = token(mixed.answer(&mut store, source, &args).unwrap());
            assert_eq!(mixed.staged_keys(), 1);
            if source == "discard" {
                mixed.discard(t);
                assert!(matches!(mixed.dispatch(t, &store), Dispatch::Missing));
            } else {
                let outcome = run(mixed.dispatch(t, &store));
                let outcome = if source == "malformed" {
                    failed()
                } else {
                    outcome
                };
                let result = mixed.parse(&mut store, source, &args, outcome);
                assert_eq!(result.is_ok(), matches!(source, "js" | "rust"));
            }
            assert_eq!(mixed.staged_keys(), 0, "retained {source} argument key");
        }
    }
    let args = [Value::str("same")];
    let first = token(mixed.answer(&mut store, "js", &args).unwrap());
    let second = token(mixed.answer(&mut store, "js", &args).unwrap());
    let dropped = token(mixed.answer(&mut store, "js", &args).unwrap());
    mixed.discard(dropped);
    assert_eq!(mixed.staged_keys(), 1);
    let outcome = run(mixed.dispatch(first, &store));
    assert!(matches!(mixed.dispatch(second, &store), Dispatch::Held));
    assert_eq!(
        value_of(mixed.parse(&mut store, "js", &args, outcome)),
        Value::Number(1.0)
    );
    assert_eq!(mixed.staged_keys(), 1);
    let (released, dispatch) = mixed.release(&store).pop().expect("second call survives");
    assert_eq!(released, second);
    let outcome = run(dispatch);
    assert_eq!(
        value_of(mixed.parse(&mut store, "js", &args, outcome)),
        Value::Number(1.0)
    );
    assert_eq!(mixed.staged_keys(), 0);
}

#[test]
fn discarded_resumes_release_yielded_argument_keys() {
    let mut placed = Placed::new(
        Threaded {
            calls: Default::default(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut store = Store::new("secret.keep rust\nsecret.keep shared", []);
    let args = [Value::str("yielded document")];
    let t = token(placed.answer(&mut store, "yield", &args).unwrap());
    let outcome = run(placed.dispatch(t, &store));
    assert!(matches!(
        placed.parse(&mut store, "yield", &args, outcome).unwrap(),
        Answer::Later(_)
    ));
    assert_eq!(placed.staged_keys(), 1);
    let resume = token(
        placed
            .parse(&mut store, "yield", &args, Outcome::Storage(Vec::new()))
            .unwrap(),
    );
    placed.discard(resume);
    assert_eq!(placed.staged_keys(), 0);
    assert!(matches!(placed.dispatch(resume, &store), Dispatch::Missing));

    let mut mixed = Mixed::new(
        Proxy::new(Source::new("js")),
        Source::new("rust"),
        &["js"],
        &["rustrequest"],
    )
    .unwrap();
    let t = token(mixed.answer(&mut store, "rustrequest", &args).unwrap());
    let outcome = run(mixed.dispatch(t, &store));
    assert!(matches!(
        mixed
            .parse(&mut store, "rustrequest", &args, outcome)
            .unwrap(),
        Answer::Later(_)
    ));
    assert_eq!(mixed.staged_keys(), 1);
    let resume = token(
        mixed
            .parse(&mut store, "rustrequest", &args, failed())
            .unwrap(),
    );
    mixed.discard(resume);
    assert_eq!(mixed.staged_keys(), 0);
    assert!(matches!(mixed.dispatch(resume, &store), Dispatch::Missing));
}

// Two targets that ask one source with equal arguments, through the runner:
// each call's stages stay its own, whatever order the replies come in.

use exact_runner::{RequestOut, Runner};

fn twins<D: DataSource>(data: D) -> Runner<D> {
    use exact_plan::{asm::Asm, builder::PlanBuilder, TypeKind};
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let text = b.primitive(TypeKind::String);
    let baked = Value::str("baked");
    let first = b.resource("first", "yield", &[], text, Some(&baked));
    let second = b.resource("second", "yield", &[], text, Some(&baked));
    let mut body = Asm::new();
    body.refresh(first).refresh(second);
    let body = b.code(body);
    b.action("both", &[], &[], body);
    b.node(
        exact_kernel::NodeType::View as u8,
        None,
        None,
        0,
        &[],
        &[],
        None,
    );
    Runner::boot(
        b.finish().unwrap(),
        data,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn names(requests: &[RequestOut]) -> Vec<&str> {
    requests.iter().map(|q| q.target.as_str()).collect()
}

fn turn_token(request: &RequestOut) -> u64 {
    request.request.continuation.expect("a turn")
}

fn resource<D: DataSource>(r: &Runner<D>, name: &str) -> String {
    r.resource(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[test]
fn two_targets_asking_a_placed_source_with_equal_arguments_keep_their_own_turns() {
    let mut placed = Placed::new(
        Threaded {
            calls: Default::default(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut r = twins(placed);
    r.act("both", vec![]).unwrap();
    let asked = r.take_requests();
    assert_eq!(names(&asked), ["first", "second"]);
    let first = run(r.dispatch_work(turn_token(&asked[0])));
    let second = run(r.dispatch_work(turn_token(&asked[1])));
    // The first turn yields storage, and that reply comes back before the
    // second turn's envelope: it resumes the first call.
    r.fulfill(asked[0].ticket, first).unwrap();
    let storage = r.take_requests();
    assert_eq!(names(&storage), ["first"]);
    r.fulfill(storage[0].ticket, Outcome::Storage(b"one".to_vec()))
        .expect("the first call's storage reply resumes the first call");
    let resumed = r.take_requests();
    assert_eq!(names(&resumed), ["first"]);
    let outcome = run(r.dispatch_work(turn_token(&resumed[0])));
    r.fulfill(resumed[0].ticket, outcome).unwrap();
    r.fulfill(asked[1].ticket, second).unwrap();
    let storage = r.take_requests();
    assert_eq!(names(&storage), ["second"]);
    r.fulfill(storage[0].ticket, Outcome::Storage(b"two".to_vec()))
        .unwrap();
    let resumed = r.take_requests();
    assert_eq!(names(&resumed), ["second"]);
    let outcome = run(r.dispatch_work(turn_token(&resumed[0])));
    r.fulfill(resumed[0].ticket, outcome).unwrap();
    assert_eq!(resource(&r, "first"), "one");
    assert_eq!(resource(&r, "second"), "two");
    assert!(!r.has_pending());
    assert_eq!(r.data().staged_keys(), 0);
}

#[test]
fn two_targets_asking_an_ordered_member_with_equal_arguments_keep_their_own_stages() {
    // The worker child makes a set; the Rust child shares `shared` with it,
    // so its calls are ordered turns too.
    let mixed = Mixed::new(
        Proxy::new(Source::new("js")),
        Threaded {
            calls: Default::default(),
        },
        &["js"],
        &["yield"],
    )
    .unwrap();
    let mut r = twins(mixed);
    r.act("both", vec![]).unwrap();
    let asked = r.take_requests();
    assert_eq!(names(&asked), ["first", "second"]);
    let first = run(r.dispatch_work(turn_token(&asked[0])));
    assert!(matches!(
        r.dispatch_work(turn_token(&asked[1])),
        Dispatch::Held
    ));
    r.fulfill(asked[0].ticket, first).unwrap();
    let storage = r.take_requests();
    assert_eq!(names(&storage), ["first"]);
    // The first turn ended, so the second starts; the first call's storage
    // reply comes back before the second turn's envelope.
    let (released, dispatch) = r.release_work().pop().expect("the second turn");
    assert_eq!(released, turn_token(&asked[1]));
    let second = run(dispatch);
    r.fulfill(storage[0].ticket, Outcome::Storage(b"one".to_vec()))
        .expect("the first call's storage reply resumes the first call");
    let resumed = r.take_requests();
    assert_eq!(names(&resumed), ["first"]);
    assert!(matches!(
        r.dispatch_work(turn_token(&resumed[0])),
        Dispatch::Held
    ));
    r.fulfill(asked[1].ticket, second).unwrap();
    let storage = r.take_requests();
    assert_eq!(names(&storage), ["second"]);
    let (released, dispatch) = r.release_work().pop().expect("the first resumes");
    assert_eq!(released, turn_token(&resumed[0]));
    r.fulfill(resumed[0].ticket, run(dispatch)).unwrap();
    r.fulfill(storage[0].ticket, Outcome::Storage(b"two".to_vec()))
        .unwrap();
    let resumed = r.take_requests();
    assert_eq!(names(&resumed), ["second"]);
    let outcome = run(r.dispatch_work(turn_token(&resumed[0])));
    r.fulfill(resumed[0].ticket, outcome).unwrap();
    assert_eq!(resource(&r, "first"), "one");
    assert_eq!(resource(&r, "second"), "two");
    assert!(!r.has_pending());
    assert_eq!(r.data().staged_keys(), 0);
}

// A request replaced by newer arguments is forgotten (LLP 1016 D5): the
// composer keeps stages only for what is still in flight.

fn retyped<D: DataSource>(data: D) -> Runner<D> {
    use exact_plan::{asm::Asm, builder::PlanBuilder, TypeKind};
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let text = b.primitive(TypeKind::String);
    let empty = b.constant(&Value::str(""));
    let query = b.slot("query", text, empty);
    let mut arg = Asm::new();
    arg.load_slot(query);
    let arg = b.code(arg);
    let found = b.resource("found", "yield", &[arg], text, Some(&Value::str("baked")));
    b.set_resource_initial_args(found, &[Value::str("")]);
    let mut body = Asm::new();
    body.load_param(0).store_slot(query);
    let body = b.code(body);
    b.action("typed", &[("value", text)], &[query], body);
    b.node(
        exact_kernel::NodeType::View as u8,
        None,
        None,
        0,
        &[],
        &[],
        None,
    );
    Runner::boot(
        b.finish().unwrap(),
        data,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn a_placed_source_lets_go_of_a_call_newer_arguments_replaced() {
    let mut placed = Placed::new(
        Threaded {
            calls: Default::default(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut r = retyped(placed);
    r.act("typed", vec![Value::str("a")]).unwrap();
    r.act("typed", vec![Value::str("ab")]).unwrap();
    let asked = r.take_requests();
    assert_eq!(names(&asked), ["found", "found"]);
    assert_eq!(r.pending().len(), 1);
    assert_eq!(r.data().staged_keys(), 1, "only the newest call is staged");
    // The forgotten call's turn is let go too; the newest runs to its answer.
    assert!(matches!(
        r.dispatch_work(turn_token(&asked[0])),
        Dispatch::Missing
    ));
    let outcome = run(r.dispatch_work(turn_token(&asked[1])));
    r.fulfill(asked[1].ticket, outcome).unwrap();
    let storage = r.take_requests();
    r.fulfill(storage[0].ticket, Outcome::Storage(b"ab".to_vec()))
        .unwrap();
    let resumed = r.take_requests();
    let outcome = run(r.dispatch_work(turn_token(&resumed[0])));
    r.fulfill(resumed[0].ticket, outcome).unwrap();
    assert_eq!(resource(&r, "found"), "ab");
    assert!(!r.has_pending());
    assert_eq!(r.data().staged_keys(), 0);
}

#[test]
fn a_mixed_set_lets_go_of_a_call_newer_arguments_replaced() {
    let mixed = Mixed::new(
        Proxy::new(Source::new("js")),
        Threaded {
            calls: Default::default(),
        },
        &["js"],
        &["yield"],
    )
    .unwrap();
    let mut r = retyped(mixed);
    r.act("typed", vec![Value::str("a")]).unwrap();
    r.act("typed", vec![Value::str("ab")]).unwrap();
    let asked = r.take_requests();
    assert_eq!(names(&asked), ["found", "found"]);
    assert_eq!(r.data().staged_keys(), 1, "only the newest call is staged");
    assert!(matches!(
        r.dispatch_work(turn_token(&asked[0])),
        Dispatch::Missing
    ));
    let outcome = run(r.dispatch_work(turn_token(&asked[1])));
    r.fulfill(asked[1].ticket, outcome).unwrap();
    let storage = r.take_requests();
    r.fulfill(storage[0].ticket, Outcome::Storage(b"ab".to_vec()))
        .unwrap();
    let resumed = r.take_requests();
    let outcome = run(r.dispatch_work(turn_token(&resumed[0])));
    r.fulfill(resumed[0].ticket, outcome).unwrap();
    assert_eq!(resource(&r, "found"), "ab");
    assert!(!r.has_pending());
    assert_eq!(r.data().staged_keys(), 0);
}

#[test]
fn a_forgotten_running_turn_frees_its_mixed_set() {
    let mixed = Mixed::new(
        Proxy::new(Source::new("js")),
        Threaded {
            calls: Default::default(),
        },
        &["js"],
        &["yield"],
    )
    .unwrap();
    let mut r = retyped(mixed);
    r.act("typed", vec![Value::str("a")]).unwrap();
    let first = r.take_requests();
    // Its turn runs; newer arguments replace its request before the reply.
    let envelope = run(r.dispatch_work(turn_token(&first[0])));
    r.act("typed", vec![Value::str("ab")]).unwrap();
    let second = r.take_requests();
    // The runner drops that reply, so the set must not wait for it.
    let dispatch = r.dispatch_work(turn_token(&second[0]));
    assert!(
        matches!(dispatch, Dispatch::Run(_)),
        "the set still waits for a turn whose reply is dropped"
    );
    assert!(r.fulfill(first[0].ticket, envelope).unwrap().is_none());
    r.fulfill(second[0].ticket, run(dispatch)).unwrap();
    let storage = r.take_requests();
    r.fulfill(storage[0].ticket, Outcome::Storage(b"ab".to_vec()))
        .unwrap();
    let resumed = r.take_requests();
    let outcome = run(r.dispatch_work(turn_token(&resumed[0])));
    r.fulfill(resumed[0].ticket, outcome).unwrap();
    assert_eq!(resource(&r, "found"), "ab");
    assert!(!r.has_pending());
    assert_eq!(r.data().staged_keys(), 0);
}

// A refresh asks again with equal arguments while the call it replaces is
// still out: only the continuation token tells the old call from the new.

fn refreshed<D: DataSource>(data: D) -> Runner<D> {
    use exact_plan::{asm::Asm, builder::PlanBuilder, TypeKind};
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let text = b.primitive(TypeKind::String);
    let found = b.resource("found", "yield", &[], text, Some(&Value::str("baked")));
    let mut body = Asm::new();
    body.refresh(found);
    let body = b.code(body);
    b.action("again", &[], &[], body);
    b.node(
        exact_kernel::NodeType::View as u8,
        None,
        None,
        0,
        &[],
        &[],
        None,
    );
    Runner::boot(
        b.finish().unwrap(),
        data,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// The rest of a `yield` call after its first turn: that turn's envelope,
/// the storage request it yields, the storage reply, and the resumed turn.
fn finish<D: DataSource>(r: &mut Runner<D>, asked: &RequestOut, turn: Outcome, stored: &[u8]) {
    r.fulfill(asked.ticket, turn)
        .expect("the turn's own envelope");
    let storage = r.take_requests();
    assert_eq!(names(&storage), ["found"]);
    r.fulfill(storage[0].ticket, Outcome::Storage(stored.to_vec()))
        .expect("the storage reply resumes the live call");
    let resumed = r.take_requests();
    assert_eq!(names(&resumed), ["found"]);
    let outcome = run(r.dispatch_work(turn_token(&resumed[0])));
    r.fulfill(resumed[0].ticket, outcome)
        .expect("the resumed turn's envelope");
}

#[test]
fn a_refresh_with_equal_arguments_frees_a_set_whose_turn_it_replaced() {
    let mixed = Mixed::new(
        Proxy::new(Source::new("js")),
        Threaded {
            calls: Default::default(),
        },
        &["js"],
        &["yield"],
    )
    .unwrap();
    let mut r = refreshed(mixed);
    r.act("again", vec![]).unwrap();
    let first = r.take_requests();
    let envelope = run(r.dispatch_work(turn_token(&first[0])));
    r.act("again", vec![]).unwrap();
    let second = r.take_requests();
    let dispatch = r.dispatch_work(turn_token(&second[0]));
    assert!(
        matches!(dispatch, Dispatch::Run(_)),
        "the set still waits for the turn the refresh replaced"
    );
    assert!(r.fulfill(first[0].ticket, envelope).unwrap().is_none());
    finish(&mut r, &second[0], run(dispatch), b"again");
    assert_eq!(resource(&r, "found"), "again");
    assert!(!r.has_pending());
    assert_eq!(r.data().staged_keys(), 0);
}

#[test]
fn a_refresh_with_equal_arguments_replaces_a_placed_source_s_stage() {
    let mut placed = Placed::new(
        Threaded {
            calls: Default::default(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mut r = refreshed(placed);
    r.act("again", vec![]).unwrap();
    let first = r.take_requests();
    let envelope = run(r.dispatch_work(turn_token(&first[0])));
    r.act("again", vec![]).unwrap();
    let second = r.take_requests();
    let turn = run(r.dispatch_work(turn_token(&second[0])));
    assert!(r.fulfill(first[0].ticket, envelope).unwrap().is_none());
    finish(&mut r, &second[0], turn, b"again");
    assert_eq!(resource(&r, "found"), "again");
    assert!(!r.has_pending());
    assert_eq!(r.data().staged_keys(), 0);
}

#[test]
fn a_refresh_with_equal_arguments_reaches_a_placed_member_of_a_set() {
    // Fieldnotes' shape: the worker-placed member of an ordered set.
    let mut placed = Placed::new(
        Threaded {
            calls: Default::default(),
        },
        Placement::Worker,
    );
    placed.activate().unwrap();
    let mixed = Mixed::new(placed, Source::new("rust"), &["yield"], &["rust"]).unwrap();
    let mut r = refreshed(mixed);
    r.act("again", vec![]).unwrap();
    let first = r.take_requests();
    let envelope = run(r.dispatch_work(turn_token(&first[0])));
    r.act("again", vec![]).unwrap();
    let second = r.take_requests();
    let dispatch = r.dispatch_work(turn_token(&second[0]));
    assert!(
        matches!(dispatch, Dispatch::Run(_)),
        "the set still waits for the turn the refresh replaced"
    );
    assert!(r.fulfill(first[0].ticket, envelope).unwrap().is_none());
    finish(&mut r, &second[0], run(dispatch), b"again");
    assert_eq!(resource(&r, "found"), "again");
    assert!(!r.has_pending());
    assert_eq!(r.data().staged_keys(), 0);
}

#[test]
fn mixed_and_placed_forward_logs_from_successful_and_refused_turns_once() {
    #[derive(Clone, Default)]
    struct Logging(Vec<String>);
    impl DataSource for Logging {
        fn app_id(&self) -> &str {
            "test.logs"
        }
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            self.0.push(format!("console {source}"));
            if source == "fail" {
                Err(DataError::Unavailable("refused".into()))
            } else {
                Ok(Value::str("answer"))
            }
        }
        fn take_logs(&mut self) -> Vec<String> {
            std::mem::take(&mut self.0)
        }
    }
    for placement in [Placement::Main, Placement::Worker] {
        let mut placed = Mixed::new(
            Placed::new(Logging::default(), placement),
            Placed::new(Logging::default(), placement),
            &["js"],
            &["rust", "fail"],
        )
        .unwrap();
        placed.activate().unwrap();
        let mut store = Store::default();
        for source in ["js", "rust", "fail"] {
            let answer = placed.answer(&mut store, source, &[]);
            let result = match answer {
                Ok(Answer::Later(request)) => {
                    let outcome = run(placed.dispatch(request.continuation.unwrap(), &store));
                    placed.parse(&mut store, source, &[], outcome)
                }
                other => other,
            };
            assert_eq!(result.is_err(), source == "fail");
            assert_eq!(placed.take_logs(), [format!("console {source}")]);
            assert!(placed.take_logs().is_empty());
        }
    }
}

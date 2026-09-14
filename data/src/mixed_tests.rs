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
    let browser = token(mixed.answer(&mut store, "jswait", &[]).unwrap());
    assert_eq!(mixed.continuation_token(browser), Some(u64::MAX));
    assert_eq!(mixed.continuation_token(browser), None);
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

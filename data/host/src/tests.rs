use super::*;

#[test]
fn mixed_storage_waits_for_both_images_without_activating_sources() {
    use std::{cell::Cell, rc::Rc};
    struct Preparing {
        ready: Rc<Cell<bool>>,
        polls: Rc<Cell<usize>>,
    }
    impl DataSource for Preparing {
        fn app_id(&self) -> &str {
            "test.preload"
        }
        fn preload(&self) -> Result<bool, DataError> {
            self.polls.set(self.polls.get() + 1);
            Ok(self.ready.get())
        }
        fn query(
            &mut self,
            _: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, DataError> {
            panic!("image preparation must not run app queries")
        }
        fn activate(&mut self) -> Result<(), DataError> {
            panic!("image preparation must not activate app instances")
        }
    }
    let first = Rc::new(Cell::new(false));
    let second = Rc::new(Cell::new(false));
    let polls = Rc::new(Cell::new(0));
    let source = Storage::new(
        exact_data::Mixed::new(
            Preparing {
                ready: first.clone(),
                polls: polls.clone(),
            },
            Preparing {
                ready: second.clone(),
                polls: polls.clone(),
            },
            &["first"],
            &["second"],
        )
        .unwrap(),
    );
    assert!(!source.preload().unwrap());
    assert_eq!(
        polls.get(),
        2,
        "both images start even while the first is pending"
    );
    first.set(true);
    assert!(!source.preload().unwrap());
    second.set(true);
    assert!(source.preload().unwrap());
    assert!(!source.ready(), "preloading does not activate storage");
}
use exact_data::storage;
use exact_runner::{FailureKind, Request};
use serde_json::{json, Value as Json};
use std::sync::atomic::{AtomicU64, Ordering};

const GRANTS: &str = "fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/test.db";

struct Paths(PathBuf);
impl Paths {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "exact-data-host-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn directories(&self) -> Directories {
        Directories {
            data: self.0.join("data"),
            cache: self.0.join("cache"),
            temporary: self.0.join("temporary"),
        }
    }
    fn configure(&self, host: &mut Storage<Fixture>) {
        let paths = self.directories();
        host.configure_storage(paths.data, paths.cache, paths.temporary)
            .unwrap();
    }
}
impl Drop for Paths {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

#[derive(Clone)]
struct Fixture {
    request: Request,
}
impl Fixture {
    fn new() -> Self {
        Self {
            request: storage::request(
                "fs.writeFile",
                json!({"path":"app:/data/note","text":"hello"}),
            ),
        }
    }
}
impl DataSource for Fixture {
    fn app_id(&self) -> &str {
        "com.exact.storage-test"
    }
    fn grants(&self) -> &str {
        GRANTS
    }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Number(7.))
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(self.request.clone()))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        storage::response(outcome)
            .map(|value| Answer::Now(Value::str(&value.to_string())))
            .map_err(unavailable)
    }
    fn replacement(&self, _: &[u8], _: &str, _: Vec<u8>) -> Result<Self, DataError> {
        Ok(self.clone())
    }
}

fn take(host: &mut Storage<Fixture>) -> Box<dyn FnOnce() -> Outcome + Send> {
    let mut store = Store::default();
    let Answer::Later(request) = host.answer(&mut store, "operation", &[]).unwrap() else {
        panic!("expected worker request")
    };
    assert!(store.reads() > 0);
    assert!(request.storage.is_none());
    let token = request.continuation.unwrap();
    let job = host.continuation(token).unwrap();
    assert!(
        host.continuation(token).is_none(),
        "a worker job is taken once"
    );
    job
}
fn run(host: &mut Storage<Fixture>, request: Request) -> Result<Json, String> {
    host.source.request = request;
    let job = take(host);
    let outcome = std::thread::spawn(job).join().unwrap();
    storage::response(outcome)
}
fn sql(host: &mut Storage<Fixture>, op: &str, commands: Json) -> Result<Json, String> {
    run(
        host,
        storage::request(op, json!({"path":"app:/data/test.db","commands":commands})),
    )
}

#[test]
fn construction_configuration_and_validation_have_no_storage_effects() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    assert!(!host.ready());
    assert!(!paths.0.exists());
    paths.configure(&mut host);
    assert!(!paths.0.exists());
    assert!(host
        .answer(&mut Store::default(), "operation", &[])
        .is_err());
    assert!(!paths.0.exists());
    host.activate_for_validation().unwrap();
    assert!(host.ready());
    assert_eq!(host.query("pure", &[]).unwrap(), Value::Number(7.));
    assert!(host
        .answer(&mut Store::default(), "operation", &[])
        .is_err());
    assert!(host.pending.is_empty());
    assert!(!paths.0.exists());
}

#[test]
fn activation_defers_io_to_the_owned_job_and_parse_returns_its_result() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();
    assert!(host.ready());
    assert!(!paths.0.exists());
    let job = take(&mut host);
    assert!(!paths.0.exists(), "creating a job must not execute it");
    let outcome = std::thread::spawn(job).join().unwrap();
    assert_eq!(std::fs::read(paths.0.join("data/note")).unwrap(), b"hello");
    assert_eq!(
        host.parse(&mut Store::default(), "operation", &[], outcome)
            .unwrap(),
        Answer::Now(Value::str("null"))
    );
    let stat = run(
        &mut host,
        storage::request("fs.stat", json!({"path":"app:/data/note"})),
    )
    .unwrap();
    assert_eq!(stat["size"], 5);
    assert_eq!(stat["isFile"], true);
    assert_eq!(stat["isDirectory"], false);
}

#[test]
fn extracted_jobs_are_aborted_when_their_source_is_retired() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();
    let job = take(&mut host);
    drop(host);
    let outcome = std::thread::spawn(job).join().unwrap();
    assert!(matches!(
        outcome,
        Outcome::Failed {
            kind: FailureKind::Aborted,
            ..
        }
    ));
    assert!(!paths.0.exists());
}

#[test]
fn replacement_retains_directories_but_validation_stays_disposable() {
    let paths = Paths::new();
    let mut original = Storage::new(Fixture::new());
    paths.configure(&mut original);
    original.activate().unwrap();
    let mut validation = original.replacement(&[], "", vec![]).unwrap();
    validation.activate_for_validation().unwrap();
    assert!(validation
        .answer(&mut Store::default(), "operation", &[])
        .is_err());
    assert!(!paths.0.exists());
    let mut committed = original.replacement(&[], "", vec![]).unwrap();
    committed.activate().unwrap();
    run(&mut committed, Fixture::new().request).unwrap();
    assert_eq!(std::fs::read(paths.0.join("data/note")).unwrap(), b"hello");
}

#[test]
fn source_scope_refuses_writes_and_cannot_expand_the_app_ceiling() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();
    host.source.request.grants = Some("fs.write app:/elsewhere".into());
    assert!(host
        .answer(&mut Store::default(), "operation", &[])
        .is_err());
    assert!(!paths.0.exists());
    for scope in ["", "fs.read app:/data"] {
        let mut request = Fixture::new().request;
        request.grants = Some(scope.into());
        assert!(run(&mut host, request).is_err());
        assert!(!paths.0.join("data/note").exists());
    }
    run(&mut host, Fixture::new().request).unwrap();
    assert_eq!(std::fs::read(paths.0.join("data/note")).unwrap(), b"hello");
}

#[test]
fn sqlite_preserves_int64_and_blobs_shares_backing_data_and_rolls_back_transactions() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();
    let numeric_types = sql(
        &mut host,
        "sqlite",
        json!([{"kind":"query","sql":"SELECT typeof(?),typeof(?)","params":[1,0.5]}]),
    )
    .unwrap();
    assert_eq!(numeric_types[0]["rows"], json!([["integer", "real"]]));
    let inserted=sql(&mut host,"sqlite",json!([
        {"kind":"execute","sql":"CREATE TABLE items(id INTEGER PRIMARY KEY,payload BLOB,label TEXT)","params":[]},
        {"kind":"execute","sql":"INSERT INTO items VALUES(?,?,?)","params":[{"integer":"9007199254740993"},{"bytes":[0,128,255]},"from portable"]},
        {"kind":"query","sql":"SELECT id,payload,label FROM items","params":[]}
    ])).unwrap();
    assert_eq!(inserted[1]["lastInsertRowid"], "9007199254740993");
    assert_eq!(
        inserted[2]["rows"],
        json!([[{"integer":"9007199254740993"},{"bytes":[0,128,255]},"from portable"]])
    );
    assert!(sql(&mut host,"sqlite.transaction",json!([
        {"kind":"execute","sql":"INSERT INTO items VALUES(2,NULL,'must rollback')","params":[]},
        {"kind":"execute","sql":"INSERT INTO items VALUES(9007199254740993,NULL,'duplicate')","params":[]}
    ])).is_err());

    // A separate host binding is the same capability TS uses; it sees this file
    // immediately and its writes are visible on the next portable request.
    let dirs = paths.directories();
    let bindings = ibex2::host::Host::new()
        .with_app_directories(
            ibex2::stdlib::app_fs::AppDirectories::new(&dirs.data, &dirs.cache, &dirs.temporary)
                .unwrap(),
        )
        .with_sqlite_provider(std::sync::Arc::new(ibex2_sqlite::SqliteProvider))
        .endow(ibex2::grant::GrantSet::parse(GRANTS).unwrap());
    let database = bindings.sqlite.open("app:/data/test.db").unwrap();
    let count = database.query("SELECT COUNT(*) FROM items", &[]).unwrap();
    assert_eq!(
        count.rows,
        vec![vec![ibex2::stdlib::sqlite::Value::Integer(1)]]
    );
    database
        .execute("INSERT INTO items VALUES(3,NULL,'from host')", &[])
        .unwrap();
    database.close().unwrap();
    let result = sql(
        &mut host,
        "sqlite",
        json!([{"kind":"query","sql":"SELECT label FROM items WHERE id=3","params":[]}]),
    )
    .unwrap();
    assert_eq!(result[0]["rows"], json!([["from host"]]));
    assert!(paths.0.join("data/test.db").is_file());
}

#[test]
fn malformed_storage_parameters_are_refused_without_running_sql() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();
    for value in [
        json!({"integer":"+1"}),
        json!({"integer":"01"}),
        json!({"integer":"9223372036854775808"}),
        json!({"bytes":[256]}),
        json!(9_007_199_254_740_992_u64),
        json!(true),
    ] {
        assert!(sql(
            &mut host,
            "sqlite",
            json!([{"kind":"query","sql":"SELECT ?","params":[value]}])
        )
        .is_err());
        assert!(!paths.0.join("data/test.db").exists());
    }
    for request in [
        Request::storage(b"not JSON".to_vec()),
        Request::storage(
            br#"{"version":2,"op":"fs.writeFile","args":{"path":"app:/data/note","text":"bad"}}"#
                .to_vec(),
        ),
        storage::request("fs.writeFile", json!({"path":"/tmp/escape","text":"bad"})),
    ] {
        assert!(run(&mut host, request).is_err());
        assert!(!paths.0.join("data/note").exists());
    }
}

#[test]
fn invalid_utf8_is_never_repaired_into_an_effectful_request() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();
    let payload = b"{\"version\":1,\"op\":\"fs.writeFile\",\"args\":{\"path\":\"app:/data/note\",\"text\":\"\xff\"}}";
    host.source.request = Request::storage(payload.to_vec());
    assert!(host
        .answer(&mut Store::default(), "operation", &[])
        .is_err());
    assert!(storage::response(super::native::run(
        host.directories.as_ref().unwrap(),
        GRANTS,
        payload
    ))
    .is_err());
    assert!(
        !paths.0.exists(),
        "malformed data creates no app directories"
    );
}

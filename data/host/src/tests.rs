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
    grants: &'static str,
}
impl Fixture {
    fn new() -> Self {
        Self {
            grants: GRANTS,
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
        self.grants
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
fn filesystem_lifecycle_preserves_results_and_two_path_authority() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();

    let request = |op, args| storage::request(op, args);
    run(
        &mut host,
        request("fs.mkdir", json!({"path":"app:/data/files"})),
    )
    .unwrap();
    run(
        &mut host,
        request(
            "fs.writeFile",
            json!({"path":"app:/data/files/source","text":"one"}),
        ),
    )
    .unwrap();
    run(
        &mut host,
        request(
            "fs.appendFile",
            json!({"path":"app:/data/files/source","bytes":[0,255]}),
        ),
    )
    .unwrap();
    assert_eq!(
        run(
            &mut host,
            request("fs.readFile", json!({"path":"app:/data/files/source"})),
        )
        .unwrap(),
        json!({"base64":"b25lAP8="})
    );
    run(
        &mut host,
        request(
            "fs.atomicWriteFile",
            json!({"path":"app:/data/files/source","text":"final"}),
        ),
    )
    .unwrap();
    let stat = run(
        &mut host,
        request("fs.stat", json!({"path":"app:/data/files/source"})),
    )
    .unwrap();
    assert_eq!(stat["size"], 5);
    assert_eq!(stat["isFile"], true);
    assert_eq!(stat["isDirectory"], false);
    assert!(stat["modifiedMs"].is_number());

    let mut denied_copy = request(
        "fs.copyFile",
        json!({"path":"app:/data/files/source","destination":"app:/data/files/denied"}),
    );
    denied_copy.grants = Some("fs.read app:/data".into());
    assert!(run(&mut host, denied_copy).is_err());
    assert!(!paths.0.join("data/files/denied").exists());

    run(
        &mut host,
        request(
            "fs.copyFile",
            json!({"path":"app:/data/files/source","destination":"app:/data/files/copy"}),
        ),
    )
    .unwrap();
    run(
        &mut host,
        request(
            "fs.rename",
            json!({"path":"app:/data/files/copy","destination":"app:/data/files/moved"}),
        ),
    )
    .unwrap();
    assert_eq!(
        run(
            &mut host,
            request("fs.readdir", json!({"path":"app:/data/files"})),
        )
        .unwrap(),
        json!(["moved", "source"])
    );
    let real = run(
        &mut host,
        request("fs.realpath", json!({"path":"app:/data/files/moved"})),
    )
    .unwrap();
    assert!(real.as_str().unwrap().ends_with("/data/files/moved"));
    for path in ["app:/data/files/source", "app:/data/files/moved"] {
        run(&mut host, request("fs.rm", json!({"path":path}))).unwrap();
    }
    assert_eq!(
        run(
            &mut host,
            request("fs.readdir", json!({"path":"app:/data/files"})),
        )
        .unwrap(),
        json!([])
    );

    let mut denied_sql =
        storage::request("sqlite", json!({"path":"app:/data/test.db","commands":[]}));
    denied_sql.grants = Some("fs.read app:/data\nfs.write app:/data".into());
    assert!(run(&mut host, denied_sql).is_err());
    assert!(!paths.0.join("data/test.db").exists());
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
        host.directories.as_ref(),
        GRANTS,
        payload,
        0,
    ))
    .is_err());
    assert!(
        !paths.0.exists(),
        "malformed data creates no app directories"
    );
}

#[test]
fn independent_storage_is_refused_before_conversion_or_disk_touch() {
    let paths = Paths::new();
    let mut source = Fixture::new();
    source.request = source.request.independent_http(4096);
    let mut host = Storage::new(source);
    paths.configure(&mut host);
    host.activate().unwrap();
    let result = host.answer(&mut Store::default(), "operation", &[]);
    assert!(
        result.is_err(),
        "storage annotation must not be erased into an executable continuation"
    );
    assert!(host.pending.is_empty());
    assert!(host.continuation(1).is_none());
    assert!(
        !paths.0.exists(),
        "no directory or file may be created on refusal"
    );
}

/// A child that hands out continuation tokens of its own, and keeps what
/// `forgotten` told it.
struct Continuing {
    heard: std::rc::Rc<std::cell::RefCell<Vec<Option<u64>>>>,
}

impl DataSource for Continuing {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Unit)
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(Request::continuation(7)))
    }
    fn dispatch(&mut self, _: u64, _: &Store) -> exact_runner::Dispatch {
        exact_runner::Dispatch::Host(7)
    }
    fn forgotten(
        &mut self,
        _store: &exact_runner::Store,
        in_flight: &[exact_runner::InFlight<'_>],
    ) {
        self.heard
            .borrow_mut()
            .extend(in_flight.iter().map(|f| f.continuation));
    }
}

#[test]
fn forgotten_hands_a_child_its_own_tokens_and_lets_go_of_the_rest() {
    let heard = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut host = Storage::new(Continuing {
        heard: heard.clone(),
    });
    let target = exact_runner::Target::Resource(0);
    let mut store = Store::default();
    let token = |answer: Result<Answer, DataError>| match answer {
        Ok(Answer::Later(request)) => request.continuation.expect("a continuation"),
        other => panic!("{other:?}"),
    };
    let first = token(host.answer_for(target, &mut store, "wait", &[]));
    let second = token(host.answer_for(target, &mut store, "wait", &[]));
    assert_ne!(first, 7, "the host's token is its own");
    let in_flight = |continuation| exact_runner::InFlight {
        target,
        source: "wait",
        args: &[],
        continuation: Some(continuation),
    };
    // The second replaced the first: the child hears its own token back.
    host.forgotten(&store, &[in_flight(second)]);
    assert_eq!(heard.borrow().as_slice(), [Some(7)]);
    assert!(matches!(
        host.dispatch(first, &store),
        exact_runner::Dispatch::Missing
    ));
    // Once dispatched, a token can't be told any more.
    assert!(matches!(
        host.dispatch(second, &store),
        exact_runner::Dispatch::Host(7)
    ));
    host.forgotten(&store, &[in_flight(second)]);
    assert_eq!(heard.borrow().as_slice(), [Some(7), None]);
}

/// LLP 1069.010 D1: a `doc:` path reads and writes the chosen file under
/// the existing grants over the namespace, with no app directories, and a
/// path never minted, or outside the grant, is refused.
#[test]
fn a_document_path_reads_and_writes_the_chosen_file_under_its_grant() {
    let dir = std::env::temp_dir().join(format!("exact-doc-storage-{}", std::process::id()));
    let folder = dir.join("notes");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.md"), "# A").unwrap();
    let doc =
        exact_data::documents::open_route(folder.join("a.md").to_str().unwrap(), 9001).unwrap();
    let run = |grants: &str, op: &str, args: serde_json::Value| {
        let payload =
            serde_json::to_vec(&serde_json::json!({"version":1,"op":op,"args":args})).unwrap();
        storage::response(super::native::run(None, grants, &payload, 9001))
    };
    let read = run(
        "fs.read doc:/",
        "fs.readFile",
        serde_json::json!({"path": doc}),
    )
    .unwrap();
    assert_eq!(read["base64"], exact_runner::agent::base64(b"# A"));
    let folder_doc = doc.rsplit_once('/').unwrap().0;
    let names = run(
        "fs.read doc:/",
        "fs.readdir",
        serde_json::json!({"path": folder_doc}),
    );
    assert_eq!(names.unwrap(), serde_json::json!(["a.md"]));
    let stat = run("fs.read doc:/", "fs.stat", serde_json::json!({"path": doc})).unwrap();
    assert_eq!(stat["size"], 3);
    let refused = run(
        "fs.read app:/data",
        "fs.readFile",
        serde_json::json!({"path": doc}),
    );
    assert!(refused.unwrap_err().starts_with("denied: "));
    let write = serde_json::json!({"path": doc, "text": "# B"});
    let refused = run("fs.read doc:/", "fs.atomicWriteFile", write.clone());
    assert_eq!(refused.unwrap_err(), "denied: fs.write");
    run("fs.read doc:/\nfs.write doc:/", "fs.atomicWriteFile", write).unwrap();
    assert_eq!(std::fs::read_to_string(folder.join("a.md")).unwrap(), "# B");
    let escape = format!("{folder_doc}/../secret");
    assert!(run(
        "fs.read doc:/",
        "fs.readFile",
        serde_json::json!({"path": escape})
    )
    .is_err());
    exact_data::documents::forget(9001);
    let gone = run(
        "fs.read doc:/",
        "fs.readFile",
        serde_json::json!({"path": doc}),
    );
    assert!(gone.unwrap_err().contains("document ENOENT:"));
    let _ = std::fs::remove_dir_all(dir);
}

/// LLP 1069.010 D1 (2026-10-26): `copyFile` takes a chosen file into an app
/// file under `fs.read` on the one and `fs.write` on the other, needing the
/// app's directories, while a copy between documents is still refused.
#[test]
fn a_chosen_file_is_copied_into_an_app_file_under_both_grants() {
    let paths = Paths::new();
    let folder = paths.0.join("chosen");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.png"), "picture").unwrap();
    let doc = exact_data::documents::mint(&folder.join("a.png"), 9007).unwrap();
    let copy = |grants: &str, to: &str| {
        let args = serde_json::json!({"path": doc, "destination": to});
        let payload =
            serde_json::to_vec(&serde_json::json!({"version":1,"op":"fs.copyFile","args":args}))
                .unwrap();
        assert_eq!(
            super::native::independent_storage(&payload),
            !to.starts_with("app:/")
        );
        storage::response(super::native::run(
            Some(&paths.directories()),
            grants,
            &payload,
            9007,
        ))
    };
    let both = "fs.read doc:/\nfs.write app:/data";
    assert_eq!(copy(both, "app:/data/a.png").unwrap(), Json::Null);
    assert_eq!(
        std::fs::read_to_string(paths.0.join("data/a.png")).unwrap(),
        "picture"
    );
    for grants in ["fs.read doc:/\nfs.write app:/cache", "fs.write app:/data"] {
        let refused = copy(grants, "app:/data/b.png").unwrap_err();
        assert!(refused.starts_with("denied: "), "{grants}: {refused}");
    }
    assert!(!paths.0.join("data/b.png").exists());
    let to_doc = format!("{}/copy.png", doc.rsplit_once('/').unwrap().0);
    assert!(copy("fs.read doc:/\nfs.write doc:/", &to_doc).is_err());
    assert!(!folder.join("copy.png").exists());
    exact_data::documents::forget(9007);
}

/// LLP 1069.010.000 D2a: a write that expects the stat it read is refused
/// `ECHANGED`, writing nothing, once another writer changed the file, and
/// lands while the file is as read.
#[test]
fn a_write_that_expects_what_it_read_is_refused_once_the_file_changed() {
    let dir = std::env::temp_dir().join(format!("exact-doc-expect-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("drawing.json");
    std::fs::write(&file, "one").unwrap();
    let doc = exact_data::documents::mint(&file, 9004).unwrap();
    let run = |op: &str, args: serde_json::Value| {
        let payload =
            serde_json::to_vec(&serde_json::json!({"version":1,"op":op,"args":args})).unwrap();
        storage::response(super::native::run(
            None,
            "fs.read doc:/\nfs.write doc:/",
            &payload,
            9004,
        ))
    };
    let read = run("fs.stat", serde_json::json!({"path": doc})).unwrap();
    let expect = serde_json::json!({"modifiedMs": read["modifiedMs"], "size": read["size"]});
    // Another program saves a longer file.
    std::fs::write(&file, "another program's").unwrap();
    let refused = run(
        "fs.atomicWriteFile",
        serde_json::json!({"path": doc, "text": "mine", "expect": expect}),
    );
    assert!(refused.unwrap_err().starts_with("document ECHANGED: "));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "another program's");
    let now = run("fs.stat", serde_json::json!({"path": doc})).unwrap();
    let expect = serde_json::json!({"modifiedMs": now["modifiedMs"], "size": now["size"]});
    run(
        "fs.atomicWriteFile",
        serde_json::json!({"path": doc, "text": "mine", "expect": expect}),
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "mine");
    exact_data::documents::forget(9004);
    let _ = std::fs::remove_dir_all(dir);
}

/// LLP 1069.010.000 D5: under `doc.keep` the app lists what it keeps and
/// forgets one; without it both are refused. The app's own atomic save keeps
/// its document (D4).
#[test]
#[cfg(unix)]
fn the_kept_list_reads_and_forgets_under_doc_keep() {
    use exact_data::documents::kept;
    let dir = std::env::temp_dir().join(format!("exact-doc-kept-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("drawing.json");
    std::fs::write(&file, "one").unwrap();
    kept::open(9005, "test.storage.kept", &dir.join("kept.json"));
    let doc = kept::keep(9005, &file, "", "readwrite").unwrap();
    let run = |grants: &str, op: &str, args: serde_json::Value| {
        let payload =
            serde_json::to_vec(&serde_json::json!({"version":1,"op":op,"args":args})).unwrap();
        storage::response(super::native::run(None, grants, &payload, 9005))
    };
    let keeps = "fs.read doc:/\nfs.write doc:/\ndoc.keep";
    let listed = run(keeps, "fs.recentDocuments", serde_json::json!({})).unwrap();
    assert_eq!(listed[0]["path"], doc.as_str());
    assert_eq!(listed[0]["kind"], "file");
    let refused = run("fs.read doc:/", "fs.recentDocuments", serde_json::json!({}));
    assert!(refused.unwrap_err().contains("needs the doc.keep grant"));
    let atomic = serde_json::json!({"path": doc, "text": "two"});
    run(keeps, "fs.atomicWriteFile", atomic).unwrap();
    kept::held(9005, std::slice::from_ref(&file));
    assert_eq!(
        run(keeps, "fs.recentDocuments", serde_json::json!({})).unwrap()[0]["path"],
        doc.as_str(),
        "the app's own save is still the document kept"
    );
    run(keeps, "fs.forgetDocument", serde_json::json!({"path": doc})).unwrap();
    let again = run(keeps, "fs.forgetDocument", serde_json::json!({"path": doc}));
    assert!(again.unwrap_err().starts_with("document ENOENT: "));
    assert_eq!(
        run(keeps, "fs.recentDocuments", serde_json::json!({})).unwrap(),
        serde_json::json!([])
    );
    kept::end(9005);
    exact_data::documents::forget(9005);
    let _ = std::fs::remove_dir_all(dir);
}

/// LLP 1069.010.000 D4: a kept file renamed while the app was closed is
/// read at its new name through the path it was kept by.
#[test]
#[cfg(unix)]
fn a_kept_file_renamed_between_launches_reads_through_its_kept_path() {
    use exact_data::documents::kept::{self, Identity};
    let dir = std::env::temp_dir().join(format!("exact-doc-renamed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = std::fs::canonicalize(&dir).unwrap();
    std::fs::write(dir.join("moved.md"), "kept").unwrap();
    let (identity, _) = Identity::of(&dir.join("moved.md")).unwrap();
    // What an earlier launch wrote, before the file was renamed.
    let n = 900_000 + u64::from(std::process::id() % 1000);
    let set = serde_json::json!({"version": 1, "next": n + 1, "documents": [{
        "n": n, "name": "chosen.md", "kind": "file", "mode": "readwrite", "opened": 1,
        "device": identity.device, "inode": identity.inode,
        "path": dir.join("chosen.md").to_str().unwrap(), "bookmark": ""}]});
    std::fs::write(dir.join("kept.json"), set.to_string()).unwrap();
    kept::open(9006, "test.storage.renamed", &dir.join("kept.json"));
    kept::restore(9006, &[(n, dir.join("moved.md"), None)]);
    let doc = format!("doc:/{n}/chosen.md");
    let run = |op: &str, path: &str| {
        let payload =
            serde_json::to_vec(&serde_json::json!({"version":1,"op":op,"args":{"path": path}}))
                .unwrap();
        storage::response(super::native::run(None, "fs.read doc:/", &payload, 9006))
    };
    let read = run("fs.readFile", &doc).unwrap();
    assert_eq!(read["base64"], exact_runner::agent::base64(b"kept"));
    assert!(run("fs.readFile", &format!("doc:/{n}/moved.md")).is_err());
    assert!(
        run("fs.readdir", &doc).is_err(),
        "still one file, not a folder"
    );
    kept::end(9006);
    exact_data::documents::forget(9006);
    let _ = std::fs::remove_dir_all(dir);
}

/// Exercise the activated Rust storage continuation, not only path resolution.
#[test]
#[cfg(unix)]
fn chosen_folders_refuse_symlinks_for_every_document_operation() {
    use std::os::unix::fs::symlink;
    let root = Paths::new();
    let folder = root.0.join("chosen");
    let outside = root.0.join("outside");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside").unwrap();
    symlink(outside.join("secret"), folder.join("link")).unwrap();
    symlink(&outside, folder.join("dir-link")).unwrap();
    let doc = exact_data::documents::mint(&folder, 9002).unwrap();
    let mut host = Storage::new(Fixture {
        grants: "fs.read doc:/\nfs.write doc:/",
        ..Fixture::new()
    });
    host.set_document_owner(9002);
    host.activate().unwrap();
    let mut call = |op: &str, relative: &str| {
        run(
            &mut host,
            storage::request(
                op,
                json!({"path":format!("{doc}/{relative}"),"text":"inside"}),
            ),
        )
    };
    for path in ["link", "dir-link/secret"] {
        for op in [
            "fs.readFile",
            "fs.writeFile",
            "fs.appendFile",
            "fs.atomicWriteFile",
            "fs.stat",
            "fs.readdir",
            "fs.mkdir",
            "fs.rm",
        ] {
            let error = call(op, path).unwrap_err();
            assert!(error.contains(&doc), "{op}: {error}");
            assert!(
                !error.contains(root.0.to_str().unwrap()),
                "physical path leaked: {error}"
            );
        }
    }
    assert!(call("fs.mkdir", "dir-link/new/deep").is_err());
    assert!(!outside.join("new").exists());
    assert_eq!(
        std::fs::read_to_string(outside.join("secret")).unwrap(),
        "outside"
    );
    call("fs.mkdir", "sub/deep").unwrap();
    call("fs.writeFile", "sub/deep/note").unwrap();
    call("fs.appendFile", "sub/deep/note").unwrap();
    assert_eq!(
        call("fs.readFile", "sub/deep/note").unwrap()["base64"],
        exact_runner::agent::base64(b"insideinside")
    );
    assert!(call("fs.rm", "sub/deep")
        .unwrap_err()
        .contains("document ENOTEMPTY:"));
    call("fs.atomicWriteFile", "sub/deep/note").unwrap();
    call("fs.rm", "sub/deep/note").unwrap();
    call("fs.rm", "sub/deep").unwrap();
    assert_eq!(call("fs.readdir", "sub").unwrap(), json!([]));
    // Metadata, deletion and atomic replacement require no read permission on
    // the file being replaced; the directory's own authority is enough.
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(folder.join("reference-mode"), "old").unwrap();
    std::fs::create_dir(folder.join("reference-dir")).unwrap();
    call("fs.writeFile", "doc-mode").unwrap();
    let mode = |name: &str| {
        std::fs::metadata(folder.join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode("doc-mode"), mode("reference-mode"));
    assert_eq!(mode("sub"), mode("reference-dir"));
    std::fs::write(folder.join("write-only"), "old").unwrap();
    std::fs::set_permissions(
        folder.join("write-only"),
        std::fs::Permissions::from_mode(0o200),
    )
    .unwrap();
    assert_eq!(call("fs.stat", "write-only").unwrap()["size"], 3);
    call("fs.atomicWriteFile", "write-only").unwrap();
    std::fs::set_permissions(
        folder.join("write-only"),
        std::fs::Permissions::from_mode(0o200),
    )
    .unwrap();
    call("fs.rm", "write-only").unwrap();
    // A symlink explicitly chosen by the person still selects its target.
    let selected = exact_data::documents::mint(&folder.join("link"), 9002).unwrap();
    assert_eq!(
        run(
            &mut host,
            storage::request("fs.readFile", json!({"path":selected}))
        )
        .unwrap()["base64"],
        exact_runner::agent::base64(b"outside")
    );
    let save = exact_data::documents::mint(&folder.join("new.txt"), 9002).unwrap();
    run(
        &mut host,
        storage::request("fs.writeFile", json!({"path":save,"text":"saved"})),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(folder.join("new.txt")).unwrap(),
        "saved"
    );
    exact_data::documents::forget(9002);
}

#[test]
fn queued_document_work_uses_its_sessions_owner() {
    let root = Paths::new();
    let folder = root.0.join("chosen");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("note"), "private").unwrap();
    let doc = exact_data::documents::mint(&folder, 9010).unwrap();
    let request = storage::request("fs.readFile", json!({"path":format!("{doc}/note")}));
    let mut owner = Storage::new(Fixture {
        grants: "fs.read doc:/",
        request: request.clone(),
    });
    owner.set_document_owner(9010);
    owner.activate().unwrap();
    let queued = take(&mut owner);
    let mut other = Storage::new(Fixture {
        grants: "fs.read doc:/",
        request: request.clone(),
    });
    other.set_document_owner(9011);
    other.activate().unwrap();
    assert_eq!(
        storage::response(queued()).unwrap()["base64"],
        exact_runner::agent::base64(b"private")
    );
    assert!(run(&mut other, request)
        .unwrap_err()
        .contains("document ENOENT:"));
    let root_path = format!("doc:/{}", doc["doc:/".len()..].split('/').next().unwrap());
    assert!(run(
        &mut other,
        storage::request("fs.readdir", json!({"path":root_path}))
    )
    .unwrap_err()
    .contains("document ENOENT:"));
    exact_data::documents::forget(9011);
    assert_eq!(
        run(
            &mut owner,
            storage::request("fs.readdir", json!({"path":doc}))
        )
        .unwrap(),
        json!(["note"])
    );
    owner.source.request = storage::request("fs.readFile", json!({"path":format!("{doc}/note")}));
    let queued = take(&mut owner);
    exact_data::documents::forget(9010);
    assert!(storage::response(queued())
        .unwrap_err()
        .contains("document ENOENT:"));
}

#[test]
fn filesystem_results_stop_at_the_encoded_response_limit() {
    let root = Paths::new();
    let paths = root.directories();
    std::fs::create_dir_all(&paths.data).unwrap();
    let boundary = ((storage::MAX_BYTES - 13) / 4) * 3;
    let file = paths.data.join("large");
    std::fs::File::create(&file)
        .unwrap()
        .set_len(boundary as u64)
        .unwrap();
    let mut host = Storage::new(Fixture::new());
    root.configure(&mut host);
    host.activate().unwrap();
    let read = || storage::request("fs.readFile", json!({"path":"app:/data/large"}));
    let answer = run(&mut host, read()).unwrap();
    assert_eq!(answer["base64"].as_str().unwrap().len(), (boundary / 3) * 4);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_len((boundary + 1) as u64)
        .unwrap();
    assert_eq!(
        run(&mut host, read()).unwrap_err(),
        "storage result exceeds its byte limit"
    );
    let document = exact_data::documents::mint(&file, 9020).unwrap();
    host.set_document_owner(9020);
    assert!(matches!(
        super::native::run(
            None,
            "fs.read doc:/",
            &serde_json::to_vec(&json!({"version":1,"op":"fs.readFile","args":{"path":document}})).unwrap(),
            9020,
        ),
        Outcome::Failed {
            kind: FailureKind::Unsupported,
            message,
        } if message == "storage result exceeds its byte limit"
    ));
    exact_data::documents::forget(9020);
}

#[test]
#[cfg(unix)]
fn document_work_stays_on_the_selected_directory_during_path_swaps() {
    use std::os::unix::fs::symlink;
    use std::sync::{atomic::AtomicBool, Arc};
    let root = Paths::new();
    let folder = root.0.join("chosen");
    let outside = root.0.join("outside");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(folder.join("note"), "inside").unwrap();
    std::fs::write(outside.join("note"), "outside").unwrap();
    let doc = exact_data::documents::mint(&folder, 9003).unwrap();
    std::fs::rename(&folder, root.0.join("moved")).unwrap();
    symlink(&outside, &folder).unwrap();
    let mut host = Storage::new(Fixture {
        grants: "fs.read doc:/\nfs.write doc:/",
        ..Fixture::new()
    });
    host.set_document_owner(9003);
    host.activate().unwrap();
    let mut call = |op: &str, name: &str| {
        run(
            &mut host,
            storage::request(op, json!({"path":format!("{doc}/{name}"),"text":"written"})),
        )
    };
    assert_eq!(
        call("fs.readFile", "note").unwrap()["base64"],
        exact_runner::agent::base64(b"inside")
    );
    call("fs.writeFile", "note").unwrap();
    assert_eq!(
        std::fs::read_to_string(root.0.join("moved/note")).unwrap(),
        "written"
    );
    let moved = root.0.join("moved");
    symlink(outside.join("note"), moved.join("race")).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let swaps = Arc::new(AtomicU64::new(0));
    let attacker = std::thread::spawn({
        let stop = stop.clone();
        let swaps = swaps.clone();
        let outside = outside.clone();
        move || {
            while !stop.load(Ordering::Acquire) {
                std::fs::write(moved.join("swap"), "inside").unwrap();
                std::fs::rename(moved.join("swap"), moved.join("race")).unwrap();
                symlink(outside.join("note"), moved.join("swap")).unwrap();
                std::fs::rename(moved.join("swap"), moved.join("race")).unwrap();
                swaps.fetch_add(1, Ordering::Release);
            }
        }
    });
    for _ in 0..128 {
        if let Ok(value) = call("fs.readFile", "race") {
            assert_ne!(value["base64"], exact_runner::agent::base64(b"outside"));
        }
        let _ = call("fs.writeFile", "race");
        let _ = call("fs.appendFile", "race");
        let _ = call("fs.rm", "race");
    }
    stop.store(true, Ordering::Release);
    attacker.join().unwrap();
    assert!(swaps.load(Ordering::Acquire) > 0);
    assert_eq!(
        std::fs::read_to_string(outside.join("note")).unwrap(),
        "outside"
    );
    exact_data::documents::forget(9003);
}

/// A drive that names no scratch store (trivia F7): the request is answered,
/// with the web's refusal word for word, which the module can handle — where
/// an unconfigured host refuses the answer itself.
#[test]
fn an_agent_drive_without_a_store_answers_with_the_webs_refusal() {
    let mut host = Storage::new(Fixture::new());
    host.agent = true;
    host.activate().unwrap();
    let refused = run(
        &mut host,
        storage::request("fs.readFile", json!({"path":"app:/data/x"})),
    );
    assert_eq!(
        refused,
        Err("storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)".into())
    );
    // The first refused read or write is said in the journal, for the driver's note; a second is not.
    let lines = host.take_logs();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("storage refused (agent): "));
    let _ = run(
        &mut host,
        storage::request("fs.writeFile", json!({"path":"app:/data/y","text":"y"})),
    );
    assert!(host.take_logs().is_empty());
    host.agent = false;
    host.source.request = storage::request("fs.readFile", json!({"path":"app:/data/x"}));
    assert!(host
        .answer(&mut Store::default(), "operation", &[])
        .is_err());
}

#[test]
fn storage_forwards_source_console_lines_once() {
    struct Logging(Vec<String>);
    impl DataSource for Logging {
        fn query(
            &mut self,
            _: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, DataError> {
            Ok(exact_plan::Value::Unit)
        }
        fn take_logs(&mut self) -> Vec<String> {
            std::mem::take(&mut self.0)
        }
    }
    let mut source = Storage::new(Logging(vec!["console error".into()]));
    assert_eq!(source.take_logs(), ["console error"]);
    assert!(source.take_logs().is_empty());
}

/// A Rust source's `fs.compressImage` (LLP 1069.002 A1.1): the operation a
/// TypeScript source's `storage.fs.compressImage` runs, through ibex2's
/// executor and the platform codec. On Apple the fixture comes back upright
/// as a JPEG; elsewhere the call is refused as `unsupported`.
#[test]
fn compress_image_answers_a_rust_source_as_typescripts() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();
    run(
        &mut host,
        storage::request("fs.mkdir", json!({"path":"app:/data/in"})),
    )
    .unwrap();
    std::fs::write(
        paths.0.join("data/in/photo.jpg"),
        include_bytes!("../../../scripts/fixtures/picker/oriented-gps.jpg"),
    )
    .unwrap();
    let compress = |to: &str, dimension: Json, bytes: Json| {
        storage::request(
            "fs.compressImage",
            json!({"path":"app:/data/in/photo.jpg","destination":to,"maxDimension":dimension,"maxBytes":bytes}),
        )
    };
    let result = run(
        &mut host,
        compress("app:/data/out.jpg", json!(32), json!(2_000_000)),
    );
    if cfg!(target_vendor = "apple") {
        let size = std::fs::metadata(paths.0.join("data/out.jpg"))
            .unwrap()
            .len();
        assert_eq!(
            result.unwrap(),
            json!({"path":"app:/data/out.jpg","type":"image/jpeg","size":size,"width":24,"height":32})
        );
        let err = run(
            &mut host,
            compress("app:/data/unfit.jpg", json!(64), json!(1)),
        )
        .unwrap_err();
        assert!(err.starts_with("compressImage: unfit: "), "{err}");
        // A source over 64 MiB is the documented `too-large`, not the
        // generic result limit its bounded read hits first.
        let big = std::fs::File::create(paths.0.join("data/in/big.jpg")).unwrap();
        big.set_len(64 * 1024 * 1024 + 1).unwrap();
        let err = run(
            &mut host,
            storage::request(
                "fs.compressImage",
                json!({"path":"app:/data/in/big.jpg","destination":"app:/data/big-out.jpg","maxDimension":32,"maxBytes":2_000_000}),
            ),
        )
        .unwrap_err();
        assert!(err.starts_with("compressImage: too-large: "), "{err}");
    } else {
        assert_eq!(
            result.unwrap_err(),
            "compressImage: unsupported: no image codec"
        );
    }
    for (request, expected) in [
        (
            compress("app:/cache/out.jpg", json!(32), json!(100)),
            "denied: fs.write",
        ),
        (
            compress("doc:/1/out.jpg", json!(32), json!(100)),
            "compressImage: needs app:/ paths",
        ),
        (
            compress("app:/data/x.jpg", json!(0), json!(100)),
            "compressImage: invalid: maxDimension",
        ),
        (
            compress("app:/data/x.jpg", json!(32), json!("100")),
            "storage: maxBytes must be a number",
        ),
    ] {
        let err = run(&mut host, request).unwrap_err();
        assert!(err.starts_with(expected), "{err}");
    }
    assert!(!paths.0.join("data/x.jpg").exists());
}

/// A Rust source's `fs.transformImage` (LLP 1133.019 D17): the record is
/// D14's options beside the two paths, and the answer is D14's record; and
/// `fs.compressImage` takes the edit as TypeScript's call does (D15).
#[test]
fn transform_image_answers_a_rust_source_as_typescripts() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.activate().unwrap();
    run(
        &mut host,
        storage::request("fs.mkdir", json!({"path":"app:/data/in"})),
    )
    .unwrap();
    std::fs::write(
        paths.0.join("data/in/photo.jpg"),
        include_bytes!("../../../scripts/fixtures/picker/oriented-gps.jpg"),
    )
    .unwrap();
    let transform = |to: &str, options: Json| {
        let mut args = json!({"path":"app:/data/in/photo.jpg","destination":to});
        args.as_object_mut()
            .unwrap()
            .extend(options.as_object().unwrap().clone());
        storage::request("fs.transformImage", args)
    };
    let result = run(
        &mut host,
        transform("app:/data/turned", json!({"rotate":90,"metadata":"none"})),
    );
    let cropped = run(
        &mut host,
        storage::request(
            "fs.compressImage",
            json!({"path":"app:/data/in/photo.jpg","destination":"app:/data/crop.jpg","maxDimension":100,"maxBytes":2_000_000,
                "crop":{"x":0,"y":0,"width":24,"height":64}}),
        ),
    );
    if cfg!(target_vendor = "apple") {
        let size = std::fs::metadata(paths.0.join("data/turned.jpg"))
            .unwrap()
            .len();
        assert_eq!(
            result.unwrap(),
            json!({"path":"app:/data/turned.jpg","type":"image/jpeg","size":size,"width":64,"height":48,
                "colorSpace":"srgb","hdr":false,"metadata":"none"})
        );
        assert_eq!(cropped.unwrap()["width"], 24);
        let err = run(
            &mut host,
            transform(
                "app:/data/x.jpg",
                json!({"crop":{"x":0,"y":100,"width":1,"height":1}}),
            ),
        )
        .unwrap_err();
        assert!(err.starts_with("transformImage: outside: "), "{err}");
    } else {
        assert_eq!(
            result.unwrap_err(),
            "transformImage: unsupported: no image codec"
        );
        assert!(cropped.is_err());
    }
    for (request, expected) in [
        (
            transform("app:/cache/out.jpg", json!({})),
            "denied: fs.write",
        ),
        (
            transform("doc:/1/out.jpg", json!({})),
            "transformImage: needs app:/ paths",
        ),
    ] {
        let err = run(&mut host, request).unwrap_err();
        assert!(err.starts_with(expected), "{err}");
    }
    if cfg!(target_vendor = "apple") {
        let err = run(
            &mut host,
            transform("app:/data/x.jpg", json!({"flip":"both"})),
        )
        .unwrap_err();
        assert!(err.starts_with("transformImage: invalid: flip"), "{err}");
    }
    assert!(!paths.0.join("data/x.jpg").exists());
}

/// A Rust source's upload from a file (LLP 1108 D6 R2): the storage composer
/// passes `Request::body_from` and `body_to` through untouched, the paths
/// alone, for the host's executor to read and write when it runs the request.
#[test]
fn a_rust_sources_body_from_passes_to_the_host_as_its_path() {
    let mut host = Storage::new(Fixture::new());
    let mut upload = Request::get("https://example.test/upload")
        .body_from("app:/data/photo.jpg")
        .body_to("app:/cache/reply.json");
    upload.method = "POST".into();
    host.source.request = upload.clone();
    host.configure_storage("data".into(), "cache".into(), "tmp".into())
        .unwrap();
    host.activate().unwrap();
    let mut store = Store::default();
    let Answer::Later(request) = host.answer(&mut store, "upload", &[]).unwrap() else {
        panic!("an upload is a request")
    };
    assert_eq!(request, upload);
}

/// A losing cancellation is not an abandoned upload. Consume the successful
/// output only after the pinned-parent publication returns its real result.
#[test]
fn image_cancellation_orders_publication_and_the_consumer_reads_only_success() {
    use ibex2::stdlib::{app_fs::AppDirectories, fs_image::*};
    use std::sync::{mpsc, Arc};
    let paths = Paths::new();
    let roots = paths.directories();
    for path in [&roots.data, &roots.cache, &roots.temporary] {
        std::fs::create_dir_all(path).unwrap();
    }
    std::fs::write(roots.data.join("source"), b"source").unwrap();
    std::fs::write(roots.data.join("out.jpg"), b"old").unwrap();
    let directories =
        Arc::new(AppDirectories::new(&roots.data, &roots.cache, &roots.temporary).unwrap());
    let grants = Arc::new(ibex2::grant::GrantSet::parse(GRANTS).unwrap());
    struct Codec;
    impl ImageCodec for Codec {
        fn limits(&self) -> CodecLimits {
            CodecLimits {
                max_pixels: 1,
                max_scratch_bytes: 1,
            }
        }
        fn encode(
            &self,
            source: &[u8],
            _: ImageRequest,
            control: &WorkControl,
        ) -> Result<CompressedImage, ibex2::boundary::HostError> {
            control.check()?;
            assert_eq!(source, b"source");
            Ok(CompressedImage {
                bytes: b"published".to_vec(),
                width: 1,
                height: 1,
                ..Default::default()
            })
        }
    }
    let limits = WorkLimits::default();
    let work = ImageWork::new(limits);
    let service = ImageService::new(Arc::new(Codec), limits).unwrap();
    let prepare = || {
        ImageJob::prepare(
            &work,
            Some(&service),
            Some(directories.clone()),
            grants.clone(),
            "app:/data/source",
            "app:/data/out.jpg",
            ImageRequest {
                max_dimension: 1,
                max_bytes: 100,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let abandoned = prepare();
    let first = abandoned.handle();
    assert_eq!(first.cancel(), CancelResult::Abandoned);
    assert!(abandoned.execute().is_err());
    assert_eq!(std::fs::read(roots.data.join("out.jpg")).unwrap(), b"old");
    assert!(first.take_outcome().unwrap().is_err());

    let job = prepare();
    let second = job.handle();
    assert_ne!(first.id(), second.id());
    let (entered, observed) = mpsc::sync_channel(1);
    let (release, proceed) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        job.execute_with_commit_observer(move |_| {
            entered.send(()).unwrap();
            proceed
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap();
        })
    });
    observed
        .recv_timeout(std::time::Duration::from_secs(20))
        .unwrap();
    assert_eq!(second.cancel(), CancelResult::CommitStarted);
    assert_eq!(
        first.cancel(),
        CancelResult::Abandoned,
        "an old handle cannot cancel new work"
    );
    assert_eq!(std::fs::read(roots.data.join("out.jpg")).unwrap(), b"old");
    release.send(()).unwrap();
    let result = worker.join().unwrap().unwrap();
    assert_eq!(second.take_outcome(), Some(Ok(result)));
    let outcome = super::native::run(
        Some(&roots),
        GRANTS,
        br#"{"version":1,"op":"fs.readFile","args":{"path":"app:/data/out.jpg"}}"#,
        0,
    );
    let Outcome::Storage(bytes) = outcome else {
        panic!("the committed output is consumable")
    };
    assert_eq!(
        serde_json::from_slice::<Json>(&bytes).unwrap()["base64"],
        exact_runner::agent::base64(b"published")
    );
    assert_eq!(work.outstanding_workers(), 0);
    assert_eq!(work.pending_completions(), 0);
}

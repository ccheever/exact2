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

/// Studio diary R14: a document a launch hands over is sent to the data
/// module before first pixel; its storage request waits for activation
/// rather than being refused and lost. A resource is still refused (it is
/// asked again at `data_ready`), and validation never runs one.
#[test]
fn a_send_before_activation_waits_for_storage_and_runs_once_it_is_ready() {
    let paths = Paths::new();
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    let mut store = Store::default();
    assert!(host
        .answer_for(Target::Resource(0), &mut store, "operation", &[])
        .is_err());
    let Answer::Later(request) = host
        .answer_for(Target::Mutation(0), &mut store, "operation", &[])
        .unwrap()
    else {
        panic!("expected a held request")
    };
    let token = request.continuation.unwrap();
    assert!(matches!(host.dispatch(token, &store), Dispatch::Held));
    assert!(
        host.release(&store).is_empty(),
        "nothing runs before activation"
    );
    assert!(!paths.0.exists());
    host.activate().unwrap();
    let mut released = host.release(&store);
    assert_eq!(released.len(), 1);
    let (released_token, Dispatch::Run(exact_runner::Work::Now(job))) = released.remove(0) else {
        panic!("expected the held work")
    };
    assert_eq!(released_token, token);
    std::thread::spawn(job).join().unwrap();
    assert_eq!(std::fs::read(paths.0.join("data/note")).unwrap(), b"hello");
    assert!(host.release(&store).is_empty(), "released once");
    // An early request outside the app's grants fails when it runs.
    let mut host = Storage::new(Fixture::new());
    paths.configure(&mut host);
    host.source.request =
        storage::request("fs.writeFile", json!({"path":"app:/cache/x","text":"no"}));
    let Answer::Later(request) = host
        .answer_for(Target::Mutation(0), &mut store, "operation", &[])
        .unwrap()
    else {
        panic!("expected a held request")
    };
    assert!(matches!(
        host.dispatch(request.continuation.unwrap(), &store),
        Dispatch::Held
    ));
    host.activate().unwrap();
    let (_, Dispatch::Run(exact_runner::Work::Now(job))) = host.release(&store).remove(0) else {
        panic!("expected the held work")
    };
    assert!(storage::response(std::thread::spawn(job).join().unwrap()).is_err());
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
        payload
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
    fn forgotten(&mut self, in_flight: &[exact_runner::InFlight<'_>]) {
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
    host.forgotten(&[in_flight(second)]);
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
    host.forgotten(&[in_flight(second)]);
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
        storage::response(super::native::run(None, grants, &payload))
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
    assert!(refused.unwrap_err().contains("fs.write doc:/"));
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
    assert!(gone.unwrap_err().contains("no such document"));
    let _ = std::fs::remove_dir_all(dir);
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
        storage::request("fs.writeFile", json!({"path":"app:/data/x","text":"x"})),
    );
    assert_eq!(
        refused,
        Err("storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)".into())
    );
    host.agent = false;
    host.source.request = storage::request("fs.readFile", json!({"path":"app:/data/x"}));
    assert!(host
        .answer(&mut Store::default(), "operation", &[])
        .is_err());
}

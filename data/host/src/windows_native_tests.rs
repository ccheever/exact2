//! @ref LLP 1027.001#proposed-windows-native-filesystem-grant-integration — real worker admission.
use super::*;
use exact_data::storage;
use exact_runner::Request;
use serde_json::{json, Value as Json};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    grants: String,
    request: Request,
}
impl DataSource for Fixture {
    fn app_id(&self) -> &str {
        "com.exact.native-grants-test"
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("fixture produces a worker request")
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(self.request.clone()))
    }
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "exact native 雪 {}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("admitted")).unwrap();
        std::fs::create_dir(root.join("outside")).unwrap();
        Self(root)
    }
    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
    fn grants(&self) -> String {
        let target = serde_json::to_string(&self.path("admitted")).unwrap();
        format!("fs.read {target}\nfs.write {target}")
    }
    fn host(&self) -> Storage<Fixture> {
        let mut host = Storage::new(Fixture {
            grants: self.grants(),
            request: storage::request("fs.stat", json!({"path":self.path("admitted")})),
        });
        host.activate().unwrap();
        host
    }
    fn configure(&self, host: &mut Storage<Fixture>) {
        host.active = false;
        host.configure_storage(
            self.0.join("app/data"),
            self.0.join("app/cache"),
            self.0.join("app/temporary"),
        )
        .unwrap();
        host.activate().unwrap();
    }
    fn untouched_app_roots(&self) {
        assert!(!self.0.join("app").exists());
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn work(host: &mut Storage<Fixture>) -> Result<Box<dyn FnOnce() -> Outcome + Send>, DataError> {
    let mut store = Store::default();
    let Answer::Later(request) = host.answer(&mut store, "request", &[])? else {
        panic!("expected a deferred storage request")
    };
    assert!(store.reads() > 0);
    assert!(request.storage.is_none());
    let token = request.continuation.unwrap();
    let work = host.continuation(token).unwrap();
    assert!(host.continuation(token).is_none());
    Ok(work)
}
fn run(host: &mut Storage<Fixture>, request: Request) -> Result<Json, String> {
    host.source.request = request;
    let work = work(host).map_err(|e| format!("{e:?}"))?;
    storage::response(std::thread::spawn(work).join().unwrap())
}

#[test]
fn quoted_native_requests_use_the_existing_worker_without_app_storage() {
    let files = Files::new();
    let mut host = files.host();
    let path = files.path("admitted/note café");
    run(
        &mut host,
        storage::request("fs.writeFile", json!({"path":path,"text":"one"})),
    )
    .unwrap();
    run(
        &mut host,
        storage::request("fs.appendFile", json!({"path":path,"text":" two"})),
    )
    .unwrap();
    assert_eq!(
        run(
            &mut host,
            storage::request("fs.readFile", json!({"path":path}))
        )
        .unwrap(),
        json!({"base64":"b25lIHR3bw=="})
    );
    let copy = files.path("admitted/copied");
    run(
        &mut host,
        storage::request("fs.copyFile", json!({"path":path,"destination":copy})),
    )
    .unwrap();
    run(
        &mut host,
        storage::request("fs.atomicWriteFile", json!({"path":copy,"text":"replaced"})),
    )
    .unwrap();
    let renamed = files.path("admitted/renamed");
    run(
        &mut host,
        storage::request("fs.rename", json!({"path":copy,"destination":renamed})),
    )
    .unwrap();
    assert_eq!(std::fs::read(&renamed).unwrap(), b"replaced");
    assert!(!std::path::Path::new(&copy).exists());
    let stat = run(
        &mut host,
        storage::request("fs.stat", json!({"path":renamed})),
    )
    .unwrap();
    assert_eq!(stat["isFile"], true);
    assert_eq!(stat["size"], 8);
    assert!(run(
        &mut host,
        storage::request("fs.realpath", json!({"path":renamed}))
    )
    .unwrap()
    .as_str()
    .unwrap()
    .ends_with("renamed"));
    run(
        &mut host,
        storage::request("fs.mkdir", json!({"path":files.path("admitted/new/deep")})),
    )
    .unwrap();
    let names = run(
        &mut host,
        storage::request("fs.readdir", json!({"path":files.path("admitted")})),
    )
    .unwrap();
    assert!(names.as_array().unwrap().contains(&json!("renamed")));
    run(
        &mut host,
        storage::request("fs.rm", json!({"path":files.path("admitted/new")})),
    )
    .unwrap();
    assert!(!files.0.join("admitted/new").exists());
    files.untouched_app_roots();
}

#[test]
fn denied_malformed_and_cross_namespace_requests_never_initialize_app_roots() {
    let files = Files::new();
    let mut host = files.host();
    files.configure(&mut host);
    let path = files.path("admitted/source");
    std::fs::write(&path, b"source").unwrap();
    let outside = files.path("outside/sentinel");
    std::fs::write(&outside, b"outside").unwrap();
    for bad in [
        outside.clone(),
        files.path("admitted/../outside/sentinel"),
        "C:relative".into(),
        r"\\server\share\x".into(),
        r"\\.\C:\x".into(),
        "C:/bad:stream".into(),
        "C:/CON.txt".into(),
        "C:/bad\0".into(),
        "C:/bad\u{85}".into(),
        "C:/trailing.".into(),
        "C:/trailing ".into(),
        format!("C:/{}", "x".repeat(256)),
        "/posix".into(),
    ] {
        assert!(
            run(
                &mut host,
                storage::request("fs.writeFile", json!({"path":bad,"text":"damage"}))
            )
            .is_err(),
            "{bad:?}"
        );
        files.untouched_app_roots();
    }
    for op in ["fs.copyFile", "fs.rename"] {
        for (from, to) in [(&path, &outside), (&outside, &path)] {
            assert!(run(
                &mut host,
                storage::request(op, json!({"path":from,"destination":to}))
            )
            .is_err());
        }
        assert!(run(
            &mut host,
            storage::request(op, json!({"path":path,"destination":"app:/data/new"}))
        )
        .is_err());
    }
    assert!(run(
        &mut host,
        storage::request("fs.unknown", json!({"path":path}))
    )
    .is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"source");
    assert_eq!(std::fs::read(outside).unwrap(), b"outside");
    files.untouched_app_roots();
}

#[test]
fn source_scope_is_original_lines_and_lifecycle_precedes_native_io() {
    let files = Files::new();
    let mut host = files.host();
    let path = files.path("admitted/new");
    let request = storage::request("fs.writeFile", json!({"path":path,"text":"allowed"}));
    let mut empty = request.clone();
    empty.grants = Some(String::new());
    assert!(run(&mut host, empty).is_err());
    let mut respelled = request.clone();
    // Semantically equal drive spelling is not an admitted source declaration.
    respelled.grants = Some(files.grants().replace("\\\\", "/"));
    assert_ne!(respelled.grants.as_deref(), Some(files.grants().as_str()));
    assert!(run(&mut host, respelled).is_err());
    host.activate_for_validation().unwrap();
    assert!(run(&mut host, request.clone()).is_err());
    host.activate().unwrap();
    host.source.request = request;
    let job = work(&mut host).unwrap();
    drop(host);
    assert!(matches!(
        std::thread::spawn(job).join().unwrap(),
        Outcome::Failed {
            kind: exact_runner::FailureKind::Aborted,
            ..
        }
    ));
    assert!(!std::path::Path::new(&path).exists());
    files.untouched_app_roots();
}

#[test]
fn native_sqlite_syntax_does_not_enable_the_app_sqlite_executor() {
    let files = Files::new();
    let mut host = files.host();
    let path = files.path("admitted/test.db");
    host.source.grants = "sqlite.open C:/".into();
    let request = storage::request("sqlite", json!({"path":path,"commands":[]}));
    assert!(run(&mut host, request).is_err());
    files.configure(&mut host);
    assert!(run(
        &mut host,
        storage::request("sqlite", json!({"path":path,"commands":[]}))
    )
    .is_err());
    assert!(!std::path::Path::new(&path).exists());
    files.untouched_app_roots();
}

#[test]
fn mkdir_does_not_create_ungranted_ancestors_and_directory_copy_does_not_create_target() {
    let files = Files::new();
    let mut host = files.host();
    let path = files.path("admitted/missing/granted");
    host.source.grants = format!("fs.write {}", serde_json::to_string(&path).unwrap());
    assert!(run(
        &mut host,
        storage::request("fs.mkdir", json!({"path":path}))
    )
    .is_err());
    assert!(!files.0.join("admitted/missing").exists());
    host.source.grants = files.grants();
    let target = files.path("admitted/not-created");
    assert!(run(
        &mut host,
        storage::request(
            "fs.copyFile",
            json!({"path":files.path("admitted"),"destination":target})
        )
    )
    .is_err());
    assert!(!std::path::Path::new(&target).exists());
    files.untouched_app_roots();
}

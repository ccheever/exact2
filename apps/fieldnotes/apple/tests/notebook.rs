//! The app's actual baked module, native SQLite and filesystem, across restarts.
use exact_js::{Module, Placement};
use exact_js_value::{to_json, Shape};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, Dispatch, Outcome, Reply, RequestOut, Runner, Store, Work};
use serde_json::{json, Value as Json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Run dispatched work the way a host's executor does (LLP 1027.002 D3):
/// `Now` on an I/O thread, `Later` by handing the reply to its owner.
fn run_work(work: Work) -> Outcome {
    match work {
        Work::Now(work) => std::thread::spawn(work).join().unwrap(),
        Work::Later(hand) => {
            let (tx, rx) = std::sync::mpsc::channel();
            hand(Reply::new(move |outcome| {
                let _ = tx.send(outcome);
            }));
            rx.recv_timeout(std::time::Duration::from_secs(30))
                .expect("the owner replied")
        }
    }
}

/// Settle a runner the way a host does: every continuation dispatched on
/// this thread with the store as committed, held work released after each
/// commit, replies fulfilled in order.
fn settle_runner<D: DataSource>(runner: &mut Runner<D>) {
    let mut parked: BTreeMap<u64, RequestOut> = BTreeMap::new();
    for _ in 0..400 {
        if !runner.has_pending() && parked.is_empty() {
            return;
        }
        let mut work = Vec::new();
        let mut take = |r: RequestOut,
                        dispatch: Dispatch,
                        parked: &mut BTreeMap<u64, RequestOut>| {
            match dispatch {
                Dispatch::Run(w) => work.push((r.ticket, w)),
                Dispatch::Held => {
                    parked.insert(r.request.continuation.unwrap(), r);
                }
                Dispatch::Host(_) | Dispatch::Missing => panic!("no native work for {}", r.target),
            }
        };
        for r in runner.take_requests() {
            let token = r.request.continuation.expect("native storage continuation");
            let dispatch = runner.dispatch_work(token);
            take(r, dispatch, &mut parked);
        }
        for (token, dispatch) in runner.release_work() {
            if let Some(r) = parked.remove(&token) {
                take(r, dispatch, &mut parked);
            }
        }
        assert!(
            !work.is_empty() || parked.is_empty(),
            "held work with nothing to release it"
        );
        for (ticket, w) in work {
            let outcome = run_work(w);
            runner.fulfill(ticket, outcome).unwrap();
        }
    }
    panic!("Fieldnotes did not settle");
}

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

/// The app's module without the production per-call deadline, which is not a
/// stable gate on a shared test machine; these fixtures test behavior.
fn module() -> Module {
    let mut module = Module::new(BYTECODE.to_vec(), APP, GRANTS);
    module.set_budget_ms(f64::INFINITY);
    module
}

// Exercise the actual replaceable-module codec on both calls and replies. A
// portable storage request cannot smuggle an executor-local Rust closure here.
struct AbiBackup(exact_logic_abi::Session<fieldnotes_data::Backup>);
impl Default for AbiBackup {
    fn default() -> Self {
        Self(exact_logic_abi::Session::new(
            fieldnotes_data::Backup::default(),
        ))
    }
}
impl DataSource for AbiBackup {
    fn app_id(&self) -> &str {
        APP
    }
    fn grants(&self) -> &str {
        GRANTS
    }
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::Unavailable(source.into()))
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, exact_runner::DataError> {
        let request = exact_logic_abi::call_request(store, source, args, None).unwrap();
        self.0.dispatch(&request).unwrap();
        exact_logic_abi::call_reply(self.0.output(), store)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: exact_runner::Outcome,
    ) -> Result<Answer, exact_runner::DataError> {
        let request = exact_logic_abi::call_request(store, source, args, Some(&outcome)).unwrap();
        self.0.dispatch(&request).unwrap();
        exact_logic_abi::call_reply(self.0.output(), store)
    }
}

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(std::env::temp_dir().join(format!(
            "fieldnotes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Notebook<D = Module> {
    module: D,
    plan: Plan,
    store: Store,
}
impl Notebook<Module> {
    fn open(root: &Root) -> Self {
        Self::with_data(root, module())
    }
}
impl<D: DataSource> Notebook<D> {
    fn with_data(root: &Root, mut module: D) -> Self {
        let plan = Plan::decode(PLAN).unwrap();
        module
            .configure_storage(
                root.0.join("data"),
                root.0.join("cache"),
                root.0.join("temporary"),
            )
            .unwrap();
        module.bind(&plan);
        module.activate().unwrap();
        Self {
            module,
            plan,
            store: Store::new(GRANTS, Vec::<(String, String)>::new()),
        }
    }
    fn call(&mut self, source: &str, args: Vec<Value>) -> Json {
        let mut answer = self.module.answer(&mut self.store, source, &args).unwrap();
        for _ in 0..200 {
            match answer {
                Answer::Now(value) => {
                    let row = self
                        .plan
                        .sources
                        .iter()
                        .find(|r| self.plan.str(r.name) == source)
                        .unwrap();
                    return to_json(&value, &Shape::from_plan(&self.plan, row.ty).unwrap())
                        .unwrap();
                }
                Answer::Later(request) => {
                    assert!(request.url.is_empty(), "offline app never uses HTTP");
                    let token = request.continuation.expect("native storage continuation");
                    // Dispatched the way a host does it (LLP 1027.002 D3):
                    // on this thread, with the store as committed.
                    let outcome = match self.module.dispatch(token, &self.store) {
                        Dispatch::Run(work) => run_work(work),
                        Dispatch::Held => panic!("nothing else holds a turn here"),
                        Dispatch::Host(_) | Dispatch::Missing => panic!("no native work"),
                    };
                    answer = self
                        .module
                        .parse(&mut self.store, source, &args, outcome)
                        .unwrap();
                }
            }
        }
        panic!("{source} did not settle");
    }
    fn library(&mut self, query: &str) -> Json {
        let answer = self.call(
            "library",
            vec![Value::str(query), Value::Number(0.), Value::Number(0.)],
        );
        assert_eq!(answer["ready"], true, "{answer}");
        answer
    }
    fn save(&mut self, id: &str, title: &str, body: &str, pinned: bool) -> Json {
        let answer = self.call(
            "saveNote",
            vec![
                Value::str(id),
                Value::str(title),
                Value::str(body),
                Value::Bool(pinned),
                Value::Number(1.),
            ],
        );
        assert_eq!(answer["failed"], false, "{answer}");
        answer
    }
}

#[test]
fn backup_moves_between_typescript_and_rust_with_the_same_database_and_file() {
    use exact_data_host::Storage;
    let root = Root::new();
    let mut ts = Notebook::open(&root);
    let saved = ts.save("", "Café 🌿", "京都\nQuotes \" and backslash \\", true);
    let id = saved["id"].as_str().unwrap();
    ts.save(
        "",
        "Other page",
        "Tabs\tand\nnewlines\u{2028}line separator\u{2029}paragraph separator",
        false,
    );
    drop(ts);
    let mut ts = Notebook::open(&root);
    let expected = ts.call("backupNotes", vec![]);
    assert_eq!(expected["failed"], false, "{expected}");
    drop(ts);

    let mut rust = Notebook::with_data(&root, Storage::new(AbiBackup::default()));
    let actual = rust.call("backupNotes", vec![]);
    assert_eq!(
        actual, expected,
        "source move preserves the full declared result"
    );
    assert_eq!(
        std::fs::read_to_string(root.0.join("data/backups/fieldnotes.json")).unwrap(),
        actual["backupText"].as_str().unwrap()
    );
    drop(rust);

    let mut ts = Notebook::open(&root);
    assert_eq!(
        ts.call("readBackup", vec![])["backupText"],
        actual["backupText"]
    );
    ts.call("deleteNote", vec![Value::str(id)]);
    assert_eq!(ts.library("")["total"], 1.0);
    assert_eq!(
        ts.call("restoreNotes", vec![Value::str("")])["failed"],
        false
    );
    assert_eq!(ts.library("")["total"], 2.0);
    ts.save(id, "Edited after Rust backup", "Preserved identifier", true);
    drop(ts);

    // The normal host composition routes only backupNotes to Rust. TypeScript
    // sees the unchanged database and can read a backup written by that source.
    let mut mixed = Notebook::with_data(
        &root,
        Storage::new(fieldnotes_data::mixed(module(), exact_js::Placement::Main)),
    );
    assert_eq!(mixed.library("")["notes"][0]["id"], id);
    let backup = mixed.call("backupNotes", vec![]);
    assert_eq!(backup["failed"], false, "{backup}");
    assert!(backup["backupText"]
        .as_str()
        .unwrap()
        .contains("Edited after Rust backup"));
    assert_eq!(
        mixed.call("readBackup", vec![])["backupText"],
        backup["backupText"]
    );
}

#[test]
fn rust_backup_cannot_write_without_the_existing_filesystem_grant() {
    use exact_runner::{DataError, Outcome};
    struct ReadOnly(fieldnotes_data::Backup);
    impl DataSource for ReadOnly {
        fn app_id(&self) -> &str {
            APP
        }
        fn grants(&self) -> &str {
            "sqlite.open app:/data/fieldnotes.db\nsecret.keep fieldnotes.revision"
        }
        fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            self.0.query(source, args)
        }
        fn answer(
            &mut self,
            store: &mut Store,
            source: &str,
            args: &[Value],
        ) -> Result<Answer, DataError> {
            self.0.answer(store, source, args)
        }
        fn parse(
            &mut self,
            store: &mut Store,
            source: &str,
            args: &[Value],
            outcome: Outcome,
        ) -> Result<Answer, DataError> {
            self.0.parse(store, source, args, outcome)
        }
    }
    let root = Root::new();
    Notebook::open(&root).save("", "Keep this note", "No write grant", false);
    let mut rust = Notebook::with_data(
        &root,
        exact_data_host::Storage::new(ReadOnly(fieldnotes_data::Backup::default())),
    );
    let refusal = rust.call("backupNotes", vec![]);
    assert_eq!(refusal["failed"], true, "{refusal}");
    assert_eq!(refusal["backupText"], "");
    assert!(!root.0.join("data/backups/fieldnotes.json").exists());
    drop(rust);
    assert_eq!(Notebook::open(&root).library("")["total"], 1.0);
}

#[test]
fn moving_backup_to_rust_preserves_the_size_error_and_previous_file() {
    let root = Root::new();
    let mut ts = Notebook::open(&root);
    ts.save("", "A small backup", "Retain this spare copy", false);
    assert_eq!(ts.call("backupNotes", vec![])["failed"], false);
    // Each backslash occupies one UTF-16 unit in the note and two in JSON.
    // Every note remains within the app's size limit; the full backup exceeds it.
    let body = "\\".repeat(20_000);
    for _ in 0..104 {
        ts.save("", "A large note", &body, false);
    }
    // Exercise a valid near-limit backup too: the oversize path alone never
    // encodes/writes a multi-megabyte file or returns it through the JS door.
    let near_limit = ts.call("backupNotes", vec![]);
    assert_eq!(near_limit["failed"], false, "{near_limit}");
    let retained = near_limit["backupText"].clone();
    assert!(retained.as_str().unwrap().len() > 4_000_000);
    let mut rust = Notebook::with_data(&root, exact_data_host::Storage::new(AbiBackup::default()));
    assert_eq!(rust.call("backupNotes", vec![])["backupText"], retained);
    drop(rust);
    ts.save("", "The note crossing the limit", &body, false);
    drop(ts);
    let mut ts = Notebook::open(&root);
    let expected = ts.call("backupNotes", vec![]);
    assert_eq!(expected["failed"], true, "{expected}");
    assert_eq!(
        expected["message"],
        "This backup exceeds 4 MB. Split or remove large notes before backing up."
    );
    drop(ts);
    let mut rust = Notebook::with_data(&root, exact_data_host::Storage::new(AbiBackup::default()));
    assert_eq!(rust.call("backupNotes", vec![]), expected);
    assert_eq!(
        std::fs::read_to_string(root.0.join("data/backups/fieldnotes.json")).unwrap(),
        retained.as_str().unwrap(),
    );
}

#[test]
fn mixed_backup_and_repeated_delete_after_reload_refresh_the_visible_library() {
    use exact_kernel::Kernel;
    use exact_runner::Event;
    let settle = settle_runner;
    let root = Root::new();
    let id = Notebook::open(&root).save("", "Delete after backup", "Persisted note", false)["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let second_id = Notebook::open(&root).save("", "Delete after reload", "Another note", false)
        ["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut data =
        exact_data_host::Storage::new(fieldnotes_data::mixed(module(), exact_js::Placement::Main));
    data.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("temporary"),
    )
    .unwrap();
    let mut runner = Runner::boot(
        Plan::decode(PLAN).unwrap(),
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    runner.data().activate().unwrap();
    runner.data_ready().unwrap();
    settle(&mut runner);
    for target in [
        &format!("note-{id}"),
        "backups",
        "save-backup",
        "backups",
        "delete-note",
        "confirm-delete",
    ] {
        let key = runner.kernel().find_by_test_id(target)[0];
        let view = runner.kernel().node_by_key(key).unwrap().id;
        runner.dispatch(view, Event::Press).unwrap();
        settle(&mut runner);
    }
    assert!(runner
        .kernel()
        .find_by_test_id(&format!("note-{id}"))
        .is_empty());
    assert_eq!(runner.store().get("fieldnotes.revision"), Some("2"));
    let carried = runner.carry();
    drop(runner);
    let mut data =
        exact_data_host::Storage::new(fieldnotes_data::mixed(module(), exact_js::Placement::Main));
    data.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("temporary"),
    )
    .unwrap();
    let mut runner = Runner::boot_carrying(
        Plan::decode(PLAN).unwrap(),
        data,
        Kernel::with_monospace(),
        &carried,
        Default::default(),
        "/",
    )
    .unwrap();
    runner.data().activate().unwrap();
    runner.data_ready().unwrap();
    settle(&mut runner);
    for target in [
        &format!("note-{second_id}"),
        "delete-note",
        "confirm-delete",
    ] {
        let key = runner.kernel().find_by_test_id(target)[0];
        let view = runner.kernel().node_by_key(key).unwrap().id;
        runner.dispatch(view, Event::Press).unwrap();
        settle(&mut runner);
    }
    assert_eq!(runner.store().get("fieldnotes.revision"), Some("3"));
    assert!(!runner.kernel().find_by_test_id("empty-notebook").is_empty());
}

#[test]
fn mutation_revisions_follow_the_store_across_language_changes_and_failures() {
    let root = Root::new();
    let mut ts = Notebook::open(&root);
    assert_eq!(
        ts.save("", "Shared revision", "One store", false)["revision"],
        1.0
    );
    assert_eq!(
        ts.store.get("fieldnotes.revision"),
        Some("1"),
        "TypeScript persists its revision in the shared Store"
    );
    let snapshot = ts.store.snapshot();
    drop(ts);
    let mut rust = Notebook::with_data(&root, exact_data_host::Storage::new(AbiBackup::default()));
    rust.store = Store::new(GRANTS, snapshot);
    assert_eq!(
        rust.store.get("fieldnotes.revision"),
        Some("1"),
        "the Rust executor receives the carried Store"
    );
    assert_eq!(rust.call("backupNotes", vec![])["revision"], 2.0);
    let snapshot = rust.store.snapshot();
    drop(rust);
    let mut ts = Notebook::open(&root);
    ts.store = Store::new(GRANTS, snapshot);
    assert_eq!(ts.call("readBackup", vec![])["revision"], 3.0);
    let failed = ts.call("restoreNotes", vec![Value::str("invalid backup")]);
    assert_eq!(failed["failed"], true);
    assert_eq!(failed["revision"], 4.0);
    assert_eq!(ts.store.get("fieldnotes.revision"), Some("4"));
}

#[test]
fn notebook_crud_search_pin_backup_restore_and_native_restart() {
    let root = Root::new();
    assert!(!root.0.exists());
    let mut app = Notebook::open(&root);
    assert_eq!(app.library("")["total"], 0.0);
    let first = app.save(
        "",
        "Café walk 🌿",
        "京都でコーヒー\nA quiet afternoon.",
        true,
    );
    let first_id = first["id"].as_str().unwrap();
    let second = app.save("", "Shopping", "Milk and bread", false);
    let second_id = second["id"].as_str().unwrap();
    assert_eq!(app.library("")["notes"][0]["id"], first_id);
    assert_eq!(app.library("QUIET")["notes"].as_array().unwrap().len(), 1);
    let updated = app.save(
        first_id,
        "Café walk 🌿",
        "京都でコーヒー\nA quiet evening.",
        true,
    );
    assert_eq!(updated["id"], first_id);
    assert_eq!(app.library("")["total"], 2.0);
    let backup = app.call("backupNotes", vec![]);
    assert_eq!(backup["failed"], false, "{backup}");
    let backup_text = backup["backupText"].as_str().unwrap().to_owned();
    let file = root.0.join("data/backups/fieldnotes.json");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), backup_text);
    let reloaded_backup = app.call("readBackup", vec![]);
    assert_eq!(reloaded_backup["backupText"], backup_text);
    drop(app);
    let mut app = Notebook::open(&root);
    assert_eq!(app.library("")["total"], 2.0);
    assert_eq!(
        app.call("openNote", vec![Value::str(first_id), Value::Number(1.)])["body"],
        "京都でコーヒー\nA quiet evening."
    );
    let deletion = app.call("deleteNote", vec![Value::str(second_id)]);
    assert_eq!(deletion["failed"], false);
    assert_eq!(app.library("")["total"], 1.0);
    let invalid=json!({"version":1,"notes":[{"id":"1","title":"Replacement","body":"x","pinned":false},{"id":"1","title":"Duplicate","body":"x","pinned":false}]}).to_string();
    let refusal = app.call("restoreNotes", vec![Value::str(&invalid)]);
    assert_eq!(refusal["failed"], true);
    assert_eq!(app.library("")["notes"][0]["title"], "Café walk 🌿");
    let restored = app.call("restoreNotes", vec![Value::str("")]);
    assert_eq!(restored["failed"], false, "{restored}");
    assert_eq!(app.library("")["total"], 2.0);
    let third = app.save("", "New after restore", "A new page", false);
    assert_ne!(third["id"], first_id);
    assert_ne!(third["id"], second_id);
    assert_eq!(app.library("")["total"], 3.0);
    drop(app);
    let other = Root::new();
    assert_eq!(Notebook::open(&other).library("")["total"], 0.0);
}

#[test]
fn apple_module_replacement_configures_storage_before_activation_and_refreshes_library() {
    use exact_apple::abi::{Bridge, Hooks};
    const CHILD: &str = "EXACT_FIELDNOTES_RELOAD_TEST";
    if std::env::var_os(CHILD).is_none() {
        for agent in [false, true] {
            let home = Root::new();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args(["--exact", "apple_module_replacement_configures_storage_before_activation_and_refreshes_library"])
                .env(CHILD, "1").env("HOME", &home.0).env_remove("EXACT_AGENT").env_remove("EXACT_STORE");
            if agent {
                child.env("EXACT_AGENT", "1");
            }
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "agent={agent}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    fn output(bridge: &Bridge<Module>, length: u32) -> Json {
        serde_json::from_slice(bridge.output_bytes(length as usize)).unwrap()
    }
    fn state(bridge: &mut Bridge<Module>) -> Json {
        let length = bridge.input_write(br#"{"op":"state"}"#);
        let length = bridge.agent(length);
        output(bridge, length)
    }
    fn settle(bridge: &mut Bridge<Module>, total: f64) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let length = bridge.pump(0.0);
            let batch = output(bridge, length);
            assert!(batch["error"].is_null(), "{batch}");
            let state = state(bridge);
            if state["pending"].as_array().is_some_and(Vec::is_empty)
                && state["resources"]["library"]["total"].as_f64() == Some(total)
            {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "library did not refresh to {total}: {state}"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    let app_root = Root(
        PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library/Application Support/exact")
            .join(APP),
    );
    let agent = std::env::var_os("EXACT_AGENT").is_some();
    let mut bridge = Bridge::new();
    let new_module = module;
    let length = bridge.boot(PLAN, new_module(), Hooks::none(), 1100.0, 760.0);
    assert!(output(&bridge, length)["error"].is_null());
    assert!(!app_root.0.exists(), "cold boot must not open app storage");
    let length = bridge.data_ready();
    assert!(output(&bridge, length)["error"].is_null());
    if !agent {
        settle(&mut bridge, 0.0);
        // Change SQLite outside the live runner's kept startup answer. Reload
        // must ask the external-reading resource again, not carry total=0.
        let mut notebook = Notebook::open(&app_root);
        notebook.save("", "Written before replacement", "Persisted note", false);
    }
    let receipt = include_bytes!(concat!(env!("OUT_DIR"), "/app.module.json"));
    let payload = [PLAN, receipt, BYTECODE].concat();
    if !agent {
        let live_state = state(&mut bridge);
        let moved = app_root.0.with_extension("retained");
        std::fs::rename(&app_root.0, &moved).unwrap();
        // Even a direct consumer whose replacement inherits configured paths
        // must not recreate directories or start storage while just preparing.
        let mut admitted = new_module();
        admitted
            .configure_storage(
                app_root.0.join("data"),
                app_root.0.join("cache"),
                app_root.0.join("temporary"),
            )
            .unwrap();
        bridge.input_write(&payload);
        let length = bridge.prepare_module(
            [PLAN.len(), receipt.len(), BYTECODE.len()],
            admitted,
            Hooks::none(),
            1100.0,
            760.0,
        );
        assert!(output(&bridge, length)["error"].is_null());
        assert!(!app_root.0.exists(), "unaccepted candidate opened storage");
        let mut sibling = Bridge::new();
        sibling.input_write(b"invalid plan");
        let length = sibling.prepare_plan(12, new_module(), Hooks::none(), 1100.0, 760.0);
        assert!(
            !output(&sibling, length)["error"].is_null(),
            "other session must refuse"
        );
        bridge.discard_plan();
        assert!(
            !app_root.0.exists(),
            "discarded candidate left storage effects"
        );
        assert_eq!(
            state(&mut bridge),
            live_state,
            "candidate refusal changed live state"
        );
        std::fs::rename(moved, &app_root.0).unwrap();
    }
    bridge.input_write(&payload);
    let length = bridge.prepare_module(
        [PLAN.len(), receipt.len(), BYTECODE.len()],
        new_module(),
        Hooks::none(),
        1100.0,
        760.0,
    );
    let prepared = output(&bridge, length);
    assert!(prepared["error"].is_null(), "{prepared}");
    let length = bridge.commit_plan();
    assert!(output(&bridge, length)["error"].is_null());
    let length = bridge.data_ready();
    let ready = output(&bridge, length);
    assert!(
        ready["error"].is_null(),
        "replacement cannot configure an already loaded module: {ready}"
    );
    if agent {
        assert!(
            !app_root.0.exists(),
            "agent replacement must not open app storage"
        );
    } else {
        settle(&mut bridge, 1.0);
        // A refused pair cannot replace the working executor or its resources.
        let mut corrupted = payload;
        *corrupted.last_mut().unwrap() ^= 1;
        bridge.input_write(&corrupted);
        bridge.prepare_module(
            [PLAN.len(), receipt.len(), BYTECODE.len()],
            new_module(),
            Hooks::none(),
            1100.0,
            760.0,
        );
        let length = bridge.commit_plan();
        assert_eq!(output(&bridge, length)["error"], "no prepared plan");
        settle(&mut bridge, 1.0);
        assert_eq!(Notebook::open(&app_root).library("")["total"], 1.0);
    }
    drop(bridge);
}

#[test]
fn rust_backup_keeps_one_snapshot_when_another_writer_changes_a_later_note() {
    let root = Root::new();
    let mut ts = Notebook::open(&root);
    let first = ts.save("", "Before", "Original body", false)["id"]
        .as_str()
        .unwrap()
        .to_string();
    for _ in 0..24 {
        ts.save("", "Other note", "Other body", false);
    }
    let mut rust = Notebook::with_data(&root, exact_data_host::Storage::new(AbiBackup::default()));
    let Answer::Later(request) = rust
        .module
        .answer(&mut rust.store, "backupNotes", &[])
        .unwrap()
    else {
        panic!("backup must read storage");
    };
    let schema = rust
        .module
        .continuation(request.continuation.unwrap())
        .unwrap();
    let Answer::Later(request) = rust
        .module
        .parse(
            &mut rust.store,
            "backupNotes",
            &[],
            std::thread::spawn(schema).join().unwrap(),
        )
        .unwrap()
    else {
        panic!("backup must read its snapshot after checking the schema");
    };
    let read = rust
        .module
        .continuation(request.continuation.unwrap())
        .unwrap();
    let outcome = std::thread::spawn(read).join().unwrap();
    // This note would belong to a later OFFSET page. The captured read must
    // remain one SQLite snapshot even when another writer commits now.
    ts.save(&first, "After", "Changed after the read", false);
    let mut answer = rust
        .module
        .parse(&mut rust.store, "backupNotes", &[], outcome)
        .unwrap();
    for _ in 0..10 {
        match answer {
            Answer::Now(_) => {
                let saved: Json = serde_json::from_slice(
                    &std::fs::read(root.0.join("data/backups/fieldnotes.json")).unwrap(),
                )
                .unwrap();
                assert_eq!(saved["notes"].as_array().unwrap().len(), 25);
                let note = saved["notes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|note| note["id"] == first)
                    .unwrap();
                assert_eq!(note["title"], "Before");
                assert_eq!(note["body"], "Original body");
                return;
            }
            Answer::Later(request) => {
                let work = rust
                    .module
                    .continuation(request.continuation.unwrap())
                    .unwrap();
                answer = rust
                    .module
                    .parse(
                        &mut rust.store,
                        "backupNotes",
                        &[],
                        std::thread::spawn(work).join().unwrap(),
                    )
                    .unwrap();
            }
        }
    }
    panic!("backup did not settle");
}

/// One notebook session, as a host drives it, under a placement pair.
fn drive(root: &Root, typescript: Placement, rust: Placement) -> Vec<Json> {
    let mut app = Notebook::with_data(
        root,
        exact_data_host::Storage::new(fieldnotes_data::mixed(module().placed(typescript), rust)),
    );
    let mut out = Vec::new();
    let first = app.save(
        "",
        "Café walk 🌿",
        "京都でコーヒー\nA quiet afternoon.",
        true,
    );
    let first_id = first["id"].as_str().unwrap().to_owned();
    out.push(first);
    out.push(app.save("", "Shopping", "Milk and bread", false));
    out.push(app.library("quiet"));
    out.push(app.call("backupNotes", vec![]));
    out.push(app.call("readBackup", vec![]));
    out.push(app.call("deleteNote", vec![Value::str(&first_id)]));
    out.push(app.library(""));
    out.push(app.call("restoreNotes", vec![Value::str("")]));
    out.push(app.library(""));
    out.push(json!(app.store.get("fieldnotes.revision")));
    out
}

#[test]
fn worker_placement_gives_the_same_notebook_results_off_the_owner_thread() {
    // LLP 1027.002 §5 step 4: the same ordered operations, the same
    // results, durable data and revisions on every placement pair.
    let expected = drive(&Root::new(), Placement::Main, Placement::Main);
    assert_eq!(expected[9], json!("6"), "{:?}", expected[9]);
    for (typescript, rust) in [
        (Placement::Worker, Placement::Main),
        (Placement::Main, Placement::Worker),
        (Placement::Worker, Placement::Worker),
    ] {
        let actual = drive(&Root::new(), typescript, rust);
        assert_eq!(actual, expected, "{typescript:?}/{rust:?}");
    }
}

#[test]
fn worker_placement_orders_the_notebook_through_the_runner_and_releases_held_turns() {
    use exact_kernel::Kernel;
    use exact_runner::Event;
    let root = Root::new();
    let id = Notebook::open(&root).save("", "Delete after backup", "Persisted note", false)["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut data = exact_data_host::Storage::new(fieldnotes_data::mixed(
        module().placed(Placement::Worker),
        Placement::Worker,
    ));
    data.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("temporary"),
    )
    .unwrap();
    let mut runner = Runner::boot(
        Plan::decode(PLAN).unwrap(),
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    runner.data().activate().unwrap();
    assert_eq!(runner.data().placement(), Placement::Worker);
    runner.data_ready().unwrap();
    settle_runner(&mut runner);
    // Both halves keep `fieldnotes.revision`: one ordered set. A backup, a
    // delete and the library refreshes they cause take their turns one at a
    // time, each against the store as committed, and land in order.
    for target in [
        &format!("note-{id}"),
        "backups",
        "save-backup",
        "backups",
        "delete-note",
        "confirm-delete",
    ] {
        let key = runner.kernel().find_by_test_id(target)[0];
        let view = runner.kernel().node_by_key(key).unwrap().id;
        runner.dispatch(view, Event::Press).unwrap();
        settle_runner(&mut runner);
    }
    assert!(runner
        .kernel()
        .find_by_test_id(&format!("note-{id}"))
        .is_empty());
    assert_eq!(runner.store().get("fieldnotes.revision"), Some("2"));
    assert!(
        std::fs::read_to_string(root.0.join("data/backups/fieldnotes.json"))
            .unwrap()
            .contains("Persisted note")
    );
    assert!(runner.journal().any(|line| line.contains("continuation")));
}

#[test]
fn apple_bridge_runs_the_placed_notebook_through_its_executor_and_pump() {
    // The host's own loop (LLP 1027.002 D3/D4): a continuation dispatched in
    // `emit` after the commit, its work handed to the executor, the reply
    // arriving through `pump`, held turns released after a later commit —
    // with both halves of Fieldnotes on owner threads of their own.
    use exact_apple::abi::{Bridge, Hooks};
    const CHILD: &str = "EXACT_FIELDNOTES_PLACED_BRIDGE_TEST";
    if std::env::var_os(CHILD).is_none() {
        let home = Root::new();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "apple_bridge_runs_the_placed_notebook_through_its_executor_and_pump",
            ])
            .env(CHILD, "1")
            .env("HOME", &home.0)
            .env_remove("EXACT_AGENT")
            // Real files and SQLite under a private HOME; the store writes
            // the backup makes stay in memory instead of the keychain, whose
            // prompt would hold a scripted run.
            .env("EXACT_STORE", "memory");
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    type Data = exact_data_host::Storage<fieldnotes_data::Data<exact_js::Placed<Module>>>;
    fn output(bridge: &Bridge<Data>, length: u32) -> Json {
        serde_json::from_slice(bridge.output_bytes(length as usize)).unwrap()
    }
    fn agent(bridge: &mut Bridge<Data>, op: &str) -> Json {
        let length = bridge.input_write(op.as_bytes());
        let length = bridge.agent(length);
        output(bridge, length)
    }
    fn settle(bridge: &mut Bridge<Data>, total: f64) -> Json {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let length = bridge.pump(0.0);
            let batch = output(bridge, length);
            assert!(batch["error"].is_null(), "{batch}");
            let state = agent(bridge, r#"{"op":"state"}"#);
            if state["pending"].as_array().is_some_and(Vec::is_empty)
                && state["resources"]["library"]["total"].as_f64() == Some(total)
            {
                return state;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "library did not refresh to {total}: {state}"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    fn press(bridge: &mut Bridge<Data>, test_id: &str) {
        let tree = agent(bridge, r#"{"op":"tree"}"#);
        let view = tree["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["props"]["testId"] == test_id)
            .unwrap_or_else(|| panic!("no {test_id} in {tree}"))["id"]
            .as_u64()
            .unwrap() as u32;
        bridge.input_write(b"");
        let length = bridge.dispatch(view, 0, 0, 0.0);
        let batch = output(bridge, length);
        assert!(batch["error"].is_null(), "{batch}");
    }
    let app_root = Root(
        PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library/Application Support/exact")
            .join(APP),
    );
    let data = || {
        exact_data_host::Storage::new(fieldnotes_data::mixed(
            module().placed(Placement::Worker),
            Placement::Worker,
        ))
    };
    let mut bridge = Bridge::new();
    let length = bridge.boot(PLAN, data(), Hooks::none(), 1100.0, 760.0);
    assert!(output(&bridge, length)["error"].is_null());
    let length = bridge.data_ready();
    assert!(output(&bridge, length)["error"].is_null());
    // The library's first real answer comes from the TypeScript owner thread,
    // through the executor's reply and this thread's pump.
    settle(&mut bridge, 0.0);
    Notebook::open(&app_root).save("", "Written beside the host", "Persisted note", false);
    // A Rust backup on its own owner, then the TypeScript library refresh
    // it causes: one ordered set, one turn at a time, both committed here.
    press(&mut bridge, "backups");
    press(&mut bridge, "save-backup");
    let state = settle(&mut bridge, 1.0);
    assert_eq!(state["pending"], json!([]), "{state}");
    assert!(
        std::fs::read_to_string(app_root.0.join("data/backups/fieldnotes.json"))
            .unwrap()
            .contains("Written beside the host"),
        "the Rust owner wrote the backup through the host's storage work"
    );
    let logs = agent(&mut bridge, r#"{"op":"logs"}"#);
    let journal = logs.to_string();
    assert!(journal.contains("continuation"), "{journal}");
    assert!(!journal.contains("dropped"), "{journal}");
}

#[test]
fn library_reads_all_supported_notes_as_previews_and_opens_one_full_body() {
    let root = Root::new();
    let mut app = Notebook::open(&root);
    let body = format!("{}ÉΣKx", "🌿".repeat(9998));
    for n in 1..=1000 {
        app.save("", &format!("Note {n}"), &body, n % 7 == 0);
    }
    let library = app.library("");
    let notes = library["notes"].as_array().unwrap();
    assert_eq!(library["total"], 1000.0);
    assert_eq!(notes.len(), 1000);
    assert!(library.to_string().len() < 300_000);
    let mut expected: Vec<_> = (1..=1000).rev().collect();
    expected.sort_by_key(|id| id % 7 != 0);
    for (note, id) in notes.iter().zip(expected) {
        assert_eq!(note["id"], id.to_string());
        assert!(
            note.get("body").is_none(),
            "list answers carry no full bodies"
        );
        assert_eq!(note["excerpt"], "🌿".repeat(50));
    }
    assert_eq!(app.library("éσk")["notes"].as_array().unwrap().len(), 1000);
    assert_eq!(app.library("absent")["notes"].as_array().unwrap().len(), 0);
    let opened = app.call("openNote", vec![Value::str("999"), Value::Number(1.)]);
    assert_eq!(opened["ready"], true);
    assert_eq!(opened["body"], body);
    app.call("deleteNote", vec![Value::str("999")]);
    let missing = app.call("openNote", vec![Value::str("999"), Value::Number(2.)]);
    assert_eq!(missing["ready"], false);
    assert_eq!(missing["body"], "");
    assert!(missing["message"].as_str().unwrap().contains("deleted"));
    let fresh = app.call("openNote", vec![Value::str(""), Value::Number(3.)]);
    assert_eq!(fresh["ready"], true);
    assert_eq!(fresh["body"], "");
    // Every preview can require a fallback without overflowing one reply.
    let body = format!("\0{}x", "\u{2003}".repeat(19998));
    for n in 1..=1000 {
        if n != 999 {
            app.save(&n.to_string(), "Fallback", &body, n % 7 == 0);
        }
    }
    let previews = app.library("");
    assert_eq!(previews["total"], 999.0);
    assert!(previews["notes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|note| note["excerpt"] == "\0 x"));
    assert_eq!(previews["notes"], app.library("fallback")["notes"]);
}

#[test]
fn loaded_note_preserves_drafts_pending_saves_failures_and_reload() {
    use exact_kernel::Kernel;
    use exact_runner::Event;
    fn event<D: DataSource>(runner: &mut Runner<D>, target: &str, event: Event) {
        let key = runner.kernel().find_by_test_id(target)[0];
        let view = runner.kernel().node_by_key(key).unwrap().id;
        runner.dispatch(view, event).unwrap();
    }
    fn state<D: DataSource>(runner: &Runner<D>) -> Json {
        serde_json::from_str(&exact_runner::agent::state(runner)).unwrap()
    }
    for placement in [Placement::Main, Placement::Worker] {
        let root = Root::new();
        let mut seed = Notebook::open(&root);
        seed.save("", "First", "Original body", false);
        seed.save("", "Second", "Other body", true);
        drop(seed);
        let data = || {
            let mut module = module().placed(placement);
            module
                .configure_storage(
                    root.0.join("data"),
                    root.0.join("cache"),
                    root.0.join("temporary"),
                )
                .unwrap();
            module
        };
        let mut runner = Runner::boot(
            Plan::decode(PLAN).unwrap(),
            data(),
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        runner.data().activate().unwrap();
        runner.data_ready().unwrap();
        settle_runner(&mut runner);
        event(&mut runner, "note-1", Event::Press);
        assert!(runner.has_pending());
        event(
            &mut runner,
            "note-body",
            Event::Input("Must not replace the loading note".into()),
        );
        // A plan-only reload can cancel a pending mutation. No loaded value
        // exists yet, so offer an explicit retry instead of a blank dead editor.
        let carried = runner.carry();
        drop(runner);
        let mut runner = Runner::boot_carrying(
            Plan::decode(PLAN).unwrap(),
            data(),
            Kernel::with_monospace(),
            &carried,
            Default::default(),
            "/",
        )
        .unwrap();
        runner.data().activate().unwrap();
        runner.data_ready().unwrap();
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["openInterrupted"], true);
        assert_eq!(state(&runner)["derives"]["openedReady"], false);
        event(&mut runner, "retry-open", Event::Press);
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["openInterrupted"], false);
        assert_eq!(state(&runner)["derives"]["body"], "Original body");
        event(&mut runner, "note-body", Event::Input("Draft".into()));
        for target in ["note-2", "new-note"] {
            let key = runner.kernel().find_by_test_id(target)[0];
            let view = runner.kernel().node_by_key(key).unwrap().id;
            let node: Json =
                serde_json::from_str(&exact_runner::agent::node(&runner, view)).unwrap();
            assert_eq!(node["props"]["disabled"], true);
        }
        assert_eq!(state(&runner)["derives"]["editingId"], "1");
        assert_eq!(state(&runner)["derives"]["body"], "Draft");
        event(&mut runner, "search", Event::Input("second".into()));
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["body"], "Draft");
        event(&mut runner, "discard", Event::Press);
        assert_eq!(state(&runner)["derives"]["body"], "Original body");
        event(
            &mut runner,
            "note-body",
            Event::Input("Saved snapshot".into()),
        );
        event(&mut runner, "save-note", Event::Press);
        event(
            &mut runner,
            "note-body",
            Event::Input("Typed during save".into()),
        );
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["body"], "Typed during save");
        assert_eq!(state(&runner)["derives"]["dirty"], true);
        event(&mut runner, "discard", Event::Press);
        assert_eq!(state(&runner)["derives"]["body"], "Saved snapshot");
        event(
            &mut runner,
            "note-title",
            Event::Input("x".repeat(161).into()),
        );
        event(&mut runner, "save-note", Event::Press);
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["dirty"], true);
        event(&mut runner, "discard", Event::Press);
        assert_eq!(state(&runner)["derives"]["title"], "First");
        assert_eq!(state(&runner)["derives"]["body"], "Saved snapshot");
        // A carried selected note retains its loaded snapshot and its draft.
        event(
            &mut runner,
            "note-body",
            Event::Input("Carried draft".into()),
        );
        let carried = runner.carry();
        drop(runner);
        let mut runner = Runner::boot_carrying(
            Plan::decode(PLAN).unwrap(),
            data(),
            Kernel::with_monospace(),
            &carried,
            Default::default(),
            "/",
        )
        .unwrap();
        runner.data().activate().unwrap();
        runner.data_ready().unwrap();
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["body"], "Carried draft");
        event(&mut runner, "discard", Event::Press);
        event(&mut runner, "new-note", Event::Press);
        settle_runner(&mut runner);
        event(
            &mut runner,
            "note-body",
            Event::Input("New saved note".into()),
        );
        event(&mut runner, "save-note", Event::Press);
        settle_runner(&mut runner);
        let new_id = state(&runner)["derives"]["editingId"].clone();
        assert_eq!(new_id, "3");
        event(
            &mut runner,
            "note-title",
            Event::Input("x".repeat(161).into()),
        );
        event(&mut runner, "save-note", Event::Press);
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["editingId"], new_id);
        event(&mut runner, "discard", Event::Press);
        assert_eq!(state(&runner)["derives"]["body"], "New saved note");
        assert_eq!(state(&runner)["derives"]["dirty"], false);
        // A previous note's settled result must not masquerade as this load.
        event(&mut runner, "close-search", Event::Press);
        settle_runner(&mut runner);
        event(&mut runner, "note-2", Event::Press);
        assert!(runner.has_pending());
        let carried = runner.carry();
        drop(runner);
        let mut runner = Runner::boot_carrying(
            Plan::decode(PLAN).unwrap(),
            data(),
            Kernel::with_monospace(),
            &carried,
            Default::default(),
            "/",
        )
        .unwrap();
        runner.data().activate().unwrap();
        runner.data_ready().unwrap();
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["openInterrupted"], true);
        event(&mut runner, "retry-open", Event::Press);
        settle_runner(&mut runner);
        assert_eq!(state(&runner)["derives"]["body"], "Other body");
        assert_eq!(state(&runner)["derives"]["openInterrupted"], false);
    }
}

#[test]
fn previews_preserve_unicode_whitespace_truncation_and_large_ids() {
    let root = Root::new();
    let mut app = Notebook::open(&root);
    let bodies = [
        ("", "An empty page.".to_owned()),
        ("   \t\n", " ".to_owned()),
        ("\u{2003}a\u{a0}\t🌿\n", " a 🌿 ".to_owned()),
        ("\0visible", "\0visible".to_owned()),
        ("before\0after", "before\0after".to_owned()),
        ("\u{feff}hello", " hello".to_owned()),
        (
            "\u{fffd}valid replacement",
            "\u{fffd}valid replacement".to_owned(),
        ),
    ];
    for (body, expected) in bodies {
        app.save("", "Excerpt", body, false);
        assert_eq!(app.library("")["notes"][0]["excerpt"], expected);
        assert_eq!(app.library("")["notes"], app.library("excerpt")["notes"]);
    }
    for body in [
        format!("{}visible", " ".repeat(300)),
        format!("{}\0visible", "x".repeat(99)),
        format!("{}\0visible", "x".repeat(100)),
        format!("{}🌿", "x".repeat(99)),
        format!("{}🌿", "x".repeat(199)),
        format!("{}終わり", "\u{2003}".repeat(300)),
    ] {
        app.save("", "Excerpt", &body, false);
        assert_eq!(app.library("")["notes"], app.library("excerpt")["notes"]);
    }
    let body = format!("{}{}🌿", "\u{2003}".repeat(10000), "x".repeat(98));
    let backup = json!({"version":1,"notes":[
        {"id":"9223372036854775807","title":"Higher","body":body,"pinned":false},
        {"id":"9223372036854775806","title":"Lower","body":"lower","pinned":false}
    ]});
    assert_eq!(
        app.call("restoreNotes", vec![Value::str(&backup.to_string())])["failed"],
        false
    );
    let list = app.library("");
    assert_eq!(list["notes"][0]["id"], "9223372036854775807");
    assert_eq!(list["notes"][1]["id"], "9223372036854775806");
    // A preview must not send half an emoji through the JSON seam.
    assert_eq!(list["notes"][0]["excerpt"], format!(" {}", "x".repeat(98)));
    assert_eq!(
        app.library("HIGHER\n")["notes"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn schema_checks_initialize_fresh_and_replaced_databases_without_cached_readiness() {
    let root = Root::new();
    let path = root.0.join("data/fieldnotes.db");
    let mut ts = Notebook::open(&root);
    for source in [
        "library",
        "saveNote",
        "backupNotes",
        "restoreNotes",
        "deleteNote",
        "openNote",
    ] {
        let _ = std::fs::remove_file(&path);
        let args = match source {
            "library" => vec![Value::str(""), Value::Number(0.), Value::Number(0.)],
            "saveNote" => vec![
                Value::str(""),
                Value::str("Fresh"),
                Value::str("Body"),
                Value::Bool(false),
                Value::Number(1.),
            ],
            "restoreNotes" => vec![Value::str(r#"{"version":1,"notes":[]}"#)],
            "deleteNote" => vec![Value::str("1")],
            "openNote" => vec![Value::str("1"), Value::Number(1.)],
            _ => vec![],
        };
        let result = ts.call(source, args);
        if source == "openNote" {
            assert!(
                result["message"].as_str().unwrap().contains("deleted"),
                "{result}"
            );
        } else {
            assert_ne!(result["failed"], true, "{source}: {result}");
        }
        assert_eq!(ts.library("")["ready"], true, "{source}");
    }
    let mut rust = Notebook::with_data(&root, exact_data_host::Storage::new(AbiBackup::default()));
    for _ in 0..2 {
        std::fs::remove_file(&path).unwrap();
        let result = rust.call("backupNotes", vec![]);
        assert_eq!(result["failed"], false, "{result}");
        let backup: Json = serde_json::from_str(result["backupText"].as_str().unwrap()).unwrap();
        assert_eq!(backup["notes"], json!([]));
    }
    // A schema query failure must leave the file alone, not replace damaged data.
    std::fs::write(&path, b"not a SQLite database").unwrap();
    assert_eq!(
        ts.call(
            "library",
            vec![Value::str(""), Value::Number(0.), Value::Number(0.)]
        )["ready"],
        false
    );
    assert_eq!(rust.call("backupNotes", vec![])["failed"], true);
    assert_eq!(std::fs::read(&path).unwrap(), b"not a SQLite database");
}

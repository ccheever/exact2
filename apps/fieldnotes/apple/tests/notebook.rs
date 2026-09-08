//! The app's actual baked module, native SQLite and filesystem, across restarts.
use exact_js::Module;
use exact_js_value::{to_json, Shape};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, Store};
use serde_json::{json, Value as Json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

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
struct Notebook {
    module: Module,
    plan: Plan,
    store: Store,
}
impl Notebook {
    fn open(root: &Root) -> Self {
        let plan = Plan::decode(PLAN).unwrap();
        let mut module = Module::new(BYTECODE.to_vec(), APP, GRANTS);
        module
            .configure_storage(
                root.0.join("data"),
                root.0.join("cache"),
                root.0.join("temporary"),
            )
            .unwrap();
        module.bind(&plan);
        assert!(!module.is_loaded());
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
                    let work = self.module.continuation(token).expect("live work");
                    answer = self
                        .module
                        .parse(
                            &mut self.store,
                            source,
                            &args,
                            std::thread::spawn(work).join().unwrap(),
                        )
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
        app.library("京都")["notes"][0]["body"],
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
    let new_module = || Module::new(BYTECODE.to_vec(), APP, GRANTS);
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

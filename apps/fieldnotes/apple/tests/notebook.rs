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

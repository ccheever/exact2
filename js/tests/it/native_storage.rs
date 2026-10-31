//! The storage a native module is handed (LLP 1154 D3): a SQLite file
//! admitted under the app's grants and held by one SQLite at a time.
#![cfg(exact_js_engine)]
use super::storage::{call, Root, GRANTS};
use exact_js::{NativeStorage, SqliteFile};
use exact_runner::{DataSource, Store};

/// The module asks for `app:/data/notes.db` (granted) or `app:/data/other.db`
/// (not) on `open` and `ungranted`, keeps the file until `close`, and
/// answers the real path's file name.
#[derive(Default)]
struct Native {
    storage: Option<NativeStorage>,
    file: Option<SqliteFile>,
}
impl exact_js::NativeModule for Native {
    fn configure_storage(&mut self, storage: NativeStorage) -> Result<(), String> {
        self.storage = Some(storage);
        Ok(())
    }
    fn call(&mut self, request: &exact_js::Json) -> Result<exact_js::Json, String> {
        let storage = self.storage.as_ref().expect("configured before any call");
        let path = match request["value"].as_str() {
            Some("close") => {
                self.file = None;
                return Ok(exact_js::json!({"text": "closed"}));
            }
            Some("open") => "app:/data/notes.db",
            _ => "app:/data/other.db",
        };
        let file = storage.sqlite(path)?;
        let name = file
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        self.file = Some(file);
        Ok(exact_js::json!({ "text": name }))
    }
}

/// Each order of the two SQLites refuses the second by name, and each
/// reservation ends when its holder lets the file go.
#[test]
fn a_native_modules_sqlite_file_is_granted_and_held_by_one_sqlite_at_a_time() {
    let root = Root::new();
    let mut m = root.module().with_native(|_| Box::new(Native::default()));
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    m.activate().unwrap();
    let mut ask = |op: &str, value: &str| call(&mut m, &mut s, op, value);

    let refused = ask("native", "ungranted");
    assert!(
        refused.contains(
            "app:/data/other.db: no grant covers it; grant `sqlite.open app:/data/other.db`"
        ),
        "{refused}"
    );
    assert!(!root.0.join("data/other.db").exists());

    // The module holds the file: the app's own open is refused, then admitted once it closes.
    assert_eq!(ask("native", "open"), "notes.db");
    let refused = ask("hold", "");
    assert!(
        refused.contains("notes.db is open in the app's native module"),
        "{refused}"
    );
    assert_eq!(ask("native", "close"), "closed");
    assert_eq!(ask("hold", ""), "held");

    // The app's storage holds it: the module is refused, then admitted once the app closes it.
    let refused = ask("native", "open");
    assert!(
        refused.contains("app:/data/notes.db is open in the app's own storage"),
        "{refused}"
    );
    assert_eq!(ask("release", ""), "released");
    assert_eq!(ask("native", "open"), "notes.db");
    assert_eq!(ask("native", "close"), "closed");
}

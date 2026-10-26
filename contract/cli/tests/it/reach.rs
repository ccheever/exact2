//! The system's ways into an app under the agent (LLP 1133.023 §10), through
//! this checkout's `@exact/quick-actions` and `@exact/search-index`
//! installed beside the app: the runner keeps the shortcut list and the
//! content index the host's stores would, refuses what the platforms do,
//! and a source of the index's entries is asked again when they change.

use exact_runner::Runner;
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};

const SOURCE: &str = r#"use setAppShortcuts, clearAppShortcuts, requestPinShortcut, ShortcutItem, PinShortcutResult from "@exact/quick-actions"
use addContentIndexEntry, deleteContentIndexEntry, setContentIndex, contentIndexEntries, ContentDescription from "@exact/search-index"

routes nav
  home "/"
  book "/book/:id"

component App
  derive current = top(nav)
  resource entries = contentIndexEntries() as shape ContentDescription[]
  mutation done as shape PinShortcutResult
  action index(ids: string[])
    setContentIndex(kind="book", entries=ids.map(i => ContentDescription(id=i, kind=none, title=`Bök ${i}`, description=some("Le Guin"), category=none, icons=none, url=`/book/${i}`, apple=none)))
  action add(i: string, url: string)
    addContentIndexEntry(description=ContentDescription(id=i, kind=some("author"), title=i, description=none, category=none, icons=none, url=url, apple=none))
  action remove(i: string)
    deleteContentIndexEntry(id=i)
  action shortcut(url: string)
    setAppShortcuts(shortcuts=[ShortcutItem(name="Open", short_name=none, description=none, url=url, icons=none, apple=none, android=none)])
  action clear
    clearAppShortcuts()
  action pin
    send done = requestPinShortcut(shortcut=ShortcutItem(name="Pinned", short_name=none, description=none, url="/", icons=none, apple=none, android=none))
  action follow(url: string)
    nav = go(nav, url)
  view
    main navigationKey=`${current.id}` navigate=follow
      text `${entries.length}`
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, source: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-reach-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        for package in ["quick-actions", "search-index"] {
            let to = packages.join(package).canonicalize().unwrap();
            symlink_dir(to, path.join("node_modules/@exact").join(package)).unwrap();
        }
        std::fs::write(path.join("app.contract"), source).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
}

fn boot(name: &str) -> (App, Runner<NoData>) {
    let app = App::new(name, SOURCE);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_module_target("ios");
    settle(&mut r);
    (app, r)
}

fn settle(r: &mut Runner<NoData>) {
    for _ in 0..8 {
        if r.apply_announced().0.is_empty() {
            return;
        }
    }
}

fn act(r: &mut Runner<NoData>, name: &str, args: Vec<exact_runner::Value>) {
    r.act(name, args).unwrap();
    settle(r);
}

fn s(text: &str) -> exact_runner::Value {
    exact_runner::Value::from(text)
}

fn ids(ids: &[&str]) -> exact_runner::Value {
    exact_runner::Value::list(ids.iter().map(|i| s(i)).collect())
}

fn logs(r: &Runner<NoData>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

fn extension(r: &Runner<NoData>, name: &str) -> Json {
    let all: Json = serde_json::from_str(&r.extensions_json().unwrap()).unwrap();
    all[name].clone()
}

/// `setContentIndex` replaces one kind's entries and records the
/// difference; an entry added alone keeps its own kind; the source of the
/// entries answers them all, asked again when they change (D4, D9).
#[test]
fn the_index_keeps_entries_by_kind_and_tells_its_readers() {
    let (_app, mut r) = boot("index");
    act(&mut r, "index", vec![ids(&["a", "b", "c"])]);
    let index = extension(&r, "search-index");
    assert_eq!(index["counts"], json!({ "book": 3 }), "{}", logs(&r));
    assert_eq!(index["last"]["added"], json!(["a", "b", "c"]));
    let entries = |r: &Runner<NoData>| match r.resource("entries") {
        Some(exact_runner::Value::List(items)) => items.len(),
        other => panic!("{other:?}"),
    };
    assert_eq!(entries(&r), 3);

    act(&mut r, "add", vec![s("ursula"), s("/book/ursula")]);
    act(&mut r, "index", vec![ids(&["c", "d"])]);
    let index = extension(&r, "search-index");
    assert_eq!(entries(&r), 3);
    assert_eq!(index["counts"], json!({ "book": 2, "author": 1 }));
    assert_eq!(index["last"]["added"], json!(["d"]));
    assert_eq!(index["last"]["deleted"], json!(["a", "b"]));
    assert_eq!(index["last"]["updated"], json!([]));

    act(&mut r, "remove", vec![s("ursula")]);
    let index = extension(&r, "search-index");
    assert_eq!(
        index["last"],
        json!({ "kind": "author", "added": [], "updated": [], "deleted": ["ursula"] })
    );

    // The entities a system query reads: title or description, folded.
    let found = r.index_entities("book", Some("BOK"));
    assert_eq!(found.len(), 2);
    assert!(r.index_entities("book", Some("guin")).len() == 2);
    assert!(r.index_entities("book", Some("tolkien")).is_empty());
}

/// An entry whose url the routes do not match is refused `TypeError`, and
/// the index is kept (D4).
#[test]
fn an_entry_outside_the_routes_is_refused() {
    let (_app, mut r) = boot("refused");
    act(&mut r, "add", vec![s("x"), s("/nowhere/x")]);
    act(&mut r, "add", vec![s("y"), s("https://example.com/y")]);
    let index = extension(&r, "search-index");
    assert_eq!(index["counts"], json!({}), "{}", logs(&r));
    let logs = logs(&r);
    assert!(logs.contains("matches none of the routes"), "{logs}");
    assert!(logs.contains("is an absolute app path"), "{logs}");
}

/// The shortcut list is the app's last, after the manifest's; a url the
/// routes do not match keeps the list; a pin is refused off Android, and
/// there requested, or unsupported while the agent says so (D3, D11).
#[test]
fn shortcuts_are_the_last_list_and_a_pin_is_requested() {
    let (_app, mut r) = boot("shortcuts");
    act(&mut r, "shortcut", vec![s("/book/1")]);
    let quick = extension(&r, "quick-actions");
    assert_eq!(quick["shortcuts"][0]["url"], "/book/1", "{}", logs(&r));
    act(&mut r, "shortcut", vec![s("/elsewhere")]);
    let quick = extension(&r, "quick-actions");
    assert_eq!(quick["shortcuts"][0]["url"], "/book/1", "kept");
    act(&mut r, "clear", vec![]);
    assert_eq!(extension(&r, "quick-actions")["shortcuts"], json!([]));

    act(&mut r, "pin", vec![]);
    assert!(
        logs(&r).contains("NotSupportedError"),
        "a pin is Android's alone"
    );
    r.set_module_target("android");
    act(&mut r, "pin", vec![]);
    assert!(
        logs(&r).contains("requestPinShortcut: requested"),
        "{}",
        logs(&r)
    );
    assert_eq!(
        extension(&r, "quick-actions")["pinRequests"][0]["name"],
        "Pinned"
    );
    let unsupported = r#"{"op":"prefer","extension":{"quick-actions.pinSupported":false}}"#;
    r.extension_agent(unsupported).unwrap().1.unwrap();
    act(&mut r, "pin", vec![]);
    assert!(
        logs(&r).contains("requestPinShortcut: unsupported"),
        "{}",
        logs(&r)
    );
    assert_eq!(
        extension(&r, "quick-actions")["pinRequests"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// Where the platform has no shortcut list, the send is refused
/// `NotSupportedError` (D17).
#[test]
fn shortcuts_are_refused_where_the_platform_has_none() {
    let (_app, mut r) = boot("web");
    r.set_module_target("web");
    act(&mut r, "shortcut", vec![s("/book/1")]);
    assert_eq!(extension(&r, "quick-actions")["shortcuts"], json!([]));
    assert!(logs(&r).contains("NotSupportedError"), "{}", logs(&r));
}

/// LLP 1133.023 D5, D8, D10: `intents.tools` in `app.json` faces a tool. A
/// face is stored with the doors it reaches and its digest; one naming no
/// tool, a parameter of another type, or a door with no confirmation for a
/// `destroys` tool is refused; and a door is held to its face's digest.
#[test]
fn a_face_is_checked_against_its_tool_and_holds_its_door() {
    let tools = "  state last = \"\"\n  tool log writes closed \"Log a page.\" = log\n  tool drop destroys \"Drop a book.\" = drop\n  action log(id: string, page: number)\n    last = `${id} ${page}`\n  action drop(id: string)\n    last = id\n  action follow(url: string)";
    let source = SOURCE.replacen("  action follow(url: string)", tools, 1);
    let app = App::new("faces", &source);
    let write = |intents: &Json| {
        let manifest =
            json!({ "app": { "id": "com.exact.faces", "name": "Faces" }, "intents": intents });
        std::fs::write(app.0.join("app.json"), manifest.to_string()).unwrap();
    };
    let compile =
        || contract::compile_path_all(&app.0.join("app.contract"), false).map(|(plan, _)| plan);
    let face = |plan: &exact_plan::Plan, tool: &str| -> Json {
        let row = plan
            .tools
            .iter()
            .find(|t| plan.str(t.name) == tool)
            .unwrap();
        serde_json::from_str(plan.str(row.face)).unwrap_or_default()
    };
    let good = json!({
        "entities": { "book": { "title": "Book" } },
        "tools": {
            "log": { "title": "Log", "parameters": { "id": { "entity": "book" }, "page": { "type": "integer" } } },
            "drop": { "title": "Drop" }
        }
    });
    write(&good);
    let plan = compile().unwrap_or_else(|e| panic!("{e:?}"));
    let log = face(&plan, "log");
    assert_eq!(log["doors"], json!(["apple", "android", "web"]));
    assert_eq!(
        face(&plan, "drop")["doors"],
        json!(["apple"]),
        "a destroys tool's default"
    );
    let digest = log["digest"].as_str().unwrap().to_owned();
    assert_eq!(digest.len(), 64);

    let refused = |intents: Json, id: &str| {
        write(&intents);
        let errors = format!("{:?}", compile().unwrap_err());
        assert!(errors.contains(id), "{id}: {errors}");
    };
    let mut destroys = good.clone();
    destroys["tools"]["drop"]["doors"] = json!(["apple", "android"]);
    refused(destroys, "intents-destroys-door");
    let mut no_kind = good.clone();
    no_kind["entities"] = json!({});
    refused(no_kind, "intents-face-type");
    let mut not_number = good.clone();
    not_number["tools"]["log"]["parameters"]["id"] = json!({ "type": "integer" });
    refused(not_number, "intents-face-type");
    let mut missing = good.clone();
    missing["tools"]["nothing"] = json!({ "title": "Nothing" });
    refused(missing, "intents-tool");

    // The description is part of the signature, so of the digest.
    let moved = source.replace("Log a page.", "Log the page.");
    std::fs::write(app.0.join("app.contract"), moved).unwrap();
    write(&good);
    let plan = compile().unwrap();
    assert_ne!(face(&plan, "log")["digest"], json!(digest));

    let kernel = exact_kernel::Kernel::with_monospace();
    let mut r = Runner::boot(plan, NoData, kernel, Default::default(), "/").unwrap();
    let mut call = |door: &str, digest: Option<&str>| {
        let how = exact_runner::CallHow {
            door: door.into(),
            digest: digest.map(str::to_owned),
            ..Default::default()
        };
        r.call_tool("log", r#"{"id":"a","page":3}"#, how).unwrap();
        r.advance(r.now_ms()).unwrap();
    };
    call("apple", Some(&digest));
    call("apple", None);
    let state = exact_runner::agent::handle(&r, r#"{"op":"state"}"#);
    let state: Json = serde_json::from_str(&state).unwrap();
    let calls = &state["intents"]["calls"];
    assert_eq!(calls[0]["outcome"], "refused", "a stale digest: {calls}");
    assert!(
        calls[0]["message"].as_str().unwrap().starts_with("face:"),
        "{calls}"
    );
    assert_eq!(calls[1]["outcome"], "ok", "{calls} {}", logs(&r));
}

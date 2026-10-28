//! LLP 1133.006 stage 1, through the repository's `@exact/clipboard`
//! installed beside an app, its implementation played as the host's native
//! slot: a keyed shape (`ClipboardItem`) in a declaration, a send that
//! answers nothing reaching an implementation that is its own substitute
//! under the agent, and each item's `app:/` path admitted against the
//! app's `fs.read` lines.

use exact_runner::{Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const APP: &str = r#"use ClipboardItem from "@exact/clipboard"

component App
  view
    text "copy"
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-clipboard-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@exact/clipboard");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/clipboard");
        for file in [
            "package.json",
            "exact.json",
            "apple/exact.json",
            "web/exact.json",
        ] {
            std::fs::create_dir_all(package.join(file).parent().unwrap()).unwrap();
            std::fs::copy(source.join(file), package.join(file)).unwrap();
        }
        std::fs::write(path.join("app.contract"), APP).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Granted;
impl exact_runner::DataSource for Granted {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "fs.read app:/tmp/copy\n"
    }
}

type Calls = Arc<Mutex<Vec<Json>>>;

/// A runner whose native slot answers each write as the implementation
/// does: with nothing.
fn boot(name: &str) -> (App, Runner<Granted>, Calls) {
    let app = App::new(name);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Granted,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_module_target("macos");
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            let v: Json = serde_json::from_slice(&body).unwrap();
            // The agent's preferred values and permissions, told at the binding, answer nothing.
            if v["send"].is_null() {
                return reply.send(Outcome::Response(Response {
                    status: 200,
                    headers: Vec::new(),
                    body: b"{}".to_vec(),
                }));
            }
            seen.lock().unwrap().push(v.clone());
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: b"{}".to_vec(),
            }));
        })));
    r.bind_extensions(&[], &["clipboard"]);
    (app, r, calls)
}

/// A data-code call of `body`, and its answer.
fn data_call(r: &mut Runner<Granted>, body: Json) -> Json {
    let request = exact_runner::Request::native(body.to_string().into_bytes());
    let Dispatch::Run(work) = r.native_work(&request) else {
        panic!("a module call is work");
    };
    let outcome = match work {
        Work::Now(work) => work(),
        Work::Later(work) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            rx.recv().unwrap()
        }
    };
    let Outcome::Response(Response { body, .. }) = outcome else {
        panic!("an answer");
    };
    serde_json::from_slice(&body).unwrap_or(Json::Null)
}

fn write(items: Json) -> Json {
    json!({ "extension": "clipboard", "send": "write", "args": { "data": items } })
}

/// D8 items 1 and 4: a keyed shape compiles as Contract's optional fields,
/// and under the agent the implementation takes the write: it is its own
/// substitute.
#[test]
fn a_write_reaches_the_implementation_under_the_agent() {
    let (_app, mut r, calls) = boot("agent");
    let _ = data_call(&mut r, write(json!([{ "text/plain": "hello" }])));
    let sent = calls.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0]["args"]["data"][0]["text/plain"], "hello");
    let _ = data_call(
        &mut r,
        json!({ "extension": "clipboard", "send": "writeText", "args": { "data": "again" } }),
    );
    assert_eq!(calls.lock().unwrap().len(), 2);
}

/// D8 item 3: each item's `app:/` path at a key the shape declares a path
/// is admitted against the calling source's `fs.read` lines before the
/// implementation hears of it; text is not a path.
#[test]
fn an_item_s_path_is_admitted_against_the_app_s_fs_read_lines() {
    let (_app, mut r, calls) = boot("paths");
    let ok = data_call(
        &mut r,
        write(json!([{ "image/png": "app:/tmp/copy/a.png", "text/plain": "app:/elsewhere" }])),
    );
    assert!(ok["error"].is_null(), "{ok}");
    assert_eq!(calls.lock().unwrap().len(), 1);
    for outside in ["app:/data/a.png", "app:/tmp/copy/../a.png", "/tmp/a.png"] {
        let refused = data_call(&mut r, write(json!([{ "image/png": outside }])));
        assert_eq!(refused["error"], "NotAllowedError", "{outside}: {refused}");
    }
    assert_eq!(calls.lock().unwrap().len(), 1, "no refused path reaches it");
}

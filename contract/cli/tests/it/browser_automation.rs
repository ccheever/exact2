//! `@exact/browser-automation`'s grant (LLP 1020.002 D9), through this
//! checkout's package installed beside the app: the compiler warns
//! `grant-missing` until the app's grants name `browser.automation`, and the
//! runner refuses every data-code call `NotAllowedError` without it, before
//! the implementation (played by the test as the host's native slot) hears
//! of it, in production and under the agent alike.

use exact_runner::{Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const APP: &str = r#"use handleUserPrompt, EmptyResult from "@exact/browser-automation"

component Shell
  mutation dismissed as shape EmptyResult
  action asked(e: FrameUserPrompt)
    send dismissed = handleUserPrompt(id="tab", accept=false)
  view
    iframe id="tab" src="https://example.com/" partition="work" userpromptopened=asked width=400 height=300
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, grants: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "exact-browser-automation-{name}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        let to = packages.join("browser-automation").canonicalize().unwrap();
        symlink_dir(to, path.join("node_modules/@exact/browser-automation")).unwrap();
        std::fs::write(path.join("app.contract"), APP).unwrap();
        std::fs::write(
            path.join("app.json"),
            r#"{"name":"B","app":{"id":"test.browser-automation","name":"B"}}"#,
        )
        .unwrap();
        std::fs::write(
            path.join("app.ts"),
            format!("export const grants = {grants:?};\n"),
        )
        .unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Grants(&'static str);
impl exact_runner::DataSource for Grants {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        self.0
    }
}

type Calls = Arc<Mutex<Vec<Json>>>;

/// A runner with the app's `grants`, the package bound as in production
/// (`real`) or under the agent, its implementation answering every call.
fn boot(name: &str, grants: &'static str, real: bool) -> (App, Runner<Grants>, Calls) {
    let app = App::new(name, grants);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Grants(grants),
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap());
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: json!({ "result": "Example" }).to_string().into_bytes(),
            }));
        })));
    let bound = ["browser-automation"];
    if real {
        r.bind_extensions(&bound, &[]);
    } else {
        r.bind_extensions(&[], &bound);
    }
    (app, r, calls)
}

/// A data-code call of `body`, its answer.
fn data_call(r: &mut Runner<Grants>, body: Json) -> Json {
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
    serde_json::from_slice(&body).unwrap()
}

/// The calls the implementation heard (an agent binding also hears its
/// preferred values and permissions, which are no call).
fn sends(calls: &Calls) -> usize {
    (calls.lock().unwrap().iter())
        .filter(|c| c["send"].is_string())
        .count()
}

fn evaluate() -> Json {
    json!({ "extension": "browser-automation", "send": "evaluate",
            "args": { "id": "tab", "expression": "document.title" } })
}

#[test]
fn the_compiler_warns_until_the_app_grants_browser_automation() {
    let warned = |app: &App| {
        let (result, warnings) =
            contract::compile_path_all_with_warnings(&app.0.join("app.contract"), false);
        let errors = result.err().unwrap_or_default();
        format!(
            "{:?}",
            (warnings.iter().chain(&errors))
                .map(|e| (&e.id, &e.message))
                .collect::<Vec<_>>()
        )
    };
    let without = App::new("warned", "net.fetch https://x.test");
    let said = warned(&without);
    assert!(said.contains("grant-missing"), "{said}");
    assert!(
        said.contains("add `browser.automation` to the app's grants"),
        "{said}"
    );
    let with = App::new("granted", "browser.automation");
    assert!(
        !warned(&with).contains("grant-missing"),
        "{}",
        warned(&with)
    );
}

#[test]
fn every_call_is_refused_without_the_grant() {
    for real in [true, false] {
        let (_app, mut r, calls) = boot("refused", "net.fetch https://x.test", real);
        assert_eq!(
            data_call(&mut r, evaluate()),
            json!({ "error": "NotAllowedError" })
        );
        let logs = exact_runner::agent::handle(&r, r#"{"op":"logs"}"#);
        assert!(
            logs.contains(
                "browser-automation.evaluate: refused, grant-missing (browser.automation)"
            ),
            "{logs}"
        );
        assert_eq!(
            sends(&calls),
            0,
            "the implementation never hears a refused call"
        );

        let (_app, mut r, calls) = boot("admitted", "browser.automation", real);
        assert_eq!(
            data_call(&mut r, evaluate()),
            json!({ "result": "Example" })
        );
        assert_eq!(sends(&calls), 1);
    }
}

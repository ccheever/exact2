//! Vision and translation admission in the runner (LLP 1133.027 D4, D16,
//! §9), through this checkout's `@exact/barcode-detection`,
//! `@exact/text-detection` and `@exact/translation` installed beside the
//! app, each implementation played by the test as the host's native slot
//! under the agent: a create is the app's foreground act, a still's path is
//! admitted against the grants before dispatch, the targets a name does not
//! exist on, and the agent-only words of translation's script.

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const APP: &str = r#"use detectBarcodes, detectedBarcodes, DetectedBarcodeList, BarcodeScan from "@exact/barcode-detection"
use detectText, DetectedTextList from "@exact/text-detection"
use createTranslator, translate, TranslatorInfo, TranslatorResult from "@exact/translation"

component App
  state track = "track-1"
  resource scan = detectedBarcodes(track=track) as shape BarcodeScan else empty()
  mutation found as shape DetectedBarcodeList
  mutation read as shape DetectedTextList
  mutation translator as shape TranslatorInfo
  mutation translated as shape TranslatorResult
  action picked
    send found = detectBarcodes(image="app:/tmp/picked/1-1.png")
  action outside
    send found = detectBarcodes(image="app:/data/none.png")
  action readText
    send read = detectText(image="app:/tmp/picked/1-1.png")
  action create
    send translator = createTranslator(sourceLanguage="ja", targetLanguage="en")
  action run
    send translated = translate(translator="translator-1", input="明日")
  view
    column
      text scan.state
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("exact-vt-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        for package in ["barcode-detection", "text-detection", "translation"] {
            let to = packages.join(package).canonicalize().unwrap();
            symlink_dir(to, path.join("node_modules/@exact").join(package)).unwrap();
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

const PICKED: &str = "fs.read app:/tmp/picked\n";

type Calls = Arc<Mutex<Vec<Json>>>;

/// A runner over the app, the three implementations bound under the agent
/// and played by the native slot, which answers each call with its own body.
fn boot(name: &str, target: &str) -> (App, Runner<Grants>, Calls) {
    let app = App::new(name);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Grants(PICKED),
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            let v: Json = serde_json::from_slice(&body).unwrap();
            seen.lock().unwrap().push(v.clone());
            let answer = match (v["send"].as_str(), v["source"].as_str()) {
                (Some("detectBarcodes"), _) => json!({ "barcodes": [], "width": 1, "height": 1 }),
                (Some("detectText"), _) => json!({ "texts": [], "width": 1, "height": 1 }),
                (Some("createTranslator"), _) => {
                    json!({ "id": "translator-1", "sourceLanguage": "ja", "targetLanguage": "en" })
                }
                (Some("translate"), _) => json!({ "text": "[en] 明日" }),
                _ => json!({ "state": "idle", "barcodes": [], "seen": [], "frame": 0, "width": 0, "height": 0 }),
            };
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: answer.to_string().into_bytes(),
            }));
        })));
    r.set_module_target(target);
    r.bind_extensions(&[], &["barcode-detection", "text-detection", "translation"]);
    (app, r, calls)
}

/// The host's part: each request dispatched, released work run, each
/// outcome fulfilled, until nothing is left to do.
fn pump(r: &mut Runner<Grants>) {
    for _ in 0..8 {
        let outs = r.take_requests();
        let released = r.release_work();
        if outs.is_empty() && released.is_empty() && r.apply_announced().0.is_empty() {
            return;
        }
        for out in outs {
            let dispatch = match out.request.continuation {
                Some(token) => r.dispatch_work(token),
                None => r.native_work(&out.request),
            };
            run(r, out.ticket, dispatch);
        }
        for (_, dispatch) in released {
            if let Dispatch::Run(Work::Now(work)) = dispatch {
                work();
            }
        }
    }
}

fn run(r: &mut Runner<Grants>, ticket: u64, dispatch: Dispatch) {
    let outcome = match dispatch {
        Dispatch::Run(Work::Now(work)) => work(),
        Dispatch::Run(Work::Later(work)) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            rx.recv().unwrap()
        }
        _ => return,
    };
    let _ = r.fulfill(ticket, outcome);
}

/// The calls of `name` the implementation was sent.
fn sent(calls: &Calls, name: &str) -> Vec<Json> {
    (calls.lock().unwrap().iter())
        .filter(|c| c["send"] == name)
        .cloned()
        .collect()
}

fn logs(r: &Runner<Grants>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

/// A create is the app's act: a tool's or a wake's fails `person-required`
/// before the implementation is asked, and a person's reaches it (D16,
/// drive 4).
#[test]
fn a_create_is_the_apps_act() {
    for origin in [CallOrigin::Tool, CallOrigin::Wake] {
        let (_app, mut r, calls) = boot("create", "ios");
        pump(&mut r);
        r.act_from(origin, "create", Vec::new()).unwrap();
        pump(&mut r);
        assert!(sent(&calls, "createTranslator").is_empty(), "{}", logs(&r));
        let said = logs(&r);
        assert!(
            said.contains("translation.createTranslator: refused, person-required"),
            "{said}"
        );
    }
    let (_app, mut r, calls) = boot("create-app", "ios");
    pump(&mut r);
    r.act("create", Vec::new()).unwrap();
    pump(&mut r);
    assert_eq!(sent(&calls, "createTranslator").len(), 1, "{}", logs(&r));
}

/// A translate carries its origin to the implementation, which refuses an
/// unattended one where the platform would show UI (D8): the runner admits
/// it.
#[test]
fn a_tools_translate_reaches_the_implementation_with_its_origin() {
    let (_app, mut r, calls) = boot("translate", "ios");
    pump(&mut r);
    r.act_from(CallOrigin::Tool, "run", Vec::new()).unwrap();
    pump(&mut r);
    let made = sent(&calls, "translate");
    assert_eq!(made.len(), 1, "{}", logs(&r));
    assert_eq!(made[0]["origin"], "tool", "{}", made[0]);
}

/// A still's path is admitted against `fs.read` before dispatch: one the
/// grants cover reaches the implementation, one outside them fails
/// `NotAllowedError` with no call (D4).
#[test]
fn a_stills_path_is_admitted_against_the_grants() {
    let (_app, mut r, calls) = boot("paths", "macos");
    pump(&mut r);
    r.act("outside", Vec::new()).unwrap();
    pump(&mut r);
    assert!(sent(&calls, "detectBarcodes").is_empty(), "{}", logs(&r));
    assert!(
        logs(&r).contains("send found failed: barcode-detection.detectBarcodes: NotAllowedError"),
        "{}",
        logs(&r)
    );
    r.act("picked", Vec::new()).unwrap();
    pump(&mut r);
    let made = sent(&calls, "detectBarcodes");
    assert_eq!(made.len(), 1, "{}", logs(&r));
    assert_eq!(
        made[0]["args"]["image"], "app:/tmp/picked/1-1.png",
        "{}",
        made[0]
    );
}

/// Text detection is not a web, Linux or Windows name: there it fails
/// `NotSupportedError` under the agent too, never reaching an
/// implementation (D18).
#[test]
fn text_detection_is_absent_where_d18_says() {
    for target in ["web", "linux", "windows"] {
        let (_app, mut r, calls) = boot("absent", target);
        pump(&mut r);
        r.act("readText", Vec::new()).unwrap();
        pump(&mut r);
        assert!(
            sent(&calls, "detectText").is_empty(),
            "{target}: {}",
            logs(&r)
        );
        assert!(
            logs(&r).contains("send read failed: text-detection.detectText: NotSupportedError"),
            "{target}: {}",
            logs(&r)
        );
    }
}

/// A live scan has no origin rule (D5): a tool's read reaches the
/// implementation, which opens nothing, since only a consumer does.
#[test]
fn a_scan_read_by_a_tool_is_admitted() {
    let (_app, mut r, calls) = boot("scan", "ios");
    pump(&mut r);
    let reads = (calls.lock().unwrap().iter())
        .filter(|c| c["source"] == "detectedBarcodes")
        .count();
    assert!(reads >= 1, "{}", logs(&r));
    assert!(
        !logs(&r).contains("detectedBarcodes: refused"),
        "{}",
        logs(&r)
    );
}

/// The script's words are the interface's agent-only sources (§9): typed by
/// their declaration, a wrong shape refused by name.
#[test]
fn the_scripts_words_are_typed() {
    let (_app, mut r, _calls) = boot("words", "ios");
    pump(&mut r);
    let script = r#"{"op":"prefer","extension":{"translation.translations":[{"text":"The meeting is at ten."},{"error":"InvalidStateError","reason":"not-installed"}],"translation.downloadSheet":"decline"}}"#;
    r.extension_agent(script).unwrap().1.unwrap();
    let wrong = r#"{"op":"prefer","extension":{"translation.downloadSheet":7}}"#;
    let said = r.extension_agent(wrong).unwrap().1.unwrap_err();
    assert!(said.contains("downloadSheet"), "{said}");
}

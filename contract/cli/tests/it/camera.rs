//! The camera's admission in the runner (LLP 1133.008 D6, §10 drive 6),
//! through this checkout's `@exact/camera` and `@exact/media-capture`,
//! installed beside the app, their implementations played by the test as the
//! host's native slot under the agent: `video=` asks the camera's permission
//! and `audio=` the microphone's, one after the other; a tool's capture
//! start is refused before anything is asked.

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const APP: &str = r#"use getUserMedia, cameraStatus, microphoneStatus, MediaStream, CameraStatus, MicrophoneStatus from "@exact/media-capture"
use takePhoto, Photo from "@exact/camera"

component App
  resource camera = cameraStatus() as shape CameraStatus
  resource microphone = microphoneStatus() as shape MicrophoneStatus
  mutation opened as shape MediaStream
  mutation shot as shape Photo
  action open
    send opened = getUserMedia(video=true)
  action openBoth
    send opened = getUserMedia(audio=true, video=true)
  action shutter
    send shot = takePhoto(track="track-1")
  view
    column
      text camera.state
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-camera-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        for package in ["camera", "media-capture"] {
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
        "device.camera purpose.camera\ndevice.microphone purpose.microphone\n"
    }
}

type Calls = Arc<Mutex<Vec<Json>>>;

/// A runner over the app, the two implementations bound under the agent and
/// played by the native slot: `getUserMedia` answers a stream of one track.
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
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            let v: Json = serde_json::from_slice(&body).unwrap();
            seen.lock().unwrap().push(v.clone());
            let answer = match v["send"].as_str() {
                Some("getUserMedia") => json!({ "id": "stream-1", "active": true, "tracks": [] }),
                Some("takePhoto") => {
                    json!({ "path": "app:/tmp/camera/1.jpg", "type": "image/jpeg", "size": 1, "width": 1, "height": 1 })
                }
                _ => json!({}),
            };
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: answer.to_string().into_bytes(),
            }));
        })));
    r.bind_extensions(&[], &["camera", "media-capture"]);
    (app, r, calls)
}

/// The host's part: each request dispatched, released work run, each
/// outcome fulfilled, until nothing is left to do.
fn pump(r: &mut Runner<Granted>, parked: &mut Vec<(u64, u64)>) {
    for _ in 0..8 {
        let outs = r.take_requests();
        let released = r.release_work();
        if outs.is_empty() && released.is_empty() && r.apply_announced().0.is_empty() {
            return;
        }
        for out in outs {
            let dispatch = match out.request.continuation {
                Some(token) => {
                    parked.push((token, out.ticket));
                    r.dispatch_work(token)
                }
                None => r.native_work(&out.request),
            };
            run(r, out.ticket, dispatch);
        }
        for (token, dispatch) in released {
            let ticket = parked.iter().find(|(t, _)| *t == token).expect("parked").1;
            run(r, ticket, dispatch);
        }
    }
}

fn run(r: &mut Runner<Granted>, ticket: u64, dispatch: Dispatch) {
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

fn sent(calls: &Calls, name: &str) -> usize {
    (calls.lock().unwrap().iter())
        .filter(|c| c["send"] == name)
        .count()
}

fn logs(r: &Runner<Granted>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

/// The one ticket held: its capability's permission, answered `allow`.
fn allow(r: &mut Runner<Granted>, parked: &mut Vec<(u64, u64)>) -> String {
    let holds = r.device_holds();
    assert_eq!(holds.len(), 1, "{}", logs(r));
    let args: Json = serde_json::from_str(&holds[0].args).unwrap();
    let tap = format!(
        r#"{{"op":"tap","ticket":{},"choice":"allow"}}"#,
        holds[0].ticket
    );
    exact_runner::agent::answer(r, &tap).unwrap();
    pump(r, parked);
    args["capability"].as_str().unwrap_or_default().to_owned()
}

/// `video=` asks the camera's permission, and `cameraStatus` reads it; a
/// call with both kinds asks each in turn before the implementation hears
/// of it (LLP 1133.008 D6, LLP 1133.009 D3).
#[test]
fn video_asks_the_camera_and_audio_the_microphone_in_turn() {
    let (_app, mut r, calls) = boot("permissions");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("open", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "getUserMedia"), 0, "held under the ticket");
    assert_eq!(allow(&mut r, &mut parked), "camera");
    assert_eq!(sent(&calls, "getUserMedia"), 1, "{}", logs(&r));
    let state: Json = serde_json::from_str(&r.extensions_json().unwrap()).unwrap();
    assert_eq!(
        state["media-capture"]["permissions"],
        json!({ "camera": "granted", "microphone": "prompt" })
    );
    assert!(format!("{:?}", r.resource("camera")).contains("\"granted\""));

    r.act("openBoth", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(allow(&mut r, &mut parked), "microphone");
    assert_eq!(sent(&calls, "getUserMedia"), 2, "{}", logs(&r));

    let denied = r#"{"op":"prefer","permission":{"camera":"denied"}}"#;
    r.extension_agent(denied).unwrap().1.unwrap();
    r.act("openBoth", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "a denied camera asks nothing");
    assert_eq!(sent(&calls, "getUserMedia"), 2);
}

/// Drive 6's admission: a tool's `takePhoto`, and a tool's `getUserMedia`
/// while the camera is `prompt`, are refused `person-required` before the
/// implementation hears of them; the app's own reaches it (§14).
#[test]
fn a_tool_s_capture_start_is_refused() {
    let (_app, mut r, calls) = boot("tool");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act_from(CallOrigin::Tool, "open", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "a tool raises no ticket");
    r.act_from(CallOrigin::Tool, "shutter", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "takePhoto"), 0);
    assert!(
        logs(&r).contains("camera.takePhoto: refused, person-required (tool origin)"),
        "{}",
        logs(&r)
    );
    r.act("shutter", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "takePhoto"), 1, "{}", logs(&r));
}

/// An implementation's disclosure (D8 item 9): what it says through its
/// `changed` is listed as its extension's `implementation`, each field
/// replacing the one of its name said before.
#[test]
fn an_implementation_s_disclosure_is_listed() {
    let (_app, mut r, _calls) = boot("disclosed");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.listen(Arc::new(|| {}));
    let say = |r: &Runner<Granted>, extension: &str, state: Json| {
        let body = json!({ "extension": extension, "state": state });
        r.native_slot().changed(&format!("exact-state:{body}"));
    };
    say(
        &r,
        "media-capture",
        json!({ "tracks": [{ "id": "track-1", "lease": true }] }),
    );
    say(&r, "camera", json!({ "photos": [] }));
    say(
        &r,
        "camera",
        json!({ "photos": [{ "path": "app:/tmp/camera/1.jpg" }] }),
    );
    r.apply_announced();
    let state: Json = serde_json::from_str(&r.extensions_json().unwrap()).unwrap();
    assert_eq!(
        state["media-capture"]["implementation"]["tracks"][0]["lease"],
        true
    );
    assert_eq!(
        state["camera"]["implementation"],
        json!({ "photos": [{ "path": "app:/tmp/camera/1.jpg" }] })
    );
    say(
        &r,
        "media-capture",
        json!({ "tracks": [{ "id": "track-1", "lease": false }] }),
    );
    r.apply_announced();
    let state: Json = serde_json::from_str(&r.extensions_json().unwrap()).unwrap();
    assert_eq!(
        state["media-capture"]["implementation"]["tracks"][0]["lease"],
        false
    );
    assert!(
        state["media-capture"]["permissions"].is_object(),
        "beside the runner's own"
    );
}

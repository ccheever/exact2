//! The photo library's admission in the runner (LLP 1133.019 D4–D7, D11),
//! through this checkout's `@exact/photos` installed beside the app, its
//! implementation played by the test as the host's native slot under the
//! agent: two permissions answered by one status, a ticket's `limited`
//! choice, the selection ticket, the narrowest permission an add asks,
//! pictures admitted only while the read is granted, and the origin rule.

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const APP: &str = r#"use photoLibraryStatus, photoAssets, addToPhotoLibrary, copyPhotoAsset, managePhotoSelection, PhotoLibraryStatus, PhotoAssetList, PhotoAdded, PhotoCopy from "@exact/photos"

component App
  resource status = photoLibraryStatus() as shape PhotoLibraryStatus
  mutation added as shape PhotoAdded
  mutation copied as shape PhotoCopy
  mutation managed as shape PhotoLibraryStatus
  action save
    send added = addToPhotoLibrary(path="app:/data/edit.jpg")
  action copy
    send copied = copyPhotoAsset(asset="agent-1")
  action manage
    send managed = managePhotoSelection()
  view
    column
      text status.state
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, recents: bool) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-photos-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        let to = packages.join("photos").canonicalize().unwrap();
        symlink_dir(to, path.join("node_modules/@exact/photos")).unwrap();
        let mut source = APP.to_owned();
        // The recents strip's read, which asks while the state is `prompt`.
        if recents {
            source = source.replace(
                "  mutation added",
                "  resource recent = photoAssets(limit=6) as shape PhotoAssetList\n  mutation added",
            );
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

const BOTH: &str =
    "device.photos purpose.recents\ndevice.photos-add purpose.save\nfs.read app:/data\n";
const ADD_ONLY: &str = "device.photos-add purpose.save\nfs.read app:/data\n";

type Calls = Arc<Mutex<Vec<Json>>>;

/// A runner over the app, the implementation bound under the agent and
/// played by the native slot.
fn boot(name: &str, grants: &'static str, recents: bool) -> (App, Runner<Grants>, Calls) {
    let app = App::new(name, recents);
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
            let v: Json = serde_json::from_slice(&body).unwrap();
            seen.lock().unwrap().push(v.clone());
            let answer = match (v["send"].as_str(), v["source"].as_str()) {
                (Some("addToPhotoLibrary"), _) => json!({ "asset": "agent-7" }),
                (Some("copyPhotoAsset"), _) => json!({ "path": "app:/tmp/photos/1.jpg", "name": "agent-1.jpg",
                    "type": "image/jpeg", "size": 1, "width": 1, "height": 1 }),
                (_, Some("photoAssets")) => json!({ "assets": [] }),
                _ => json!({ "available": true, "state": "prompt", "access": "none", "addState": "prompt" }),
            };
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: answer.to_string().into_bytes(),
            }));
        })));
    r.bind_extensions(&[], &["photos"]);
    (app, r, calls)
}

/// The host's part: each request dispatched, released work run, each
/// outcome fulfilled, until nothing is left to do.
fn pump(r: &mut Runner<Grants>, parked: &mut Vec<(u64, u64)>) {
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

fn sent(calls: &Calls, name: &str) -> usize {
    (calls.lock().unwrap().iter())
        .filter(|c| c["send"] == name)
        .count()
}

fn logs(r: &Runner<Grants>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

fn state(r: &Runner<Grants>) -> Json {
    serde_json::from_str(&r.extensions_json().unwrap()).unwrap()
}

/// The one ticket held, answered with `op` (`tap` and a choice, or `type`
/// and a text): its capability and choices.
fn answer(
    r: &mut Runner<Grants>,
    parked: &mut Vec<(u64, u64)>,
    op: &str,
    word: &str,
) -> (String, Vec<String>) {
    let holds = r.device_holds();
    assert_eq!(holds.len(), 1, "{}", logs(r));
    let hold = holds[0].clone();
    let key = if op == "tap" { "choice" } else { "text" };
    let line = json!({ "op": op, "ticket": hold.ticket, key: word }).to_string();
    exact_runner::agent::answer(r, &line).unwrap();
    pump(r, parked);
    let args: Json = serde_json::from_str(&hold.args).unwrap();
    let capability = args["capability"]
        .as_str()
        .unwrap_or(&hold.capability)
        .to_owned();
    (capability, hold.choices)
}

/// One status answers both permissions, each in its field; reading is
/// granted with `limited`, a choice beyond allow and deny, and Apple's
/// read-write grant includes adding (D4, D5, D11 items 2 and 3).
#[test]
fn a_read_ticket_offers_limited_and_the_status_answers_both() {
    let (_app, mut r, calls) = boot("limited", BOTH, true);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let (capability, choices) = answer(&mut r, &mut parked, "tap", "limited");
    assert_eq!(capability, "photos");
    assert_eq!(choices, ["allow", "deny", "limited"]);
    let s = state(&r);
    assert_eq!(
        s["photos"]["permissions"],
        json!({ "photos": "granted", "photos-add": "granted" })
    );
    let status = format!("{:?}", r.resource("status"));
    assert!(status.contains("\"limited\""), "{status}");
    // The implementation heard the states and the limited status.
    let told = (calls.lock().unwrap().iter())
        .rev()
        .find(|c| c["preferred"].is_object() && c["source"].is_null())
        .cloned()
        .unwrap();
    assert_eq!(told["permissions"]["photos"], "granted");
    assert_eq!(told["preferred"]["photoLibraryStatus"]["access"], "limited");
}

/// The selection sheet is a ticket under limited access, whose typed ids
/// become the agent's `selection` and reach the implementation, which then
/// answers the call (§10, D11 item 3).
#[test]
fn the_selection_ticket_sets_the_agent_s_selection() {
    let (_app, mut r, calls) = boot("selection", BOTH, false);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let limited = r#"{"op":"prefer","permission":{"photos":"granted"}}"#;
    r.extension_agent(limited).unwrap().1.unwrap();
    r.act("manage", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    // Full access shows no sheet: the implementation answers at once.
    assert!(r.device_holds().is_empty());
    assert_eq!(sent(&calls, "managePhotoSelection"), 1, "{}", logs(&r));

    let prefer =
        r#"{"op":"prefer","extension":{"photos.photoLibraryStatus":{"access":"limited"}}}"#;
    r.extension_agent(prefer).unwrap().1.unwrap();
    r.act("manage", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(
        sent(&calls, "managePhotoSelection"),
        1,
        "held under the selection ticket"
    );
    let (capability, _) = answer(&mut r, &mut parked, "type", "agent-1, agent-3");
    assert_eq!(capability, "selection");
    assert!(
        logs(&r).contains("photos.selection: chosen by the agent"),
        "{}",
        logs(&r)
    );
    let told = (calls.lock().unwrap().iter())
        .rev()
        .find(|c| c["preferred"]["selection"].is_array())
        .cloned()
        .expect("the implementation was told the selection");
    assert_eq!(
        told["preferred"]["selection"],
        json!(["agent-1", "agent-3"])
    );
    assert_eq!(sent(&calls, "managePhotoSelection"), 2, "{}", logs(&r));
}

/// An add asks add-only access where the app declares that line; a tool
/// cannot add; a tool's copy while the read is `prompt` raises no ticket
/// (D4, D7).
#[test]
fn an_add_asks_the_narrowest_permission_and_tools_cannot_add() {
    let (_app, mut r, calls) = boot("add", BOTH, false);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act_from(CallOrigin::Tool, "save", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "a tool raises no ticket");
    assert!(
        logs(&r).contains("photos.addToPhotoLibrary: refused, person-required (tool origin)"),
        "{}",
        logs(&r)
    );
    r.act_from(CallOrigin::Tool, "copy", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(
        r.device_holds().is_empty(),
        "a tool's copy raises no ticket"
    );
    assert_eq!(sent(&calls, "copyPhotoAsset"), 0);

    r.act("save", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (capability, choices) = answer(&mut r, &mut parked, "tap", "allow");
    assert_eq!(capability, "photos-add");
    assert_eq!(choices, ["allow", "deny"]);
    assert_eq!(sent(&calls, "addToPhotoLibrary"), 1, "{}", logs(&r));
    let s = state(&r);
    assert_eq!(
        s["photos"]["permissions"],
        json!({ "photos": "prompt", "photos-add": "granted" })
    );

    // Now the person's own copy asks the read, and a tool's is refused.
    r.act("copy", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (capability, _) = answer(&mut r, &mut parked, "tap", "allow");
    assert_eq!(capability, "photos");
    assert_eq!(sent(&calls, "copyPhotoAsset"), 1, "{}", logs(&r));
}

/// An app that declares only the add line asks only that; with none of the
/// add's lines the bake refuses nothing here, and the call is refused
/// `grant-missing` at admission.
#[test]
fn an_add_only_app_asks_add_only() {
    let (_app, mut r, calls) = boot("add-only", ADD_ONLY, false);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("save", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (capability, _) = answer(&mut r, &mut parked, "tap", "allow");
    assert_eq!(capability, "photos-add");
    assert_eq!(sent(&calls, "addToPhotoLibrary"), 1);
    r.act("copy", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty());
    assert!(
        logs(&r).contains("photos.copyPhotoAsset: refused, grant-missing (permission photos)"),
        "{}",
        logs(&r)
    );
    // On Android the app's own photo is read under the add line, with no
    // dialog; MediaStore decides whether it is the app's (D6).
    r.set_module_target("android");
    r.act("copy", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty());
    assert_eq!(sent(&calls, "copyPhotoAsset"), 1, "{}", logs(&r));
}

/// A picture is a read admitted without a ticket: refused while the read is
/// `prompt` or `denied`, or without the line; admitted once granted; on
/// Android an add-only app's own photo goes through as `owned` (D2, D6, D11
/// items 1 and 4).
#[test]
fn pictures_are_admitted_only_while_the_read_is_granted() {
    let (_app, mut r, _calls) = boot("pictures", BOTH, false);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    assert_eq!(r.picture_schemes(), ["photos"]);
    assert_eq!(r.admit_picture("https://example.test/a.jpg", "ios"), None);
    let refused = r.admit_picture("photos:agent-1", "ios").unwrap();
    assert_eq!(refused["error"], "NotAllowedError");
    assert_eq!(refused["message"], "permission photos is prompt");
    assert!(r.device_holds().is_empty(), "a draw never asks");
    let granted = r#"{"op":"prefer","permission":{"photos":"granted"}}"#;
    r.extension_agent(granted).unwrap().1.unwrap();
    let admitted = r.admit_picture("photos:agent-1", "ios").unwrap();
    assert_eq!(
        admitted,
        json!({ "extension": "photos", "scheme": "photos", "agent": true })
    );
    let s = state(&r);
    assert_eq!(
        s["photos"]["pictures"],
        json!({ "admitted": 1, "refused": 1 })
    );
    assert!(logs(&r).contains("photos: picture refused, permission photos is prompt"));

    let (_app, mut r, _calls) = boot("owned", ADD_ONLY, false);
    let refused = r.admit_picture("photos:agent-7", "ios").unwrap();
    assert_eq!(refused["message"], "grant-missing (permission photos)");
    r.set_module_target("android");
    let owned = r
        .admit_picture("photos:content://media/7", "android")
        .unwrap();
    assert_eq!(owned["owned"], true);
}

/// With no implementation bound, a picture is refused, never fetched.
#[test]
fn a_picture_with_no_implementation_is_refused() {
    let app = App::new("unbound", false);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false).unwrap();
    let mut r = Runner::boot(
        plan,
        Grants(BOTH),
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let refused = r.admit_picture("photos:agent-1", "macos").unwrap();
    assert_eq!(refused["message"], "no implementation bound");
}

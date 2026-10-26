//! Contacts and calendar admission in the runner (LLP 1133.018 D4, D11,
//! D13), through this checkout's `@exact/contacts` and `@exact/calendar`
//! installed beside the app, each implementation played by the test as the
//! host's native slot under the agent: a pick that needs no line and whose
//! sheet takes typed ids, a decision that grants what its request asked
//! (`asks`), the raise from write-only, the editor's taps, the targets a
//! name does not exist on, and the origin rule.

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const CONTACTS: &str = r#"use selectContacts, showContactAccessPicker, contactsStatus, ContactSelection, ContactPage, ContactsStatus from "@exact/contacts"

component App
  resource status = contactsStatus() as shape ContactsStatus
  mutation picked as shape ContactSelection
  mutation shared as shape ContactSelection
  action pick
    send picked = selectContacts(properties=["name", "email"], multiple=true)
  action pickOne
    send picked = selectContacts(properties=["name"])
  action share
    send shared = showContactAccessPicker()
  view
    column
      text status.access
"#;

const EVENT: &str = r#"CalendarEvent(id=none, uid=none, calendarId=none, title="Tea", description=none, start="2026-09-16T15:00:00", timeZone=none, duration="PT30M", showWithoutTime=false, location=none, url=none, status="confirmed", freeBusyStatus="busy", recurrenceId=none, utcStart=none, utcEnd=none, apple=none)"#;

const CALENDAR: &str = r#"use calendarStatus, saveEvent, showEventEditor, requestFullCalendarAccess, CalendarStatus, CalendarEvent, EventPage, EventEditorResult from "@exact/calendar"

component App
  resource status = calendarStatus() as shape CalendarStatus
  mutation saved as shape CalendarEvent
  mutation edited as shape EventEditorResult
  mutation raised as shape CalendarStatus
  action save
    send saved = saveEvent(event=EVENT)
  action edit
    send edited = showEventEditor(event=EVENT)
  action raise
    send raised = requestFullCalendarAccess()
  view
    column
      text status.access
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, source: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("exact-cc-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        for package in ["contacts", "calendar"] {
            let to = packages.join(package).canonicalize().unwrap();
            symlink_dir(to, path.join("node_modules/@exact").join(package)).unwrap();
        }
        std::fs::write(path.join("app.contract"), source.replace("EVENT", EVENT)).unwrap();
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

const BOTH: &str = "device.calendar purpose.read\ndevice.calendar-write-only purpose.add\n";
const FULL: &str = "device.calendar purpose.read\n";

type Calls = Arc<Mutex<Vec<Json>>>;

/// A runner over the app, both implementations bound under the agent and
/// played by the native slot, which answers each call with its own body.
fn boot(name: &str, source: &str, grants: &'static str) -> (App, Runner<Grants>, Calls) {
    let app = App::new(name, source);
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
            let answer = match v["send"].as_str() {
                Some("selectContacts" | "showContactAccessPicker") => json!({ "contacts": [] }),
                Some("saveEvent") => json!({ "title": "Tea", "start": "2026-09-16T15:00:00", "duration": "PT30M" }),
                Some("showEventEditor") => json!({ "action": "saved" }),
                _ => json!({ "available": true, "state": "prompt", "access": "none", "editor": true }),
            };
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: answer.to_string().into_bytes(),
            }));
        })));
    r.bind_extensions(&[], &["contacts", "calendar"]);
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

/// The status the runner answers now, as JSON.
fn status(r: &Runner<Grants>) -> String {
    format!("{:?}", r.resource("status"))
}

/// The one ticket held: its capability, choices and arguments.
fn held(r: &Runner<Grants>) -> (u64, String, Vec<String>, Json) {
    let holds = r.device_holds();
    assert_eq!(holds.len(), 1, "{}", logs(r));
    let hold = holds[0].clone();
    let args: Json = serde_json::from_str(&hold.args).unwrap();
    let capability = (args["capability"].as_str())
        .unwrap_or(&hold.capability)
        .to_owned();
    (hold.ticket, capability, hold.choices, args)
}

/// Ticket `ticket` answered with `op` (`tap` and a choice, or `type` and a text).
fn answer(
    r: &mut Runner<Grants>,
    parked: &mut Vec<(u64, u64)>,
    ticket: u64,
    op: &str,
    word: &str,
) -> Result<(), String> {
    let key = if op == "tap" { "choice" } else { "text" };
    let line = json!({ "op": op, "ticket": ticket, key: word }).to_string();
    let (said, _) = exact_runner::agent::answer(r, &line).expect("a module's ticket");
    pump(r, parked);
    match said.contains("\"error\"") {
        true => Err(said),
        false => Ok(()),
    }
}

/// A pick needs no grant line and no permission: its sheet shows what the
/// call asked, takes the typed ids, which reach the implementation as the
/// call's `preferred`, and more than one only where `multiple` is set; a
/// cancel reaches it too (D4, D13 item 4).
#[test]
fn a_pick_needs_no_line_and_its_sheet_takes_typed_ids() {
    let (_app, mut r, calls) = boot("pick", CONTACTS, "");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("pick", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, capability, choices, args) = held(&r);
    assert_eq!(capability, "contacts");
    assert!(choices.is_empty(), "{choices:?}");
    assert_eq!(args["options"]["properties"], json!(["name", "email"]));
    assert!(
        sent(&calls, "selectContacts").is_empty(),
        "held before the implementation"
    );
    answer(&mut r, &mut parked, ticket, "type", "c2, c5").unwrap();
    let picks = sent(&calls, "selectContacts");
    assert_eq!(picks.len(), 1, "{}", logs(&r));
    assert_eq!(picks[0]["preferred"], json!({ "typed": ["c2", "c5"] }));

    r.act("pickOne", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, ..) = held(&r);
    let refused = answer(&mut r, &mut parked, ticket, "type", "c2,c5").unwrap_err();
    assert!(refused.contains("one id, not 2"), "{refused}");
    answer(&mut r, &mut parked, ticket, "tap", "cancel").unwrap();
    let picks = sent(&calls, "selectContacts");
    assert_eq!(picks.len(), 2, "{}", logs(&r));
    assert_eq!(picks[1]["preferred"], json!({ "tap": "cancel" }));
}

/// A pick opens system UI, so a tool's fails `person-required` before any
/// sheet (D4).
#[test]
fn a_tool_cannot_pick() {
    let (_app, mut r, calls) = boot("tool", CONTACTS, "");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act_from(CallOrigin::Tool, "pick", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));
    assert!(sent(&calls, "selectContacts").is_empty());
    assert!(
        logs(&r).contains("contacts.selectContacts: refused, person-required (tool origin)"),
        "{}",
        logs(&r)
    );
}

/// The access picker is iOS's: its sheet holds there only under limited
/// access, and on macOS the name fails `NotSupportedError`, under the agent
/// too (D9, LLP 1133.002 D14 step 2).
#[test]
fn the_access_picker_is_ios_s_and_holds_only_under_limited() {
    let (_app, mut r, calls) = boot("share", CONTACTS, "device.contacts purpose.read\n");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.set_module_target("macos");
    r.act("share", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty());
    assert!(
        sent(&calls, "showContactAccessPicker").is_empty(),
        "{}",
        logs(&r)
    );

    r.set_module_target("ios");
    let granted = r#"{"op":"prefer","permission":{"contacts":"granted"}}"#;
    r.extension_agent(granted).unwrap().1.unwrap();
    r.act("share", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "full access shows no sheet");
    assert_eq!(
        sent(&calls, "showContactAccessPicker").len(),
        1,
        "{}",
        logs(&r)
    );

    let limited = r#"{"op":"prefer","extension":{"contacts.contactsStatus":{"access":"limited"}}}"#;
    r.extension_agent(limited).unwrap().1.unwrap();
    r.act("share", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, capability, ..) = held(&r);
    assert_eq!(capability, "share");
    answer(&mut r, &mut parked, ticket, "type", "c7").unwrap();
    let shares = sent(&calls, "showContactAccessPicker");
    assert_eq!(shares.len(), 2, "{}", logs(&r));
    assert_eq!(shares[1]["preferred"], json!({ "typed": ["c7"] }));
}

/// With both calendar lines, a first save asks write-only, the least that
/// serves it, and `allow` grants that; the raise then holds its own ticket,
/// whose `allow` sets full access and whose `deny` leaves write-only (D5,
/// D11, D13 items 3 and 4).
#[test]
fn a_save_asks_write_only_and_the_raise_asks_full() {
    let (_app, mut r, calls) = boot("asks", CALENDAR, BOTH);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("save", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, capability, _, args) = held(&r);
    assert_eq!(capability, "calendar");
    assert_eq!(args["asks"], json!({ "access": "write-only" }), "{args}");
    answer(&mut r, &mut parked, ticket, "tap", "allow").unwrap();
    assert!(status(&r).contains("\"write-only\""), "{}", status(&r));
    assert_eq!(sent(&calls, "saveEvent").len(), 1, "{}", logs(&r));
    let told = (calls.lock().unwrap().iter())
        .rev()
        .find(|c| c["preferred"]["calendarStatus"].is_object())
        .cloned()
        .expect("the implementation was told the status");
    assert_eq!(told["preferred"]["calendarStatus"]["access"], "write-only");

    // A tool's raise prompts no one.
    r.act_from(CallOrigin::Tool, "raise", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));

    r.act("raise", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, capability, ..) = held(&r);
    assert_eq!(capability, "access");
    answer(&mut r, &mut parked, ticket, "tap", "deny").unwrap();
    assert!(
        status(&r).contains("\"write-only\""),
        "declined: {}",
        status(&r)
    );
    r.act("raise", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, ..) = held(&r);
    answer(&mut r, &mut parked, ticket, "tap", "allow").unwrap();
    assert!(status(&r).contains("\"full\""), "{}", status(&r));
    // Under full access the raise answers at once.
    r.act("raise", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));
}

/// With only the full line a save asks full; on Android a calendar ask of
/// either level is granted in full, since Android has no write-only (D11).
#[test]
fn a_save_asks_full_with_only_that_line_and_android_grants_full() {
    let (_app, mut r, _calls) = boot("full", CALENDAR, FULL);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("save", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, _, _, args) = held(&r);
    assert_eq!(args["asks"], json!({ "access": "full" }), "{args}");
    answer(&mut r, &mut parked, ticket, "tap", "allow").unwrap();
    assert!(status(&r).contains("\"full\""), "{}", status(&r));

    let (_app, mut r, _calls) = boot("android", CALENDAR, BOTH);
    r.set_module_target("android");
    pump(&mut r, &mut parked);
    r.act("save", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, _, _, args) = held(&r);
    assert_eq!(args["asks"], json!({ "access": "full" }), "{args}");
    answer(&mut r, &mut parked, ticket, "tap", "allow").unwrap();
    assert!(status(&r).contains("\"full\""), "{}", status(&r));
}

/// The editor needs no line and no permission: its sheet takes `save` or
/// `cancel`, never a typed answer, and the tap reaches the implementation
/// (D4, D13 item 4).
#[test]
fn the_editor_takes_save_or_cancel() {
    let (_app, mut r, calls) = boot("editor", CALENDAR, "");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("edit", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, capability, choices, args) = held(&r);
    assert_eq!(capability, "editor");
    assert_eq!(choices, ["save"]);
    assert_eq!(args["options"]["event"]["title"], "Tea");
    let refused = answer(&mut r, &mut parked, ticket, "type", "x").unwrap_err();
    assert!(refused.contains("answered by tap"), "{refused}");
    answer(&mut r, &mut parked, ticket, "tap", "save").unwrap();
    let edits = sent(&calls, "showEventEditor");
    assert_eq!(edits.len(), 1, "{}", logs(&r));
    assert_eq!(edits[0]["preferred"], json!({ "tap": "save" }));
    // A save under no line is refused at admission.
    r.act("save", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty());
    assert!(
        logs(&r).contains("calendar.saveEvent: refused, grant-missing"),
        "{}",
        logs(&r)
    );
}

/// The bake's warnings and errors for `app`, by id and message.
fn warned(app: &App) -> String {
    let manifest = r#"{"name":"C","app":{"id":"test.cc","name":"C"}}"#;
    std::fs::write(app.0.join("app.json"), manifest).unwrap();
    let (result, warnings) =
        contract::compile_path_all_with_warnings(&app.0.join("app.contract"), false);
    let errors = result.err().unwrap_or_default();
    let all: Vec<_> = (warnings.iter().chain(&errors))
        .map(|e| (&e.id, &e.message))
        .collect();
    format!("{all:?}")
}

/// The bake names a store name without its line (`grant-missing`), and a
/// pick, which needs none, not at all (D4, D11).
#[test]
fn the_bake_names_a_store_name_without_its_line() {
    let said = warned(&App::new("bake", CALENDAR));
    assert!(said.contains("grant-missing"), "{said}");
    assert!(said.contains("saveEvent"), "{said}");
    assert!(!said.contains("showEventEditor"), "{said}");
    let said = warned(&App::new("bake-pick", CONTACTS));
    assert!(!said.contains("selectContacts"), "{said}");
}

/// Android's `WRITE_CALENDAR` follows a name that writes, under a line the
/// app grants (D11): an app that only reads its calendar derives none.
#[test]
fn a_writing_name_derives_android_s_write_permission() {
    const READS: &str = r#"use calendarStatus, calendars, CalendarStatus, CalendarList from "@exact/calendar"

component App
  resource status = calendarStatus() as shape CalendarStatus
  resource list = calendars() as shape CalendarList
  view
    column
      text status.access
"#;
    let plan = |name: &str, source: &str| {
        let app = App::new(name, source);
        contract::compile_path_all(&app.0.join("app.contract"), false)
            .unwrap_or_else(|e| panic!("{e:?}"))
            .0
    };
    // The plan holds every name of a module used; what the app calls is its source's.
    let reads = plan("writes-none", READS);
    let writes = plan("writes", CALENDAR);
    let used = |source: &'static str| move |name: &str| source.contains(name);
    let derive = contract::native::android_write_permissions;
    let full = Some("device.calendar calendarPurpose");
    assert!(derive(&reads, full, used(READS)).is_empty());
    assert_eq!(
        derive(&writes, full, used(CALENDAR)),
        ["android.permission.WRITE_CALENDAR"]
    );
    assert_eq!(
        derive(&writes, None, used(CALENDAR)),
        ["android.permission.WRITE_CALENDAR"]
    );
    // The write-only line derives both itself; no granted line, nothing.
    let write_only = Some("device.calendar-write-only calendarPurpose");
    assert!(derive(&writes, write_only, used(CALENDAR)).is_empty());
    assert!(derive(&writes, Some(""), used(CALENDAR)).is_empty());
}

/// The Android entry finds a writing name in the app's source, the
/// Contract's or the data crate's binding call, and writes what it derives.
#[test]
fn the_android_entry_writes_what_the_names_derive() {
    let entry = |name: &str, source: &str, rust: Option<&str>| {
        let app = App::new(name, source);
        if let Some(rust) = rust {
            std::fs::create_dir_all(app.0.join("data/src")).unwrap();
            std::fs::write(app.0.join("data/src/lib.rs"), rust).unwrap();
        }
        let manifest = r#"{"name":"C","app":{"id":"test.cc","name":"C"}}"#;
        std::fs::write(app.0.join("app.json"), manifest).unwrap();
        let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let out = app.0.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let compat = r#"{"inputs":{"grantCeiling":"device.calendar calendarPurpose"}}"#;
        std::fs::write(out.join("compat.json"), compat).unwrap();
        let manifest = contract::Manifest::read(&app.0).unwrap();
        contract::native::android_entry(&app.0, &manifest, &plan, &out).unwrap();
        std::fs::read_to_string(out.join("permissions.txt")).unwrap()
    };
    let reads = CALENDAR
        .lines()
        .filter(|l| !l.contains("saveEvent(") && !l.contains("action save"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("use calendarStatus, saveEvent,", "use calendarStatus,")
        .replace("  mutation saved as shape CalendarEvent\n", "");
    assert_eq!(entry("entry-reads", &reads, None), "");
    assert_eq!(
        entry("entry-contract", CALENDAR, None),
        "android.permission.WRITE_CALENDAR"
    );
    let rust = "fn f() { calendar::save_event(&options); }";
    assert_eq!(
        entry("entry-rust", &reads, Some(rust)),
        "android.permission.WRITE_CALENDAR"
    );
}

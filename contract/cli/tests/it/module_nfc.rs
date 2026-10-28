//! LLP 1133.035 stage 0, through the repository's `@exact/nfc` installed
//! beside an app: the declared substitute's arrivals (§10), a request let
//! go on the data-code route (D16 item 4), and the origin rule over every
//! cause (D16 item 6), whose kept request carries each origin that gave it
//! arguments into the commit its answer makes.

use exact_runner::{
    agent, Answer, CallOrigin, DataError, Dispatch, Outcome, Reply, Request, Response, Runner,
    Store, Value, Work,
};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

const APP: &str = r#"use nfcStatus, scan, abortScan, write, makeReadOnly, abortWrite, reading, readingerror, NFCStatus, NDEFScanEnd, NDEFWriteResult, NDEFReading, NFCTag, NDEFMessageInit, NDEFRecordInit from "@exact/nfc"

shape Result
  ok: bool
  message: string

component App
  state heard = ""
  state key = ""
  resource status = nfcStatus() as shape NFCStatus
  resource parsed = lookup(key) as shape Result
  mutation scanned as shape NDEFScanEnd
  mutation written as shape NDEFWriteResult
  mutation tagged as shape Result
  mutation records as shape Result
  mutation commanded as shape Result
  mutation slowed as shape Result
  action startScan
    send scanned = scan(once=true)
  action keepScanning
    send scanned = scan()
  action stopScan
    abortScan()
  action writeUrl
    send written = write(message=NDEFMessageInit(records=[NDEFRecordInit(recordType="url", mediaType=none, id=none, encoding=none, lang=none, data=some("https://openlibrary.org/works/OL893415W"))]))
  action writeEmpty
    send written = write(message=NDEFMessageInit(records=[]))
  action lock
    send written = makeReadOnly()
  action stopWrite
    abortWrite()
  action tagData
    send tagged = tag()
  action recordsOf(id: string)
    send records = records(id)
  action command
    send commanded = command()
  action slow
    send slowed = slow()
  action toolKey
    key = "tool"
  action tagRead(r: NDEFReading)
    heard = `read ${r.id}`
    key = r.id
  action unread(t: NFCTag)
    heard = `unread ${t.id}`
  view
    column reading=tagRead readingerror=unread
      text heard
"#;

/// The fixture tags of §11's drives.
const TAGS: &str = r#"[{"id":"blank","serialNumber":"04:a2","records":[]},
  {"id":"dune","serialNumber":"04:d1","records":[{"recordType":"url","data":"https://openlibrary.org/works/OL893415W"},
    {"recordType":"com.exact.shelf:book","data":{"base64":"Ym9vay0x"}}]},
  {"id":"locked","records":[],"write":"read-only"},
  {"id":"card","ndef":false}]"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-nfc-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@exact/nfc");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/nfc");
        for file in [
            "package.json",
            "exact.json",
            "apple/exact.json",
            "android/exact.json",
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

/// The app's data code, as its `app.ts` would run: `tag` writes the
/// book's records through the module, `records` reads a reading's,
/// `command` sends a tag command, `slow` waits on a fetch, and `lookup`
/// fetches, its parser yielding a command once a reading set its key.
#[derive(Clone, Default)]
struct Data(Arc<Mutex<Vec<Json>>>);
impl exact_runner::DataSource for Data {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let native = |send: &str, args: Json| {
            let body = json!({ "extension": "nfc", "send": send, "args": args });
            Ok(Answer::Later(Request::native(
                body.to_string().into_bytes(),
            )))
        };
        match source {
            "tag" => native(
                "write",
                json!({ "message": { "records": [
                    { "recordType": "url", "data": "https://openlibrary.org/works/OL893415W" },
                    { "recordType": "com.exact.shelf:book", "data": { "base64": "Ym9vay0x" } }] } }),
            ),
            "records" => native("ndefRecords", json!({ "reading": args[0].as_str() })),
            "command" => native(
                "sendCommand",
                json!({ "tag": "t", "apdu": { "base64": "YA==" } }),
            ),
            "slow" | "lookup" => Ok(Answer::Later(Request::get("https://fixture.test/slow"))),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Outcome::Response(Response { body, .. }) = outcome else {
            return Err(DataError::Unavailable("no reply".into()));
        };
        let reply: Json = serde_json::from_slice(&body).unwrap_or(Json::Null);
        if source == "lookup" {
            // Yielded once: the command's own reply is what it answers.
            if args[0].as_str().is_some_and(|k| !k.is_empty()) && reply.is_null() {
                let body = json!({ "extension": "nfc", "send": "sendCommand",
                    "args": { "tag": "t", "apdu": { "base64": "YA==" } } });
                return Ok(Answer::Later(Request::native(
                    body.to_string().into_bytes(),
                )));
            }
        }
        self.0.lock().unwrap().push(reply.clone());
        let message = match reply["error"].as_str() {
            Some(code) => format!("{code}: {}", reply["message"].as_str().unwrap_or_default()),
            None => reply.to_string(),
        };
        Ok(Answer::Now(Value::record(vec![
            Value::Bool(reply["error"].is_null()),
            Value::str(&message),
        ])))
    }
    fn grants(&self) -> &str {
        "device.nfc"
    }
}

/// The host: the runner, its requests run, and the fetches the test
/// answers itself, by target.
struct Host {
    r: Runner<Data>,
    data: Data,
    waiting: Vec<(u64, mpsc::Receiver<Outcome>)>,
    fetches: Vec<(u64, String)>,
    _app: App,
}

impl Host {
    fn boot(name: &str, target: &str) -> Self {
        let app = App::new(name);
        let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let data = Data::default();
        let mut r = Runner::boot(
            plan,
            data.clone(),
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.set_module_target(target);
        let mut host = Self {
            r,
            data,
            waiting: Vec::new(),
            fetches: Vec::new(),
            _app: app,
        };
        host.pump();
        host
    }

    /// Each request run, each reply that came fulfilled, events delivered,
    /// until nothing is left; a plain fetch waits for [`Host::answer`].
    fn pump(&mut self) {
        for _ in 0..16 {
            let outs = self.r.take_requests();
            let mut moved = !outs.is_empty() | !self.r.apply_announced().0.is_empty();
            for out in outs {
                if let Some(token) = out.request.continuation {
                    match self.r.dispatch_work(token) {
                        Dispatch::Run(Work::Now(work)) => {
                            let _ = self.r.fulfill(out.ticket, work());
                        }
                        _ => self.parked(out.ticket, token),
                    }
                    continue;
                }
                if !out.request.is_native() {
                    self.fetches.push((out.ticket, out.target.clone()));
                    continue;
                }
                let (tx, rx) = mpsc::channel();
                match self.r.native_work(&out.request) {
                    Dispatch::Run(Work::Now(work)) => tx.send(work()).unwrap(),
                    Dispatch::Run(Work::Later(work)) => work(Reply::new(move |o| {
                        let _ = tx.send(o);
                    })),
                    _ => continue,
                }
                self.waiting.push((out.ticket, rx));
            }
            for (token, dispatch) in self.r.release_work() {
                if let (Some(ticket), Dispatch::Run(Work::Now(work))) = (
                    PARKED.with(|p| {
                        p.borrow()
                            .iter()
                            .find(|(t, _)| *t == token)
                            .map(|(_, k)| *k)
                    }),
                    dispatch,
                ) {
                    moved = true;
                    let _ = self.r.fulfill(ticket, work());
                }
            }
            for (ticket, rx) in std::mem::take(&mut self.waiting) {
                match rx.try_recv() {
                    Ok(outcome) => {
                        moved = true;
                        let _ = self.r.fulfill(ticket, outcome);
                    }
                    Err(_) => self.waiting.push((ticket, rx)),
                }
            }
            if !moved {
                return;
            }
        }
    }

    fn parked(&self, ticket: u64, token: u64) {
        PARKED.with(|p| p.borrow_mut().push((token, ticket)));
    }

    /// Answer the fetch in flight for `target` with an empty body.
    fn answer(&mut self, target: &str) {
        let at = (self.fetches.iter())
            .rposition(|(_, t)| t == target)
            .unwrap_or_else(|| panic!("no fetch for {target}: {:?}", self.fetches));
        let (ticket, _) = self.fetches.remove(at);
        let _ = self.r.fulfill(
            ticket,
            Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: b"null".to_vec(),
            }),
        );
        self.pump();
    }

    fn act(&mut self, action: &str) {
        self.r.act(action, Vec::new()).unwrap();
        self.pump();
    }

    fn act_from(&mut self, origin: CallOrigin, action: &str) {
        self.r.act_from(origin, action, Vec::new()).unwrap();
        self.pump();
    }

    fn prefer(&mut self, extension: Json) -> Result<String, String> {
        let request = json!({ "op": "prefer", "extension": extension }).to_string();
        let (_, reply) = self.r.extension_agent(&request).expect("a prefer");
        self.pump();
        reply
    }

    fn field(&mut self, id: &str) {
        self.prefer(json!({ "nfc.field": id })).unwrap();
    }

    fn tags(&mut self) {
        let tags: Json = serde_json::from_str(TAGS).unwrap();
        self.prefer(json!({ "nfc.tags": tags })).unwrap();
    }

    fn logs(&self) -> String {
        agent::handle(&self.r, r#"{"op":"logs"}"#)
    }

    fn nfc(&self) -> Json {
        let state: Json =
            serde_json::from_str(&agent::handle(&self.r, r#"{"op":"state"}"#)).unwrap();
        state["extensions"]["nfc"].clone()
    }

    fn slot(&self, name: &str) -> String {
        format!("{:?}", self.r.resource(name).or_else(|| self.r.slot(name)))
    }

    fn heard(&self) -> String {
        format!("{:?}", self.r.slot("heard"))
    }

    /// The device tickets held for `capability`, oldest first.
    fn tickets(&self, capability: &str) -> Vec<u64> {
        (self.r.device_holds().iter())
            .filter(|h| h.capability == capability)
            .map(|h| h.ticket)
            .collect()
    }

    fn tap(&mut self, ticket: u64, choice: &str) -> String {
        let request = format!(r#"{{"op":"tap","ticket":{ticket},"choice":"{choice}"}}"#);
        let reply = agent::answer(&mut self.r, &request).unwrap().0;
        self.pump();
        reply
    }

    fn hidden(&mut self, hidden: bool) {
        let mut page = self.r.page();
        page.hidden = hidden;
        self.r.set_page(page).unwrap();
        self.pump();
    }
}

thread_local! {
    /// Each held call's continuation and the ticket it went out under.
    static PARKED: std::cell::RefCell<Vec<(u64, u64)>> = Default::default();
}

/// §10: a tag in the field reads into a waiting scan, raising `reading`,
/// and a `once` scan answers `read` with the reading's id; a tag with no
/// NDEF raises `readingerror`, and the scan goes on until the person's
/// cancel; a tag outside a scan is the system's.
#[test]
fn a_tag_in_the_field_reads_into_a_waiting_scan() {
    let mut h = Host::boot("read", "android");
    h.tags();
    h.field("dune");
    assert!(
        h.logs().contains("tag `dune` read by no scan"),
        "{}",
        h.logs()
    );
    h.act("startScan");
    let scans = h.tickets("scan");
    assert_eq!(scans.len(), 1, "{}", h.logs());
    assert_eq!(h.nfc()["scans"][0]["once"], true, "{}", h.nfc());
    // The same tag still in the field is no new arrival; a clear and the same again is.
    h.field("dune");
    assert!(h.tickets("scan").len() == 1);
    h.field("clear");
    h.field("dune");
    assert!(h.heard().contains("read r1"), "{} {}", h.heard(), h.logs());
    assert!(
        h.slot("scanned").contains("\"read\""),
        "{}",
        h.slot("scanned")
    );
    assert!(h.slot("scanned").contains("r1"), "{}", h.slot("scanned"));
    assert!(h.tickets("scan").is_empty());
    assert_eq!(h.nfc()["readings"][0], json!({ "id": "r1", "tag": "dune" }));

    h.act("keepScanning");
    h.field("card");
    assert!(h.heard().contains("unread card"), "{}", h.heard());
    let scan = h.tickets("scan");
    assert_eq!(scan.len(), 1, "the scan goes on: {}", h.logs());
    h.tap(scan[0], "cancel");
    assert!(
        h.slot("scanned").contains("cancelled"),
        "{}",
        h.slot("scanned")
    );
    assert!(h.tickets("scan").is_empty());
}

/// D11: an NDEF reader session never reports a tag with no NDEF, so on iOS
/// a tier 1 scan hears nothing of one; the system's `timeout` ends a scan.
#[test]
fn ios_reads_no_tag_without_ndef_and_its_scan_times_out() {
    let mut h = Host::boot("ios", "ios");
    h.tags();
    assert!(h.slot("status").contains("system"), "{}", h.slot("status"));
    h.act("keepScanning");
    h.field("card");
    assert!(!h.heard().contains("unread"), "{}", h.heard());
    let scan = h.tickets("scan");
    let refused = h.tap(scan[0], "later");
    assert!(refused.contains("timeout"), "the taps are named: {refused}");
    h.tap(scan[0], "timeout");
    assert!(
        h.slot("scanned").contains("timeout"),
        "{}",
        h.slot("scanned")
    );
}

/// §10's writes: a waiting write settles against the tag that arrives and
/// raises no reading; the fixture then holds the records; a locked tag
/// fails `NetworkError`; a newer write replaces the older; `abortWrite`
/// and a cancel fail `AbortError`; a message with no record, `TypeError`.
#[test]
fn a_waiting_write_settles_against_the_tag() {
    let mut h = Host::boot("write", "android");
    h.tags();
    h.act("writeUrl");
    assert_eq!(h.tickets("write").len(), 1, "{}", h.logs());
    assert_eq!(h.nfc()["write"]["name"], "write");
    h.field("blank");
    assert!(
        h.slot("written").contains("04:a2"),
        "{} {}",
        h.slot("written"),
        h.logs()
    );
    assert!(
        !h.heard().contains("read"),
        "a write raises no reading: {}",
        h.heard()
    );
    let blank = &h.nfc()["tags"][0];
    assert_eq!(blank["records"][0]["recordType"], "url", "{blank}");

    h.act("writeUrl");
    h.field("locked");
    assert!(h.logs().contains("write: NetworkError"), "{}", h.logs());

    h.act("writeUrl");
    h.act("lock");
    assert!(
        h.logs().contains("write: replaced by a newer write"),
        "{}",
        h.logs()
    );
    assert_eq!(h.tickets("write").len(), 1);
    h.act("stopWrite");
    assert!(
        h.logs().contains("makeReadOnly: stopped by abortWrite"),
        "{}",
        h.logs()
    );
    assert!(h.tickets("write").is_empty());

    h.act("writeEmpty");
    assert!(h.logs().contains("TypeError"), "{}", h.logs());
    assert!(h.tickets("write").is_empty());

    h.act("writeUrl");
    let write = h.tickets("write");
    h.tap(write[0], "cancel");
    assert!(h.logs().contains("AbortError"), "{}", h.logs());
}

/// `abortScan` ends each waiting scan `stopped`; a scan superseded by
/// another leaves one.
#[test]
fn abort_and_a_superseded_scan_leave_one_claim() {
    let mut h = Host::boot("abort", "android");
    h.act("keepScanning");
    h.act("keepScanning");
    assert_eq!(
        h.nfc()["scans"].as_array().map(Vec::len),
        Some(1),
        "{}",
        h.nfc()
    );
    h.act("stopScan");
    assert!(
        h.slot("scanned").contains("stopped"),
        "{}",
        h.slot("scanned")
    );
    assert_eq!(h.nfc()["scans"], json!([]));
}

/// D6 by `readerUI`: leaving the foreground ends a scan behind the
/// system's sheet `interrupted`, and suspends one the app draws for, which
/// on return reads a tag still in the field; a waiting write fails.
#[test]
fn the_background_interrupts_or_suspends_by_reader_ui() {
    let mut h = Host::boot("bg-ios", "ios");
    h.act("keepScanning");
    h.hidden(true);
    assert!(
        h.slot("scanned").contains("interrupted"),
        "{}",
        h.slot("scanned")
    );

    let mut h = Host::boot("bg-android", "android");
    h.tags();
    h.act("keepScanning");
    h.act("writeUrl");
    h.hidden(true);
    assert!(h.logs().contains("write: aborted"), "{}", h.logs());
    assert_eq!(h.nfc()["scans"][0]["suspended"], true, "{}", h.nfc());
    h.field("dune");
    assert!(!h.heard().contains("read"), "suspended: {}", h.heard());
    h.hidden(false);
    assert!(h.heard().contains("read r1"), "{} {}", h.heard(), h.logs());
}

/// D16 item 4: a write data code started is let go with the request that
/// made it, so a later tag is not written; `ndefRecords` answers a kept
/// reading's records with their bytes.
#[test]
fn a_data_code_write_is_let_go_with_its_request() {
    let mut h = Host::boot("letgo", "android");
    h.tags();
    h.act("tagData");
    h.act("tagData");
    assert!(
        h.logs().contains("nfc.write: its call was let go"),
        "{}",
        h.logs()
    );
    assert_eq!(h.tickets("write").len(), 1, "{}", h.logs());
    h.field("blank");
    let blank = &h.nfc()["tags"][0];
    assert_eq!(
        blank["records"][1]["recordType"], "com.exact.shelf:book",
        "{blank}"
    );
    assert!(h.slot("tagged").contains("04:a2"), "{}", h.slot("tagged"));

    h.act("keepScanning");
    h.field("dune");
    h.r.act("recordsOf", vec![Value::str("r1")]).unwrap();
    h.pump();
    let seen = h.data.0.lock().unwrap().last().cloned().unwrap();
    assert_eq!(seen[1]["data"], json!({ "base64": "Ym9vay0x" }), "{seen}");
    assert_eq!(
        seen[0]["text"], "https://openlibrary.org/works/OL893415W",
        "{seen}"
    );
    h.r.act("recordsOf", vec![Value::str("r9")]).unwrap();
    h.pump();
    assert!(
        h.slot("records").contains("NotFoundError"),
        "{}",
        h.slot("records")
    );
}

/// D15: only the person scans or writes; a tool's request is refused
/// `person-required`, and an absent target answers absence.
#[test]
fn a_tool_never_scans_and_an_absent_target_answers_absence() {
    let mut h = Host::boot("tool", "android");
    h.act_from(CallOrigin::Tool, "startScan");
    assert!(h.logs().contains("person-required"), "{}", h.logs());
    assert!(h.tickets("scan").is_empty());

    let mut h = Host::boot("absent", "macos");
    assert!(h.slot("status").contains("false"), "{}", h.slot("status"));
    h.act("startScan");
    assert!(h.logs().contains("NotSupportedError"), "{}", h.logs());
}

/// The web starts at `prompt` (D15): a scan holds the permission's ticket
/// first, then its own.
#[test]
fn the_web_asks_before_its_first_scan() {
    let mut h = Host::boot("web", "web");
    h.act("startScan");
    let asked = h.tickets("permission");
    assert_eq!(asked.len(), 1, "{}", h.logs());
    h.tap(asked[0], "allow");
    assert_eq!(h.tickets("scan").len(), 1, "{}", h.logs());
}

/// D16 item 6: a command is refused `person-required` from a tool or a
/// wake while a tag's lease is live; beside a tool's request still in
/// flight; and when a parser yields it after a tool's newer arguments
/// reached a kept request that a reading started. A reading's own command
/// passes admission, to the substitute's `NotSupportedError`.
#[test]
fn a_command_is_refused_with_a_tool_or_a_wake_among_its_causes() {
    let mut h = Host::boot("causes", "ios");
    h.tags();
    h.answer("parsed");
    h.act("keepScanning");
    h.field("dune");
    h.act_from(CallOrigin::Tool, "command");
    assert!(
        h.slot("commanded").contains("person-required"),
        "{}",
        h.logs()
    );
    h.act_from(CallOrigin::Wake, "command");
    assert!(h.logs().contains("a wake among its causes"), "{}", h.logs());
    h.act("command");
    assert!(
        h.slot("commanded").contains("NotSupportedError"),
        "{} {}",
        h.slot("commanded"),
        h.logs()
    );

    h.act_from(CallOrigin::Tool, "slow");
    h.act("command");
    assert!(
        h.slot("commanded").contains("person-required"),
        "beside a tool's request: {}",
        h.logs()
    );
    h.answer("slowed");

    // The reading set `key` (the device's), then a tool's newer key reached the kept request.
    assert!(
        h.fetches.iter().any(|(_, t)| t == "parsed"),
        "{:?} {}",
        h.fetches,
        h.logs()
    );
    h.act_from(CallOrigin::Tool, "toolKey");
    assert!(h.logs().contains("keep request"), "{}", h.logs());
    h.answer("parsed");
    let parsed = h.slot("parsed");
    assert!(parsed.contains("person-required"), "{parsed} {}", h.logs());
}

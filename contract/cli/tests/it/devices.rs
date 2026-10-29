//! `@exact/serial`, `@exact/hid` and `@exact/usb` under their declared
//! substitute (LLP 1133.037 §11). The interfaces are the packages' own
//! `exact.json`s:
//!
//! - `requestPort` holds a chooser ticket listing the preferred ports;
//!   `tap @t <id>` answers its record and remembers it, `cancel` fails
//!   `NotFoundError` (HID: an empty list);
//! - a port opens once: a second open fails `InvalidStateError`;
//! - `serialInput` answers every byte since its last answer, the fixture's
//!   events delivered when the clock passes their `at`, errors at their
//!   offsets, and `done` with `NetworkError` once the port is unplugged;
//! - writes and signals are journaled; a tool's write fails
//!   `person-required`;
//! - HID's reports are a batch (the latest first, then every report since)
//!   and a feature report is the fixture's by its id;
//! - a USB control transfer's key answers its list in turn, and an
//!   interface's request on an unclaimed interface fails;
//! - on Android an ungranted device's open holds a permission ticket, and
//!   HID is absent there.

use exact_runner::{Dispatch, Outcome, Reply, Request, Runner, Work};
use serde_json::{json, Value as Json};
use std::path::PathBuf;

const APP: &str = r##"use serialStatus, requestPort, getPorts, attachedPorts, open as openPort, serialConnection, setSignals, SerialStatus, SerialPortRecord, SerialOpened, SerialConnection from "@exact/serial"
use hidStatus, HIDStatus from "@exact/hid"
use usbStatus, USBStatus from "@exact/usb"

component App
  state handle = ""
  resource status = serialStatus() as shape SerialStatus else empty()
  resource hid = hidStatus() as shape HIDStatus else empty()
  resource usb = usbStatus() as shape USBStatus else empty()
  resource ports = getPorts() as shape SerialPortRecord[] else empty()
  resource attached = attachedPorts() as shape SerialPortRecord[] else empty()
  resource conn = serialConnection(connection=handle) as shape SerialConnection else empty()
  mutation chosen as shape SerialPortRecord
  mutation opened as shape SerialOpened then keep
  action choose
    send chosen = requestPort()
  action connect
    send opened = openPort(port="s3", baudRate=115200)
  action keep
    handle = match opened { case some(o) => o.connection, case none => "" }
  action reset
    setSignals(connection=handle, requestToSend=true)
  view
    column
      text `${status.available} ${status.attachedList} ${status.state} ${hid.available} ${usb.available}` testId="status"
      text ports.map(p => `${p.id}:${p.connected}`).join(",") testId="ports"
      text attached.map(p => p.name).join(",") testId="attached"
      text `${conn.open} ${conn.connected}` testId="conn"
      match chosen
        case some(p)
          text p.name testId="chosen"
        case none
          text "" testId="chosen"
      match failure(chosen)
        case some(f)
          text f.code testId="chosen-error"
        case none
          text "" testId="chosen-error"
      match failure(opened)
        case some(f)
          text f.code testId="open-error"
        case none
          text "" testId="open-error"
"##;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-devices-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        for package in ["serial", "hid", "usb"] {
            let to = path.join("node_modules/@exact").join(package);
            let from = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages")
                .join(package);
            for file in [
                "package.json",
                "exact.json",
                "web/exact.json",
                "apple/exact.json",
                "linux/exact.json",
                "windows/exact.json",
            ] {
                if from.join(file).exists() {
                    std::fs::create_dir_all(to.join(file).parent().unwrap()).unwrap();
                    std::fs::copy(from.join(file), to.join(file)).unwrap();
                }
            }
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

struct Data;
impl exact_runner::DataSource for Data {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
}

fn boot(name: &str, target: &str) -> (App, Runner<Data>, Vec<(u64, u64)>) {
    let app = App::new(name);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Data,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_module_target(target);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    (app, r, parked)
}

fn pump(r: &mut Runner<Data>, parked: &mut Vec<(u64, u64)>) {
    for _ in 0..8 {
        // An answer's `then` runs as a host's advance would run it.
        r.land_then();
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

fn run(r: &mut Runner<Data>, ticket: u64, dispatch: Dispatch) {
    if let Some(outcome) = settle(dispatch) {
        let _ = r.fulfill(ticket, outcome);
    }
}

fn settle(dispatch: Dispatch) -> Option<Outcome> {
    match dispatch {
        Dispatch::Run(Work::Now(work)) => Some(work()),
        Dispatch::Run(Work::Later(work)) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            rx.try_recv().ok()
        }
        _ => None,
    }
}

/// Data code's call of a module name: its answer, or `null` while held.
fn call(r: &mut Runner<Data>, extension: &str, verb: &str, name: &str, args: Json) -> Json {
    let body = json!({ "extension": extension, verb: name, "args": args }).to_string();
    let dispatch = r.native_work(&Request::native(body.into_bytes()));
    match settle(dispatch) {
        Some(Outcome::Response(reply)) => serde_json::from_slice(&reply.body).unwrap(),
        _ => Json::Null,
    }
}

fn prefer(r: &mut Runner<Data>, key: &str, value: Json) {
    let request = json!({ "op": "prefer", "extension": { key: value } }).to_string();
    let (_, reply) = r.extension_agent(&request).expect("a prefer");
    reply.unwrap();
}

fn text(r: &Runner<Data>, id: &str) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    let node = r.kernel().node_by_key(key).unwrap();
    (node.props.iter())
        .find_map(|(p, v)| match v {
            exact_kernel::PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn extension(r: &Runner<Data>, name: &str) -> Json {
    let state: Json =
        serde_json::from_str(&exact_runner::agent::handle(r, r#"{"op":"state"}"#)).unwrap();
    state["extensions"][name].clone()
}

fn b64(bytes: &[u8]) -> String {
    exact_runner::agent::base64(bytes)
}

fn ports() -> Json {
    json!([{
        "id": "s3", "name": "USB JTAG/serial debug unit", "usbVendorId": 12346, "usbProductId": 4097,
        "input": [
            { "at": 0, "data": b64(b"ESP-ROM:esp32s3\r\nboot:0x8\r\n") },
            { "at": 200, "data": b64("caf\u{e9} au ".as_bytes()) },
            { "at": 300, "error": "BreakError" },
            { "at": 400, "data": b64(b"lait\r\n") }
        ]
    }])
}

#[test]
fn the_status_reads_each_target() {
    let (_app, r, _) = boot("status-mac", "macos");
    assert_eq!(text(&r, "status"), "true true granted true true");
    let (_app, r, _) = boot("status-web", "web");
    assert_eq!(text(&r, "status"), "true false granted true true");
    let (_app, r, _) = boot("status-android", "android");
    assert_eq!(text(&r, "status"), "true true prompt false true");
    let (_app, r, _) = boot("status-ios", "ios");
    assert_eq!(text(&r, "status"), "false false denied false false");
}

#[test]
fn the_chooser_lists_the_preferred_ports_and_remembers_the_one_chosen() {
    let (_app, mut r, mut parked) = boot("chooser", "web");
    prefer(&mut r, "serial.ports", ports());
    r.act("choose", vec![]).unwrap();
    pump(&mut r, &mut parked);
    let hold = r.device_holds().last().cloned().expect("a chooser ticket");
    assert_eq!(hold.capability, "chooser");
    assert_eq!(hold.choices, ["s3"]);
    r.answer_hold(hold.ticket, &exact_runner::HoldAnswer::Choice("s3".into()))
        .unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "chosen"), "USB JTAG/serial debug unit");
    assert_eq!(text(&r, "ports"), "s3:true");
    // Cancel fails NotFoundError, as Web Serial's does.
    r.act("choose", vec![]).unwrap();
    pump(&mut r, &mut parked);
    let hold = r.device_holds().last().cloned().unwrap();
    r.answer_hold(
        hold.ticket,
        &exact_runner::HoldAnswer::Choice("cancel".into()),
    )
    .unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "chosen-error"), "NotFoundError");
    // No attached list on the web.
    assert_eq!(text(&r, "attached"), "");
}

#[test]
fn a_port_streams_every_byte_once_with_its_errors_and_ends_when_unplugged() {
    let (_app, mut r, mut parked) = boot("stream", "macos");
    prefer(&mut r, "serial.ports", ports());
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "attached"), "USB JTAG/serial debug unit");
    r.act("connect", vec![]).unwrap();
    pump(&mut r, &mut parked);
    let logs = exact_runner::agent::handle(&r, r#"{"op":"logs"}"#);
    assert_eq!(
        text(&r, "conn"),
        "true true",
        "{} {logs}",
        text(&r, "open-error")
    );
    assert_eq!(text(&r, "ports"), "s3:true", "an opened port is remembered");
    // A second open of the port fails before anything else.
    r.act("connect", vec![]).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "open-error"), "InvalidStateError");
    let handle = extension(&r, "serial")["connections"][0]["handle"].clone();
    let read = |r: &mut Runner<Data>| {
        call(
            r,
            "serial",
            "source",
            "serialInput",
            json!({ "connection": handle }),
        )
    };
    let first = read(&mut r);
    assert_eq!(
        first["data"],
        b64(b"ESP-ROM:esp32s3\r\nboot:0x8\r\n"),
        "{first}"
    );
    assert_eq!(read(&mut r)["data"], "", "nothing twice");
    r.advance(450.0).unwrap();
    let next = read(&mut r);
    assert_eq!(next["data"], b64("caf\u{e9} au lait\r\n".as_bytes()));
    assert_eq!(next["errors"], json!([{ "at": 36, "code": "BreakError" }]));
    // Writes and signals are journaled.
    let wrote = call(
        &mut r,
        "serial",
        "send",
        "write",
        json!({ "connection": handle, "data": b64(b"help\r\n") }),
    );
    assert_eq!(wrote, Json::Null);
    r.act("reset", vec![]).unwrap();
    pump(&mut r, &mut parked);
    let writes = &extension(&r, "serial")["writes"];
    assert_eq!(writes[0]["data"], b64(b"help\r\n"), "{writes}");
    assert_eq!(writes[1]["name"], "setSignals");
    assert_eq!(writes[1]["requestToSend"], true);
    // Unplugged: the stream ends with NetworkError, then reading fails.
    let mut gone = ports();
    gone[0]["connected"] = json!(false);
    prefer(&mut r, "serial.ports", gone);
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "conn"), "false false");
    let end = read(&mut r);
    assert_eq!(end["done"], true);
    assert_eq!(end["errors"][0]["code"], "NetworkError");
    assert_eq!(read(&mut r)["error"], "InvalidStateError");
}

#[test]
fn hid_reports_are_a_batch_and_feature_reports_are_the_fixtures() {
    let (_app, mut r, mut parked) = boot("hid", "macos");
    let light = json!([{
        "id": "b1", "vendorId": 10168, "productId": 493, "productName": "blink(1) mk3",
        "collections": [{ "usagePage": 65280, "usage": 1, "featureReports": [{ "reportId": 1 }] }],
        "inputReports": [
            { "at": 0, "reportId": 1, "data": b64(&[1]) },
            { "at": 10, "reportId": 1, "data": b64(&[2]) },
            { "at": 300, "reportId": 1, "data": b64(&[3]) },
            { "at": 310, "reportId": 1, "data": b64(&[4]) }
        ],
        "featureReports": { "1": b64(&[1, 0x63, 0xff]) }
    }]);
    prefer(&mut r, "hid.devices", light);
    pump(&mut r, &mut parked);
    let listed = call(
        &mut r,
        "hid",
        "source",
        "attachedDevices",
        json!({ "filters": [{ "vendorId": 10168 }] }),
    );
    assert_eq!(listed[0]["productName"], "blink(1) mk3");
    let opened = call(&mut r, "hid", "send", "open", json!({ "device": "b1" }));
    let connection = opened["connection"].clone();
    assert_eq!(connection, "hid-1", "{opened}");
    // A second open of a HID device is a lease, not a refusal.
    assert_eq!(
        call(&mut r, "hid", "send", "open", json!({ "device": "b1" }))["connection"],
        "hid-2"
    );
    r.advance(100.0).unwrap();
    let first = call(
        &mut r,
        "hid",
        "source",
        "inputReports",
        json!({ "connection": connection }),
    );
    assert_eq!(
        first["reports"].as_array().unwrap().len(),
        1,
        "the latest: {first}"
    );
    assert_eq!(first["reports"][0]["data"], b64(&[2]));
    r.advance(400.0).unwrap();
    let next = call(
        &mut r,
        "hid",
        "source",
        "inputReports",
        json!({ "connection": connection, "history": 1 }),
    );
    // A reader of other arguments is another reader: its first answer is the latest.
    assert_eq!(next["reports"][0]["data"], b64(&[4]));
    let again = call(
        &mut r,
        "hid",
        "source",
        "inputReports",
        json!({ "connection": connection }),
    );
    assert_eq!(again["reports"].as_array().unwrap().len(), 2, "{again}");
    let feature = call(
        &mut r,
        "hid",
        "send",
        "receiveFeatureReport",
        json!({ "connection": connection, "reportId": 1 }),
    );
    assert_eq!(feature["data"], b64(&[1, 0x63, 0xff]));
    let missing = call(
        &mut r,
        "hid",
        "send",
        "receiveFeatureReport",
        json!({ "connection": connection, "reportId": 2 }),
    );
    assert_eq!(missing["error"], "NotAllowedError");
    let sent = call(
        &mut r,
        "hid",
        "send",
        "sendFeatureReport",
        json!({ "connection": connection, "reportId": 1, "data": b64(&[0x63, 0xff, 0, 0, 0, 30, 0]) }),
    );
    assert_eq!(sent, Json::Null);
    assert_eq!(extension(&r, "hid")["writes"][0]["reportId"], 1);
}

#[test]
fn a_usb_control_key_answers_its_list_in_turn_and_claims_are_kept() {
    let (_app, mut r, mut parked) = boot("usb", "linux");
    let status = |state: u8| json!({ "status": "ok", "data": b64(&[0, 0, 0, 0, state, 0]) });
    let board = json!([{
        "id": "stm", "vendorId": 1155, "productId": 57105, "productName": "DFU in FS Mode",
        "configurations": [{ "configurationValue": 1, "interfaces": [{ "interfaceNumber": 0, "alternates": [{
            "alternateSetting": 0, "interfaceClass": 254, "interfaceSubclass": 1, "interfaceProtocol": 2,
            "interfaceName": "@Internal Flash  /0x08000000/04*016Kg", "endpoints": [] }] }] }],
        "control": { "in class interface 3 0 0": [status(2), status(4), status(5)] }
    }]);
    prefer(&mut r, "usb.devices", board);
    pump(&mut r, &mut parked);
    let connection =
        call(&mut r, "usb", "send", "open", json!({ "device": "stm" }))["connection"].clone();
    let get_status = json!({ "connection": connection, "length": 6, "setup": { "requestType": "class", "recipient": "interface", "request": 3, "value": 0, "index": 0 } });
    // An interface's request before its claim fails.
    let unclaimed = call(
        &mut r,
        "usb",
        "send",
        "controlTransferIn",
        get_status.clone(),
    );
    assert_eq!(unclaimed["error"], "InvalidStateError", "{unclaimed}");
    call(
        &mut r,
        "usb",
        "send",
        "claimInterface",
        json!({ "connection": connection, "interfaceNumber": 0 }),
    );
    let states: Vec<u8> = (0..4)
        .map(|_| {
            let answer = call(
                &mut r,
                "usb",
                "send",
                "controlTransferIn",
                get_status.clone(),
            );
            exact_runner::agent::unbase64(answer["data"].as_str().unwrap()).unwrap()[4]
        })
        .collect();
    assert_eq!(states, [2, 4, 5, 5], "each entry once, then the last again");
    let out = call(
        &mut r,
        "usb",
        "send",
        "controlTransferOut",
        json!({ "connection": connection, "data": b64(&[0x21, 0, 0, 0, 8]), "setup": { "requestType": "class", "recipient": "interface", "request": 1, "value": 0, "index": 0 } }),
    );
    assert_eq!(out, json!({ "status": "ok", "bytesWritten": 5 }));
    // A second connection on the board cannot take the claimed interface.
    let other =
        call(&mut r, "usb", "send", "open", json!({ "device": "stm" }))["connection"].clone();
    let taken = call(
        &mut r,
        "usb",
        "send",
        "claimInterface",
        json!({ "connection": other, "interfaceNumber": 0 }),
    );
    assert_eq!(taken["error"], "NetworkError");
    let missing = call(
        &mut r,
        "usb",
        "send",
        "controlTransferIn",
        json!({ "connection": connection, "length": 4, "setup": { "requestType": "vendor", "recipient": "device", "request": 9, "value": 0, "index": 0 } }),
    );
    assert_eq!(missing["status"], "stall");
    assert_eq!(
        extension(&r, "usb")["sequences"]["stm control.in class interface 3 0 0"],
        4
    );
}

#[test]
fn android_asks_at_open_for_an_ungranted_device_and_has_no_hid() {
    let (_app, mut r, mut parked) = boot("android", "android");
    let mut list = ports();
    list[0]["granted"] = json!(false);
    prefer(&mut r, "serial.ports", list);
    r.act("connect", vec![]).unwrap();
    pump(&mut r, &mut parked);
    let hold = r
        .device_holds()
        .last()
        .cloned()
        .expect("a permission ticket");
    assert_eq!(hold.capability, "permission");
    r.answer_hold(
        hold.ticket,
        &exact_runner::HoldAnswer::Choice("allow".into()),
    )
    .unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "conn"), "true true");
    let hid = call(&mut r, "hid", "source", "getDevices", json!({}));
    assert_eq!(hid["error"], "NotSupportedError");
}

#[test]
fn a_tool_cannot_open_a_port() {
    let (_app, mut r, mut parked) = boot("tool", "macos");
    prefer(&mut r, "serial.ports", ports());
    r.act_from(exact_runner::CallOrigin::Tool, "connect", vec![])
        .unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "open-error"), "NotAllowedError");
    assert!(extension(&r, "serial")["connections"].is_null());
}

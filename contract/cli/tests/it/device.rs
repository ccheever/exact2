//! `@exact/device` under its declared substitute (LLP 1133.031 §7). The
//! interface is the package's own `exact.json`:
//!
//! - it loads with no `topic`, and each read answers the declaration's
//!   default: the specification's no-battery values, `type` `unknown`,
//!   neither low power nor heat;
//! - a preferred value is laid over the default (`partial`), each `prefer`
//!   replacing the last, and `clear` restores the default, every reader
//!   asked again;
//! - a target the declaration names `unsupported` fails `NotSupportedError`,
//!   and Linux and Windows answer no thermal state;
//! - an answer carries no platform's words the substitute did not give;
//! - a rendered document's checkpoint holds none of the three.

use exact_kernel::PropValue;
use exact_runner::{Dispatch, Outcome, Reply, Runner, Work};
use std::path::PathBuf;

const APP: &str = r##"use getBattery, connection, powerState, BatteryManager, NetworkInformation, PowerState from "@exact/device"

component App
  resource battery = getBattery() as shape BatteryManager else empty(level=-1)
  resource network = connection() as shape NetworkInformation else empty(type="unknown")
  resource power = powerState() as shape PowerState else empty()
  view
    column
      text `${battery.level} ${battery.charging}` testId="battery"
      text `${network.type} ${network.saveData}` testId="network"
      text `${power.lowPowerMode} ${match power.thermalState { case some(t) => t, case none => "none" }}` testId="power"
      text `${match failure(battery) { case some(f) => f.code, case none => "" }}` testId="battery-error"
      text `${match failure(network) { case some(f) => f.code, case none => "" }}` testId="network-error"
      text `${match failure(power) { case some(f) => f.code, case none => "" }}` testId="power-error"
      text `${match battery.android { case some(a) => a.status, case none => "none" }} ${match network.apple { case some(a) => a.expensive ? "expensive" : "cheap", case none => "none" }}` testId="words"
"##;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("exact-device-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@exact/device");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/device");
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(source.join("package.json")).unwrap()).unwrap();
        // The interface and every implementation it endorses, as an install lays them out.
        for exported in manifest["exports"].as_object().unwrap().values() {
            let file = exported.as_str().unwrap();
            let to = package.join(file);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(source.join(file), to).unwrap();
        }
        std::fs::copy(source.join("package.json"), package.join("package.json")).unwrap();
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

fn boot(name: &str, target: &str) -> (App, Runner<Data>) {
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
    pump(&mut r);
    (app, r)
}

/// The host's part: each request dispatched and answered, until nothing is left.
fn pump(r: &mut Runner<Data>) {
    for _ in 0..8 {
        let outs = r.take_requests();
        if outs.is_empty() && r.apply_announced().0.is_empty() {
            return;
        }
        for out in outs {
            let outcome: Outcome = match r.native_work(&out.request) {
                Dispatch::Run(Work::Now(work)) => work(),
                Dispatch::Run(Work::Later(work)) => {
                    let (tx, rx) = std::sync::mpsc::channel();
                    work(Reply::new(move |o| tx.send(o).unwrap()));
                    match rx.try_recv() {
                        Ok(o) => o,
                        Err(_) => continue,
                    }
                }
                _ => continue,
            };
            let _ = r.fulfill(out.ticket, outcome);
        }
    }
}

fn prefer(r: &mut Runner<Data>, source: &str, value: &str) {
    let request = format!(r#"{{"op":"prefer","extension":{{"device.{source}":{value}}}}}"#);
    let (_, reply) = r.extension_agent(&request).expect("a prefer");
    reply.unwrap();
    pump(r);
}

fn text(r: &Runner<Data>, id: &str) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    let node = r.kernel().node_by_key(key).unwrap();
    (node.props.iter())
        .find_map(|(p, v)| match v {
            PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn texts(r: &Runner<Data>) -> [String; 3] {
    ["battery", "network", "power"].map(|id| text(r, id))
}

#[test]
fn the_interface_loads_and_answers_its_defaults() {
    let interface: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/device/exact.json"),
        )
        .unwrap(),
    )
    .unwrap();
    for name in interface["names"].as_array().unwrap() {
        assert!(name["topic"].is_null(), "no topic is declared (D5): {name}");
        assert_eq!(name["live"], true, "{name}");
    }
    let (_app, r) = boot("defaults", "ios");
    assert_eq!(texts(&r), ["1 true", "unknown false", "false nominal"]);
    assert_eq!(
        text(&r, "words"),
        "none none",
        "the default names no platform's words"
    );
}

#[test]
fn a_preferred_value_is_laid_over_the_default_and_clear_restores_it() {
    let (_app, mut r) = boot("prefer", "macos");
    prefer(&mut r, "getBattery", r#"{"level":0.42,"charging":false}"#);
    assert_eq!(text(&r, "battery"), "0.42 false");
    prefer(
        &mut r,
        "connection",
        r#"{"type":"cellular","saveData":true}"#,
    );
    assert_eq!(text(&r, "network"), "cellular true");
    prefer(&mut r, "powerState", r#"{"lowPowerMode":true}"#);
    assert_eq!(text(&r, "power"), "true nominal");
    // Each prefer replaces the last overlay: the second forgets low power.
    prefer(&mut r, "powerState", r#"{"thermalState":"serious"}"#);
    assert_eq!(text(&r, "power"), "false serious");
    prefer(&mut r, "getBattery", r#""clear""#);
    assert_eq!(text(&r, "battery"), "1 true");
    let state: serde_json::Value =
        serde_json::from_str(&exact_runner::agent::handle(&r, r#"{"op":"state"}"#)).unwrap();
    let device = &state["extensions"]["device"];
    assert_eq!(
        device["preferred"]["powerState"]["thermalState"], "serious",
        "{device}"
    );
}

#[test]
fn an_unsupported_target_fails_not_supported() {
    let (_app, r) = boot("web", "web");
    assert_eq!(text(&r, "battery"), "1 true");
    assert_eq!(text(&r, "power-error"), "NotSupportedError");
    // The target is set after boot, whose first read had this Mac's answer,
    // and a failed read keeps the answer it had. A web build's first read
    // fails, so it shows the placeholder (§10 drive 5, `test/drives.mjs`).
    assert_eq!(text(&r, "power"), "false nominal");
    assert_eq!(
        ["battery-error", "network-error"].map(|id| text(&r, id)),
        ["", ""]
    );
}

#[test]
fn linux_and_windows_answer_no_thermal_state() {
    for target in ["linux", "windows"] {
        let (_app, r) = boot(target, target);
        assert_eq!(
            texts(&r),
            ["1 true", "unknown false", "false none"],
            "{target}"
        );
        assert_eq!(
            ["battery-error", "network-error", "power-error"].map(|id| text(&r, id)),
            ["", "", ""].map(String::from),
            "{target}"
        );
    }
}

#[test]
fn a_rendered_document_keeps_no_device_fact() {
    // A render answers from the declaration, not the reader's device; a
    // seeded answer is never asked again, so the page would never read it.
    let (_app, r) = boot("document", "web");
    let checkpoint = r.document_checkpoint("/");
    let names: Vec<&str> = (checkpoint.answers.iter())
        .map(|(n, ..)| n.as_str())
        .collect();
    for name in ["battery", "network", "power"] {
        assert!(!names.contains(&name), "{name} is in {names:?}");
        assert!(
            !checkpoint.pending.iter().any(|p| p == name),
            "{name} pending"
        );
    }
}

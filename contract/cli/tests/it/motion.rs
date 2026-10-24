//! `@exact/motion`'s seam under the declared substitute (LLP 1133.013 §9):
//! the batch and its runner-held cursor, the poll's floor, step tracks, the
//! permission that starts granted, a tool's join, and a canvas input's
//! samples on its grid. The package is the repository's own, installed
//! beside the app as `exact add` would.

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};

const APP: &str = r#"use gravitySensor, accelerometer, motionStatus, requestMotionPermission, startSensorInput, stopSensorInput, GravitySensor, Accelerometer, MotionStatus, SensorInput from "@exact/motion"

component App
  state reading = true
  state rate = 200
  resource status = motionStatus() as shape MotionStatus
  resource level = gravitySensor(frequency=60, interval=100, history=64) as shape GravitySensor
  resource peek = gravitySensor(frequency=10, interval=rate) as shape GravitySensor
  mutation asked as shape MotionStatus
  mutation input as shape SensorInput
  action ask
    send asked = requestMotionPermission()
  action tilt
    send input = startSensorInput(surface="world", sensor="gravity", frequency=60)
  action untilt
    send input = stopSensorInput(surface="world", sensor="gravity")
  action retune
    rate = 300
  view
    column
      text status.state
      text `${level.sequence}`
      text `${peek.sequence}`
      canvas surface=world()
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, contract: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-motion-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/motion");
        let to = path.join("node_modules/@exact/motion");
        for sub in ["", "web", "apple", "android"] {
            std::fs::create_dir_all(to.join(sub)).unwrap();
            std::fs::copy(
                from.join(sub).join("exact.json"),
                to.join(sub).join("exact.json"),
            )
            .unwrap();
        }
        std::fs::copy(from.join("package.json"), to.join("package.json")).unwrap();
        std::fs::write(path.join("app.contract"), contract).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The app's data grants motion.
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
        "device.motion purpose.motion\n"
    }
}

fn boot(app: &App) -> Runner<Granted> {
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    Runner::boot(
        plan,
        Granted,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// Every request dispatched (a held one parked), released work run, each
/// outcome fulfilled, until nothing is left.
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
            let outcome = match dispatch {
                Dispatch::Run(Work::Now(work)) => work(),
                _ => continue,
            };
            let _ = r.fulfill(out.ticket, outcome);
        }
        for (token, dispatch) in released {
            let ticket = parked.iter().find(|(t, _)| *t == token).expect("parked").1;
            if let Dispatch::Run(Work::Now(work)) = dispatch {
                let _ = r.fulfill(ticket, work());
            }
        }
    }
}

fn logs(r: &Runner<Granted>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

/// Resource `name`'s answer as JSON, as the agent's `state` shows it.
fn answer(r: &Runner<Granted>, name: &str) -> Json {
    let state = exact_runner::agent::handle(r, r#"{"op":"state"}"#);
    let state: Json = serde_json::from_str(&state).unwrap();
    let v = state["resources"][name].clone();
    assert!(!v.is_null(), "{name}: no answer\n{}", logs(r));
    v
}

/// A data-code call of `body`, its answer.
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
    serde_json::from_slice(&body).unwrap()
}

fn prefer(r: &mut Runner<Granted>, request: Json) {
    let (_, reply) = r.extension_agent(&request.to_string()).expect("a prefer");
    reply.unwrap_or_else(|e| panic!("{e}"));
}

/// D5, D7, §9: a reader's first answer is the latest reading and no batch;
/// each poll's answer carries every sample on its grid since the last one
/// delivered, at most `history`, with what the bound left out counted; the
/// cursor is the runner's, so a data-code reader's own grid and cursor are
/// its own; a track's steps hold from their instants; a hidden page's gap
/// reads `interrupted`.
#[test]
fn a_reader_s_batch_is_every_sample_since_its_last_delivered_answer() {
    let app = App::new("batch", APP);
    let mut r = boot(&app);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let first = answer(&r, "level");
    assert_eq!(
        first["z"], 9.80665,
        "the default pose lies face up: {first}"
    );
    assert_eq!(
        (first["sequence"].clone(), first["samples"].clone()),
        (json!(0), json!([]))
    );
    assert_eq!(answer(&r, "status")["state"], "granted", "{}", logs(&r));

    r.advance(100.0).unwrap();
    pump(&mut r, &mut parked);
    let polled = answer(&r, "level");
    assert_eq!(polled["sequence"], 6, "{polled}\n{}", logs(&r));
    assert_eq!(
        polled["samples"].as_array().unwrap().len(),
        6,
        "{polled} {}",
        r.extensions_json().unwrap()
    );
    assert_eq!(polled["dropped"], 0);

    // A jump of a second crosses ten periods and asks once: sixty samples,
    // the grid's points in the half-open span.
    r.advance(1100.0).unwrap();
    pump(&mut r, &mut parked);
    let jumped = answer(&r, "level");
    let samples = jumped["samples"].as_array().unwrap();
    assert_eq!(
        (jumped["sequence"].clone(), samples.len()),
        (json!(66), 60),
        "{jumped}"
    );
    assert!((samples[0]["timestamp"].as_f64().unwrap() - 7.0 * 1000.0 / 60.0).abs() < 1e-6);
    assert_eq!(samples[59]["timestamp"], jumped["timestamp"]);
    assert_eq!(
        logs(&r).matches("poll motion.gravitySensor").count(),
        2,
        "{}",
        logs(&r)
    );

    // Past the bound: the newest 64, the rest counted.
    r.advance(3100.0).unwrap();
    pump(&mut r, &mut parked);
    let behind = answer(&r, "level");
    assert_eq!(behind["samples"].as_array().unwrap().len(), 64);
    assert_eq!(behind["dropped"], 120 - 64, "{behind}");

    // Steps, each holding from its instant after the moment it is preferred.
    prefer(
        &mut r,
        json!({ "op": "prefer", "extension": { "motion.gravitySensor":
            [{ "at": 0, "x": 0, "y": 4.9, "z": 8.49 }, { "at": 500, "x": 3.0, "y": 4.9, "z": 7.9 }] } }),
    );
    r.advance(4100.0).unwrap();
    pump(&mut r, &mut parked);
    let stepped = answer(&r, "level");
    let xs: Vec<f64> = (stepped["samples"].as_array().unwrap().iter())
        .map(|s| s["x"].as_f64().unwrap())
        .collect();
    assert_eq!(stepped["x"], 3.0, "{stepped}");
    // Preferred at 3100 ms: the second step holds from 3600 ms, grid point 216.
    let first = xs.iter().position(|x| *x == 3.0);
    assert_eq!((xs.len(), first), (60, Some(29)), "{xs:?}");

    // Data code's reader has a grid and cursor of its own.
    let call = json!({ "extension": "motion", "source": "gravitySensor", "args": { "frequency": 10, "history": 100 } });
    let d1 = data_call(&mut r, call.clone());
    assert_eq!(d1["samples"], json!([]), "{d1}");
    r.advance(5100.0).unwrap();
    pump(&mut r, &mut parked);
    let d2 = data_call(&mut r, call.clone());
    assert_eq!(d2["samples"].as_array().unwrap().len(), 10, "{d2}");

    // The page hidden: no samples meanwhile, and the next answer says so.
    let page = r.page();
    r.set_page(exact_runner::Page {
        hidden: true,
        ..page
    })
    .unwrap();
    r.advance(5600.0).unwrap();
    r.set_page(page).unwrap();
    r.advance(6100.0).unwrap();
    pump(&mut r, &mut parked);
    let d3 = data_call(&mut r, call);
    // Hidden over [5100, 5600): the points from 5600 to 6100.
    let samples = d3["samples"].as_array().unwrap();
    assert_eq!(
        (d3["interrupted"].clone(), samples.len()),
        (json!(true), 6),
        "{d3}"
    );
    assert_eq!(samples[0]["timestamp"], 5600.0);
}

/// D11, §9: the permission starts granted, as iOS and Android grant without
/// asking; under `prompt` a reader fails naming the request, and the request
/// held under a ticket sets it; a tool's reader joins a stream only while
/// the app runs one; a literal poll below the floor is refused at bake.
#[test]
fn a_prompt_holds_the_request_and_a_tool_only_joins() {
    let app = App::new("prompt", APP);
    let mut r = boot(&app);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    prefer(
        &mut r,
        json!({ "op": "prefer", "permission": { "motion": "prompt" } }),
    );
    pump(&mut r, &mut parked);
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let hold = r.device_holds()[0].ticket;
    let tap = format!(r#"{{"op":"tap","ticket":{hold},"choice":"allow"}}"#);
    exact_runner::agent::answer(&mut r, &tap).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "status")["state"], "granted", "{}", logs(&r));

    // The app's reader runs the stream, so a tool's read joins it; a tool
    // starts no canvas input.
    r.act_from(CallOrigin::Tool, "retune", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "peek")["z"], 9.80665, "{}", logs(&r));
    r.act_from(CallOrigin::Tool, "tilt", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(!r.surface_sensor_inputs());
    assert!(
        logs(&r).contains("person-required (tool origin)"),
        "{}",
        logs(&r)
    );

    let low = APP.replace("interval=100", "interval=20");
    let app = App::new("floor", &low);
    let Err(refused) = contract::compile_path_all(&app.0.join("app.contract"), false) else {
        panic!("a poll below the floor is refused at bake");
    };
    assert!(
        format!("{refused:?}").contains("contract-module-bound"),
        "{refused:?}"
    );
}

/// D6, §9: a canvas input's samples, on its grid of the session clock, as
/// the surface's input events; its end delivered once.
#[test]
fn a_canvas_input_takes_every_sample_on_its_grid() {
    let app = App::new("input", APP);
    let mut r = boot(&app);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("tilt", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.surface_sensor_inputs());
    let first = r.surface_sensor_events(0.0);
    assert_eq!(first.len(), 1, "the start's own sample: {first:?}");
    r.advance(1000.0).unwrap();
    let events = r.surface_sensor_events(1000.0);
    assert_eq!(events.len(), 60, "{events:?}");
    let last: Json = serde_json::from_str(&events[59].1).unwrap();
    assert_eq!(last["values"], json!([0.0, 0.0, 9.80665, 0.0]));
    assert_eq!(
        (events[59].0.as_str(), last["t"].as_str()),
        ("world", Some("sensor"))
    );
    r.act("untilt", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let ended = r.surface_sensor_events(1000.0);
    assert_eq!(ended.len(), 1);
    assert!(ended[0].1.contains("sensorEnded"), "{ended:?}");
    assert!(!r.surface_sensor_inputs());
}

/// D5: each reader's cursor is its own, whichever source it reads; letting
/// go of the readers no resource holds keeps every other source's.
#[test]
fn each_source_s_readers_keep_their_cursors() {
    let src = APP.replace(
        "  mutation asked",
        "  resource shake = accelerometer(frequency=60, interval=100, history=64) as shape Accelerometer\n  mutation asked",
    );
    let app = App::new("cursors", &src);
    let mut r = boot(&app);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.advance(750.0).unwrap();
    pump(&mut r, &mut parked);
    for name in ["level", "shake"] {
        let batch = answer(&r, name);
        assert_eq!(
            (
                batch["sequence"].clone(),
                batch["samples"].as_array().map(Vec::len)
            ),
            (json!(45), Some(45)),
            "{name}: {batch}\n{}",
            r.extensions_json().unwrap()
        );
    }
}

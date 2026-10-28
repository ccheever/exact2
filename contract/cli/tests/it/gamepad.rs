//! `@exact/gamepad` under its declared substitute (LLP 1133.022 §9, D10
//! items 2, 3 and 6). The interface is the package's own `exact.json`:
//!
//! - a preferred pad is filled in by its family's labels, and the pads that
//!   come and go raise `gamepadconnected` and `gamepaddisconnected`;
//! - `playEffect` answers `complete` after its span on the agent clock, and
//!   `preempted` when another reaches the same pad first; a send without
//!   `index` is for the current pad; no pad, or an effect the pad lacks,
//!   fails;
//! - `setLight`'s colour is a CSS colour, refused at bake when a literal is
//!   not one and failing `TypeError` when a computed one is not;
//! - a tool's effect fails `person-required`, and so does one a world's
//!   message carries a tool's origin back into (D6).

use exact_kernel::PropValue;
use exact_runner::{CallOrigin, Dispatch, Event, Outcome, Reply, Runner, Work};
use std::path::PathBuf;

const APP: &str = r##"use getGamepads, playEffect, resetEffects, setLight, gamepadconnected, gamepaddisconnected, GamepadList, Gamepad, GamepadHapticsResult from "@exact/gamepad"

fn west(p: Gamepad): string = match p.labels.at(2) { case some(l) => l, case none => "" }

component App
  state came = ""
  state went = ""
  state shade = "#e8ad61"
  resource pads = getGamepads() as shape GamepadList
  mutation buzz as shape GamepadHapticsResult
  mutation other as shape GamepadHapticsResult
  mutation stop as shape GamepadHapticsResult
  action connected(p: Gamepad)
    came = `${came}${p.index}:${p.family}:${west(p)} `
  action disconnected(p: Gamepad)
    went = `${went}${p.index} `
  action rumble(ms: number)
    send buzz = playEffect(type="dual-rumble", duration=ms, strongMagnitude=0.6, weakMagnitude=0.3)
  action rumbleOn(i: number)
    send other = playEffect(index=i, type="dual-rumble", duration=500)
  action triggers
    send other = playEffect(type="trigger-rumble", duration=100)
  action reset
    send stop = resetEffects()
  action amber
    setLight(color="#e8ad61")
  action shaded
    setLight(color=shade)
  action giveBack
    setLight()
  action spoil
    shade = "not a colour"
  action heard(text: string)
    send buzz = playEffect(type="dual-rumble", duration=50)
  view
    column gamepadconnected=connected gamepaddisconnected=disconnected
      text testId="came" came
      text testId="went" went
      match buzz
        case some(x)
          text x.result testId="buzz"
        case none
          text "" testId="buzz"
      match failure(buzz)
        case some(f)
          text f.code testId="buzz-error"
        case none
          text "" testId="buzz-error"
      match other
        case some(x)
          text x.result testId="other"
        case none
          text "" testId="other"
      match failure(other)
        case some(f)
          text f.code testId="other-error"
        case none
          text "" testId="other-error"
      match stop
        case some(x)
          text x.result testId="stop"
        case none
          text "" testId="stop"
      match failure(stop)
        case some(f)
          text f.code testId="stop-error"
        case none
          text "" testId="stop-error"
      iframe src="/frame.html" sandbox="allow-scripts" testId="frame" message=heard
"##;

struct App(PathBuf);
impl App {
    fn new(name: &str, contract: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-gamepad-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@exact/gamepad");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/gamepad");
        std::fs::create_dir_all(&package).unwrap();
        for file in ["package.json", "exact.json"] {
            std::fs::copy(source.join(file), package.join(file)).unwrap();
        }
        for platform in ["apple", "android", "web", "linux", "windows"] {
            std::fs::create_dir_all(package.join(platform)).unwrap();
            std::fs::copy(
                source.join(platform).join("exact.json"),
                package.join(platform).join("exact.json"),
            )
            .unwrap();
        }
        std::fs::write(path.join("app.contract"), contract).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn compile(&self) -> Result<exact_plan::Plan, Vec<contract::CompileError>> {
        contract::compile_path_all(&self.0.join("app.contract"), false).map(|(plan, _)| plan)
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

fn boot(name: &str) -> (App, Runner<Data>) {
    let app = App::new(name, APP);
    let plan = app.compile().unwrap_or_else(|e| panic!("{e:?}"));
    let r = Runner::boot(
        plan,
        Data,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    (app, r)
}

/// The host's part: each request dispatched, released work run, each
/// outcome fulfilled and each event delivered, until nothing is left.
fn pump(r: &mut Runner<Data>, parked: &mut Vec<(u64, u64)>) {
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

fn run(r: &mut Runner<Data>, ticket: u64, dispatch: Dispatch) {
    let outcome: Outcome = match dispatch {
        Dispatch::Run(Work::Now(work)) => work(),
        Dispatch::Run(Work::Later(work)) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            match rx.try_recv() {
                Ok(o) => o,
                // Held: its continuation answers when the runner releases it.
                Err(_) => return,
            }
        }
        _ => return,
    };
    let _ = r.fulfill(ticket, outcome);
}

fn prefer(r: &mut Runner<Data>, pads: &str) {
    let request = format!(r#"{{"op":"prefer","extension":{{"gamepad.getGamepads":{pads}}}}}"#);
    let (_, reply) = r.extension_agent(&request).expect("a prefer");
    reply.unwrap();
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

/// A send's last answer, or its failure's code.
fn slot(r: &Runner<Data>, name: &str) -> String {
    format!("{}{}", text(r, name), text(r, &format!("{name}-error")))
}

fn logs(r: &Runner<Data>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

fn extension(r: &Runner<Data>) -> serde_json::Value {
    let state: serde_json::Value =
        serde_json::from_str(&exact_runner::agent::handle(r, r#"{"op":"state"}"#)).unwrap();
    state["extensions"]["gamepad"].clone()
}

#[test]
fn a_preferred_pad_is_filled_by_its_family_and_its_coming_and_going_are_events() {
    let (_app, mut r) = boot("list");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    prefer(
        &mut r,
        r#"{"gamepads":[{"index":0,"family":"playstation"},{"index":1}],"current":0}"#,
    );
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "came"), "0:playstation:Square 1:generic:X ");
    let pads = &extension(&r)["preferred"]["getGamepads"];
    assert_eq!(pads["gamepads"][0]["index"], 0, "{pads}");
    prefer(&mut r, r#"{"gamepads":[{"index":1}],"current":1}"#);
    pump(&mut r, &mut parked);
    assert_eq!(text(&r, "went"), "0 ");
    assert_eq!(
        text(&r, "came"),
        "0:playstation:Square 1:generic:X ",
        "no event for a pad that stayed"
    );
}

#[test]
fn an_effect_answers_after_its_span_and_is_preempted_on_its_pad() {
    let (_app, mut r) = boot("effects");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    // No pad: the current pad's send fails.
    r.act("rumble", vec![exact_runner::Value::Number(180.0)])
        .unwrap();
    pump(&mut r, &mut parked);
    assert!(
        slot(&r, "buzz").contains("NotFoundError"),
        "{}",
        slot(&r, "buzz")
    );
    prefer(
        &mut r,
        r#"{"gamepads":[{"index":0,"family":"xbox"},{"index":1}],"current":0}"#,
    );
    pump(&mut r, &mut parked);
    // The current pad's effect, complete after 180 ms of clock.
    r.act("rumble", vec![exact_runner::Value::Number(180.0)])
        .unwrap();
    pump(&mut r, &mut parked);
    let played = &extension(&r)["effects"];
    assert_eq!(played[0]["index"], 0, "{played}");
    assert_eq!(played[0]["origin"], "app", "{played}");
    assert!(played[0]["result"].is_null(), "{played}");
    r.advance(100.0).unwrap();
    pump(&mut r, &mut parked);
    assert!(
        !slot(&r, "buzz").contains("complete"),
        "{}",
        slot(&r, "buzz")
    );
    r.advance(200.0).unwrap();
    pump(&mut r, &mut parked);
    assert!(
        slot(&r, "buzz").contains("complete"),
        "{}",
        slot(&r, "buzz")
    );
    assert_eq!(extension(&r)["effects"][0]["result"], "complete");
    // A second effect on pad 1 is not preempted by one on pad 0; a reset of
    // the current pad preempts pad 0's.
    r.act("rumbleOn", vec![exact_runner::Value::Number(1.0)])
        .unwrap();
    r.act("rumble", vec![exact_runner::Value::Number(1000.0)])
        .unwrap();
    pump(&mut r, &mut parked);
    r.act("reset", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(
        slot(&r, "buzz").contains("preempted"),
        "{}",
        slot(&r, "buzz")
    );
    assert!(
        slot(&r, "stop").contains("complete"),
        "{}",
        slot(&r, "stop")
    );
    assert!(
        !slot(&r, "other").contains("preempted"),
        "{}",
        slot(&r, "other")
    );
    r.advance(900.0).unwrap();
    pump(&mut r, &mut parked);
    assert!(
        slot(&r, "other").contains("complete"),
        "{}",
        slot(&r, "other")
    );
    // An effect the pad lacks.
    r.act("triggers", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(
        slot(&r, "other").contains("NotSupportedError"),
        "{}",
        slot(&r, "other")
    );
    // A disconnect: no current pad, and a send for it fails.
    prefer(&mut r, r#"{"gamepads":[],"current":null}"#);
    pump(&mut r, &mut parked);
    r.act("rumble", vec![exact_runner::Value::Number(10.0)])
        .unwrap();
    pump(&mut r, &mut parked);
    assert!(slot(&r, "buzz").contains("NotFoundError"), "{}", logs(&r));
}

#[test]
fn a_light_takes_a_css_colour_and_is_kept_per_pad() {
    let (_app, mut r) = boot("light");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    prefer(
        &mut r,
        r#"{"gamepads":[{"index":0,"family":"playstation"}],"current":0}"#,
    );
    pump(&mut r, &mut parked);
    r.act("amber", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let lights = &extension(&r)["lights"];
    assert_eq!(lights[0]["color"], "#e8ad61", "{lights}");
    r.act("giveBack", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let lights = &extension(&r)["lights"];
    assert_eq!(lights.as_array().map(Vec::len), Some(1), "{lights}");
    assert!(lights[0]["color"].is_null(), "given back: {lights}");
    // A computed colour that is none fails `TypeError`, journaled.
    r.act("spoil", Vec::new()).unwrap();
    r.act("shaded", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(logs(&r).contains("is no CSS colour"), "{}", logs(&r));
    // A literal that is none is refused at bake.
    let bad = App::new(
        "bad-colour",
        &APP.replace("color=\"#e8ad61\"", "color=\"#nope\""),
    );
    let errors = bad.compile().map(|_| ()).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("#nope")),
        "{errors:?}"
    );
}

#[test]
fn a_tool_or_a_world_it_reached_cannot_start_the_motors() {
    let (_app, mut r) = boot("origin");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    prefer(&mut r, r#"{"gamepads":[{"index":0}],"current":0}"#);
    pump(&mut r, &mut parked);
    r.act_from(
        CallOrigin::Tool,
        "rumble",
        vec![exact_runner::Value::Number(10.0)],
    )
    .unwrap();
    pump(&mut r, &mut parked);
    assert!(
        slot(&r, "buzz").contains("NotAllowedError"),
        "{}",
        slot(&r, "buzz")
    );
    assert!(logs(&r).contains("person-required"), "{}", logs(&r));
    // A message a world emitted under a tool's mark runs its handler as the tool.
    let key = r.kernel().find_by_test_id("frame")[0];
    let frame = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(frame, Event::Message("\u{1}origin:tool\u{1}lit".into()))
        .unwrap();
    pump(&mut r, &mut parked);
    assert!(
        slot(&r, "buzz").contains("NotAllowedError"),
        "{}",
        slot(&r, "buzz")
    );
    // An unmarked one is the app's.
    r.dispatch(frame, Event::Message("lit".into())).unwrap();
    pump(&mut r, &mut parked);
    r.advance(100.0).unwrap();
    pump(&mut r, &mut parked);
    assert!(slot(&r, "buzz").contains("complete"), "{}", logs(&r));
}

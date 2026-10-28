//! LLP 1133.017 stage 0, through a module outside the tree (`tests/tide`,
//! copied out of the checkout as an installed package is): an event of its
//! own that a wake raises, with its budget; registrations the declared
//! substitute lists, a one-shot and a periodic one; a warm wake (`tap @ref`),
//! a cold one (`--wake`), and one that expires. A wake's invocation owns the
//! request its handler makes, stops the session's timers while unseen, and
//! refuses the host commands D6 names.

use exact_runner::{agent, CallOrigin, Runner, Value, WakeHow};
use std::path::{Path, PathBuf};

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-tide-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tide");
        copy(&fixture, &path.join("node_modules/@fixture/tide"), &["app"]);
        copy(&fixture.join("app"), &path, &[]);
        Self(path.canonicalize().unwrap())
    }
    fn compile(&self) -> Result<exact_plan::Plan, Vec<contract::CompileError>> {
        contract::compile_path_all(&self.0.join("app.contract"), false).map(|(plan, _)| plan)
    }
    fn write(&self, file: &str, text: &str) {
        std::fs::write(self.0.join(file), text).unwrap();
    }
    fn edit_interface(&self, from: &str, to: &str) {
        let path = self.0.join("node_modules/@fixture/tide/exact.json");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{from}");
        std::fs::write(&path, text.replace(from, to)).unwrap();
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy(from: &Path, to: &Path, skip: &[&str]) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if skip.iter().any(|s| name == *s) {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(&name), &[]);
        } else {
            std::fs::copy(entry.path(), to.join(&name)).unwrap();
        }
    }
}

/// `pull` answers later, as a request to a server would: the wake's owned
/// work until the test fulfills it.
struct Later;
impl exact_runner::DataSource for Later {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        Ok(exact_runner::Answer::Later(exact_runner::Request::native(
            b"{}".to_vec(),
        )))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        outcome: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        match outcome {
            exact_runner::Outcome::Response(_) => {
                Ok(exact_runner::Answer::Now(Value::record(vec![
                    Value::Number(7.0),
                ])))
            }
            other => Err(exact_runner::DataError::Unavailable(format!("{other:?}"))),
        }
    }
    fn grants(&self) -> &str {
        ""
    }
}

fn boot(app: &App) -> Runner<Later> {
    Runner::boot(
        app.compile().unwrap_or_else(|e| panic!("{e:?}")),
        Later,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// Move the session's clock on by `ms` (`advance` takes the time itself).
fn by(r: &mut Runner<Later>, ms: f64) {
    let at = r.now_ms() + ms;
    r.advance(at).unwrap();
}

fn logs(r: &Runner<Later>) -> String {
    agent::handle(r, r#"{"op":"logs"}"#)
}

fn state(r: &Runner<Later>) -> serde_json::Value {
    serde_json::from_str(&agent::handle(r, r#"{"op":"state"}"#)).unwrap()
}

fn slot(r: &Runner<Later>, name: &str) -> String {
    format!("{:?}", r.slot(name))
}

fn tap(r: &mut Runner<Later>, reference: u64) -> Result<String, String> {
    let request = format!(r#"{{"op":"tap","ticket":{reference}}}"#);
    let reply = agent::answer(r, &request).expect("an agent op").0;
    let parsed: serde_json::Value = serde_json::from_str(&reply).unwrap_or_default();
    match parsed["error"].as_str() {
        Some(error) => Err(error.to_owned()),
        None => Ok(reply),
    }
}

/// The one entry of `state.tides` of `kind`.
fn entry(r: &Runner<Later>, kind: &str) -> serde_json::Value {
    let tides = state(r)["tides"].clone();
    (tides.as_array().into_iter().flatten())
        .find(|e| e["kind"] == kind)
        .cloned()
        .unwrap_or_else(|| panic!("no {kind} entry: {tides} {}", logs(r)))
}

/// The pull in flight, answered.
fn answer_pull(r: &mut Runner<Later>) {
    let out = r.take_requests();
    assert_eq!(out.len(), 1, "one pull in flight: {}", logs(r));
    let ok = exact_runner::Outcome::Response(exact_runner::Response {
        status: 200,
        headers: Vec::new(),
        body: b"7".to_vec(),
    });
    r.fulfill(out[0].ticket, ok).unwrap();
}

#[test]
fn a_wake_event_carries_its_budget_and_the_bake_holds_its_bounds() {
    let app = App::new("compile");
    let plan = app.compile().unwrap_or_else(|e| panic!("{e:?}"));
    let events: Vec<String> = (plan.module_events.iter())
        .map(|e| {
            format!(
                "{}.{} {} {} {} {}",
                plan.str(e.extension),
                plan.str(e.declared),
                plan.str(plan.action(e.action).name),
                plan.str(e.raised),
                e.hold,
                e.budget
            )
        })
        .collect();
    assert_eq!(events, ["tide.tide ebbed wake 2 5000"]);
    // An interval below the declared floor, written literally.
    let low = App::new("low");
    let text = std::fs::read_to_string(low.0.join("app.contract")).unwrap();
    low.write("app.contract", &text.replace("every=60000", "every=1000"));
    let refused = low.compile().unwrap_err();
    assert!(
        refused.iter().any(|e| e.id == "contract-module-bound"),
        "{refused:?}"
    );
    // A wake event names its budget; no other event has one.
    let unbudgeted = App::new("unbudgeted");
    unbudgeted.edit_interface(r#", "budget": 5000"#, "");
    let refused = format!("{:?}", unbudgeted.compile().unwrap_err());
    assert!(refused.contains("declares its `budget`"), "{refused}");
    let emitted = App::new("emitted");
    emitted.edit_interface(r#""raised": "wake""#, r#""raised": "emit""#);
    let refused = format!("{:?}", emitted.compile().unwrap_err());
    assert!(refused.contains("only an event a wake raises"), "{refused}");
}

/// §10's warm wake on a periodic registration: refused until due; then the
/// handler runs as the wake's, its request owned until it lands, the
/// session's page hidden and timers stopped meanwhile and its `copyText`
/// refused; the entry stays, with its run.
#[test]
fn a_tap_on_a_periodic_registration_is_a_warm_wake() {
    let app = App::new("warm");
    let mut r = boot(&app);
    r.act("periodic", Vec::new()).unwrap();
    let e = entry(&r, "every");
    assert_eq!(e["tag"], "e1");
    assert_eq!(e["every"], 60000.0);
    assert_eq!(e["lastRun"], serde_json::Value::Null);
    let reference = e["ref"].as_u64().unwrap();
    // The listed source answers the registration, asked again as it changed.
    r.apply_announced();
    let every = format!("{:?}", r.resource("every"));
    assert!(every.contains("e1") && every.contains("60000"), "{every}");
    let early = tap(&mut r, reference).unwrap_err();
    assert!(early.contains("is not due until"), "{early}");
    by(&mut r, 60_000.0);
    let ticks = slot(&r, "ticks");
    let reply = tap(&mut r, reference).unwrap();
    assert!(reply.contains(r#""as":"wake""#), "{reply}");
    by(&mut r, 0.0);
    assert!(slot(&r, "heard").contains("e1"), "{}", logs(&r));
    let wake = state(&r)["wake"].clone();
    assert_eq!(wake["invocations"][0]["state"], "running", "{wake}");
    assert_eq!(wake["cold"], false);
    // The page reads hidden, and timers stop, while the wake runs unseen.
    assert!(r.page().hidden);
    by(&mut r, 3_000.0);
    assert_eq!(slot(&r, "ticks"), ticks, "{}", logs(&r));
    answer_pull(&mut r);
    by(&mut r, 0.0);
    let journal = logs(&r);
    assert!(
        journal.contains("wake tide e1: heard by ebbed"),
        "{journal}"
    );
    assert!(
        journal.contains("copyText: refused: during a wake"),
        "{journal}"
    );
    assert!(journal.contains("wake tide e1: completed"), "{journal}");
    assert!(journal.contains("wake: completed"), "{journal}");
    assert!(slot(&r, "pulled").contains('7'), "{}", slot(&r, "pulled"));
    assert_eq!(state(&r)["wake"]["outcome"], "completed");
    assert!(!r.page().hidden, "the page the wake hid is shown again");
    let after = entry(&r, "every");
    assert!(after["lastRun"].is_number(), "{after}");
    r.apply_announced();
    let every = format!("{:?}", r.resource("every"));
    assert!(
        !every.contains("Null"),
        "its run is in the listed source: {every}"
    );
    assert!(after["due"].as_f64() > e["due"].as_f64(), "{after} {e}");
    // The timers run again.
    by(&mut r, 1_000.0);
    assert_ne!(slot(&r, "ticks"), ticks, "{}", logs(&r));
}

/// D1's one-shot: refused `offline` while the page is, retired when its
/// invocation completes.
#[test]
fn a_one_shot_waits_for_the_network_and_is_retired_when_it_completes() {
    let app = App::new("once");
    let mut r = boot(&app);
    r.act("once", Vec::new()).unwrap();
    let reference = entry(&r, "once")["ref"].as_u64().unwrap();
    let mut page = r.page();
    page.on_line = false;
    r.set_page(page).unwrap();
    let offline = tap(&mut r, reference).unwrap_err();
    assert!(offline.ends_with("offline"), "{offline}");
    page.on_line = true;
    r.set_page(page).unwrap();
    tap(&mut r, reference).unwrap();
    by(&mut r, 0.0);
    answer_pull(&mut r);
    by(&mut r, 0.0);
    let journal = logs(&r);
    assert!(
        journal.contains(&format!("tides @{reference}: retired")),
        "{journal}"
    );
    assert_eq!(state(&r)["tides"], serde_json::json!([]));
}

/// D6's expiry: past the budget with its request in flight, the invocation
/// ends `expired`, the request answers as a failure, and a one-shot stays
/// listed as an attempt, not due again until its backoff has passed.
#[test]
fn a_wake_past_its_budget_expires() {
    let app = App::new("expire");
    let mut r = boot(&app);
    r.act("once", Vec::new()).unwrap();
    let reference = entry(&r, "once")["ref"].as_u64().unwrap();
    tap(&mut r, reference).unwrap();
    by(&mut r, 0.0);
    assert_eq!(r.take_requests().len(), 1);
    by(&mut r, 5_001.0);
    let journal = logs(&r);
    assert!(journal.contains("wake tide o1: expired"), "{journal}");
    assert!(journal.contains("wake: expired"), "{journal}");
    assert_eq!(state(&r)["wake"]["outcome"], "expired");
    assert!(!slot(&r, "pulled").contains('7'), "{}", slot(&r, "pulled"));
    let e = entry(&r, "once");
    let backoff = tap(&mut r, reference).unwrap_err();
    assert!(backoff.contains("is not due until"), "{backoff} {e}");
}

/// §10's cold wake: `--wake '<entry>'` lists the entry again in a fresh
/// session and raises it there, `cold`.
#[test]
fn a_launch_with_an_entry_is_a_cold_wake() {
    let app = App::new("cold");
    let mut first = boot(&app);
    first.act("periodic", Vec::new()).unwrap();
    let printed = entry(&first, "every").to_string();
    drop(first);
    let mut r = boot(&app);
    let request = serde_json::json!({ "op": "launchEntry", "launch": "wake", "entry": printed });
    let reply = agent::answer(&mut r, &request.to_string()).unwrap().0;
    assert!(reply.contains("tide"), "{reply}");
    by(&mut r, 0.0);
    assert_eq!(state(&r)["wake"]["cold"], true, "{}", logs(&r));
    answer_pull(&mut r);
    by(&mut r, 0.0);
    assert!(logs(&r).contains("wake: completed"), "{}", logs(&r));
    assert_eq!(entry(&r, "every")["tag"], "e1");
}

/// D5: only the host raises a wake's event, at the platform's call; the
/// events it raises for one platform object are one wake, invocations one
/// at a time, and the host takes each ended wake to tell the platform.
#[test]
fn the_host_raises_a_wake_and_takes_its_end() {
    let app = App::new("host");
    let mut r = boot(&app);
    let refused = r
        .raise_module_event("tide", "tide", r#"["x"]"#, CallOrigin::Device)
        .unwrap_err();
    assert!(refused.contains("a wake raises it"), "{refused}");
    let how = WakeHow {
        id: 41,
        cold: false,
        hidden: false,
    };
    r.raise_wake("tide", "tide", r#"["a"]"#, how).unwrap();
    r.raise_wake("tide", "tide", r#"["b"]"#, how).unwrap();
    by(&mut r, 0.0);
    let first = r.take_requests();
    assert_eq!(first.len(), 1, "one invocation at a time");
    let wake = state(&r)["wake"].clone();
    assert_eq!(wake["invocations"][1]["state"], "queued", "{wake}");
    // The first's pull lands; the second is delivered, and its pull lands.
    let ticket = first[0].ticket;
    let ok = || {
        exact_runner::Outcome::Response(exact_runner::Response {
            status: 200,
            headers: Vec::new(),
            body: b"7".to_vec(),
        })
    };
    r.fulfill(ticket, ok()).unwrap();
    by(&mut r, 0.0);
    answer_pull(&mut r);
    by(&mut r, 0.0);
    let ended = r.take_ended_wakes();
    assert_eq!(ended.len(), 1, "{}", logs(&r));
    let ended: serde_json::Value = serde_json::from_str(&ended[0]).unwrap();
    assert_eq!(ended["id"], 41);
    assert_eq!(ended["outcome"], "completed");
    assert_eq!(ended["invocations"].as_array().unwrap().len(), 2);
    assert!(r.take_ended_wakes().is_empty());
    // An event nothing hears still makes a wake, which ends at once.
    r.raise_wake("tide", "nothing", "[]", WakeHow { id: 42, ..how })
        .unwrap();
    by(&mut r, 0.0);
    assert!(
        logs(&r).contains("tide.nothing: no handler hears it"),
        "{}",
        logs(&r)
    );
    assert_eq!(r.take_ended_wakes().len(), 1);
}

/// D1's retry: a handler that registers its own one-shot again keeps it,
/// not due until a backoff of 5 minutes has passed, then 10; after the third
/// attempt in a row it is retired unless registered again.
#[test]
fn a_one_shot_its_handler_registers_again_backs_off() {
    let app = App::new("retry");
    let text = std::fs::read_to_string(app.0.join("app.contract")).unwrap();
    app.write(
        "app.contract",
        &text.replace("    copyText(tag)\n", "    tideOnce(tag=tag)\n"),
    );
    let mut r = boot(&app);
    r.act("once", Vec::new()).unwrap();
    // Registered at clock zero, so its first due time is the epoch there.
    let epoch = entry(&r, "once")["due"].as_f64().unwrap();
    let mut backoffs = Vec::new();
    for attempt in 0..3 {
        let e = entry(&r, "once");
        let reference = e["ref"].as_u64().unwrap();
        let wait = (e["due"].as_f64().unwrap() - (r.now_ms() + epoch)).max(0.0);
        if attempt > 0 {
            assert!(tap(&mut r, reference)
                .unwrap_err()
                .contains("not due until"));
            backoffs.push(wait);
        }
        by(&mut r, wait);
        tap(&mut r, reference).unwrap_or_else(|e| panic!("{e} {}", logs(&r)));
        by(&mut r, 0.0);
        answer_pull(&mut r);
        by(&mut r, 0.0);
    }
    assert_eq!(backoffs, [300_000.0, 600_000.0], "{}", logs(&r));
    // Registered again during its third attempt: kept at the longest backoff.
    let e = entry(&r, "once");
    let wait = e["due"].as_f64().unwrap() - (r.now_ms() + epoch);
    assert_eq!(wait, 3_600_000.0, "{e}");
}

/// A bound implementation the platform ran raises a wake through its
/// module's `changed` (LLP 1133.017 D8) and is told how it ended; one the
/// substitute answers raises nothing.
#[test]
fn an_implementation_raises_a_wake_and_is_told_its_end() {
    use std::sync::{Arc, Mutex};
    let app = App::new("told");
    let mut r = boot(&app);
    r.listen(Arc::new(|| {}));
    let told = |r: &Runner<Later>, id: &str| {
        let body =
            serde_json::json!({ "extension": "tide", "event": "tide", "args": [id], "id": id });
        r.native_slot().changed(&format!("exact-wake:{body}"));
    };
    told(&r, "w0");
    r.apply_announced();
    assert!(
        logs(&r).contains("a wake raised, but the substitute answers"),
        "{}",
        logs(&r)
    );
    let heard: Arc<Mutex<Vec<serde_json::Value>>> = Default::default();
    let seen = heard.clone();
    r.native_slot().host(Some(Arc::new(
        move |body: Vec<u8>, reply: exact_runner::Reply| {
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap());
            reply.send(exact_runner::Outcome::Response(exact_runner::Response {
                status: 200,
                headers: Vec::new(),
                body: b"{}".to_vec(),
            }));
        },
    )));
    r.bind_extensions(&["tide"], &[]);
    let woke = |heard: &Arc<Mutex<Vec<serde_json::Value>>>| {
        (heard.lock().unwrap().iter())
            .filter(|v| v.get("woke").is_some())
            .map(|v| format!("{} {}", v["woke"], v["outcome"]))
            .collect::<Vec<_>>()
    };
    told(&r, "w1");
    r.apply_announced();
    assert!(slot(&r, "heard").contains("w1"), "{}", logs(&r));
    assert_eq!(state(&r)["wake"]["invocations"][0]["state"], "running");
    assert!(woke(&heard).is_empty());
    // Bound, the listed source asks the implementation too; each answered.
    for out in r.take_requests() {
        let ok = exact_runner::Outcome::Response(exact_runner::Response {
            status: 200,
            headers: Vec::new(),
            body: b"7".to_vec(),
        });
        r.fulfill(out.ticket, ok).unwrap();
    }
    by(&mut r, 0.0);
    assert_eq!(woke(&heard), [r#""w1" "completed""#], "{}", logs(&r));
    // An idle live session gives a new wake its whole budget.
    let clock = std::rc::Rc::new(std::cell::Cell::new(60_000.0));
    let sample = clock.clone();
    r.set_live_clock(Some(Box::new(move || sample.get())));
    told(&r, "w2");
    r.apply_announced();
    by(&mut r, 0.0);
    assert_eq!(state(&r)["wake"]["invocations"][0]["state"], "running");
    assert_eq!(woke(&heard).len(), 1, "{}", logs(&r));
    // Its budget passes with the request still out: expired.
    clock.set(66_000.0);
    by(&mut r, 6_000.0);
    assert_eq!(woke(&heard)[1], r#""w2" "expired""#, "{}", logs(&r));
    // An event no handler hears is still a wake, ended at once.
    let body =
        serde_json::json!({ "extension": "tide", "event": "nothing", "args": [], "id": "w3" });
    r.native_slot().changed(&format!("exact-wake:{body}"));
    r.apply_announced();
    by(&mut r, 0.0);
    assert_eq!(woke(&heard)[2], r#""w3" "unheard""#, "{}", logs(&r));
}

/// The platform's time runs out before the wake's own budget (a `BGTask`'s
/// expiration handler): the implementation says so, the running wake
/// expires at once and its request fails, and a wake still waiting behind
/// it ends `expired` unrun.
#[test]
fn an_implementation_expires_the_wake_it_raised() {
    use std::sync::{Arc, Mutex};
    let app = App::new("told-expired");
    let mut r = boot(&app);
    r.listen(Arc::new(|| {}));
    let heard: Arc<Mutex<Vec<serde_json::Value>>> = Default::default();
    let seen = heard.clone();
    r.native_slot().host(Some(Arc::new(
        move |body: Vec<u8>, reply: exact_runner::Reply| {
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap());
            reply.send(exact_runner::Outcome::Response(exact_runner::Response {
                status: 200,
                headers: Vec::new(),
                body: b"{}".to_vec(),
            }));
        },
    )));
    r.bind_extensions(&["tide"], &[]);
    let say = |r: &Runner<Later>, topic: &str, id: &str| {
        let body =
            serde_json::json!({ "extension": "tide", "event": "tide", "args": [id], "id": id });
        r.native_slot().changed(&format!("{topic}:{body}"));
    };
    let woke = |heard: &Arc<Mutex<Vec<serde_json::Value>>>| {
        (heard.lock().unwrap().iter())
            .filter(|v| v.get("woke").is_some())
            .map(|v| format!("{} {}", v["woke"], v["outcome"]))
            .collect::<Vec<_>>()
    };
    say(&r, "exact-wake", "w1");
    say(&r, "exact-wake", "w2");
    r.apply_announced();
    by(&mut r, 0.0);
    assert_eq!(
        state(&r)["wake"]["invocations"][0]["state"],
        "running",
        "{}",
        logs(&r)
    );
    say(&r, "exact-wake-expired", "w2");
    r.apply_announced();
    assert_eq!(woke(&heard), [r#""w2" "expired""#], "{}", logs(&r));
    assert_eq!(state(&r)["wake"]["invocations"][0]["state"], "running");
    say(&r, "exact-wake-expired", "w1");
    r.apply_announced();
    by(&mut r, 0.0);
    assert_eq!(
        woke(&heard),
        [r#""w2" "expired""#, r#""w1" "expired""#],
        "{}",
        logs(&r)
    );
    assert_eq!(state(&r)["wake"]["outcome"], "expired", "{}", logs(&r));
    assert!(
        logs(&r).contains("the platform expired wake w1"),
        "{}",
        logs(&r)
    );
    // Said again after the wake ended, it changes nothing.
    say(&r, "exact-wake-expired", "w1");
    r.apply_announced();
    assert_eq!(woke(&heard).len(), 2);
}

/// LLP 1133.023 D7: a tool call is an invocation as a wake's event is, one
/// at a time with the wakes: a call that arrives while a wake runs waits
/// for it, and runs once the wake has ended.
#[test]
fn a_call_waits_for_a_running_wake() {
    let app = App::new("call");
    let text = std::fs::read_to_string(app.0.join("app.contract")).unwrap();
    let tool = "  tool poke writes closed \"Pull once.\" = poke\n  action poke\n    send pulled = pull(\"call\")\n  action once\n";
    app.write("app.contract", &text.replacen("  action once\n", tool, 1));
    let mut r = boot(&app);
    r.act("periodic", Vec::new()).unwrap();
    by(&mut r, 60_000.0);
    let reference = entry(&r, "every")["ref"].as_u64().unwrap();
    tap(&mut r, reference).unwrap();
    by(&mut r, 0.0);
    assert_eq!(state(&r)["wake"]["invocations"][0]["state"], "running");
    let how = exact_runner::CallHow {
        door: "agent".into(),
        ..Default::default()
    };
    r.call_tool("poke", "{}", how).unwrap();
    by(&mut r, 0.0);
    let calls = state(&r)["intents"]["calls"].clone();
    assert_eq!(calls[0]["state"], "waiting", "{calls}");
    answer_pull(&mut r);
    by(&mut r, 0.0);
    assert_eq!(state(&r)["wake"]["outcome"], "completed", "{}", logs(&r));
    let calls = state(&r)["intents"]["calls"].clone();
    assert_eq!(calls[0]["state"], "running", "{calls} {}", logs(&r));
    answer_pull(&mut r);
    by(&mut r, 0.0);
    let calls = state(&r)["intents"]["calls"].clone();
    assert_eq!(calls[0]["outcome"], "ok", "{calls} {}", logs(&r));
    assert_eq!(calls[0]["certainty"], "applied");
}

/// LLP 1133.033 D21: Android's reader under the agent starts no foreground
/// service and says each claim through the claimant's `changed`; the runner
/// journals it and lists the claims held in the claimant's
/// `state.extensions` entry until their release.
#[test]
fn a_foreground_service_claim_is_listed_while_held() {
    let app = App::new("claim");
    let mut r = boot(&app);
    r.listen(std::sync::Arc::new(|| {}));
    let told = |r: &Runner<Later>, claimed: bool, refused: Option<&str>| {
        let mut body = serde_json::json!({ "extension": "tide", "kind": "capture", "types": ["microphone"], "claimed": claimed, "text": "Recording" });
        if let Some(why) = refused {
            body["refused"] = why.into();
        }
        r.native_slot()
            .changed(&format!("exact-foreground-service:{body}"));
    };
    let claims = |r: &Runner<Later>| state(r)["extensions"]["tide"]["foregroundService"].clone();
    told(&r, true, None);
    r.apply_announced();
    let held = claims(&r);
    assert_eq!(held[0]["type"], "microphone", "{held}");
    assert_eq!(
        (held[0]["package"].as_str(), held[0]["text"].as_str()),
        (Some("tide"), Some("Recording"))
    );
    assert!(
        logs(&r).contains("foreground service microphone: claimed"),
        "{}",
        logs(&r)
    );
    told(&r, false, None);
    r.apply_announced();
    assert!(claims(&r).is_null(), "{}", claims(&r));
    assert!(logs(&r).contains("foreground service microphone: released"));
    told(&r, false, Some("ForegroundServiceStartNotAllowedException"));
    r.apply_announced();
    assert!(logs(&r).contains(
        "foreground service microphone: refused: ForegroundServiceStartNotAllowedException"
    ));
    assert!(claims(&r).is_null());
}

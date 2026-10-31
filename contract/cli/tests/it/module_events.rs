//! LLP 1133.004 stage 0, through a module outside the tree (`tests/bell`,
//! copied out of the checkout as an installed package is): a send that
//! answers nothing is a statement; an event is heard on the root and
//! delivered once, in order, as its own commit; an emitted event carries
//! the device's origin, so the request its handler makes while `prompt`
//! fails; and the declared list, value and admission forms answer under
//! the agent, the value app-wide.

use exact_runner::{agent, CallOrigin, Runner, Value};
use std::path::{Path, PathBuf};

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-bell-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/bell");
        copy(&fixture, &path.join("node_modules/@fixture/bell"), &["app"]);
        copy(&fixture.join("app"), &path, &[]);
        Self(path.canonicalize().unwrap())
    }
    fn compile(&self) -> Result<exact_plan::Plan, Vec<contract::CompileError>> {
        contract::compile_path_all(&self.0.join("app.contract"), false).map(|(plan, _)| plan)
    }
    fn write(&self, file: &str, text: &str) {
        std::fs::write(self.0.join(file), text).unwrap();
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

struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        ""
    }
}

fn boot(plan: exact_plan::Plan) -> Runner<NoData> {
    Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn logs(r: &Runner<NoData>) -> String {
    agent::handle(r, r#"{"op":"logs"}"#)
}

fn state(r: &Runner<NoData>) -> serde_json::Value {
    serde_json::from_str(&agent::handle(r, r#"{"op":"state"}"#)).unwrap()
}

fn slot(r: &Runner<NoData>, name: &str) -> String {
    format!("{:?}", r.slot(name))
}

fn tap(r: &mut Runner<NoData>, ticket: u64, choice: Option<&str>) -> String {
    let choice = choice
        .map(|c| format!(r#","choice":"{c}""#))
        .unwrap_or_default();
    agent::answer(r, &format!(r#"{{"op":"tap","ticket":{ticket}{choice}}}"#))
        .unwrap()
        .0
}

/// The host's part: held module work run, events delivered.
fn pump(r: &mut Runner<NoData>) {
    for out in r.take_requests() {
        let token = out.request.continuation.expect("held work");
        if let exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) = r.dispatch_work(token) {
            r.fulfill(out.ticket, work()).unwrap();
        }
    }
    for (_, dispatch) in r.release_work() {
        drop(dispatch);
    }
    let (_, error) = r.apply_announced();
    assert!(error.is_none(), "{error:?}");
}

/// Run the nested component's action named `suffix`, by its lifted name.
fn act_nested(r: &mut Runner<NoData>, suffix: &str) {
    let plan = r.plan();
    let names: Vec<String> = (plan.actions.iter())
        .map(|a| plan.str(a.name).to_owned())
        .collect();
    let name = (names.iter())
        .find(|n| n.starts_with(&format!("{suffix}#")))
        .unwrap_or_else(|| panic!("no lifted {suffix} in {names:?}"))
        .clone();
    r.act(&name, Vec::new()).unwrap();
}

fn permission_ticket(r: &Runner<NoData>) -> u64 {
    (r.device_holds().iter())
        .find(|h| h.capability == "permission")
        .unwrap_or_else(|| panic!("no permission ticket: {}", logs(r)))
        .ticket
}

#[test]
fn suggestion_sets_include_module_events_and_frames_accept_declared_host_sends() {
    let app = App::new("suggestion-callers");
    let property =
        contract::properties("always \"heard\"\n  expect state heard in \"one\" \"two\"\nafter \"clearHostCounter\" \"clear\"\n  writes none\n")
            .unwrap();
    for event in ["", " chime=hostValue"] {
        app.write(
            "app.contract",
            &format!(
                r#"use chime, clearCount from "@fixture/bell"
component App
  state heard = "one"
  action hear(v: string)
    heard = v
  action hostValue(v: string)
    hear(v)
  action clearHostCounter
    clearCount()
  view
    column{event}
      button press=hear("two")
        text "Two"
"#
            ),
        );
        app.compile().unwrap_or_else(|e| panic!("{e:?}"));
        let checked =
            contract::check_suggestions_path(&app.0.join("app.contract"), &property).unwrap();
        if event.is_empty() {
            assert_eq!(checked[0], contract::SuggestionCheck::Keep);
        } else {
            assert!(
                matches!(
                    &checked[0],
                    contract::SuggestionCheck::Drop { rule: "set", .. }
                ),
                "{checked:?}"
            );
        }
        assert_eq!(checked[1], contract::SuggestionCheck::Keep);
    }
}

#[test]
fn the_fixture_compiles_its_statements_and_its_root_events() {
    let app = App::new("compile");
    let plan = app.compile().unwrap_or_else(|e| panic!("{e:?}"));
    let events: Vec<String> = (plan.module_events.iter())
        .map(|e| {
            format!(
                "{}.{} {} {} {}",
                plan.str(e.extension),
                plan.str(e.declared),
                plan.str(plan.action(e.action).name),
                plan.str(e.raised),
                e.hold
            )
        })
        .collect();
    assert_eq!(
        events,
        [
            "bell.ringclick rang activation 4",
            "bell.chime chimed emit 4"
        ]
    );
    let unit = |name: &str| {
        let s = (plan.sources.iter())
            .find(|s| plan.str(s.name) == name)
            .unwrap();
        plan.types[s.ty.0 as usize].kind == exact_plan::TypeKind::Unit
    };
    assert!(unit("ring") && unit("clearCount") && unit("silence"));
}

#[test]
fn a_send_that_answers_nothing_and_an_event_are_refused_where_they_do_not_belong() {
    let refused = |body: &str| -> Vec<(String, String)> {
        let app = App::new("refused");
        app.write("app.contract", body);
        match app.compile() {
            Ok(_) => panic!("compiled:\n{body}"),
            Err(all) => all.into_iter().map(|e| (e.id, e.message)).collect(),
        }
    };
    let head = "use ring, ringclick, requestBell, BellStatus from \"@fixture/bell\"\n";
    let mutation = refused(&format!("{head}component App\n  mutation m as shape BellStatus\n  action go\n    send m = ring(tag=\"a\")\n  view\n    button press=go\n      text \"go\"\n"));
    assert!(
        mutation
            .iter()
            .any(|(id, m)| id == "contract-module-answers" && m.contains("answers nothing")),
        "{mutation:?}"
    );
    let statement = refused(&format!("{head}component App\n  action go\n    requestBell()\n  view\n    button press=go\n      text \"go\"\n"));
    assert!(
        statement
            .iter()
            .any(|(id, m)| id == "contract-module-kind" && m.contains("a mutation sends it")),
        "{statement:?}"
    );
    let nested = refused(&format!("{head}component App\n  action opened(tag: string)\n    ring(tag=tag)\n  view\n    column\n      button ringclick=opened\n        text \"go\"\n"));
    assert!(
        nested.iter().any(|(id, _)| id == "contract-event-root"),
        "{nested:?}"
    );
    let typed = refused(&format!("{head}component App\n  action opened(tag: number)\n    ring(tag=\"a\")\n  view\n    column ringclick=opened\n      text \"go\"\n"));
    assert!(
        typed
            .iter()
            .any(|(id, m)| id == "contract-event-handler" && m.contains("carries `string`")),
        "{typed:?}"
    );
}

/// D5 and §7: a statement lists the post (held first on the permission
/// ticket while `prompt`), a newer one replacing its tag; a value is set
/// app-wide; `tap @ref` raises the activation event, heard once by the
/// root's action with the entry's data; a not-yet-delivered entry is
/// refused until the agent clock passes its time.
#[test]
fn the_declared_substitute_lists_sets_and_taps() {
    let app = App::new("substitute");
    let mut r = boot(app.compile().unwrap_or_else(|e| panic!("{e:?}")));
    r.act("post", Vec::new()).unwrap();
    pump(&mut r);
    assert_eq!(state(&r)["rings"], serde_json::json!([]), "{}", logs(&r));
    let ticket = permission_ticket(&r);
    let reply = tap(&mut r, ticket, Some("allow"));
    assert!(reply.contains("allow"), "{reply}");
    pump(&mut r);
    let rings = state(&r)["rings"].clone();
    assert_eq!(rings[0]["tag"], "t1", "{rings} {}", logs(&r));
    assert_eq!(rings[0]["data"], "thread-1");
    assert_eq!(rings[0]["delivered"], true);
    r.act("post", Vec::new()).unwrap();
    pump(&mut r);
    let again = state(&r)["rings"].clone();
    assert_eq!(
        again.as_array().unwrap().len(),
        1,
        "a tag replaces: {again}"
    );
    r.act("badge", Vec::new()).unwrap();
    assert_eq!(state(&r)["count"], 3);
    r.act("flag", Vec::new()).unwrap();
    assert_eq!(state(&r)["count"], "flag");
    // The tap: the event, then the handler's request, which a person's
    // origin may ask (granted already: it answers the status).
    let reference = again[0]["ref"].as_u64().unwrap();
    let reply = tap(&mut r, reference, None);
    assert!(reply.contains("ringclick"), "{reply}");
    pump(&mut r);
    assert!(
        slot(&r, "heard").contains("t1 thread-1"),
        "{} {}",
        slot(&r, "heard"),
        logs(&r)
    );
    assert!(
        logs(&r).contains("bell.ringclick: heard by rang"),
        "{}",
        logs(&r)
    );
    assert!(
        slot(&r, "answered").contains("granted"),
        "{}",
        slot(&r, "answered")
    );
    assert_eq!(state(&r)["rings"], serde_json::json!([]));
    // A ring for a minute from now: not delivered, so not tappable, until the clock passes it.
    r.act("later", Vec::new()).unwrap();
    pump(&mut r);
    let later = state(&r)["rings"][0].clone();
    assert_eq!(later["delivered"], false, "{later}");
    let refused = tap(&mut r, later["ref"].as_u64().unwrap(), None);
    assert!(refused.contains("not delivered yet"), "{refused}");
    r.advance(61_000.0).unwrap();
    assert_eq!(state(&r)["rings"][0]["delivered"], true);
    // From a nested component: `silence` unlists, `clearCount` clears.
    act_nested(&mut r, "clear");
    assert_eq!(state(&r)["count"], serde_json::Value::Null);
}

/// D6's origin rule: an event a package emits runs its handler with the
/// device's origin, so the request it makes while `prompt` fails, reason
/// `person-required`, where an activation's would hold a ticket.
#[test]
fn an_emitted_event_carries_the_device_s_origin() {
    let app = App::new("origin");
    let mut r = boot(app.compile().unwrap());
    r.raise_module_event("bell", "chime", r#"["c1"]"#, CallOrigin::App)
        .unwrap();
    pump(&mut r);
    assert!(slot(&r, "heard").contains("chime c1"), "{}", logs(&r));
    assert!(
        logs(&r).contains("bell.requestBell: refused, person-required (device origin)"),
        "{}",
        logs(&r)
    );
    assert!(
        r.device_holds().is_empty(),
        "no ticket: {:?}",
        r.device_holds()
    );
    r.raise_module_event(
        "bell",
        "ringclick",
        r#"["t9",{"tag":"t9","data":null}]"#,
        CallOrigin::App,
    )
    .unwrap();
    pump(&mut r);
    assert!(slot(&r, "heard").contains("t9 none"), "{}", logs(&r));
    permission_ticket(&r);
}

/// D6: events are held up to their bound, the oldest dropped and
/// journaled, and the rest delivered in order, one commit each.
#[test]
fn events_are_held_in_order_and_bounded() {
    let app = App::new("hold");
    let mut r = boot(app.compile().unwrap());
    for i in 0..6 {
        r.raise_module_event("bell", "chime", &format!(r#"["c{i}"]"#), CallOrigin::Device)
            .unwrap();
    }
    let queued = &state(&r)["extensions"]["bell"]["queued"];
    assert_eq!(queued.as_array().map(Vec::len), Some(4), "{queued}");
    assert!(
        logs(&r).contains("bell.chime: dropped (c0)"),
        "{}",
        logs(&r)
    );
    pump(&mut r);
    let journal = logs(&r);
    let heard = journal.matches("heard by chimed").count();
    assert_eq!(heard, 4, "{journal}");
    assert!(slot(&r, "heard").contains("chime c5"));
}

/// §7: a relaunched app's `--notification '<entry>'` delivers the entry's data to
/// the handler, from nothing the session that posted it kept.
#[test]
fn a_launch_entry_carries_its_data_to_the_handler() {
    let app = App::new("launch");
    let mut r = boot(app.compile().unwrap());
    let entry =
        r#"{"tag":"t1","data":"thread-1","at":null,"navigate":null,"delivered":true,"ref":4}"#;
    let request =
        serde_json::json!({ "op": "launchEntry", "launch": "notification", "entry": entry });
    let reply = agent::answer(&mut r, &request.to_string()).unwrap().0;
    assert!(reply.contains("ringclick"), "{reply}");
    pump(&mut r);
    assert!(slot(&r, "heard").contains("t1 thread-1"), "{}", logs(&r));
}

/// §7: `"scope": "app"` keeps a value in the host's app-wide state, so a
/// second session reads what the first set and clears it for both.
#[test]
fn an_app_scoped_value_is_one_value_across_sessions() {
    let app = App::new("app");
    let shared = exact_runner::AppState::default();
    let mut a = boot(app.compile().unwrap());
    let mut b = boot(app.compile().unwrap());
    a.share_app_state(shared.clone());
    b.share_app_state(shared);
    a.act("badge", Vec::new()).unwrap();
    assert_eq!(state(&b)["count"], 3);
    act_nested(&mut b, "clear");
    assert_eq!(state(&a)["count"], serde_json::Value::Null);
}

/// A source that answers `quiet` later, as an app's data module would.
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
        _: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        Ok(exact_runner::Answer::Now(Value::record(vec![Value::Bool(
            true,
        )])))
    }
    fn grants(&self) -> &str {
        ""
    }
}

const LATER: &str = "use requestBell, chime, BellStatus from \"@fixture/bell\"\n\nshape Done\n  done: bool\n\ncomponent App\n  state heard = \"\"\n  mutation hushed as shape Done then afterHush\n  mutation answered as shape BellStatus\n  action chimed(tag: string)\n    heard = tag\n    send hushed = quiet()\n  action afterHush\n    send answered = requestBell()\n  action press\n    send answered = requestBell()\n  view\n    column chime=chimed\n      text heard\n";

/// LLP 1133.002 D12, origin follows causation: an emitted event's request,
/// its answer's commit and the `then` that arms are the device's, so the
/// request there is refused while `prompt`; a data-code call while that
/// request is in flight is the device's too (least privileged), and once it
/// lands a person's own request asks again.
#[test]
fn the_device_s_origin_follows_its_requests_answers_and_thens() {
    let app = App::new("later");
    app.write("app.contract", LATER);
    let mut r = Runner::boot(
        app.compile().unwrap_or_else(|e| panic!("{e:?}")),
        Later,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.raise_module_event("bell", "chime", r#"["c1"]"#, CallOrigin::Device)
        .unwrap();
    r.apply_announced();
    let out = r.take_requests();
    assert_eq!(out.len(), 1, "the chime's request is in flight");
    // Data code while it is: the device's, refused while prompt.
    let call = br#"{"extension":"bell","send":"requestBell","args":{}}"#.to_vec();
    let dispatch = r.native_work(&exact_runner::Request::native(call));
    let exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) = dispatch else {
        panic!("answered at once");
    };
    let exact_runner::Outcome::Response(reply) = work() else {
        panic!("a reply")
    };
    assert!(String::from_utf8_lossy(&reply.body).contains("NotAllowedError"));
    // The answer lands; its `then` runs as the device's commit.
    r.fulfill(
        out[0].ticket,
        exact_runner::Outcome::Response(exact_runner::Response {
            status: 200,
            headers: Vec::new(),
            body: b"{}".to_vec(),
        }),
    )
    .unwrap();
    r.advance(1.0).unwrap();
    let journal = agent::handle(&r, r#"{"op":"logs"}"#);
    assert_eq!(
        journal.matches("refused, person-required").count(),
        2,
        "{journal}"
    );
    // A person's own press afterwards asks: a permission ticket, no refusal.
    r.act("press", Vec::new()).unwrap();
    assert!(
        r.device_holds()
            .iter()
            .any(|h| h.capability == "permission"),
        "{journal}"
    );
}

/// D6: each event keeps its own bound; one kind's flood drops none of
/// another's, and each drop is journaled under its own event.
#[test]
fn each_event_keeps_its_own_bound() {
    let app = App::new("mixed");
    let mut r = boot(app.compile().unwrap());
    r.raise_module_event(
        "bell",
        "ringclick",
        r#"["r1",{"tag":"r1","data":null}]"#,
        CallOrigin::App,
    )
    .unwrap();
    for i in 0..6 {
        r.raise_module_event("bell", "chime", &format!(r#"["c{i}"]"#), CallOrigin::Device)
            .unwrap();
    }
    let queued = state(&r)["extensions"]["bell"]["queued"].clone();
    let kinds: Vec<&str> = (queued.as_array().unwrap().iter())
        .map(|q| q["event"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "ringclick").count(),
        1,
        "{queued}"
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "chime").count(),
        4,
        "{queued}"
    );
    let journal = logs(&r);
    assert!(
        journal.contains("bell.chime: dropped (c1)")
            && !journal.contains("bell.ringclick: dropped"),
        "{journal}"
    );
}

/// §7: a declared state name is one module's and never one of the agent's
/// own keys.
#[test]
fn a_declared_state_name_is_one_module_s_and_not_the_agent_s() {
    let app = App::new("reserved");
    let path = app.0.join("node_modules/@fixture/bell/exact.json");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("\"state\": \"rings\"", "\"state\": \"pending\""),
    )
    .unwrap();
    let all = app.compile().unwrap_err();
    assert!(
        all.iter()
            .any(|e| e.id == "contract-module-state" && e.message.contains("`pending`")),
        "{all:?}"
    );
}

struct Named(&'static str);
impl exact_runner::DataSource for Named {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn app_id(&self) -> &str {
        self.0
    }
    fn grants(&self) -> &str {
        ""
    }
}

/// §7: an app-wide value is the app's, so two apps one embedder hosts, which
/// share the process's state, each keep their own.
#[test]
fn an_app_wide_value_is_one_app_s() {
    let app = App::new("apps");
    let shared = exact_runner::AppState::default();
    let boot_as = |id: &'static str| {
        let mut r = Runner::boot(
            app.compile().unwrap(),
            Named(id),
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.share_app_state(shared.clone());
        r
    };
    let mut one = boot_as("com.example.one");
    let two = boot_as("com.example.two");
    one.act("badge", Vec::new()).unwrap();
    let read = |r: &Runner<Named>| -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(&agent::handle(r, r#"{"op":"state"}"#)).unwrap()
            ["count"]
            .clone()
    };
    assert_eq!(
        (read(&one), read(&two)),
        (serde_json::json!(3), serde_json::Value::Null)
    );
}

//! The shipped bytecode calls the real native device through the host data seam.
use exact_js::Module;
use exact_js_value::{to_json, Shape};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Store};
use serde_json::Value as Json;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Device {
    root: PathBuf,
    module: Option<Module>,
    plan: Plan,
    store: Store,
}

#[test]
fn router_drafts_survive_async_links_and_device_reopen() {
    use exact_runner::{Event, Runner};
    fn settle(runner: &mut Runner<Module>) {
        for _ in 0..200 {
            let requests = runner.take_requests();
            if requests.is_empty() {
                return;
            }
            for request in requests {
                let outcome = if let Some(token) = request.request.continuation {
                    std::thread::spawn(runner.data().continuation(token).unwrap())
                        .join()
                        .unwrap()
                } else {
                    Outcome::Failed {
                        kind: FailureKind::Network,
                        message: "offline fixture".into(),
                    }
                };
                runner.fulfill(request.ticket, outcome).unwrap();
            }
        }
        panic!("Messages navigation did not settle");
    }
    let mut device = Device::new();
    for (id, draft, reply) in [("maya", "Saved Maya", "m10"), ("dad", "Saved Dad", "")] {
        device.call(
            "saveDraft",
            vec![Value::str(id), Value::str(draft), Value::str(reply)],
        );
    }
    let boot = |device: &mut Device, url: &str| {
        Runner::boot(
            Plan::decode(super::PLAN).unwrap(),
            device.module.take().unwrap(),
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            url,
        )
        .unwrap()
    };
    let mut runner = boot(&mut device, "/t/maya");
    settle(&mut runner);
    assert_eq!(runner.derive("draft"), Some(&Value::str("Saved Maya")));
    assert_eq!(runner.derive("replying"), Some(&Value::str("m10")));
    runner
        .act("write", vec![Value::str("Maya edited")])
        .unwrap();
    runner
        .dispatch(runner.roots()[0], Event::Navigate("/t/dad".into()))
        .unwrap();
    // Navigate again before the destination's resource settles. It must not save
    // a placeholder over Dad's existing draft or reuse Maya's composer.
    assert_ne!(runner.derive("draft"), Some(&Value::str("Maya edited")));
    runner
        .dispatch(runner.roots()[0], Event::Navigate("/t/maya".into()))
        .unwrap();
    settle(&mut runner);
    assert_eq!(runner.derive("draft"), Some(&Value::str("Maya edited")));
    drop(runner);
    device.reopen();
    let mut runner = boot(&mut device, "/t/dad");
    settle(&mut runner);
    assert_eq!(runner.derive("draft"), Some(&Value::str("Saved Dad")));
    runner
        .dispatch(runner.roots()[0], Event::Navigate("/t/maya".into()))
        .unwrap();
    settle(&mut runner);
    assert_eq!(runner.derive("draft"), Some(&Value::str("Maya edited")));
    assert_eq!(runner.derive("replying"), Some(&Value::str("m10")));
}

impl Device {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "messages-device-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut device = Self {
            root,
            module: None,
            plan: Plan::decode(super::PLAN).unwrap(),
            store: Store::new(super::GRANTS, Vec::<(String, String)>::new()),
        };
        device.reopen();
        device
    }
    fn reopen(&mut self) {
        self.module = None;
        let mut m = super::native::module(super::BYTECODE, super::APP, super::GRANTS);
        m.configure_storage(
            self.root.join("data"),
            self.root.join("cache"),
            self.root.join("tmp"),
        )
        .unwrap();
        m.bind(&self.plan);
        assert!(!m.ready());
        m.activate().unwrap();
        self.module = Some(m);
    }
    fn call(&mut self, name: &str, args: Vec<Value>) -> Json {
        self.try_call(name, args).unwrap()
    }
    fn try_call(&mut self, name: &str, args: Vec<Value>) -> Result<Json, DataError> {
        let m = self.module.as_mut().unwrap();
        let mut answer = m.answer(&mut self.store, name, &args)?;
        for _ in 0..200 {
            answer = match answer {
                Answer::Now(value) => {
                    let source = self
                        .plan
                        .sources
                        .iter()
                        .find(|row| self.plan.str(row.name) == name)
                        .unwrap();
                    return Ok(
                        to_json(&value, &Shape::from_plan(&self.plan, source.ty).unwrap()).unwrap(),
                    );
                }
                Answer::Later(request) => {
                    let outcome = if let Some(token) = request.continuation {
                        std::thread::spawn(m.continuation(token).expect("host continuation"))
                            .join()
                            .unwrap()
                    } else {
                        Outcome::Failed {
                            kind: FailureKind::Network,
                            message: "offline fixture".into(),
                        }
                    };
                    m.parse(&mut self.store, name, &args, outcome)?
                }
            };
        }
        panic!("{name} did not settle")
    }
    fn thread(&mut self) -> Json {
        self.call(
            "conversation",
            vec![
                Value::str("maya"),
                Value::Number(0.),
                Value::str(""),
                Value::str(""),
                Value::str(""),
            ],
        )
    }
}

#[test]
fn refused_native_save_preserves_draft_and_allows_the_next_edit() {
    let mut d = Device::new();
    let before = d.thread()["messages"].as_array().unwrap().len();
    d.call(
        "saveDraft",
        vec![
            Value::str("maya"),
            Value::str("Keep my draft"),
            Value::str(""),
        ],
    );
    let send = |body: &str| {
        vec![
            Value::str("maya"),
            Value::str(body),
            Value::str(""),
            Value::Number(0.),
            Value::Number(45_000.),
        ]
    };
    assert!(d
        .try_call("sendMessage", send(&"🌲".repeat(20_000)))
        .is_err());
    for reopen in [false, true] {
        if reopen {
            d.reopen();
        }
        assert_eq!(d.thread()["messages"].as_array().unwrap().len(), before);
        let inbox = d.call("inbox", vec![Value::str(""), Value::Number(0.)]);
        let person = inbox["people"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "maya")
            .unwrap();
        assert_eq!(person["draft"], "Keep my draft");
    }
    d.call("sendMessage", send("A valid later edit"));
    assert_eq!(
        d.thread()["messages"].as_array().unwrap().last().unwrap()["body"],
        "A valid later edit"
    );
}
impl Drop for Device {
    fn drop(&mut self) {
        self.module = None;
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn offline_messages_drafts_and_reactions_survive_the_real_native_reopen() {
    let mut d = Device::new();
    let before = d.thread()["messages"].as_array().unwrap().len();
    d.call(
        "sendMessage",
        vec![
            Value::str("maya"),
            Value::str("Kept offline 🌲"),
            Value::str("m9"),
            Value::Number(0.),
            Value::Number(45_000.),
        ],
    );
    let sent = d.thread()["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    let id = sent["id"].as_str().unwrap();
    d.call(
        "react",
        vec![Value::str("maya"), Value::str(id), Value::str("❤️")],
    );
    d.call(
        "saveDraft",
        vec![
            Value::str("dad"),
            Value::str("A durable draft"),
            Value::str(""),
        ],
    );
    d.call("syncMessages", vec![Value::Number(3_000.)]);
    d.reopen();
    let thread = d.thread();
    let messages = thread["messages"].as_array().unwrap();
    assert_eq!(messages.len(), before + 1);
    assert_eq!(messages.last().unwrap()["body"], "Kept offline 🌲");
    assert_eq!(messages.last().unwrap()["reaction"], "❤️");
    assert_eq!(messages.last().unwrap()["replyRoot"], "m9");
    let inbox = d.call("inbox", vec![Value::str(""), Value::Number(0.)]);
    assert_eq!(
        inbox["people"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "dad")
            .unwrap()["draft"],
        "A durable draft"
    );
    d.call(
        "sendMessage",
        vec![
            Value::str("maya"),
            Value::str("After restart"),
            Value::str(""),
            Value::Number(0.),
            Value::Number(46_000.),
        ],
    );
    let next = d.thread();
    assert_ne!(
        next["messages"].as_array().unwrap().last().unwrap()["id"],
        sent["id"]
    );
    assert!(
        d.plan
            .resources
            .iter()
            .filter(|r| ["inbox", "chat", "syncStatus"].contains(&d.plan.str(r.name)))
            .all(|r| r.reader),
        "bake records the storage dependency for post-pixel refresh"
    );
}

#[test]
fn one_ui_edit_is_one_mutation_and_refused_batches_leave_no_partial_edits() {
    fn partition(d: &Device) -> exact_snapback4::Module {
        assert!(
            d.module.is_none(),
            "the app releases its partition before inspection"
        );
        let mut core = exact_snapback4::Module::new(super::APP, super::GRANTS).unwrap();
        core.configure_storage(
            d.root.join("data"),
            d.root.join("cache"),
            d.root.join("tmp"),
        )
        .unwrap();
        let path = super::GRANTS
            .lines()
            .find_map(|line| line.strip_prefix("sqlite.open "))
            .unwrap();
        core.call(&serde_json::json!({"op":"open", "path":path,
            "origin":"http://127.0.0.1:4400", "viewer":"dev:alice"}))
            .unwrap();
        core
    }
    fn rows(core: &mut exact_snapback4::Module) -> Json {
        core.call(&serde_json::json!({"op":"query", "name":"records",
            "viewer":"dev:alice", "args":{"c":null}, "now":0}))
            .unwrap()["ok"]
            .clone()
    }
    fn queued(core: &mut exact_snapback4::Module) -> usize {
        core.call(&serde_json::json!({"op":"queued"})).unwrap()["ok"]
            .as_array()
            .unwrap()
            .len()
    }
    let mut d = Device::new();
    d.thread();
    d.module = None;
    let mut core = partition(&d);
    // A fresh offline device keeps intents. Acquire actual server facts before
    // testing prediction constraints; do not label an unacquired view empty.
    let backend = core.call(&serde_json::json!({"op":"backend"})).unwrap()["ok"].clone();
    let programs: Vec<snapback4_core::ir::Program> =
        serde_json::from_value(backend["programs"].clone()).unwrap();
    let mut server = snapback4_core::engine::Engine::memory(
        serde_json::from_value(backend["schema"].clone()).unwrap(),
        programs.clone(),
    )
    .unwrap();
    let entries = core.call(&serde_json::json!({"op":"queued"})).unwrap()["ok"].clone();
    for entry in entries.as_array().unwrap() {
        let program = programs
            .iter()
            .find(|p| p.name == entry["op"].as_str().unwrap())
            .unwrap();
        let args = program
            .args
            .iter()
            .map(|(name, kind)| {
                (
                    name.clone(),
                    snapback4_core::wire::from_json(kind, &entry["args"][name]).unwrap(),
                )
            })
            .collect();
        let sent = server
            .mutate(
                &program.name,
                entry["id"].as_str(),
                snapback4_core::engine::Context::new("dev:alice", args),
            )
            .unwrap();
        assert!(matches!(
            sent.state,
            snapback4_core::engine::WriteState::Sent { .. }
        ));
        core.call(&serde_json::json!({"op":"dequeue", "id":entry["id"]}))
            .unwrap();
    }
    let (events, watermark, next, held) = server
        .snapshot_page_with_groups_at("dev:alice", None, 4000, 0)
        .unwrap();
    assert!(next.is_none());
    let events: Vec<_> = events
        .iter()
        .map(|event| {
            serde_json::json!({
                "table":event.table,"kind":event.kind.word(),"id":event.id,
                "data":event.data.as_ref().map(snapback4_core::wire::to_json)
            })
        })
        .collect();
    let applied = core
        .call(&serde_json::json!({"op":"apply", "first":true, "page":{
            "snapshot":true,"events":events,"watermark":watermark,"more":false,
            "generation":server.generation(), "held":held
        }}))
        .unwrap();
    assert!(applied.get("ok").is_some(), "{applied}");
    let before = queued(&mut core);
    drop(core);
    d.reopen();
    d.call(
        "sendMessage",
        vec![
            Value::str("maya"),
            Value::str("One complete edit"),
            Value::str(""),
            Value::Number(0.),
            Value::Number(1_000.),
        ],
    );
    d.module = None;
    let mut core = partition(&d);
    assert_eq!(
        queued(&mut core),
        before + 1,
        "preview, draft and message share one upload"
    );
    let kept = rows(&mut core);
    let first = &kept["data"][0];
    let mut changed = first["payload"].clone();
    changed["uncommitted"] = Json::Bool(true);
    let refused = core
        .call(&serde_json::json!({"op":"predict", "name":"putRecords",
        "viewer":"dev:alice", "now":0, "newIds":[], "entropy":1,
        "args":{"recordIds":[first["id"], format!("{}:refused", first["id"].as_str().unwrap())],
            "keys":[first["key"], first["key"]], "payloads":[changed, {"uncommitted":true}]}}))
        .unwrap();
    assert_eq!(refused["ok"]["denied"]["code"], "E_PREDICT", "{refused}");
    assert_eq!(
        rows(&mut core),
        kept,
        "missing facts for the second row leave the first row unchanged"
    );
    assert_eq!(queued(&mut core), before + 1, "refusal adds no upload");
    drop(core);
    let mut reopened = partition(&d);
    assert_eq!(
        rows(&mut reopened),
        kept,
        "reopen retains the complete prior image"
    );

    // Prediction refuses unknown facts before claiming a constraint failure.
    // The authoritative server can establish the duplicate key and must undo
    // the earlier payload update in this same mutation.
    let server_rows = |server: &mut snapback4_core::engine::Engine| {
        server
            .query(
                "records",
                snapback4_core::engine::Context::new(
                    "dev:alice",
                    [("c".into(), snapback4_core::value::Value::Null)].into(),
                ),
            )
            .unwrap()
    };
    let prior = server_rows(&mut server);
    let data = snapback4_core::wire::to_json(&prior.data);
    let first = &data[0];
    let mut changed = first["payload"].clone();
    changed["uncommitted"] = Json::Bool(true);
    let args = serde_json::json!({
        "recordIds":[first["id"], format!("{}:refused", first["id"].as_str().unwrap())],
        "keys":[first["key"], first["key"]],
        "payloads":[changed, {"uncommitted":true}]
    });
    let program = programs.iter().find(|p| p.name == "putRecords").unwrap();
    let args = program
        .args
        .iter()
        .map(|(name, kind)| {
            (
                name.clone(),
                snapback4_core::wire::from_json(kind, &args[name]).unwrap(),
            )
        })
        .collect();
    let refused = server
        .mutate(
            "putRecords",
            Some("refused-batch"),
            snapback4_core::engine::Context::new("dev:alice", args),
        )
        .unwrap();
    match refused.state {
        snapback4_core::engine::WriteState::Failed { why, .. } => {
            assert_eq!(why.code, "E_CONSTRAINT", "{why:?}");
        }
        other => panic!("duplicate key was accepted: {other:?}"),
    }
    assert_eq!(refused.events, 0, "a refused batch publishes no row events");
    assert_eq!(
        server_rows(&mut server).data,
        prior.data,
        "the second row's unique violation rolls back the first payload update"
    );
}

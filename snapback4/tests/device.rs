use exact_plan::Value as PlanValue;
use exact_runner::DataSource;
use exact_snapback4::Module;
use serde_json::{json, Value};
use snapback4_core::{
    engine::{Context, Engine, WriteState},
    value::{object, Value as CoreValue},
    wire,
};
use snapback4_device::native::Backend;
use std::{collections::BTreeMap, path::PathBuf};

const APP: &str = "test.exact.snapback4";
const ORIGIN: &str = "https://messages.example.test";
const VIEWER: &str = "dev:alice";
const PATH: &str = "app:/data/alice.sqlite";
const GRANTS: &str = "sqlite.open app:/data";
const SCHEMA: &str = r#"
use identity
table messages:
  author: principal
  body: text <=50
  at: time
  by byTime: at, id
  public 'messages are public'
  insert <- .author = viewer
  update <- deny
  delete <- .author = viewer
  sync public last 100 by byTime

query inbox():
  return messages last 50 by byTime

mutation send(body: text <=50):
  require body != '' else EMPTY
  row = insert messages { author: viewer, body, at: now }
  return { id: row.id }
"#;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut id = [0u8; 16];
        ibex2::stdlib::crypto::get_random_values(&mut id).unwrap();
        let id: String = id.iter().map(|b| format!("{b:02x}")).collect();
        Self(std::env::temp_dir().join(format!("exact-snapback4-{id}")))
    }
    fn configure(&self, module: &mut Module) {
        module
            .configure_storage(
                self.0.join("data"),
                self.0.join("cache"),
                self.0.join("tmp"),
            )
            .unwrap();
    }
    fn module(&self) -> Module {
        let mut module = Module::new(APP, GRANTS).unwrap();
        self.configure(&mut module);
        module
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn server() -> (Engine, Backend) {
    let bound = snapback4_lang::compile(&[("schema.q".into(), SCHEMA.into())]);
    assert!(bound.diagnostics.is_empty(), "{:?}", bound.diagnostics);
    let programs: Vec<_> = bound.programs.into_iter().map(|p| p.program).collect();
    let mut engine = Engine::memory(bound.schema.clone(), programs.clone()).unwrap();
    engine
        .seed(vec![(
            "messages".into(),
            object([
                ("id", CoreValue::Id("first".into())),
                ("author", CoreValue::Id(VIEWER.into())),
                ("body", CoreValue::String("already confirmed".into())),
                ("at", CoreValue::Int(100)),
            ]),
        )])
        .unwrap();
    let backend = Backend {
        generation: engine.generation(),
        schema: bound.schema,
        programs,
    };
    (engine, backend)
}

fn open(backend: Option<&Backend>) -> Value {
    json!({"op":"open", "path":PATH, "origin":ORIGIN, "viewer":VIEWER, "backend":backend})
}

fn call(module: &mut Module, request: Value) -> Value {
    let answer = module.call(&request).unwrap();
    assert!(answer.get("ok").is_some(), "{answer}");
    answer["ok"].clone()
}

fn inbox(module: &mut Module) -> Value {
    call(
        module,
        json!({"op":"query", "name":"inbox", "viewer":VIEWER}),
    )["data"]
        .clone()
}

fn snapshot(module: &mut Module, engine: &mut Engine) {
    let (events, watermark, next) = engine.snapshot_page(VIEWER, None, 100).unwrap();
    assert!(next.is_none());
    let events: Vec<_> = events.iter().map(|event| {
        json!({"table":event.table,"kind":event.kind.word(),"id":event.id,"data":event.data.as_ref().map(wire::to_json)})
    }).collect();
    call(
        module,
        json!({"op":"apply", "first":true,"page":{
            "snapshot":true,"events":events,"watermark":watermark,
            "more":false,"generation":engine.generation()
        }}),
    );
}

#[test]
fn native_partition_predicts_keeps_outbox_reopens_and_applies_server_confirmation() {
    let fixture = Fixture::new();
    let (mut engine, backend) = server();
    let mut module = fixture.module();
    call(&mut module, open(Some(&backend)));
    snapshot(&mut module, &mut engine);
    assert_eq!(inbox(&mut module)[0]["body"], "already confirmed");
    let identity = call(&mut module, json!({"op":"meta", "key":"exact:device"}));
    assert_eq!(identity.as_str().unwrap().len(), 32);
    let prediction = call(
        &mut module,
        json!({"op":"predict", "name":"send", "viewer":VIEWER,
        "args":{"body":"offline"}, "now":200, "newIds":["second"], "entropy":7}),
    );
    assert_eq!(prediction["result"]["id"], "second");
    let seq = call(&mut module, json!({"op":"next_submission"}));
    call(
        &mut module,
        json!({"op":"enqueue", "entry":{"id":"write1","seq":seq,
        "op":"send","args":{"body":"offline"},"new_ids":["second"],"predicted":prediction["predicted"],"viewer":VIEWER}}),
    );
    assert_eq!(inbox(&mut module)[0]["pending"], true);
    drop(module);

    // No backend/network: real SQLite supplies the schema, rows, and outbox.
    let mut module = fixture.module();
    call(&mut module, open(None));
    assert_eq!(
        call(&mut module, json!({"op":"meta", "key":"exact:device"})),
        identity
    );
    assert_eq!(inbox(&mut module).as_array().unwrap().len(), 2);
    let queue = call(&mut module, json!({"op":"queued"}));
    assert_eq!(queue[0]["id"], "write1");
    assert_eq!(queue[0]["new_ids"][0], "second");
    let watermark = call(&mut module, json!({"op":"state"}))["watermark"]
        .as_u64()
        .unwrap();
    let context = Context::new(
        VIEWER,
        BTreeMap::from([("body".into(), CoreValue::String("offline".into()))]),
    )
    .at(250)
    .with_ids(["second".to_owned()]);
    let sent = engine.mutate("send", Some("write1"), context).unwrap();
    assert!(matches!(sent.state, WriteState::Sent { .. }));
    let page = engine.stream(VIEWER, watermark, 100).unwrap();
    let events: Vec<_> = page.events.iter().map(|event| {
        json!({"table":event.table,"kind":event.kind,"id":event.id,"data":event.data.as_ref().map(wire::to_json)})
    }).collect();
    call(
        &mut module,
        json!({"op":"apply","first":false,"page":{
        "snapshot":false,"events":events,"watermark":page.watermark,"more":page.more,
        "generation":engine.generation()}}),
    );
    call(&mut module, json!({"op":"dequeue","id":"write1"}));
    assert!(call(&mut module, json!({"op":"queued"}))
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(inbox(&mut module)[0]["id"], "second");
    assert!(inbox(&mut module)[0].get("pending").is_none());

    let before = inbox(&mut module);
    let refused = call(
        &mut module,
        json!({"op":"predict","name":"send","viewer":VIEWER,
        "args":{"body":""},"now":300,"newIds":["refused"]}),
    );
    assert_eq!(refused["denied"]["code"], "EMPTY");
    assert_eq!(inbox(&mut module), before);
    let predicted = call(
        &mut module,
        json!({"op":"predict","name":"send","viewer":VIEWER,
        "args":{"body":"withdraw this"},"now":300,"newIds":["withdrawn"]}),
    );
    call(
        &mut module,
        json!({"op":"withdraw","predicted":predicted["predicted"]}),
    );
    assert_eq!(inbox(&mut module), before);
    call(&mut module, json!({"op":"close"}));
    let mut newer = backend;
    newer.generation += 1;
    // Opening never replaces the kept backend; adoption is a sync round's.
    call(&mut module, open(Some(&newer)));
    assert_eq!(
        call(&mut module, json!({"op":"state"}))["generation"],
        newer.generation - 1
    );
    call(&mut module, json!({"op":"adopt","backend":newer}));
    assert_eq!(
        call(&mut module, json!({"op":"sync_state"}))["acquired"],
        false
    );
    assert!(call(
        &mut module,
        json!({"op":"query", "name":"inbox", "viewer":VIEWER})
    )["denied"]
        .is_object());
    assert_eq!(call(&mut module, json!({"op":"state"}))["watermark"], 0);
}

#[test]
fn construction_configuration_and_validation_do_no_io_and_grants_refuse_before_creation() {
    let fixture = Fixture::new();
    let (_, backend) = server();
    let mut module = Module::new(APP, GRANTS).unwrap();
    assert!(!module.ready());
    assert!(module.call(&open(Some(&backend))).is_err());
    fixture.configure(&mut module);
    let request = PlanValue::str(&open(Some(&backend)).to_string());
    assert!(module
        .query("snapback4", std::slice::from_ref(&request))
        .is_err());
    assert!(!fixture.0.exists());
    module.activate_for_validation().unwrap();
    assert!(module.ready());
    assert!(module.query("snapback4", &[request]).is_err());
    assert!(!fixture.0.exists());
    let mut denied = Module::new(APP, "fs.write app:/data").unwrap();
    fixture.configure(&mut denied);
    assert!(denied.call(&open(Some(&backend))).is_err());
    assert!(!fixture.0.exists());
    // A grant for another file names the one it wanted, and what admits it.
    let mut other = Module::new(APP, "sqlite.open app:/data/inbox").unwrap();
    fixture.configure(&mut other);
    let refused = other.call(&open(Some(&backend))).unwrap_err();
    assert!(
        refused.starts_with(&format!("denied: sqlite.open {PATH}: ")),
        "{refused}"
    );
    assert!(
        refused.contains(&format!("`sqlite.open {PATH}`, or `sqlite.open app:/data`")),
        "{refused}"
    );
    assert!(!fixture.0.exists());
}

#[test]
fn source_interface_activates_then_round_trips_real_device_json() {
    let fixture = Fixture::new();
    let (_, backend) = server();
    let mut module = fixture.module();
    module.activate().unwrap();
    assert!(!fixture.0.exists());
    let answer = module
        .query(
            "snapback4",
            &[PlanValue::str(&open(Some(&backend)).to_string())],
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(answer.as_str().unwrap()).unwrap(),
        json!({"ok":true})
    );
    assert_eq!(module.app_id(), APP);
    assert_eq!(module.grants(), GRANTS);
    assert!(module.query("other", &[]).is_err());
    assert!(module.query("snapback4", &[PlanValue::Bool(true)]).is_err());
    assert!(module
        .query("snapback4", &[PlanValue::str("bad JSON")])
        .is_err());
}

#[test]
fn partition_owner_is_immutable_and_checked_before_adopting_another_backend() {
    let fixture = Fixture::new();
    let (mut engine, backend) = server();
    let mut module = fixture.module();
    call(&mut module, open(Some(&backend)));
    snapshot(&mut module, &mut engine);
    assert!(module.call(&open(None)).is_err());
    assert!(module
        .call(&json!({"op":"query","name":"inbox","viewer":"dev:bob"}))
        .is_err());
    assert!(module
        .call(&json!({"op":"admit","entry":{"viewer":"dev:bob"}}))
        .is_err());
    assert!(module
        .call(&json!({"op":"set_meta","key":"exact2:partition","value":"different"}))
        .is_err());
    call(&mut module, json!({"op":"close"}));
    let mut newer = backend.clone();
    newer.generation += 1;
    for (key, value) in [
        ("viewer", "dev:bob"),
        ("origin", "https://other.example.test"),
    ] {
        let mut request = open(Some(&newer));
        request[key] = json!(value);
        assert!(module.call(&request).is_err());
    }
    let mut other_app = Module::new("other.app", GRANTS).unwrap();
    fixture.configure(&mut other_app);
    assert!(other_app.call(&open(Some(&newer))).is_err());
    call(&mut module, open(None));
    assert_eq!(
        call(&mut module, json!({"op":"state"}))["generation"],
        backend.generation
    );
    assert_eq!(inbox(&mut module)[0]["body"], "already confirmed");
}

#[test]
fn paths_stay_under_granted_app_data_and_symlinks_cannot_escape() {
    let fixture = Fixture::new();
    let (_, backend) = server();
    let mut module = fixture.module();
    for path in [
        "/tmp/exact-snapback4.sqlite",
        "app:/cache/a.sqlite",
        "app:/data/../a.sqlite",
        "app:/data/dir/a.sqlite",
        "app:/data/.sqlite",
        "app:/data/a.sqlite\0",
    ] {
        let mut request = open(Some(&backend));
        request["path"] = json!(path);
        assert!(module.call(&request).is_err(), "{path}");
        assert!(!fixture.0.exists());
    }
    // Never synced: the client opens it on the server's backend in its first round.
    assert_eq!(call(&mut module, open(None)), json!({"opened": false}));
    assert!(!fixture.0.join("data/alice.sqlite").exists());
    #[cfg(unix)]
    {
        let outside = Fixture::new();
        std::fs::create_dir_all(&outside.0).unwrap();
        let target = outside.0.join("outside.sqlite");
        std::fs::write(&target, b"untouched").unwrap();
        std::os::unix::fs::symlink(&target, fixture.0.join("data/alice.sqlite")).unwrap();
        assert!(module.call(&open(Some(&backend))).is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"untouched");
    }
}

#[test]
fn encoded_partition_filenames_are_literal_and_reopen_offline() {
    let fixture = Fixture::new();
    let (_, backend) = server();
    let mut module = fixture.module();
    let filename = "https%3A%2F%2Fmessages.example.test%3Adev%3Aalice.sqlite";
    let mut request = open(Some(&backend));
    request["path"] = json!(format!("app:/data/{filename}"));
    call(&mut module, request.clone());
    assert!(fixture.0.join("data").join(filename).is_file());
    assert!(!fixture.0.join("data/https:").exists());
    call(&mut module, json!({"op":"close"}));
    request["backend"] = Value::Null;
    call(&mut module, request);
    assert_eq!(
        call(&mut module, json!({"op":"state"}))["generation"],
        backend.generation
    );
}

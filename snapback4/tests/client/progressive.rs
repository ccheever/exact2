//! The pinned server engine and real SQLite device at each publication boundary.
use super::{Device, Value};
use serde_json::json;
use snapback4_core::{engine::Engine, partition::StreamPage, wire};

const VIEWER: &str = "dev:alice";
const GROUPS: usize = 40;
const HISTORY: usize = 100;
const SCHEMA: &str = r#"
use personas alice
 table participants:
  threadId: threads
  personId: principal
  unique threadId, personId
  cap 50 by threadId
  read <- exists participants[.threadId, viewer]
  sync by .personId
 table threads:
  subject: text <=100
  read <- exists participants[.id, viewer]
  sync to participants[.id] by .personId
 table messages:
  conversation: threads
  body: text <=100
  at: time
  read <- exists participants[.conversation, viewer]
  insert <- exists participants[.conversation, viewer]
  sync to participants[.conversation] by .personId last 100 by .at, .id
 query inbox():
  return participants where .personId = viewer first 50 by .threadId
 query history(c: threads, page: cursor ?):
  return messages where .conversation = c last 10 by .at, .id after page
 mutation add(c: threads, body: text <=100):
  insert messages {conversation: c, body, at: now}
"#;

struct Server {
    engine: Engine,
    backend: Value,
    requests: Vec<Value>,
    pages: Vec<Value>,
    advertised: bool,
    limit: usize,
}
impl Server {
    fn start() -> Self {
        let mut rows = Vec::new();
        for group in 0..GROUPS {
            let id = format!("g{group:02}");
            rows.push(("threads", json!({"id": id, "subject": id})));
            rows.push((
                "participants",
                json!({"id": format!("p{group:02}"), "threadId": id, "personId": VIEWER}),
            ));
            for at in 0..HISTORY {
                rows.push(("messages", json!({"id": format!("{id}-{at:03}"), "conversation": id, "body": format!("{at}"), "at": at})));
            }
        }
        Self::seeded(SCHEMA, rows)
    }
    fn seeded(source: &str, rows: Vec<(&str, Value)>) -> Self {
        let bound = snapback4_lang::compile(&[("schema.q".into(), source.replace("\n ", "\n"))]);
        assert!(bound.diagnostics.is_empty(), "{:?}", bound.diagnostics);
        let programs: Vec<_> = bound.programs.into_iter().map(|p| p.program).collect();
        let mut engine = Engine::memory(bound.schema.clone(), programs.clone()).unwrap();
        let backend = json!({"generation": engine.generation(), "schema": bound.schema,
            "programs": programs, "device_compatibility": snapback4_core::device_protocol::compatibility(engine.schema())});
        let rows: Vec<(String, snapback4_core::value::Value)> = rows
            .into_iter()
            .map(|(table, row)| {
                (
                    table.into(),
                    wire::row_from_json(&engine.schema().tables[table], &row).unwrap(),
                )
            })
            .collect();
        engine.seed(rows).unwrap();
        Self {
            engine,
            backend,
            requests: vec![],
            pages: vec![],
            advertised: true,
            limit: 400,
        }
    }
    fn stream(&self, page: StreamPage) -> Value {
        let mut body = serde_json::to_value(page).unwrap();
        // LogEvent.data uses the interpreter's value type, not the JSON wire.
        for event in body["events"].as_array_mut().unwrap() {
            if !event["data"].is_null() {
                let value = serde_json::from_value(event["data"].clone()).unwrap();
                event["data"] = wire::to_json(&value);
            }
        }
        body["snapshot"] = json!(false);
        body
    }
    fn fetch(&mut self, fetch: &Value) -> Value {
        self.requests.push(fetch.clone());
        if fetch["path"] == "/schema" {
            let mut backend = self.backend.clone();
            if !self.advertised {
                backend["device_compatibility"]
                    .as_object_mut()
                    .unwrap()
                    .remove("advertised");
            }
            return json!({"status": 200, "body": backend});
        }
        assert_eq!(fetch["path"], "/sync", "{fetch}");
        let req = &fetch["body"];
        let limit = self.limit.min(req["limit"].as_u64().unwrap() as usize);
        let from = req["from"].as_u64().unwrap();
        let pending: Vec<String> = serde_json::from_value(req["pending"].clone()).unwrap();
        let mut body = if let Some(acquire) = req.get("acquire") {
            assert!(self.advertised, "an old server must never receive acquire");
            assert!(acquire["skip"].as_array().unwrap().len() <= 32);
            let hint = acquire
                .get("hint")
                .map(|h| serde_json::from_value(h.clone()).unwrap());
            let skip: Vec<snapback4_core::partition::AcquisitionGroup> =
                serde_json::from_value(acquire["skip"].clone()).unwrap();
            let answer = match self.engine.acquisition_page_at(
                VIEWER,
                acquire["cursor"].as_str(),
                hint.as_ref(),
                &skip,
                from,
                limit,
                1000,
                &pending,
            ) {
                Ok(answer) => answer,
                Err(error) => {
                    let body = json!({"denied": error.refusal(), "store_id": "progressive-fixture",
                        "generation": self.backend["generation"], "watermark": from});
                    self.pages.push(body.clone());
                    return json!({"status": 200, "body": body});
                }
            };
            let mut page = self.stream(answer.page);
            page["acquire"] = serde_json::to_value(answer.acquire).unwrap();
            page["acknowledged"] = json!(answer.acknowledged);
            page["committed_above"] = json!(answer.committed_above);
            page
        } else if from == 0 && req["stream"] != true {
            let page = self
                .engine
                .snapshot_acquisition_page_at(VIEWER, req["after"].as_str(), limit, 1000)
                .unwrap();
            let events: Vec<_> = page
                .events
                .iter()
                .map(|e| {
                    json!({"table": e.table, "kind": e.kind.word(), "id": e.id,
                "data": e.data.as_ref().map(wire::to_json)})
                })
                .collect();
            json!({"snapshot": true, "watermark": page.watermark, "events": events,
                "more": page.next.is_some(), "next": page.next.map(|c| c.encode()), "after": req["after"],
                "acquisitions": page.acquisitions, "held": page.held})
        } else {
            let answer = self
                .engine
                .certified_stream_at(VIEWER, from, limit, 1000, &pending)
                .unwrap();
            let mut page = self.stream(answer.page);
            page["acknowledged"] = json!(answer.acknowledged);
            page["committed_above"] = json!(answer.committed_above);
            page
        };
        body["store_id"] = json!("progressive-fixture");
        body["generation"] = self.backend["generation"].clone();
        body["now"] = json!(1000);
        self.pages.push(body.clone());
        json!({"status": 200, "body": body})
    }
    fn sync(&mut self, device: &mut Device) -> Value {
        drive(device, |fetch| self.fetch(fetch))
    }
    fn open(&self, device: &mut Device) {
        device.call(
            json!({"op": "open", "path": "app:/data/inbox.sqlite", "origin": "http://127.0.0.1:1",
            "viewer": VIEWER, "backend": {"initial_backend": self.backend}}),
        );
        device.call(json!({"op": "close"}));
        assert!(device.open(1));
    }
}
fn drive(device: &mut Device, mut fetch: impl FnMut(&Value) -> Value) -> Value {
    let mut step = device.call(json!({"op": "sync"}));
    for _ in 0..1000 {
        if let Some(done) = step.get("done") {
            return done.clone();
        }
        let request = &step["fetch"];
        let reply = fetch(request);
        step =
            device.call(json!({"op": "deliver", "exchange": request["exchange"], "reply": reply}));
    }
    panic!("round did not yield: {step}")
}
fn history(device: &mut Device, group: usize) -> Value {
    device.call(json!({"op": "read", "name": "history", "args": {"c": format!("g{group:02}"), "page": null}, "now": 1000}))
}
fn complete(device: &mut Device, group: usize) {
    let read = history(device, group);
    assert_eq!(read["complete"], true, "{read}");
    let ids: Vec<_> = read["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        ids,
        (90..100)
            .rev()
            .map(|i| format!("g{group:02}-{i:03}"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn inbox_and_hinted_history_publish_before_full_acquisition_and_resume_offline() {
    let mut server = Server::start();
    let mut device = Device::new("alice");
    server.open(&mut device);
    assert_ne!(history(&mut device, 39)["complete"], true);
    let first = server.sync(&mut device);
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(first["acquired"], false);
    let status = device.call(json!({"op": "status"}));
    assert_eq!(first["acquisition"], status["acquisition"]);
    assert_eq!(
        status["acquisition"]["acquiredTables"],
        json!(["participants", "threads"])
    );
    assert_eq!(status["acquisition"]["flooredGroups"], 3);
    assert_eq!(status["acquisition"]["rows"], 380);
    assert_eq!(status["acquisition"]["tables"]["messages"], 300);
    assert_eq!(
        device.call(json!({"op": "read", "name": "inbox", "args": {}, "now": 1000}))["complete"],
        true
    );
    complete(&mut device, 39);
    assert_ne!(history(&mut device, 38)["complete"], true);
    assert_eq!(server.pages[0]["acquire"]["pinned"], true);
    assert_eq!(server.pages[0]["watermark"], server.pages[0]["images_at"]);
    assert_eq!(
        server
            .requests
            .iter()
            .filter(|r| r["path"] == "/schema")
            .count(),
        1
    );

    device.reopen();
    assert!(device.open(1));
    let reopened = device.call(json!({"op": "status"}));
    assert_eq!(
        reopened["acquisition"]["acquiredTables"],
        status["acquisition"]["acquiredTables"]
    );
    assert_eq!(reopened["acquisition"]["flooredGroups"], 3);
    assert_eq!(reopened["acquisition"]["rows"], 0);
    complete(&mut device, 39);
    let offline = drive(&mut device, |_| json!({"error": "offline"}));
    assert_eq!(offline["offline"], true);
    assert_eq!(offline["acquired"], false);
    complete(&mut device, 39);
    let start = server.requests.len();
    let mut rounds = 0;
    loop {
        let done = server.sync(&mut device);
        assert_eq!(done["ok"], true, "{done}");
        rounds += 1;
        assert!(rounds < 50, "{done}");
        if done["acquired"] == true {
            break;
        }
    }
    assert!(rounds > 1);
    assert_eq!(server.requests[start]["path"], "/schema");
    assert_eq!(server.requests[start + 1]["body"]["stream"], true);
    assert_eq!(
        server.requests[start..]
            .iter()
            .filter(|r| r["path"] == "/schema")
            .count(),
        1
    );
    for group in 0..GROUPS {
        complete(&mut device, group);
    }
    let finished = device.call(json!({"op": "status"}));
    assert_eq!(finished["acquisition"]["exhausted"], true);
    assert_eq!(finished["acquisition"]["flooredGroups"], GROUPS);
    device.reopen();
    assert!(device.open(1));
    let offline = drive(&mut device, |_| json!({"error": "offline"}));
    assert_eq!(offline["acquired"], true);
    assert_eq!(offline["acquisition"]["rows"], 0);
    for group in 0..GROUPS {
        complete(&mut device, group);
    }
}

#[test]
fn absent_advertisement_uses_staged_snapshot_even_for_a_resumed_walk() {
    for partial in [false, true] {
        let mut server = Server::start();
        let mut device = Device::new("alice");
        server.open(&mut device);
        if partial {
            assert_eq!(server.sync(&mut device)["acquired"], false);
            device.reopen();
            assert!(device.open(1));
        }
        server.advertised = false;
        server.requests.clear();
        server.pages.clear();
        let done = server.sync(&mut device);
        assert_eq!(done["ok"], true, "{done}");
        assert_eq!(done["acquired"], true);
        assert!(server
            .requests
            .iter()
            .all(|r| r["body"].get("acquire").is_none()));
        assert_eq!(server.pages[0]["snapshot"], true);
        for group in 0..GROUPS {
            complete(&mut device, group);
        }
    }
}

#[test]
fn heap_bound_acquisition_row_recovers_through_staged_replacement() {
    let mut server = Server::seeded(
        "use personas alice\ntable payloads:\n  body: json <=20000000\n  public 'fixture'\n  sync\n",
        vec![("payloads", json!({"id": "large", "body": vec![0; 1_000_000]}))],
    );
    server.limit = 1;
    let mut device = Device::new("alice");
    server.open(&mut device);
    let done = server.sync(&mut device);
    assert_eq!(done["ok"], true, "{done}");
    assert_eq!(done["acquired"], true);
    assert!(done["acquisition"].get("failure").is_none());
    let why = &server
        .pages
        .iter()
        .find(|p| p.get("denied").is_some())
        .unwrap()["denied"];
    assert_eq!(why["code"], "E_BOUND");
    assert_eq!(why["family"], "bound");
    assert!(why["message"].as_str().unwrap().contains("heap"));
    assert!(server.requests.iter().any(|r| r["path"] == "/sync"
        && r["body"].get("acquire").is_none()
        && r["body"]["stream"] == false
        && r["body"]["from"] == 0));
    let snapshot = server.pages.iter().find(|p| p["snapshot"] == true).unwrap();
    assert_eq!(
        snapshot["events"][0]["data"]["body"]
            .as_array()
            .unwrap()
            .len(),
        1_000_000
    );
    device.reopen();
    assert!(device.open(1));
    let offline = drive(&mut device, |_| json!({"error": "offline"}));
    assert_eq!(offline["acquired"], true);
}

#[test]
fn indivisible_replacement_is_latched_until_reopen_and_does_not_claim_completion() {
    let mut server = Server::start();
    let mut device = Device::new("alice");
    server.open(&mut device);
    let acquire_why = json!({"code": "E_BOUND", "family": "bound", "message": "atomic group exceeds heap", "retryable": false});
    let why = json!({"code": "E_BOUND", "family": "bound", "message": "the HTTP JSON response exceeds 16 MiB; request a smaller page or return fewer values", "retryable": false});
    let mut requests = Vec::new();
    let done = drive(&mut device, |fetch| {
        if fetch["path"] == "/sync" {
            let body = &fetch["body"];
            requests.push(body.clone());
            json!({"status": 200, "body": {"denied": if body.get("acquire").is_some() { &acquire_why } else { &why },
                "store_id": "progressive-fixture", "generation": server.backend["generation"], "watermark": body["from"]}})
        } else {
            server.fetch(fetch)
        }
    });
    assert_eq!(done["ok"], false);
    assert_eq!(done["acquired"], false);
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.get("acquire").is_some())
            .count(),
        1
    );
    assert!(requests
        .iter()
        .any(|r| r.get("acquire").is_none() && r["from"] == 0 && r["limit"] == 1));
    assert_eq!(done["acquisition"]["failure"], why);
    assert_eq!(
        device.call(json!({"op": "status"}))["acquisition"]["failure"],
        why
    );
    assert_eq!(
        drive(&mut device, |_| panic!("latched refusal retried"))["acquisition"]["failure"],
        why
    );
    device.reopen();
    assert!(device.open(1));
    let resumed = server.sync(&mut device);
    assert_eq!(resumed["ok"], true, "{resumed}");
    assert!(resumed["acquisition"].get("failure").is_none());
}

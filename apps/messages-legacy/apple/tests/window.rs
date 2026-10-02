//! Bounded answers from the shipped Hermes bytecode, with the bake's no-storage host.
use exact_js::Module;
use exact_js_value::{to_json, Shape};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, Event, Runner, Store};
use serde_json::Value as Json;
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));
#[path = "../../native.rs"]
mod native;

const K: usize = 200;

struct Model {
    module: Module,
    plan: Plan,
    store: Store,
    shapes: HashMap<String, Shape>,
}
impl Model {
    fn new() -> Self {
        Self::with_module(Module::new(BYTECODE.to_vec(), APP, GRANTS))
    }
    fn with_module(mut module: Module) -> Self {
        let plan = Plan::decode(PLAN).unwrap();
        // Compare warm answer minima separately from the production per-call
        // deadline, which is not a stable gate on a shared test machine.
        module.set_budget_ms(f64::INFINITY);
        module.bind(&plan);
        module.activate().unwrap();
        let shapes = plan
            .sources
            .iter()
            .map(|s| {
                (
                    plan.str(s.name).to_owned(),
                    Shape::from_plan(&plan, s.ty).unwrap(),
                )
            })
            .collect();
        Self {
            module,
            plan,
            store: Store::new(GRANTS, Vec::<(String, String)>::new()),
            shapes,
        }
    }
    fn try_call(&mut self, name: &str, args: Vec<Value>) -> Result<Json, DataError> {
        let Answer::Now(value) = self.module.answer(&mut self.store, name, &args)? else {
            panic!("{name} requested external work");
        };
        Ok(to_json(&value, &self.shapes[name]).unwrap())
    }
    fn call(&mut self, name: &str, args: Vec<Value>) -> Json {
        self.try_call(name, args).unwrap()
    }
    fn chat(&mut self, id: &str, cursor: &str, reply: &str, selection: &str) -> Json {
        let chat = self.call("conversation", chat_args(id, cursor, reply, selection));
        assert!(rows(&chat).len() <= K, "unbounded answer for {id}");
        chat
    }
    fn send(&mut self, id: &str, body: &str, reply: &str, now: f64) -> String {
        let change = self.call(
            "sendMessage",
            vec![
                Value::str(id),
                Value::str(body),
                Value::str(reply),
                Value::Number(now),
                Value::Number(now * 1_000.),
            ],
        );
        format!("sent-{}", change["revision"].as_f64().unwrap() as u64)
    }
    fn delete(&mut self, id: &str, selection: &str) {
        self.call(
            "deleteMessages",
            vec![Value::str(id), Value::str(selection), Value::Number(0.)],
        );
    }
    fn recover(&mut self, id: &str) {
        self.call(
            "recoverConversations",
            vec![Value::str(id), Value::Number(0.)],
        );
    }
    // The real send/receive/react sources seed both fixtures; no alternate TS,
    // native storage, test-only source, or source compilation is involved.
    fn grow(&mut self, id: &str, mut count: usize, target: usize) {
        let root = format!("{id}-1");
        while count < target {
            let reply = if count % 97 == 5 { &root } else { "" };
            let sent = self.send(id, &format!("row-{count:05}"), reply, count as f64 * 20.);
            count += 1;
            if count % 101 == 6 {
                self.call(
                    "react",
                    vec![Value::str(id), Value::str(&sent), Value::str("❤️")],
                );
            }
            if count % 97 == 6 && count < target {
                self.call(
                    "advanceReplies",
                    vec![
                        Value::Number(count as f64 * 20.),
                        Value::str(id),
                        Value::Number(count as f64 * 20_000.),
                    ],
                );
                count += 1;
            }
        }
    }
}
fn chat_args(id: &str, cursor: &str, reply: &str, selection: &str) -> Vec<Value> {
    vec![
        Value::str(id),
        Value::Number(0.),
        Value::str(reply),
        Value::str(selection),
        Value::str(cursor),
    ]
}
fn rows(chat: &Json) -> &[Json] {
    chat["messages"].as_array().unwrap()
}
fn text<'a>(value: &'a Json, field: &str) -> &'a str {
    value[field].as_str().unwrap()
}
fn remember(chat: &Json, seen: &mut HashMap<String, Json>) -> usize {
    let mut compared = 0;
    for row in rows(chat) {
        let id = text(row, "id").to_owned();
        if let Some(previous) = seen.insert(id.clone(), row.clone()) {
            compared += 1;
            assert_eq!(
                row, &previous,
                "decoration changed for {id} at a window edge"
            );
        }
    }
    compared
}
fn traverse(model: &mut Model, id: &str, n: usize) -> HashMap<String, Json> {
    let tail = model.chat(id, "", "", "");
    assert_eq!(rows(&tail).len(), n.min(K));
    assert_eq!(tail["hasEarlier"], n > K);
    assert_eq!(tail["hasLater"], false);
    let mut seen = HashMap::new();
    let mut current = tail.clone();
    let mut steps = 0;
    loop {
        remember(&current, &mut seen);
        if current["hasEarlier"] == false {
            break;
        }
        let next = model.chat(id, text(&current, "earlier"), "", "");
        assert_ne!(next["earlier"], current["earlier"]);
        current = next;
        steps += 1;
        assert!(steps < n);
    }
    assert_eq!(rows(&current)[0]["id"], format!("{id}-1"));
    loop {
        remember(&current, &mut seen);
        if current["hasLater"] == false {
            break;
        }
        let next = model.chat(id, text(&current, "later"), "", "");
        assert_ne!(next["later"], current["later"]);
        current = next;
        steps += 1;
        assert!(steps < n);
    }
    assert_eq!(
        rows(&current),
        &rows(&tail)[rows(&tail).len() - rows(&current).len()..]
    );
    assert_eq!(seen.len(), n);
    seen
}

// Six windows per thread cover head/middle/tail overlap without a 25k walk.
fn sample_overlaps(model: &mut Model, id: &str) -> usize {
    let mut total = 0;
    for (region, cursor, direction) in [
        ("head", "0:before-first", "later"),
        ("middle", "12500:before-first", "later"),
        ("tail", "", "earlier"),
    ] {
        let first = model.chat(id, cursor, "", "");
        let adjacent = model.chat(id, text(&first, direction), "", "");
        assert_ne!(first["earlier"], adjacent["earlier"]);
        let mut seen = HashMap::new();
        remember(&first, &mut seen);
        let compared = remember(&adjacent, &mut seen);
        assert!(
            compared >= K / 2,
            "{id} {region}: only {compared} overlapping rows"
        );
        eprintln!(
            "N=25000 {id} {region}: compared all decorated fields of {compared} overlapping rows"
        );
        total += compared;
    }
    total
}

#[test]
fn bounded_bytecode_answers_round_trip_and_keep_decoration_at_25_1000_25000() {
    let mut model = Model::new();
    let mut day_model = Model::new();
    let mut weekend = 5;
    let mut dad = 3;
    for n in [25, 1_000, 25_000] {
        let seeded = Instant::now();
        model.grow("weekend", weekend, n);
        day_model.grow("dad", dad, n);
        weekend = n;
        dad = n;
        eprintln!(
            "seed two threads to N={n} through sources: {:?}",
            seeded.elapsed()
        );
        let compared_rows;
        if n <= 1_000 {
            let group = traverse(&mut model, "weekend", n);
            assert_eq!(group["weekend-3"]["senderName"], "Alex Rivera");
            assert_eq!(group["weekend-3"]["showSender"], true);
            assert_eq!(group["weekend-3"]["tail"], false);
            assert_eq!(group["weekend-4"]["showSender"], false);
            assert!(group.values().any(|row| row["reaction"] == "❤️"));
            assert!(group.values().any(|row| row["sender"] == "alex"
                && row["id"] != "weekend-3"
                && row["id"] != "weekend-4"));
            // Fixture calendar labels intentionally never advance. Dad is the
            // source-authored Yesterday -> Today boundary; Weekend has senders.
            let days = traverse(&mut day_model, "dad", n);
            compared_rows = group.len() + days.len();
            assert_eq!(days["dad-1"]["timeLabel"], "Yesterday 9:20 AM");
            assert_eq!(days.values().filter(|r| r["timeLabel"] != "").count(), 2);
            if n == 1_000 {
                let head = day_model.chat("dad", "0:before-first", "", "");
                // Sends have consecutive order keys after the three Yesterday
                // fixtures. Center on row 103 to put Today's first row at 0.
                let order = text(&head, "later")
                    .split_once(':')
                    .unwrap()
                    .0
                    .parse::<u64>()
                    .unwrap()
                    - 96;
                let cursor = format!("{order}:{}", text(&rows(&head)[103], "id"));
                let boundary = day_model.chat("dad", &cursor, "", "");
                assert_eq!(rows(&boundary)[0]["body"], "row-00003");
                assert!(text(&rows(&boundary)[0], "timeLabel").starts_with("Today "));
                for row in rows(&boundary) {
                    assert_eq!(row, &days[text(row, "id")]);
                }
            }
        } else {
            compared_rows =
                sample_overlaps(&mut model, "weekend") + sample_overlaps(&mut day_model, "dad");
        }
        assert!(
            compared_rows > 0,
            "N={n}: no overlapping-window decoration checks ran"
        );
        let reply = model.chat("weekend", "0:before-first", "weekend-1", "");
        assert_eq!(reply["replies"][0]["id"], "weekend-1");
        assert_eq!(
            rows(&reply)[0]["replyCount"].as_f64().unwrap() as usize,
            reply["replies"].as_array().unwrap().len() - 1
        );
        assert!(reply["replies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["replyRoot"] == "weekend-1"));
    }
}

fn timed_answer(model: &mut Model) -> (Duration, usize) {
    let args = chat_args("weekend", "", "", "");
    let began = Instant::now();
    let Answer::Now(value) = model
        .module
        .answer(&mut model.store, "conversation", &args)
        .unwrap()
    else {
        panic!("unexpected async answer");
    };
    let elapsed = began.elapsed();
    let json = to_json(&value, &model.shapes["conversation"]).unwrap();
    assert_eq!(rows(&json).len(), K);
    (elapsed, serde_json::to_vec(&json).unwrap().len())
}

#[test]
fn answer_work_stays_bounded_at_1000_and_25000() {
    let mut small = Model::new();
    let mut large = Model::new();
    small.grow("weekend", 5, 1_000);
    large.grow("weekend", 5, 25_000);
    // Warm both live engines, then interleave to expose each to the same load.
    // Measure the typed crossing; JSON rendering is outside the timed region.
    for _ in 0..3 {
        timed_answer(&mut small);
        timed_answer(&mut large);
    }
    let mut small_times = Vec::new();
    let mut large_times = Vec::new();
    let (mut small_bytes, mut large_bytes) = (0, 0);
    for _ in 0..9 {
        let (elapsed, bytes) = timed_answer(&mut small);
        small_times.push(elapsed);
        small_bytes = bytes;
        let (elapsed, bytes) = timed_answer(&mut large);
        large_times.push(elapsed);
        large_bytes = bytes;
    }
    let small_min = *small_times.iter().min().unwrap();
    let large_min = *large_times.iter().min().unwrap();
    eprintln!("conversation N=1000: minimum={small_min:?}, JSON bytes={small_bytes}, samples={small_times:?}");
    eprintln!("conversation N=25000: minimum={large_min:?}, JSON bytes={large_bytes}, samples={large_times:?}");
    eprintln!(
        "warm answer minimum ratio 25000/1000: {:.3}",
        large_min.as_secs_f64() / small_min.as_secs_f64()
    );
    assert!(
        large_min < small_min * 3,
        "answer work grew with the thread: {large_min:?} >= 3 * {small_min:?}"
    );
}

#[test]
fn arrivals_append_after_deleting_a_cursor_anchor_and_all_later_rows() {
    let mut model = Model::new();
    model.grow("weekend", 5, 1_000);
    model.send(
        "weekend",
        "Schedule an incoming reply",
        "weekend-1",
        30_000.,
    );
    let tail = model.chat("weekend", "", "", "");
    let cursor = text(&tail, "earlier");
    assert!(!cursor.is_empty());
    let deleted: Vec<_> = rows(&tail).iter().map(|row| text(row, "id")).collect();
    model.delete("weekend", &deleted.join("|"));
    let history = model.chat("weekend", cursor, "", "");
    assert_eq!(history["hasLater"], false);
    assert_eq!(rows(&history).len(), K / 2 + 1);
    assert!(rows(&history)
        .iter()
        .all(|row| !deleted.contains(&text(row, "id"))));
    model.call(
        "advanceReplies",
        vec![
            Value::Number(30_020.),
            Value::str("weekend"),
            Value::Number(30_020_000.),
        ],
    );
    let arrived = model.chat("weekend", cursor, "", "");
    assert_eq!(
        rows(&arrived)[0]["id"],
        rows(&history)[0]["id"],
        "arrival moved the first row after the cursor became past-end"
    );
    assert_eq!(rows(&arrived).len(), rows(&history).len() + 1);
    for (position, previous) in rows(&history).iter().enumerate() {
        assert_eq!(rows(&arrived)[position]["id"], previous["id"]);
    }
    let incoming = rows(&arrived).last().unwrap();
    assert!(text(incoming, "id").starts_with("received-"));
    assert_eq!(incoming["replyRoot"], "weekend-1");
    assert_eq!(incoming["outgoing"], false);
}

#[test]
fn deleted_cursors_selection_recovery_and_reply_indexes_use_surviving_rows() {
    let mut model = Model::new();
    model.grow("weekend", 5, 1_000);
    let tail = model.chat("weekend", "", "", "");
    let middle = model.chat("weekend", text(&tail, "earlier"), "", "");
    let anchor = text(&middle, "earlier");
    let deleted = text(&rows(&middle)[0], "id");
    let centered = model.chat("weekend", anchor, "", "");
    let predecessor = text(&rows(&centered)[K / 2 - 1], "id");
    model.delete("weekend", deleted);
    let resolved = model.chat("weekend", anchor, "", "");
    assert_eq!(rows(&resolved)[K / 2]["id"], predecessor);
    assert!(!rows(&resolved).iter().any(|m| m["id"] == deleted));
    assert_eq!(
        rows(&model.chat("weekend", "9007199254740991:past-end", "", "")),
        &rows(&tail)[K - (K / 2 + 1)..]
    );
    for invalid in [
        "bad",
        "-1",
        " 22",
        "22 ",
        "2.5",
        "1e3",
        "+1",
        "NaN",
        "Infinity",
        "9007199254740992",
        "200:",
        ":id",
        "-1:id",
        " 22:id",
        "22 :id",
        "2.5:id",
        "1e3:id",
        "+1:id",
        "NaN:id",
        "Infinity:id",
        "9007199254740992:id",
    ] {
        let error = model
            .try_call("conversation", chat_args("weekend", invalid, "", ""))
            .unwrap_err();
        assert!(format!("{error:?}").contains("Invalid conversation cursor"));
    }
    let last = rows(&tail).last().unwrap();
    let selection = format!(
        "{}|weekend-1|weekend-3|{}|missing|{}",
        text(last, "id"),
        deleted,
        text(last, "id")
    );
    let selected = model.chat("weekend", "500:before-first", "", &selection);
    assert_eq!(selected["selectionCount"], 3.);
    assert_eq!(
        selected["selectedText"],
        format!(
            "Anyone up for a hike on Saturday?\nWho’s bringing snacks?\n{}",
            text(last, "body")
        )
    );
    assert!(rows(&selected).iter().all(|m| m["chosen"] == false));
    for row in rows(&selected) {
        assert!(text(row, "selection").contains("weekend-1"));
        assert!(!text(row, "selection").contains(deleted));
    }
    let first = model.chat("weekend", "0:before-first", "", &selection);
    assert_eq!(rows(&first)[0]["chosen"], true);
    assert!(!text(&rows(&first)[0], "selection").contains("weekend-1"));
    model.recover("weekend");
    let restored = model.chat("weekend", anchor, "", "");
    assert_eq!(rows(&restored)[K / 2]["id"], deleted);
    let before = model.chat("weekend", "0:before-first", "weekend-1", "");
    model.delete("weekend", "weekend-1");
    let orphan = model.send("weekend", "Reply after root deletion", "weekend-1", 40_000.);
    let replies = model.chat("weekend", "", "weekend-1", "");
    assert_eq!(
        replies["replies"].as_array().unwrap().len(),
        before["replies"].as_array().unwrap().len()
    );
    assert_eq!(
        replies["replies"].as_array().unwrap().last().unwrap()["reply"],
        "Anyone up for a hike on Saturday?"
    );
    model.recover("weekend");
    let recovered = model.chat("weekend", "0:before-first", "weekend-1", "");
    assert_eq!(
        rows(&recovered)[0]["replyCount"].as_f64().unwrap(),
        rows(&before)[0]["replyCount"].as_f64().unwrap() + 1.
    );
    model.delete("weekend", &orphan);
    assert_eq!(
        model.chat("weekend", "0:before-first", "weekend-1", "")["replies"],
        before["replies"]
    );
    model.call(
        "deleteConversation",
        vec![Value::str("weekend"), Value::Number(0.)],
    );
    let empty = model.chat("weekend", anchor, "weekend-1", &selection);
    assert!(rows(&empty).is_empty());
    assert_eq!(empty["replies"], serde_json::json!([]));
    assert_eq!(empty["selectionCount"], 0.);
    assert_eq!(empty["earlier"], "");
    assert_eq!(empty["later"], "");
    assert_eq!(empty["hasEarlier"], false);
    assert_eq!(empty["hasLater"], false);
    model.recover("weekend");
    let final_chat = model.chat("weekend", "0:before-first", "weekend-1", "");
    assert_eq!(rows(&final_chat).len(), K);
    assert_eq!(
        final_chat["replies"].as_array().unwrap().len(),
        before["replies"].as_array().unwrap().len() + 1
    );
}

fn runner_chat(runner: &Runner<Module>) -> Json {
    let state: Json = serde_json::from_str(&exact_runner::agent::state(runner)).unwrap();
    state["resources"]["chat"].clone()
}

#[test]
fn contract_shifts_keep_history_and_send_open_and_links_reset_to_latest() {
    let mut model = Model::new();
    model.grow("weekend", 5, 1_000);
    let mut runner = Runner::boot(
        model.plan,
        model.module,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/t/weekend",
    )
    .unwrap();
    runner
        .act("write", vec![Value::str("Schedule an arrival")])
        .unwrap();
    runner.act("sendDraft", vec![]).unwrap();
    let tail = runner_chat(&runner);
    runner
        .act("shiftWindow", vec![Value::str(text(&tail, "earlier"))])
        .unwrap();
    assert_eq!(runner_chat(&runner)["hasLater"], true);
    // Return toward the tail without choosing latest. This partial window
    // must append the pending arrival, never drop a row from its beginning.
    for _ in 0..2 {
        let current = runner_chat(&runner);
        runner
            .act("shiftWindow", vec![Value::str(text(&current, "later"))])
            .unwrap();
    }
    let history = runner_chat(&runner);
    assert_eq!(history["hasLater"], false);
    assert!(rows(&history).len() < K);
    let cursor = runner.slot("cursor").unwrap().clone();
    assert_ne!(cursor, Value::str(""));
    for _ in 0..20 {
        runner.act("tick", vec![]).unwrap();
    }
    let arrived = runner_chat(&runner);
    assert_eq!(arrived["earlier"], history["earlier"]);
    assert_eq!(rows(&arrived).len(), rows(&history).len() + 1);
    for (position, previous) in rows(&history).iter().enumerate() {
        assert_eq!(rows(&arrived)[position]["id"], previous["id"]);
    }
    assert!(text(rows(&arrived).last().unwrap(), "id").starts_with("received-"));
    assert_eq!(runner.derive("pendingReplies"), Some(&Value::Bool(false)));
    assert_eq!(runner.slot("cursor"), Some(&cursor));
    runner
        .act("write", vec![Value::str("Sent while reading history")])
        .unwrap();
    runner.act("sendDraft", vec![]).unwrap();
    assert_eq!(runner.slot("cursor"), Some(&Value::str("")));
    let sent = runner_chat(&runner);
    assert_eq!(sent["hasLater"], false);
    assert_eq!(
        rows(&sent).last().unwrap()["body"],
        "Sent while reading history"
    );
    runner
        .act("shiftWindow", vec![Value::str(text(&sent, "earlier"))])
        .unwrap();
    runner.act("shiftWindow", vec![Value::str("")]).unwrap();
    assert_eq!(rows(&runner_chat(&runner)), rows(&sent));
    runner
        .act("shiftWindow", vec![Value::str(text(&sent, "earlier"))])
        .unwrap();
    runner
        .dispatch(runner.roots()[0], Event::Navigate("/t/dad".into()))
        .unwrap();
    assert_eq!(runner.slot("cursor"), Some(&Value::str("")));
    runner
        .act(
            "open",
            vec![Value::str("weekend"), Value::str(""), Value::str("")],
        )
        .unwrap();
    assert_eq!(runner.slot("cursor"), Some(&Value::str("")));
    assert_eq!(rows(&runner_chat(&runner)), rows(&sent));
    runner
        .act("shiftWindow", vec![Value::str(text(&sent, "earlier"))])
        .unwrap();
    runner
        .act(
            "open",
            vec![Value::str("maya"), Value::str(""), Value::str("")],
        )
        .unwrap();
    assert_eq!(runner.slot("cursor"), Some(&Value::str("")));
    assert_eq!(runner_chat(&runner)["id"], "maya");
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn open(root: &Directory) -> Model {
    let mut module = native::module(BYTECODE, APP, GRANTS);
    module
        .configure_storage(
            root.0.join("data"),
            root.0.join("cache"),
            root.0.join("tmp"),
        )
        .unwrap();
    Model::with_module(module)
}

#[path = "window/deleted.rs"]
mod deleted;

fn replica_fixture(label: &str, entries: Vec<(String, u64, bool)>) -> Directory {
    replica_fixture_with_delivery(label, entries, "Delivered")
}
fn replica_fixture_with_delivery(
    label: &str,
    entries: Vec<(String, u64, bool)>,
    delivery: &str,
) -> Directory {
    record_fixture(label, |queued| {
        let template = queued
            .iter()
            .flat_map(|entry| entry["args"]["payloads"].as_array().unwrap())
            .find(|payload| payload["kind"] == "message" && payload["conversation"] == "maya")
            .unwrap()
            .clone();
        let payloads: Vec<_> = entries
            .iter()
            .map(|(id, order, outgoing)| {
                let mut row = template.clone();
                row["message"]["id"] = id.as_str().into();
                row["message"]["body"] = format!("Body {id}").into();
                row["message"]["order"] = (*order).into();
                row["message"]["outgoing"] = (*outgoing).into();
                row["message"]["sender"] = if *outgoing { "me" } else { "maya" }.into();
                row["message"]["delivery"] = if *outgoing { delivery } else { "" }.into();
                row["message"]["replyRoot"] = if *outgoing { "m9" } else { id.as_str() }.into();
                row["expires"] = Json::Null;
                row
            })
            .collect();
        // snapshot encodes the message ID in the key; the replica then encodes
        // the whole key in its record ID. These fixture IDs contain only ASCII
        // letters, digits, hyphens and colons.
        let keys: Vec<_> = entries
            .iter()
            .map(|(id, _, _)| format!("message:maya:{}", id.replace(':', "%3A")))
            .collect();
        keys.into_iter().zip(payloads).collect()
    })
}

fn record_fixture(label: &str, records: impl FnOnce(&[Json]) -> Vec<(String, Json)>) -> Directory {
    let root = Directory(
        std::env::temp_dir().join(format!("messages-window-{label}-{}", std::process::id())),
    );
    std::fs::create_dir(&root.0).unwrap();
    let mut model = open(&root);
    model.chat("maya", "", "", "");
    drop(model);
    // Write offline-device-shaped records through the real native replica,
    // then reopen the shipped bytecode so its normal restore builds indexes.
    let mut core = exact_snapback4::Module::new(APP, GRANTS).unwrap();
    core.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("tmp"),
    )
    .unwrap();
    let path = GRANTS
        .lines()
        .find_map(|line| line.strip_prefix("sqlite.open "))
        .unwrap();
    core.call(&serde_json::json!({"op":"open", "path":path,
        "origin":"http://127.0.0.1:4400", "viewer":"dev:alice"}))
        .unwrap();
    // An unacquired device holds intents, not server facts. Seed from the
    // app's durable outbox and admit this batch just as an offline edit does.
    let queued = core.call(&serde_json::json!({"op":"queued"})).unwrap();
    let queued = queued["ok"].as_array().unwrap();
    let (keys, payloads): (Vec<_>, Vec<_>) = records(queued).into_iter().unzip();
    let record_ids: Vec<_> = keys
        .iter()
        .map(|key| format!("dev:alice:{}", key.replace('%', "%25").replace(':', "%3A")))
        .collect();
    let counter = core
        .call(&serde_json::json!({"op":"meta", "key":"exact:counter"}))
        .unwrap()["ok"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let seq = counter + 1;
    let result = core
        .call(&serde_json::json!({"op":"admit", "entry":{
            "id":format!("window-fixture:{label}"), "seq":seq, "op":"putRecords",
            "viewer":"dev:alice", "now":0, "new_ids":[], "predicted":[], "predictable":true,
            "args":{
                "recordIds":record_ids,
                "keys":keys,
                "payloads":payloads
            }
        }}))
        .unwrap();
    assert!(result.get("ok").is_some(), "{result}");
    assert!(result["ok"].get("denied").is_none(), "{result}");
    let saved = core
        .call(&serde_json::json!({"op":"set_meta", "key":"exact:counter", "value":seq.to_string()}))
        .unwrap();
    assert!(saved.get("ok").is_some(), "{saved}");
    drop(core);
    root
}

#[test]
fn equal_orders_survive_restore_delete_recover_with_consistent_reply_receipts() {
    fn inspect(model: &mut Model, expected: &[&str]) {
        let chat = model.chat("maya", "", "m9", "tie-z|tie-a");
        let tied = |rows: &[Json]| {
            rows.iter()
                .filter_map(|row| {
                    let id = text(row, "id");
                    matches!(id, "tie-a" | "tie-z").then_some(id.to_owned())
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(tied(rows(&chat)), expected);
        assert_eq!(tied(chat["replies"].as_array().unwrap()), expected);
        let last = *expected.last().unwrap();
        for transcript in [rows(&chat), chat["replies"].as_array().unwrap()] {
            let receipts: Vec<_> = transcript.iter().filter(|m| m["delivery"] != "").collect();
            assert_eq!(receipts.len(), 1);
            assert_eq!(receipts[0]["id"], last);
            assert_eq!(receipts[0]["delivery"], "Delivered");
        }
        let bodies: Vec<_> = expected.iter().map(|id| format!("Body {id}")).collect();
        assert_eq!(chat["selectedText"], bodies.join("\n"));
        assert_eq!(
            chat["selectionCount"].as_f64().unwrap(),
            expected.len() as f64
        );
        assert_eq!(rows(&chat).last().unwrap()["id"], "later-incoming");
    }
    let root = replica_fixture(
        "order",
        vec![
            ("tie-z".into(), 200, true),
            ("later-incoming".into(), 201, false),
            ("tie-a".into(), 200, true),
        ],
    );
    let mut model = open(&root);
    inspect(&mut model, &["tie-a", "tie-z"]);
    // Recover the earlier tie after its sibling: the main thread inserts it
    // before that sibling, and the root index and receipt must agree.
    model.delete("maya", "tie-a");
    inspect(&mut model, &["tie-z"]);
    model.recover("maya");
    inspect(&mut model, &["tie-a", "tie-z"]);
    model.delete("maya", "tie-z");
    inspect(&mut model, &["tie-a"]);
    model.recover("maya");
    inspect(&mut model, &["tie-a", "tie-z"]);
    drop(model);
    inspect(&mut open(&root), &["tie-a", "tie-z"]);
}

#[test]
fn cursors_traverse_and_resolve_deleted_anchors_inside_large_order_ties() {
    let root = replica_fixture(
        "cursor-ties",
        (0..450)
            .map(|i| (format!("tie:{i:03}:device"), 200, true))
            .collect(),
    );
    let mut model = open(&root);
    let tail = model.chat("maya", "", "", "");
    let mut current = tail.clone();
    let mut seen = HashMap::new();
    for direction in ["earlier", "later"] {
        let flag = if direction == "earlier" {
            "hasEarlier"
        } else {
            "hasLater"
        };
        let mut cursors = std::collections::HashSet::new();
        loop {
            remember(&current, &mut seen);
            if current[flag] == false {
                break;
            }
            let cursor = text(&current, direction).to_owned();
            assert!(
                cursors.insert(cursor.clone()),
                "{direction} repeated cursor {cursor:?} while {flag}=true"
            );
            current = model.chat("maya", &cursor, "", "");
        }
        if direction == "earlier" {
            assert_eq!(rows(&current)[0]["id"], "m1");
        }
    }
    assert_eq!(seen.len(), 460);
    assert_eq!(rows(&current).last(), rows(&tail).last());
    let cursor = text(&tail, "earlier");
    let anchor = text(&rows(&tail)[0], "id");
    let centered = model.chat("maya", cursor, "", "");
    let predecessor = text(&rows(&centered)[K / 2 - 1], "id");
    assert_eq!(rows(&centered)[K / 2]["id"], anchor);
    model.delete("maya", anchor);
    let resolved = model.chat("maya", cursor, "", "");
    assert_eq!(rows(&resolved)[K / 2]["id"], predecessor);
    assert!(!rows(&resolved).iter().any(|row| row["id"] == anchor));
    model.recover("maya");
    assert_eq!(
        rows(&model.chat("maya", cursor, "", ""))[K / 2]["id"],
        anchor
    );
}

#[test]
fn idle_reply_ticks_preserve_receipts_replies_and_refused_save_retry() {
    let root = replica_fixture(
        "reply-retry",
        (0..513)
            .map(|i| (format!("receipt-{i:03}"), 200 + i, true))
            .collect(),
    );
    let mut model = open(&root);
    let tick = |now| {
        vec![
            Value::Number(now),
            Value::str("maya"),
            Value::Number(now * 1000.),
        ]
    };
    // Idle before sending, then while the reply is waiting to start.
    model.call("advanceReplies", tick(100.));
    model.send("maya", "Keep the receipt and reply", "m9", 100.);
    let delivered = model.chat("maya", "", "", "");
    let sent_id = text(rows(&delivered).last().unwrap(), "id").to_owned();
    assert_eq!(rows(&delivered).last().unwrap()["delivery"], "Delivered");
    for now in [100., 101., 102.999] {
        model.call("advanceReplies", tick(now));
        assert_eq!(
            model.chat("maya", "", "", "")["messages"],
            delivered["messages"]
        );
    }
    // The first receipt crosses Snapback's real 512-record edit limit. Its
    // refused save must restore the old clock and keep the pending reply.
    assert!(model.try_call("advanceReplies", tick(103.)).is_err());
    assert_eq!(
        model.chat("maya", "", "", "")["messages"],
        delivered["messages"]
    );
    let remove = (0..20)
        .map(|i| format!("receipt-{i:03}"))
        .collect::<Vec<_>>()
        .join("|");
    model.delete("maya", &remove);
    model.call("advanceReplies", tick(103.));
    let read = model.chat("maya", "", "", "");
    assert_eq!(rows(&read).last().unwrap()["delivery"], "Read");
    assert!(!text(&read, "typingName").is_empty());
    for now in [103., 104., 114.999] {
        model.call("advanceReplies", tick(now));
        assert_eq!(model.chat("maya", "", "", "")["messages"], read["messages"]);
    }
    model.call("advanceReplies", tick(115.));
    let received = model.chat("maya", "", "", "");
    let reply = rows(&received).last().unwrap();
    assert_eq!(reply["outgoing"], false);
    assert_eq!(reply["replyRoot"], "m9");
    assert!(text(reply, "id").starts_with("received-"));
    assert_eq!(text(&received, "typingName"), "");
    for now in [115., 116., 130.] {
        model.call("advanceReplies", tick(now));
        assert_eq!(
            model.chat("maya", "", "", "")["messages"],
            received["messages"]
        );
    }
    drop(model);
    let reopened = open(&root).chat("maya", "", "", "");
    assert_eq!(reopened["messages"], received["messages"]);
    assert_eq!(
        rows(&reopened)
            .iter()
            .find(|m| text(m, "id") == sent_id)
            .unwrap()["delivery"],
        "Read"
    );
}

#[test]
fn recovery_expiry_without_revision_change_is_still_durable() {
    let root = replica_fixture("expiry", vec![("expiring".into(), 200, false)]);
    let mut model = open(&root);
    model.delete("maya", "expiring");
    let deleted = |now| {
        vec![
            Value::str(""),
            Value::Number(0.),
            Value::Number(now),
            Value::str(""),
        ]
    };
    assert_eq!(
        model.call("recentlyDeleted", deleted(0.))["count"].as_f64(),
        Some(1.)
    );
    assert_eq!(
        model.call("recentlyDeleted", deleted(31. * 86400000.))["count"].as_f64(),
        Some(0.)
    );
    drop(model);
    // Inspect with the earlier clock so a missed durable removal cannot be
    // hidden by expiring the same row again during this read.
    assert_eq!(
        open(&root).call("recentlyDeleted", deleted(0.))["count"].as_f64(),
        Some(0.)
    );
}

#[test]
fn oversized_conversation_delete_keeps_every_record_and_allows_a_later_edit() {
    fn history(model: &mut Model) -> std::collections::BTreeSet<String> {
        let mut ids = std::collections::BTreeSet::new();
        let mut cursor = String::new();
        for _ in 0..16 {
            let chat = model.chat("maya", &cursor, "", "");
            ids.extend(rows(&chat).iter().map(|row| text(row, "id").to_owned()));
            cursor = text(&chat, "earlier").to_owned();
            if chat["hasEarlier"] == false {
                return ids;
            }
        }
        panic!("history traversal did not reach the first page");
    }
    let root = replica_fixture(
        "bulk-delete",
        (0..600)
            .map(|i| (format!("bulk-{i:03}"), 200 + i, false))
            .collect(),
    );
    let mut model = open(&root);
    let all = history(&mut model);
    assert_eq!(all.len(), 610);
    let before = model.chat("maya", "", "", "");
    assert!(model
        .try_call(
            "deleteConversation",
            vec![Value::str("maya"), Value::Number(0.)]
        )
        .is_err());
    assert_eq!(
        model.chat("maya", "", "", "")["messages"],
        before["messages"]
    );
    assert_eq!(history(&mut model), all);
    let recovery = model.call(
        "recentlyDeleted",
        vec![
            Value::str(""),
            Value::Number(0.),
            Value::Number(0.),
            Value::str(""),
        ],
    );
    assert_eq!(recovery["count"].as_f64(), Some(0.));
    model.call(
        "saveDraft",
        vec![
            Value::str("maya"),
            Value::str("After refused delete"),
            Value::str("m9"),
        ],
    );
    drop(model);
    let mut model = open(&root);
    assert_eq!(
        model.chat("maya", "", "", "")["messages"],
        before["messages"]
    );
    assert_eq!(history(&mut model), all);
    let draft = model.call(
        "conversationDraft",
        vec![Value::str("maya"), Value::Number(0.)],
    );
    assert_eq!(draft["draft"], "After refused delete");
    assert_eq!(draft["reply"], "m9");
}

#[test]
fn receipt_index_tracks_delete_recovery_and_reopen_without_touching_read_history() {
    let root = replica_fixture_with_delivery(
        "read-history",
        (0..1000)
            .map(|i| (format!("read-{i:04}"), 200 + i, true))
            .collect(),
        "Read",
    );
    let mut model = open(&root);
    let tick = |now| {
        vec![
            Value::Number(now),
            Value::str("maya"),
            Value::Number(now * 1000.),
        ]
    };
    model.send("maya", "Recover this delivered message", "m9", 0.);
    let sent = model.chat("maya", "", "m9", "");
    let id = text(rows(&sent).last().unwrap(), "id").to_owned();
    model.delete("maya", &id);
    model.call("advanceReplies", tick(3.));
    model.recover("maya");
    let recovered = model.chat("maya", "", "m9", "");
    assert_eq!(rows(&recovered).last().unwrap()["id"], id);
    assert_eq!(
        rows(&recovered).last().unwrap()["delivery"],
        "Delivered",
        "deleted messages must not receive a read receipt while archived"
    );
    // Reopen with the recovered Delivered message: rebuilding the derived
    // index must retain it even though the in-memory pending reply is gone.
    drop(model);
    let mut model = open(&root);
    model.send("maya", "A second reply root", "m10", 10.);
    model.call("advanceReplies", tick(13.));
    let chat = model.chat("maya", "", "m9", "");
    assert_eq!(rows(&chat).last().unwrap()["delivery"], "Read");
    let reply = chat["replies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| text(r, "id") == id)
        .unwrap();
    assert_eq!(reply["delivery"], "Read");
    let saved = chat["messages"].clone();
    drop(model);
    let mut model = open(&root);
    let chat = model.chat("maya", "", "m9", "");
    assert_eq!(chat["messages"], saved);
    assert_eq!(
        chat["replies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| text(r, "id") == id)
            .unwrap()["delivery"],
        "Read"
    );
    model.send("maya", "Only this message needs a receipt", "", 20.);
    model.call("advanceReplies", tick(23.));
    assert_eq!(
        rows(&model.chat("maya", "", "", "")).last().unwrap()["delivery"],
        "Read"
    );
}

fn people_fixture(label: &str, count: usize) -> Directory {
    people_fixture_at(label, count, 6.)
}

fn people_fixture_at(label: &str, count: usize, first_position: f64) -> Directory {
    record_fixture(label, |queued| {
        let template = queued
            .iter()
            .flat_map(|entry| entry["args"]["payloads"].as_array().unwrap())
            .find(|row| row["kind"] == "person" && row["person"]["id"] == "maya")
            .unwrap();
        (0..count)
            .map(|i| {
                let id = format!("person-{i:05}");
                let mut row = template.clone();
                row["person"]["id"] = id.clone().into();
                row["person"]["name"] = format!("Person {i}").into();
                row["person"]["address"] = format!("person{i}@example.test").into();
                row["position"] = (first_position + i as f64).into();
                row["conversation"] = false.into();
                (format!("person:{id}"), row)
            })
            .collect()
    })
}

#[test]
fn new_conversations_preserve_order_with_more_people_than_the_edit_cap() {
    let root = people_fixture("people-order", 1000);
    let mut model = open(&root);
    let inbox = |model: &mut Model| {
        model.call(
            "inbox",
            vec![Value::str(""), Value::Number(0.), Value::str("")],
        )
    };
    let order = |inbox: Json| {
        inbox["people"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| text(row, "id").to_owned())
            .collect::<Vec<_>>()
    };
    let original = order(inbox(&mut model));
    let first = "address:first%40example.test";
    let second = "address:second%40example.test";
    model.send(first, "New conversation", "", 0.);
    let mut expected = vec![first.to_owned()];
    expected.extend(original.clone());
    assert_eq!(order(inbox(&mut model)), expected);
    assert!(model
        .try_call(
            "sendMessage",
            vec![
                Value::str("address:refused%40example.test"),
                Value::str(&"🌲".repeat(20000)),
                Value::str(""),
                Value::Number(1.),
                Value::Number(1000.)
            ]
        )
        .is_err());
    assert_eq!(order(inbox(&mut model)), expected);
    model.send(second, "After refused prepend", "", 2.);
    expected.insert(0, second.to_owned());
    assert_eq!(order(inbox(&mut model)), expected);
    model.call(
        "createLocalContact",
        vec![
            Value::str("First"),
            Value::str("Renamed"),
            Value::str(""),
            Value::str(""),
            Value::str("first@example.test"),
            Value::str("Saved"),
        ],
    );
    assert_eq!(order(inbox(&mut model)), expected);
    drop(model);
    let mut model = open(&root);
    let restored = inbox(&mut model);
    assert_eq!(order(restored.clone()), expected);
    assert_eq!(
        restored["people"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| text(row, "id") == first)
            .unwrap()["name"],
        "First Renamed"
    );
    assert_eq!(
        rows(&model.chat(first, "", "", "")).last().unwrap()["body"],
        "New conversation"
    );
    assert_eq!(
        rows(&model.chat(second, "", "", "")).last().unwrap()["body"],
        "After refused prepend"
    );
}

#[test]
fn oversized_position_rebase_refuses_whole_and_allows_a_later_edit() {
    let root = people_fixture_at("people-extreme", 520, -f64::MAX);
    let mut model = open(&root);
    let inbox = |model: &mut Model| {
        model.call(
            "inbox",
            vec![Value::str(""), Value::Number(0.), Value::str("")],
        )
    };
    let original = inbox(&mut model);
    let error = model
        .try_call(
            "sendMessage",
            vec![
                Value::str("address:refused-rebase%40example.test"),
                Value::str("Refuse atomically"),
                Value::str(""),
                Value::Number(0.),
                Value::Number(0.),
            ],
        )
        .unwrap_err();
    assert!(format!("{error:?}").contains("512"));
    assert_eq!(inbox(&mut model)["people"], original["people"]);
    model.call(
        "saveDraft",
        vec![
            Value::str("maya"),
            Value::str("After refused rebase"),
            Value::str(""),
        ],
    );
    let saved = inbox(&mut model);
    drop(model);
    let mut model = open(&root);
    assert_eq!(inbox(&mut model)["people"], saved["people"]);
    let draft = model.call(
        "conversationDraft",
        vec![Value::str("maya"), Value::Number(0.)],
    );
    assert_eq!(draft["draft"], "After refused rebase");
}

#[test]
fn recovery_edits_skip_unrelated_archive_and_oversized_expiry_refuses_whole() {
    let root = record_fixture("recovery-footprint", |queued| {
        let template = queued
            .iter()
            .flat_map(|entry| entry["args"]["payloads"].as_array().unwrap())
            .find(|row| row["kind"] == "message" && row["conversation"] == "sam")
            .unwrap();
        (0..1000)
            .map(|i| {
                let mut row = template.clone();
                let id = format!("archived-{i}");
                row["message"]["id"] = id.clone().into();
                row["message"]["order"] = (i + 100).into();
                row["message"]["replyRoot"] = id.clone().into();
                row["expires"] = 2592000000u64.into();
                (format!("message:sam:{id}"), row)
            })
            .collect()
    });
    let mut model = open(&root);
    let archived = |model: &mut Model| {
        model.call(
            "recentlyDeleted",
            vec![
                Value::str(""),
                Value::Number(0.),
                Value::Number(0.),
                Value::str(""),
            ],
        )
    };
    assert_eq!(archived(&mut model)["count"], 1000.);
    model.delete("maya", "m1");
    assert_eq!(archived(&mut model)["count"], 1001.);
    model.recover("maya");
    assert_eq!(archived(&mut model)["count"], 1000.);
    model.delete("maya", "m2");
    model.call(
        "purgeConversations",
        vec![Value::str("maya"), Value::Number(0.)],
    );
    assert_eq!(archived(&mut model)["count"], 1000.);
    let error = model
        .try_call(
            "recentlyDeleted",
            vec![
                Value::str(""),
                Value::Number(0.),
                Value::Number(2592000000.),
                Value::str(""),
            ],
        )
        .unwrap_err();
    assert!(format!("{error:?}").contains("512"));
    assert_eq!(archived(&mut model)["count"], 1000.);
    let error = model
        .try_call(
            "purgeConversations",
            vec![Value::str("sam"), Value::Number(0.)],
        )
        .unwrap_err();
    assert!(format!("{error:?}").contains("512"));
    assert_eq!(archived(&mut model)["count"], 1000.);
    model.call(
        "saveDraft",
        vec![
            Value::str("maya"),
            Value::str("After refused expiry"),
            Value::str(""),
        ],
    );
    drop(model);
    let mut model = open(&root);
    assert_eq!(archived(&mut model)["count"], 1000.);
    let chat = model.chat("maya", "", "", "");
    assert!(rows(&chat).iter().any(|row| text(row, "id") == "m1"));
    assert!(!rows(&chat).iter().any(|row| text(row, "id") == "m2"));
    assert_eq!(
        model.call(
            "conversationDraft",
            vec![Value::str("maya"), Value::Number(0.)]
        )["draft"],
        "After refused expiry"
    );
}

#[test]
fn contact_pages_keep_selection_drafts_and_reset_only_when_the_query_changes() {
    let mut model = Model::new();
    for cursor in ["not json", "[]", r#"["maya",0,-1]"#] {
        for (name, mut args) in [
            ("inbox", vec![Value::str(""), Value::Number(0.)]),
            (
                "recipients",
                vec![Value::str(""), Value::str(""), Value::Number(0.)],
            ),
        ] {
            args.push(Value::str(cursor));
            assert!(
                matches!(model.try_call(name, args), Err(DataError::BadArguments(message)) if message.contains("Invalid contact cursor"))
            );
        }
    }

    for i in 0..450 {
        model.send(
            &format!("address:contact-{i:03}%40example.test"),
            "Contact page",
            "",
            1_000_000.,
        );
    }
    let mut runner = Runner::boot(
        model.plan,
        model.module,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let state = |runner: &Runner<Module>| -> Json {
        serde_json::from_str(&exact_runner::agent::state(runner)).unwrap()
    };
    let first = state(&runner)["resources"]["inbox"].clone();
    assert_eq!(first["people"].as_array().unwrap().len(), K);
    assert_eq!(first["earlier"], "");
    runner
        .act("pageInbox", vec![Value::str(text(&first, "later"))])
        .unwrap();
    let second = state(&runner)["resources"]["inbox"].clone();
    runner.act("search", vec![Value::str("")]).unwrap();
    assert_eq!(
        state(&runner)["resources"]["inbox"]["people"],
        second["people"]
    );
    assert_eq!(second["people"].as_array().unwrap().len(), K);
    assert_ne!(first["people"][0]["id"], second["people"][0]["id"]);
    runner
        .act("pageInbox", vec![Value::str(text(&second, "later"))])
        .unwrap();
    let last = state(&runner)["resources"]["inbox"].clone();
    assert_eq!(last["people"].as_array().unwrap().len(), 56);
    assert_eq!(last["later"], "");
    runner
        .act("pageInbox", vec![Value::str(text(&last, "earlier"))])
        .unwrap();
    assert_eq!(
        state(&runner)["resources"]["inbox"]["people"],
        second["people"]
    );
    runner
        .act("search", vec![Value::str("contact-00")])
        .unwrap();
    assert_eq!(runner.slot("inboxCursor"), Some(&Value::str("")));
    assert_eq!(
        state(&runner)["resources"]["inbox"]["people"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    runner.act("newMessage", vec![]).unwrap();
    runner.act("browseContacts", vec![]).unwrap();
    runner
        .act("writeNew", vec![Value::str("Keep this draft")])
        .unwrap();
    let contacts = state(&runner)["resources"]["contacts"].clone();
    runner
        .act("pageRecipients", vec![Value::str(text(&contacts, "later"))])
        .unwrap();
    let cursor = runner.slot("recipientCursor").unwrap().clone();
    runner.act("searchRecipient", vec![Value::str("")]).unwrap();
    assert_eq!(runner.slot("recipientCursor"), Some(&cursor));
    for key in ["ArrowLeft", "Escape"] {
        runner.act("recipientKey", vec![Value::str(key)]).unwrap();
        assert_eq!(runner.slot("recipientCursor"), Some(&cursor));
    }
    let selected = state(&runner)["resources"]["contacts"]["people"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    runner
        .act("chooseRecipient", vec![Value::str(&selected)])
        .unwrap();
    assert_eq!(runner.slot("recipientCursor"), Some(&Value::str("")));
    assert_eq!(
        runner.slot("newDraft"),
        Some(&Value::str("Keep this draft"))
    );
    let contacts = state(&runner)["resources"]["contacts"].clone();
    assert_eq!(contacts["selected"][0]["id"], selected);
    runner.act("browseContacts", vec![]).unwrap();
    runner
        .act("pageRecipients", vec![Value::str(text(&contacts, "later"))])
        .unwrap();
    assert_eq!(
        state(&runner)["resources"]["contacts"]["selected"],
        contacts["selected"]
    );
    runner
        .act("searchRecipient", vec![Value::str("contact-44")])
        .unwrap();
    assert_eq!(runner.slot("recipientCursor"), Some(&Value::str("")));
    assert_eq!(
        runner.slot("newDraft"),
        Some(&Value::str("Keep this draft"))
    );
    assert_eq!(
        state(&runner)["resources"]["contacts"]["selected"],
        contacts["selected"]
    );
}

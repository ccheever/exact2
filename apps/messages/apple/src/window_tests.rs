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

const K: usize = 200;

struct Model {
    module: Module,
    plan: Plan,
    store: Store,
    shapes: HashMap<String, Shape>,
}
impl Model {
    fn new() -> Self {
        Self::with_module(Module::new(
            super::BYTECODE.to_vec(),
            super::APP,
            super::GRANTS,
        ))
    }
    fn with_module(mut module: Module) -> Self {
        let plan = Plan::decode(super::PLAN).unwrap();
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
            store: Store::new(super::GRANTS, Vec::<(String, String)>::new()),
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
fn remember(chat: &Json, seen: &mut HashMap<String, Json>) {
    for row in rows(chat) {
        let id = text(row, "id").to_owned();
        if let Some(previous) = seen.insert(id.clone(), row.clone()) {
            assert_eq!(
                row, &previous,
                "decoration changed for {id} at a window edge"
            );
        }
    }
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
            assert_eq!(days["dad-1"]["timeLabel"], "Yesterday 9:20 AM");
            assert_eq!(days.values().filter(|r| r["timeLabel"] != "").count(), 2);
            if n == 1_000 {
                let head = day_model.chat("dad", "0", "", "");
                // Sends have consecutive order keys after the three Yesterday
                // fixtures. Center on row 103 to put Today's first row at 0.
                let cursor = (text(&head, "later").parse::<u64>().unwrap() - 96).to_string();
                let boundary = day_model.chat("dad", &cursor, "", "");
                assert_eq!(rows(&boundary)[0]["body"], "row-00003");
                assert!(text(&rows(&boundary)[0], "timeLabel").starts_with("Today "));
                for row in rows(&boundary) {
                    assert_eq!(row, &days[text(row, "id")]);
                }
            }
        }
        let reply = model.chat("weekend", "0", "weekend-1", "");
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
fn deleted_cursors_selection_recovery_and_reply_indexes_use_surviving_rows() {
    let mut model = Model::new();
    model.grow("weekend", 5, 1_000);
    let tail = model.chat("weekend", "", "", "");
    let middle = model.chat("weekend", text(&tail, "earlier"), "", "");
    let anchor = text(&middle, "earlier");
    let deleted = text(&rows(&middle)[0], "id");
    let neighbor = text(&rows(&middle)[1], "id");
    model.delete("weekend", deleted);
    let resolved = model.chat("weekend", anchor, "", "");
    assert_eq!(rows(&resolved)[K / 2]["id"], neighbor);
    assert!(!rows(&resolved).iter().any(|m| m["id"] == deleted));
    assert_eq!(
        rows(&model.chat("weekend", "9007199254740991", "", "")),
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
    let selected = model.chat("weekend", "500", "", &selection);
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
    let first = model.chat("weekend", "0", "", &selection);
    assert_eq!(rows(&first)[0]["chosen"], true);
    assert!(!text(&rows(&first)[0], "selection").contains("weekend-1"));
    model.recover("weekend");
    let restored = model.chat("weekend", anchor, "", "");
    assert_eq!(rows(&restored)[K / 2]["id"], deleted);
    let before = model.chat("weekend", "0", "weekend-1", "");
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
    let recovered = model.chat("weekend", "0", "weekend-1", "");
    assert_eq!(
        rows(&recovered)[0]["replyCount"].as_f64().unwrap(),
        rows(&before)[0]["replyCount"].as_f64().unwrap() + 1.
    );
    model.delete("weekend", &orphan);
    assert_eq!(
        model.chat("weekend", "0", "weekend-1", "")["replies"],
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
    let final_chat = model.chat("weekend", "0", "weekend-1", "");
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

#[test]
fn equal_orders_survive_restore_delete_recover_with_consistent_reply_receipts() {
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn open(root: &Directory) -> Model {
        let mut module = super::native::module(super::BYTECODE, super::APP, super::GRANTS);
        module
            .configure_storage(
                root.0.join("data"),
                root.0.join("cache"),
                root.0.join("tmp"),
            )
            .unwrap();
        Model::with_module(module)
    }
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
    let root = Directory(
        std::env::temp_dir().join(format!("messages-window-order-{}", std::process::id())),
    );
    std::fs::create_dir(&root.0).unwrap();
    let mut model = open(&root);
    model.chat("maya", "", "", "");
    drop(model);
    // Write offline-device-shaped records through the real native replica,
    // then reopen the shipped bytecode so its normal restore builds indexes.
    let mut core = exact_snapback4::Module::new(super::APP, super::GRANTS).unwrap();
    core.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("tmp"),
    )
    .unwrap();
    let path = super::GRANTS
        .lines()
        .find_map(|line| line.strip_prefix("sqlite.open "))
        .unwrap();
    core.call(&serde_json::json!({"op":"open", "path":path,
        "origin":"http://127.0.0.1:4400", "viewer":"dev:alice"}))
        .unwrap();
    let stored = core
        .call(&serde_json::json!({"op":"query", "name":"records",
        "viewer":"dev:alice", "args":{"c":null}, "now":0}))
        .unwrap();
    let template = stored["ok"]["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["payload"]["kind"] == "message" && row["payload"]["conversation"] == "maya")
        .unwrap()["payload"]
        .clone();
    let ids = ["tie-z", "later-incoming", "tie-a"];
    let payloads: Vec<_> = ids
        .iter()
        .map(|id| {
            let mut row = template.clone();
            let outgoing = *id != "later-incoming";
            row["message"]["id"] = (*id).into();
            row["message"]["body"] = format!("Body {id}").into();
            row["message"]["order"] = if outgoing { 200 } else { 201 }.into();
            row["message"]["outgoing"] = outgoing.into();
            row["message"]["sender"] = if outgoing { "me" } else { "maya" }.into();
            row["message"]["delivery"] = if outgoing { "Delivered" } else { "" }.into();
            row["message"]["replyRoot"] = if outgoing { "m9" } else { id }.into();
            row["expires"] = Json::Null;
            row
        })
        .collect();
    let result = core
        .call(&serde_json::json!({"op":"predict", "name":"putRecords",
        "viewer":"dev:alice", "now":0, "newIds":[], "entropy":1,
        "args":{
            "recordIds":ids.map(|id| format!("dev:alice:message%3Amaya%3A{id}")),
            "keys":ids.map(|id| format!("message:maya:{id}")),
            "payloads":payloads
        }}))
        .unwrap();
    assert!(result.get("denied").is_none(), "{result}");
    assert!(result["ok"].get("denied").is_none(), "{result}");
    drop(core);
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

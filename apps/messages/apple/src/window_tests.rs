//! Bounded answers from the shipped Hermes bytecode, with the bake's no-storage host.
use exact_js::Module;
use exact_js_value::{to_json, Shape};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, Event, Runner, Store};
use serde_json::Value as Json;
use std::{collections::HashMap, time::Instant};

const K: usize = 200;

struct Model {
    module: Module,
    plan: Plan,
    store: Store,
    shapes: HashMap<String, Shape>,
}
impl Model {
    fn new() -> Self {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        // Wall time is diagnostic here, not a gate on a shared machine. Keep
        // the production executor budget unchanged outside this test harness.
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
            panic!("{name} requested external work in the no-storage host");
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
    assert_eq!(rows(&current), rows(&tail));
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
        if n >= 1_000 {
            // Diagnostic only: the same process, module, source and 200 rows.
            // Measure answer/typed crossing separately from Rust JSON rendering.
            let mut timings = Vec::new();
            let mut bytes = 0;
            for _ in 0..9 {
                let began = Instant::now();
                let Answer::Now(value) = model
                    .module
                    .answer(
                        &mut model.store,
                        "conversation",
                        &chat_args("weekend", "", "", ""),
                    )
                    .unwrap()
                else {
                    panic!("unexpected async answer");
                };
                timings.push(began.elapsed());
                let json = to_json(&value, &model.shapes["conversation"]).unwrap();
                assert_eq!(rows(&json).len(), K);
                bytes = serde_json::to_vec(&json).unwrap().len();
            }
            timings.sort();
            eprintln!(
                "conversation N={n}: answer median={:?}, JSON bytes={bytes}, rows={K}",
                timings[4]
            );
        }
    }
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
        rows(&tail)
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
    let history = runner_chat(&runner);
    assert_eq!(history["hasLater"], true);
    assert!(rows(&history).len() <= K);
    assert_ne!(runner.slot("cursor"), Some(&Value::str("")));
    for _ in 0..20 {
        runner.act("tick", vec![]).unwrap();
    }
    assert_eq!(runner_chat(&runner)["earlier"], history["earlier"]);
    assert_eq!(runner_chat(&runner)["later"], history["later"]);
    assert_eq!(runner.derive("pendingReplies"), Some(&Value::Bool(false)));
    assert_eq!(
        runner.slot("cursor"),
        Some(&Value::str(text(&tail, "earlier")))
    );
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

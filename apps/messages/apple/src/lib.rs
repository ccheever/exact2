//! Messages: the shared Contract UI and deferred TypeScript data module on Apple.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
#[path = "../../native.rs"]
mod native;
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

type ExactEmbeddedData = exact_js::Module;
fn embedded_data() -> ExactEmbeddedData {
    native::module(BYTECODE, APP, GRANTS)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
exact_apple::host!(AppData, PLAN, COMPAT, None, std::ptr::null(), app_data);

#[cfg(test)]
mod snapback_tests;
#[cfg(test)]
mod window_tests;

#[cfg(test)]
mod tests {
    use exact_js::Module;
    use exact_js_value::{to_json, Shape};
    use exact_plan::{Plan, Value};
    use exact_runner::{Answer, DataSource, Store};
    use serde_json::Value as Json;

    fn call(module: &mut Module, plan: &Plan, name: &str, args: Vec<Value>) -> Json {
        let mut store = Store::new(super::GRANTS, Vec::<(String, String)>::new());
        let Answer::Now(value) = module.answer(&mut store, name, &args).unwrap() else {
            panic!("the local chat model must not perform external work");
        };
        let row = plan
            .sources
            .iter()
            .find(|r| plan.str(r.name) == name)
            .unwrap();
        to_json(&value, &Shape::from_plan(plan, row.ty).unwrap()).unwrap()
    }

    #[test]
    fn router_links_back_forward_and_cold_launch_keep_each_threads_composer() {
        use exact_runner::{Event, Runner};
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        for (id, draft, reply) in [("maya", "Saved Maya", "m10"), ("dad", "Saved Dad", "")] {
            call(
                &mut module,
                &plan,
                "saveDraft",
                vec![Value::str(id), Value::str(draft), Value::str(reply)],
            );
        }
        let mut runner = Runner::boot(
            plan,
            module,
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/t/maya",
        )
        .unwrap();
        let assert_composer = |r: &Runner<Module>, thread: &str, draft: &str, reply: &str| {
            assert_eq!(r.derive("chatThread"), Some(&Value::str(thread)));
            assert_eq!(r.derive("draft"), Some(&Value::str(draft)));
            assert_eq!(r.derive("replying"), Some(&Value::str(reply)));
        };
        let navigate = |r: &mut Runner<Module>, url: &str| {
            r.dispatch(r.roots()[0], Event::Navigate(url.into()))
                .unwrap();
        };
        assert_composer(&runner, "maya", "Saved Maya", "m10");
        runner
            .act("write", vec![Value::str("Maya edited")])
            .unwrap();
        let before_refusal = runner.slot("nav").cloned();
        navigate(&mut runner, "/missing-route");
        assert_eq!(runner.slot("nav"), before_refusal.as_ref());
        assert_composer(&runner, "maya", "Maya edited", "m10");
        runner
            .act("chooseMessage", vec![Value::str("m10")])
            .unwrap();
        runner.act("more", vec![]).unwrap();
        navigate(&mut runner, "/t/dad");
        assert_composer(&runner, "dad", "Saved Dad", "");
        assert_eq!(runner.slot("selecting"), Some(&Value::Bool(false)));
        assert_eq!(runner.slot("selection"), Some(&Value::str("")));
        let state: Json = serde_json::from_str(&exact_runner::agent::state(&runner)).unwrap();
        let stack = state["slots"]["nav"]["tabs"][0]["stack"]
            .as_array()
            .unwrap();
        assert_eq!(
            stack.len(),
            2,
            "one inbox and one conversation, with a correct swipe underlay"
        );
        runner.act("write", vec![Value::str("Dad edited")]).unwrap();
        // Browser predecessor traversal dispatches Back; Forward dispatches Navigate.
        runner.act("back", vec![]).unwrap();
        navigate(&mut runner, "/t/dad");
        assert_composer(&runner, "dad", "Dad edited", "");
        navigate(&mut runner, "/t/maya/details");
        assert_composer(&runner, "maya", "Maya edited", "m10");
        runner.act("closeDetails", vec![]).unwrap();
        assert_composer(&runner, "maya", "Maya edited", "m10");
        runner.act("newMessage", vec![]).unwrap();
        assert_composer(&runner, "maya", "Maya edited", "m10");
        navigate(&mut runner, "/t/dad");
        assert_composer(&runner, "dad", "Dad edited", "");
        navigate(&mut runner, "/t/maya");
        assert_composer(&runner, "maya", "Maya edited", "m10");
    }

    #[test]
    fn bubble_runs_use_precise_elapsed_time_and_recompute_after_deletion() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let id = "address:%2B14155550199";
        let send = |m: &mut Module, body: &str, reply: &str, now_ms: f64| {
            call(
                m,
                &plan,
                "sendMessage",
                vec![
                    Value::str(id),
                    Value::str(body),
                    Value::str(reply),
                    Value::Number(0.),
                    Value::Number(now_ms),
                ],
            )
        };
        let thread = |m: &mut Module, reply: &str| {
            call(
                m,
                &plan,
                "conversation",
                vec![
                    Value::str(id),
                    Value::Number(0.),
                    Value::str(reply),
                    Value::str(""),
                ],
            )
        };
        let inbox_time = |m: &mut Module| {
            let inbox = call(m, &plan, "inbox", vec![Value::str(""), Value::Number(0.)]);
            inbox["people"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == id)
                .unwrap()["time"]
                .clone()
        };
        // Native Simulator captures join 30/59-second gaps and split 61 seconds.
        for (body, now) in [
            ("Baseline", 0.),
            ("Thirty", 30_000.),
            ("Fifty nine", 89_000.),
            ("Sixty one", 150_000.),
        ] {
            send(&mut module, body, "", now);
        }
        let chat = thread(&mut module, "");
        let rows = chat["messages"].as_array().unwrap();
        assert_eq!(
            rows.iter()
                .map(|m| m["tail"].as_bool().unwrap())
                .collect::<Vec<_>>(),
            [false, false, true, true]
        );
        for (message, time) in rows
            .iter()
            .zip(["9:42 AM", "9:42 AM", "9:43 AM", "9:44 AM"])
        {
            assert_eq!(message["time"], time);
        }
        assert_eq!(rows.iter().filter(|m| m["timeLabel"] != "").count(), 1);
        assert_eq!(inbox_time(&mut module), "9:44 AM");
        let root = rows[0]["id"].as_str().unwrap();
        call(
            &mut module,
            &plan,
            "deleteMessages",
            vec![
                Value::str(id),
                Value::str(rows[1]["id"].as_str().unwrap()),
                Value::Number(150_000.),
            ],
        );
        assert_eq!(thread(&mut module, "")["messages"][0]["tail"], true);
        send(&mut module, "Focused later reply", root, 150_000.);
        let focused = thread(&mut module, root);
        let replies = focused["replies"].as_array().unwrap();
        assert_eq!(replies.len(), 2);
        assert!(replies.iter().all(|m| m["tail"] == true));
        call(
            &mut module,
            &plan,
            "advanceReplies",
            vec![Value::Number(15.), Value::str(""), Value::Number(210_000.)],
        );
        let received = thread(&mut module, root);
        let reply = received["replies"].as_array().unwrap().last().unwrap();
        assert_eq!(reply["time"], "9:45 AM");
        assert_eq!(inbox_time(&mut module), reply["time"]);
        call(
            &mut module,
            &plan,
            "deleteMessages",
            vec![
                Value::str(id),
                Value::str(reply["id"].as_str().unwrap()),
                Value::Number(210_000.),
            ],
        );
        assert_eq!(inbox_time(&mut module), "9:44 AM");
    }

    #[test]
    fn drafts_and_reply_targets_are_isolated_and_clear_when_sent() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let inbox =
            |m: &mut Module| call(m, &plan, "inbox", vec![Value::str(""), Value::Number(0.)]);
        for (id, body, reply) in [("maya", "Coffee?\nTomorrow", "m10"), ("dad", "Hi Dad", "")] {
            call(
                &mut module,
                &plan,
                "saveDraft",
                vec![Value::str(id), Value::str(body), Value::str(reply)],
            );
        }
        let saved = inbox(&mut module);
        let people = saved["people"].as_array().unwrap();
        let maya = people.iter().find(|p| p["id"] == "maya").unwrap();
        assert_eq!(maya["draft"], "Coffee?\nTomorrow");
        assert_eq!(maya["reply"], "m10");
        let dad = people.iter().find(|p| p["id"] == "dad").unwrap();
        assert_eq!(dad["draft"], "Hi Dad");
        call(
            &mut module,
            &plan,
            "sendMessage",
            vec![
                Value::str("maya"),
                Value::str("Coffee?\nTomorrow"),
                Value::str("m10"),
                Value::Number(0.),
                Value::Number(0.),
            ],
        );
        let sent = inbox(&mut module);
        let people = sent["people"].as_array().unwrap();
        let maya = people.iter().find(|p| p["id"] == "maya").unwrap();
        assert_eq!(maya["draft"], "");
        assert_eq!(maya["reply"], "");
        assert_eq!(
            people.iter().find(|p| p["id"] == "dad").unwrap()["draft"],
            "Hi Dad"
        );
        call(
            &mut module,
            &plan,
            "saveDraft",
            vec![Value::str("dad"), Value::str(""), Value::str("")],
        );
        assert!(inbox(&mut module)["people"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["draft"] == ""));
    }

    #[test]
    fn changing_my_tapback_preserves_other_people_in_both_transcripts() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let inspect = |module: &mut Module| {
            call(
                module,
                &plan,
                "conversation",
                vec![
                    Value::str("maya"),
                    Value::Number(0.),
                    Value::str("m7"),
                    Value::str(""),
                ],
            )
        };
        let initial = inspect(&mut module);
        let other = initial["replies"][0]["reactionEntries"][0].clone();
        assert_eq!(other["id"], "maya");
        assert_eq!(other["value"], "❤️");
        assert_eq!(other["own"], false);
        assert_eq!(initial["replies"][0]["reaction"], "");
        for (selection, own, count, groups) in
            [("❤️", "❤️", 2, 1), ("👍", "👍", 2, 2), ("👍", "", 1, 1)]
        {
            call(
                &mut module,
                &plan,
                "react",
                vec![Value::str("maya"), Value::str("m7"), Value::str(selection)],
            );
            let current = inspect(&mut module);
            let main = current["messages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|message| message["id"] == "m7")
                .unwrap();
            let focused = &current["replies"][0];
            for message in [main, focused] {
                assert_eq!(message["reaction"], own);
                let entries = message["reactionEntries"].as_array().unwrap();
                assert_eq!(entries.len(), count);
                let maya = entries.iter().find(|entry| entry["id"] == "maya").unwrap();
                assert_eq!(maya["value"], other["value"]);
                assert_eq!(maya["own"], false);
                let grouped = message["reactionGroups"].as_array().unwrap();
                assert_eq!(grouped.len(), groups);
                if selection == "❤️" {
                    assert_eq!(grouped[0]["people"].as_array().unwrap().len(), 2);
                }
            }
        }
    }

    #[test]
    fn local_messages_replies_and_toggle_reactions_stay_in_their_conversation() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let thread = |m: &mut Module, id: &str| {
            call(
                m,
                &plan,
                "conversation",
                vec![
                    Value::str(id),
                    Value::Number(0.),
                    Value::str(""),
                    Value::str(""),
                ],
            )
        };
        let before = thread(&mut module, "maya");
        let other = thread(&mut module, "dad");
        let count = before["messages"].as_array().unwrap().len();
        call(
            &mut module,
            &plan,
            "sendMessage",
            vec![
                Value::str("maya"),
                Value::str(" \n "),
                Value::str(""),
                Value::Number(0.),
                Value::Number(0.),
            ],
        );
        assert_eq!(
            thread(&mut module, "maya")["messages"]
                .as_array()
                .unwrap()
                .len(),
            count
        );
        call(
            &mut module,
            &plan,
            "sendMessage",
            vec![
                Value::str("maya"),
                Value::str("See you soon!\n☕️"),
                Value::str("m9"),
                Value::Number(0.),
                Value::Number(0.),
            ],
        );
        let after = thread(&mut module, "maya");
        let sent = after["messages"].as_array().unwrap().last().unwrap();
        assert_eq!(sent["body"], "See you soon!\n☕️");
        assert_eq!(sent["reply"], "I’ll get us a table");
        assert_eq!(sent["outgoing"], true);
        assert_eq!(sent["replyRoot"], "m9");
        let replies = call(
            &mut module,
            &plan,
            "conversation",
            vec![
                Value::str("maya"),
                Value::Number(0.),
                Value::str("m9"),
                Value::str(""),
            ],
        );
        assert_eq!(replies["replies"].as_array().unwrap().len(), 2);
        assert_eq!(replies["replies"][0]["id"], "m9");
        assert_eq!(replies["replies"][0]["replyCount"], 1.0);
        assert_eq!(replies["replies"][1]["id"], sent["id"]);
        assert_eq!(thread(&mut module, "dad")["messages"], other["messages"]);
        let id = sent["id"].as_str().unwrap();
        let choices = before["reactions"].as_array().unwrap();
        assert_eq!(
            choices.iter().find(|r| r["id"] == "laugh").unwrap()["value"],
            "haha"
        );
        assert_eq!(
            choices.iter().find(|r| r["id"] == "laugh-face").unwrap()["value"],
            "😂"
        );
        for (value, expected) in [
            ("👍", "👍"),
            ("👍", ""),
            ("haha", "haha"),
            ("😂", "😂"),
            ("😂", ""),
        ] {
            call(
                &mut module,
                &plan,
                "react",
                vec![Value::str("maya"), Value::str(id), Value::str(value)],
            );
            let current = thread(&mut module, "maya");
            assert_eq!(
                current["messages"].as_array().unwrap().last().unwrap()["reaction"],
                expected
            );
        }
        call(
            &mut module,
            &plan,
            "deleteMessages",
            vec![Value::str("maya"), Value::str(id), Value::Number(0.)],
        );
        assert_eq!(thread(&mut module, "maya")["messages"], before["messages"]);
        let selection = call(
            &mut module,
            &plan,
            "conversation",
            vec![
                Value::str("maya"),
                Value::Number(0.),
                Value::str(""),
                Value::str("m3|m1|m1|dad-1|missing"),
            ],
        );
        assert_eq!(selection["selectionCount"], 2.0);
        assert_eq!(selection["selectedText"], "Hey! Are you around this morning?\nWant to grab coffee? There’s a new place on Valencia I’ve been wanting to try");
        assert_eq!(selection["messages"][0]["chosen"], true);
        assert_eq!(selection["messages"][1]["chosen"], false);
        call(
            &mut module,
            &plan,
            "deleteMessages",
            vec![
                Value::str("maya"),
                Value::str("m1|m3|m1|dad-1|missing"),
                Value::Number(0.),
            ],
        );
        let remaining = thread(&mut module, "maya");
        assert_eq!(remaining["messages"].as_array().unwrap().len(), count - 2);
        assert!(remaining["messages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["id"] != "m1" && m["id"] != "m3"));
        assert_eq!(thread(&mut module, "dad")["messages"], other["messages"]);
    }
    #[test]
    fn deleting_a_reply_root_keeps_outgoing_and_pending_incoming_replies_visible() {
        let plan = Plan::decode(super::PLAN).unwrap();
        for delete_before_send in [true, false] {
            let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
            module.bind(&plan);
            module.activate().unwrap();
            let delete = |m: &mut Module| {
                call(
                    m,
                    &plan,
                    "deleteMessages",
                    vec![Value::str("maya"), Value::str("m9"), Value::Number(0.)],
                )
            };
            if delete_before_send {
                delete(&mut module);
            }
            call(
                &mut module,
                &plan,
                "sendMessage",
                vec![
                    Value::str("maya"),
                    Value::str("Still in this thread"),
                    Value::str("m9"),
                    Value::Number(0.),
                    Value::Number(0.),
                ],
            );
            if !delete_before_send {
                delete(&mut module);
            }
            call(
                &mut module,
                &plan,
                "advanceReplies",
                vec![Value::Number(3.), Value::str("maya"), Value::Number(900.)],
            );
            let thread = |m: &mut Module| {
                call(
                    m,
                    &plan,
                    "conversation",
                    vec![
                        Value::str("maya"),
                        Value::Number(0.),
                        Value::str("m9"),
                        Value::str(""),
                    ],
                )
            };
            let typing = thread(&mut module);
            assert_eq!(typing["typingRoot"], "m9");
            assert_eq!(typing["replies"][0]["body"], "Still in this thread");
            call(
                &mut module,
                &plan,
                "advanceReplies",
                vec![Value::Number(15.), Value::str("maya"), Value::Number(4500.)],
            );
            let chat = thread(&mut module);
            let replies = chat["replies"].as_array().unwrap();
            assert_eq!(replies.len(), 2);
            assert_eq!(replies[1]["outgoing"], false);
            assert!(replies.iter().all(|m| m["replyRoot"] == "m9"));
            assert!(chat["messages"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["id"] != "m9"));
        }
    }

    #[test]
    fn offline_typing_and_replies_follow_explicit_time_and_stay_in_their_thread() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let thread = |m: &mut Module, id: &str, reply: &str| {
            call(
                m,
                &plan,
                "conversation",
                vec![
                    Value::str(id),
                    Value::Number(0.),
                    Value::str(reply),
                    Value::str(""),
                ],
            )
        };
        let advance = |m: &mut Module, tick: f64, active: &str| {
            call(
                m,
                &plan,
                "advanceReplies",
                vec![
                    Value::Number(tick),
                    Value::str(active),
                    Value::Number(tick * 300.),
                ],
            )
        };
        for (id, root, tick) in [("maya", "m9", 0.), ("weekend", "", 2.)] {
            let sent = call(
                &mut module,
                &plan,
                "sendMessage",
                vec![
                    Value::str(id),
                    Value::str("Okay"),
                    Value::str(root),
                    Value::Number(tick),
                    Value::Number(tick * 300.),
                ],
            );
            assert_eq!(sent["pending"], true);
        }
        let before = thread(&mut module, "maya", "m9");
        assert_eq!(before["replies"][1]["delivery"], "Delivered");
        advance(&mut module, 2., "maya");
        assert_eq!(thread(&mut module, "maya", "m9")["typingName"], "");
        advance(&mut module, 3., "maya");
        let typing = thread(&mut module, "maya", "m9");
        assert_eq!(typing["typingName"], "Maya Chen");
        assert_eq!(typing["typingRoot"], "m9");
        assert_eq!(typing["typingAvatar"], "");
        assert_eq!(typing["replies"][1]["delivery"], "Read");
        assert_eq!(thread(&mut module, "weekend", "")["typingName"], "");
        advance(&mut module, 5., "maya");
        let group = thread(&mut module, "weekend", "");
        assert_eq!(group["typingName"], "Alex Rivera");
        assert_eq!(group["typingAvatar"], "AR");
        assert_eq!(advance(&mut module, 15., "weekend")["pending"], true);
        let received = thread(&mut module, "maya", "m9");
        assert_eq!(received["typingName"], "");
        assert_eq!(received["scrollRevision"], before["scrollRevision"]);
        assert_eq!(received["replies"].as_array().unwrap().len(), 3);
        assert_eq!(received["replies"][2]["outgoing"], false);
        assert_eq!(received["replies"][2]["replyRoot"], "m9");
        assert_eq!(received["replies"][1]["delivery"], "Read");
        assert_eq!(received["replies"][2]["delivery"], "");
        assert_eq!(advance(&mut module, 17., "weekend")["pending"], false);
        let inbox = call(
            &mut module,
            &plan,
            "inbox",
            vec![Value::str(""), Value::Number(0.)],
        );
        let people = inbox["people"].as_array().unwrap();
        assert_eq!(
            people.iter().find(|p| p["id"] == "maya").unwrap()["unread"],
            true
        );
        assert_eq!(
            people.iter().find(|p| p["id"] == "weekend").unwrap()["unread"],
            false
        );
        advance(&mut module, 100., "weekend");
        assert_eq!(
            thread(&mut module, "maya", "m9")["messages"],
            received["messages"]
        );
    }

    #[test]
    fn recipient_sets_reuse_threads_and_create_groups_only_on_a_nonempty_send() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let recipients = |m: &mut Module, ids: &str, body: &str| {
            call(
                m,
                &plan,
                "recipients",
                vec![
                    Value::str(ids),
                    Value::str(""),
                    Value::str(body),
                    Value::Number(0.),
                ],
            )
        };
        let inbox =
            |m: &mut Module| call(m, &plan, "inbox", vec![Value::str(""), Value::Number(0.)]);
        let thread = |m: &mut Module, id: &str| {
            call(
                m,
                &plan,
                "conversation",
                vec![
                    Value::str(id),
                    Value::Number(0.),
                    Value::str(""),
                    Value::str(""),
                ],
            )
        };
        let send = |m: &mut Module, id: &str, body: &str| {
            call(
                m,
                &plan,
                "sendMessage",
                vec![
                    Value::str(id),
                    Value::str(body),
                    Value::str(""),
                    Value::Number(0.),
                    Value::Number(0.),
                ],
            )
        };
        let initial = inbox(&mut module);
        let maya = thread(&mut module, "maya");
        let selection = recipients(&mut module, "|maya|alex|maya|unknown", "Hello both");
        assert_eq!(selection["selected"].as_array().unwrap().len(), 2);
        assert_eq!(selection["selected"][0]["without"], "alex");
        assert_eq!(selection["target"], "group:alex|maya");
        assert_eq!(selection["canSend"], true);
        assert!(selection["people"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["id"] != "maya" && p["id"] != "alex" && p["id"] != "weekend"));
        assert_eq!(
            recipients(&mut module, "alex|maya", "Hello")["target"],
            selection["target"]
        );
        assert_eq!(recipients(&mut module, "maya", "Hello")["target"], "maya");
        assert_eq!(
            recipients(&mut module, "maya|jules|alex", "Hello")["target"],
            "weekend"
        );
        assert_eq!(recipients(&mut module, "", "Hello")["canSend"], false);
        assert_eq!(
            recipients(&mut module, "maya|alex", " \n ")["canSend"],
            false
        );
        assert_eq!(inbox(&mut module), initial);
        send(&mut module, "group:alex|maya", " \n ");
        assert_eq!(inbox(&mut module), initial);
        send(&mut module, "group:alex|maya", "Hello both");
        let created = thread(&mut module, "group:alex|maya");
        assert_eq!(created["messages"].as_array().unwrap().len(), 1);
        assert_eq!(created["name"], "Alex, Maya");
        assert_eq!(created["messages"][0]["body"], "Hello both");
        assert_eq!(thread(&mut module, "maya")["messages"], maya["messages"]);
        assert_eq!(inbox(&mut module)["people"].as_array().unwrap().len(), 7);
        call(
            &mut module,
            &plan,
            "advanceReplies",
            vec![
                Value::Number(3.),
                Value::str("group:alex|maya"),
                Value::Number(900.),
            ],
        );
        let typing = thread(&mut module, "group:alex|maya");
        assert_eq!(typing["typingName"], "Alex Rivera");
        assert_eq!(typing["typingAvatar"], "AR");
        call(
            &mut module,
            &plan,
            "advanceReplies",
            vec![
                Value::Number(15.),
                Value::str("group:alex|maya"),
                Value::Number(4500.),
            ],
        );
        let replied = thread(&mut module, "group:alex|maya");
        assert_eq!(replied["messages"][1]["senderName"], "Alex Rivera");
        assert_eq!(replied["messages"][1]["showSender"], true);
        let same = recipients(&mut module, "maya|alex", "Again");
        send(&mut module, same["target"].as_str().unwrap(), "Again");
        assert_eq!(
            thread(&mut module, "group:alex|maya")["messages"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(inbox(&mut module)["people"].as_array().unwrap().len(), 7);
    }
    #[test]
    fn inbox_actions_preserve_muted_messages_and_cancel_deleted_thread_activity() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let inbox =
            |m: &mut Module| call(m, &plan, "inbox", vec![Value::str(""), Value::Number(0.)]);
        let chat = |m: &mut Module| {
            call(
                m,
                &plan,
                "conversation",
                vec![
                    Value::str("maya"),
                    Value::Number(0.),
                    Value::str(""),
                    Value::str(""),
                ],
            )
        };
        let before = chat(&mut module);
        call(
            &mut module,
            &plan,
            "saveDraft",
            vec![
                Value::str("maya"),
                Value::str("Keep this draft"),
                Value::str("m9"),
            ],
        );
        for unread in [false, true, true, false] {
            call(
                &mut module,
                &plan,
                "setConversationUnread",
                vec![Value::str("maya"), Value::Bool(unread)],
            );
            let current = inbox(&mut module);
            assert_eq!(current["people"][0]["unread"], unread);
            assert_eq!(current["people"][0]["draft"], "Keep this draft");
            assert_eq!(current["people"][0]["reply"], "m9");
            assert_eq!(current["people"][1]["unread"], true);
            assert_eq!(chat(&mut module)["messages"], before["messages"]);
        }
        call(
            &mut module,
            &plan,
            "muteConversation",
            vec![Value::str("maya")],
        );
        let muted = inbox(&mut module);
        assert_eq!(muted["people"][0]["muted"], true);
        assert_eq!(chat(&mut module)["muted"], true);
        assert_eq!(chat(&mut module)["contactAddress"], "+1 (415) 555-0101");
        assert_eq!(chat(&mut module)["knownContact"], true);
        assert_eq!(chat(&mut module)["messages"], before["messages"]);
        call(
            &mut module,
            &plan,
            "sendMessage",
            vec![
                Value::str("maya"),
                Value::str("Before deleting"),
                Value::str(""),
                Value::Number(0.),
                Value::Number(0.),
            ],
        );
        call(
            &mut module,
            &plan,
            "saveDraft",
            vec![Value::str("maya"), Value::str("Unsent"), Value::str("")],
        );
        let marked = call(
            &mut module,
            &plan,
            "setConversationUnread",
            vec![Value::str("maya"), Value::Bool(true)],
        );
        assert_eq!(marked["pending"], true);
        assert_eq!(inbox(&mut module)["people"][0]["unread"], true);
        let deleted = call(
            &mut module,
            &plan,
            "deleteConversation",
            vec![Value::str("maya"), Value::Number(0.)],
        );
        assert_eq!(deleted["pending"], false);
        call(
            &mut module,
            &plan,
            "advanceReplies",
            vec![Value::Number(30.), Value::str(""), Value::Number(9000.)],
        );
        assert_eq!(inbox(&mut module)["people"].as_array().unwrap().len(), 5);
        let contacts = call(
            &mut module,
            &plan,
            "recipients",
            vec![
                Value::str(""),
                Value::str("Maya"),
                Value::str(""),
                Value::Number(0.),
            ],
        );
        assert_eq!(contacts["people"][0]["id"], "maya");
        call(
            &mut module,
            &plan,
            "sendMessage",
            vec![
                Value::str("maya"),
                Value::str("New conversation"),
                Value::str(""),
                Value::Number(30.),
                Value::Number(9000.),
            ],
        );
        let fresh = chat(&mut module);
        assert_eq!(fresh["messages"].as_array().unwrap().len(), 1);
        assert_eq!(fresh["messages"][0]["body"], "New conversation");
        assert_eq!(inbox(&mut module)["people"][0]["draft"], "");
        call(
            &mut module,
            &plan,
            "muteConversation",
            vec![Value::str("maya")],
        );
        assert_eq!(inbox(&mut module)["people"][0]["muted"], false);
    }

    #[test]
    fn saved_contacts_rename_existing_threads_without_creating_unsent_conversations() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let inbox =
            |m: &mut Module| call(m, &plan, "inbox", vec![Value::str(""), Value::Number(0.)]);
        let recipients = |m: &mut Module, query: &str| {
            call(
                m,
                &plan,
                "recipients",
                vec![
                    Value::str(""),
                    Value::str(query),
                    Value::str("Hello"),
                    Value::Number(0.),
                ],
            )
        };
        let save = |m: &mut Module, first: &str, last: &str, phone: &str, email: &str| {
            call(
                m,
                &plan,
                "createLocalContact",
                [first, last, "Example", phone, email, "Local notes"]
                    .into_iter()
                    .map(Value::str)
                    .collect(),
            )
        };
        let phone = recipients(&mut module, "8885551212")["target"]
            .as_str()
            .unwrap()
            .to_owned();
        call(
            &mut module,
            &plan,
            "sendMessage",
            vec![
                Value::str(&phone),
                Value::str("Local conversation"),
                Value::str(""),
                Value::Number(0.),
                Value::Number(0.),
            ],
        );
        call(
            &mut module,
            &plan,
            "saveDraft",
            vec![
                Value::str(&phone),
                Value::str("Keep this draft"),
                Value::str(""),
            ],
        );
        let before = inbox(&mut module)["people"].as_array().unwrap().len();
        save(
            &mut module,
            "Jordan",
            "Avery",
            "+1 (888) 555-1212",
            "jordan@example.com",
        );
        let saved = inbox(&mut module);
        let people = saved["people"].as_array().unwrap();
        assert_eq!(people.len(), before);
        let row = people.iter().find(|p| p["id"] == phone).unwrap();
        assert_eq!(row["name"], "Jordan Avery");
        assert_eq!(row["draft"], "Keep this draft");
        let chat = call(
            &mut module,
            &plan,
            "conversation",
            vec![
                Value::str(&phone),
                Value::Number(0.),
                Value::str(""),
                Value::str(""),
            ],
        );
        assert_eq!(chat["knownContact"], true);
        assert_eq!(chat["messages"][0]["body"], "Local conversation");
        assert_eq!(recipients(&mut module, "+18885551212")["target"], phone);
        assert_eq!(
            recipients(&mut module, "jordan@example.com")["people"][0]["name"],
            "Jordan Avery"
        );
        save(&mut module, "Name", "Only", "", "");
        let unavailable = recipients(&mut module, "Name Only");
        assert_eq!(unavailable["canSend"], false);
        assert_eq!(unavailable["target"], "");
        assert!(unavailable["people"].as_array().unwrap().is_empty());
        assert_eq!(
            inbox(&mut module)["people"].as_array().unwrap().len(),
            before
        );
    }

    #[test]
    fn typed_addresses_join_the_intended_recipients_without_creating_unsent_threads() {
        let plan = Plan::decode(super::PLAN).unwrap();
        let mut module = Module::new(super::BYTECODE.to_vec(), super::APP, super::GRANTS);
        module.bind(&plan);
        module.activate().unwrap();
        let recipients = |m: &mut Module, ids: &str, query: &str, body: &str| {
            call(
                m,
                &plan,
                "recipients",
                vec![
                    Value::str(ids),
                    Value::str(query),
                    Value::str(body),
                    Value::Number(0.),
                ],
            )
        };
        let inbox =
            |m: &mut Module| call(m, &plan, "inbox", vec![Value::str(""), Value::Number(0.)]);
        let send = |m: &mut Module, id: &str, body: &str| {
            call(
                m,
                &plan,
                "sendMessage",
                vec![
                    Value::str(id),
                    Value::str(body),
                    Value::str(""),
                    Value::Number(0.),
                    Value::Number(0.),
                ],
            )
        };
        let thread = |m: &mut Module, id: &str| {
            call(
                m,
                &plan,
                "conversation",
                vec![
                    Value::str(id),
                    Value::Number(0.),
                    Value::str(""),
                    Value::str(""),
                ],
            )
        };
        let initial = inbox(&mut module);
        assert_eq!(
            recipients(&mut module, "", "4155550101", "Hello")["target"],
            "maya"
        );
        let phone = recipients(&mut module, "", "(415) 555-0199", "Hello");
        let phone_id = phone["target"].as_str().unwrap();
        assert_eq!(phone["canSend"], true);
        assert_eq!(phone["people"][0]["name"], "+1 (415) 555-0199");
        for variant in ["4155550199", "+1 415 555 0199", "1-415-555-0199"] {
            assert_eq!(
                recipients(&mut module, "", variant, "Hello")["target"],
                phone_id
            );
        }
        assert_eq!(
            recipients(&mut module, "", "+44 20 7123 4567", "Hello")["people"][0]["name"],
            "+442071234567"
        );
        let duplicate = recipients(&mut module, phone_id, "+14155550199", "Hello");
        assert_eq!(duplicate["resolved"], phone_id);
        assert!(duplicate["people"].as_array().unwrap().is_empty());
        let email = recipients(&mut module, "", " Chat.Example@Example.com ", "Hello");
        let email_id = email["target"].as_str().unwrap();
        assert_eq!(email["people"][0]["name"], "chat.example@example.com");
        assert_eq!(
            recipients(&mut module, "", "chat.example@example.com", "Hello")["target"],
            email_id
        );
        assert_eq!(
            recipients(&mut module, "maya", "Maya Chen", "Hello")["resolved"],
            "maya"
        );
        for invalid in [
            "Unknown person",
            "chat@",
            "a@b",
            "12",
            "++14155550199",
            "maya|alex",
        ] {
            let unresolved = recipients(&mut module, "maya", invalid, "Keep my draft");
            assert_eq!(unresolved["canSend"], false, "{invalid}");
            assert_eq!(unresolved["resolved"], "");
            assert_eq!(unresolved["selected"][0]["id"], "maya");
        }
        assert_eq!(
            recipients(&mut module, "", "chat.example@example.com", " ")["canSend"],
            false
        );
        assert_eq!(inbox(&mut module), initial);
        send(&mut module, email_id, " ");
        assert_eq!(inbox(&mut module), initial);
        let group = recipients(
            &mut module,
            "maya",
            "chat.example@example.com",
            "Hello both",
        );
        let group_id = group["target"].as_str().unwrap();
        assert_eq!(group["canSend"], true);
        send(&mut module, group_id, "Hello both");
        let sent = thread(&mut module, group_id);
        assert_eq!(sent["messages"].as_array().unwrap().len(), 1);
        assert_eq!(sent["messages"][0]["body"], "Hello both");
        assert_eq!(inbox(&mut module)["people"].as_array().unwrap().len(), 7);
        assert_eq!(
            recipients(&mut module, &format!("{email_id}|maya"), "", "Again")["target"],
            group_id
        );
        call(
            &mut module,
            &plan,
            "advanceReplies",
            vec![
                Value::Number(15.),
                Value::str(group_id),
                Value::Number(4500.),
            ],
        );
        let replied = thread(&mut module, group_id);
        assert_eq!(
            replied["messages"][1]["senderName"],
            "chat.example@example.com"
        );
        assert_eq!(replied["messages"][1]["showSender"], true);
        send(&mut module, email_id, "One to one");
        assert_eq!(
            thread(&mut module, email_id)["messages"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(inbox(&mut module)["people"].as_array().unwrap().len(), 8);
        send(&mut module, phone_id, "By number");
        assert_eq!(
            thread(&mut module, phone_id)["messages"][0]["body"],
            "By number"
        );
        assert_eq!(inbox(&mut module)["people"].as_array().unwrap().len(), 9);
    }
}

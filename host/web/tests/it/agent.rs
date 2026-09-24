//! The agent API's read operations over the Caltrain app (LLP 1012): the
//! tree in preorder with props by their schema names, the state as typed
//! JSON, the journal, `settle` from the springs' engine — the same answers
//! on every host, because they come from the runner and kernel.

use exact_runner::Event;
use exact_web::Host;

fn boot() -> Host<caltrain_data::Caltrain> {
    let plan = caltrain::build().unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap()
    .0
}

fn view_with_test_id(host: &Host<caltrain_data::Caltrain>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

#[test]
fn tree_lists_every_live_node_in_preorder_with_props_and_handlers() {
    let host = boot();
    let tree = host.agent(r#"{"op":"tree"}"#);
    assert!(tree.starts_with("{\"epoch\":"), "{tree}");
    assert!(tree.contains("\"incarnation\":1,\"clock\":0,\"roots\":[1],\"nodes\":[{\"id\":1,\"parent\":null,\"depth\":0,\"type\":"), "{tree}");
    assert!(tree.contains("\"testId\":\"caltrain-main\""));
    assert!(tree.contains("\"text\":\"Mountain View\""));
    assert!(tree.contains("\"handlers\":[\"press\"]"));
    assert!(tree.contains("\"handlers\":[]"));
    assert!(tree.contains("\"children\":["));
    let live = host.runner().kernel().live_count();
    assert_eq!(
        tree.matches("{\"id\":").count(),
        live,
        "one row per live node"
    );
}

#[test]
fn state_is_typed_json_with_field_names() {
    let host = boot();
    let state = host.agent(r#"{"op":"state"}"#);
    // Tagged with the epoch and incarnation (LLP 1035.002 D3), then the
    // clock, then the slots in the plan's order.
    assert!(state.starts_with("{\"epoch\":"), "{state}");
    assert!(state.contains(",\"clock\":0,\"slots\":{"), "{state}");
    // Every slot by name, typed (LLP 1014 added the sky's material, the
    // deck, and its focus).
    for slot in [
        "\"nowMs\":1787915400000",
        "\"screen\":\"home\"",
        "\"stationId\":null",
        "\"query\":\"\"",
        "\"material\":\"glass\"",
        "\"deck\":false",
        "\"focus\":null",
    ] {
        assert!(state.contains(slot), "{slot} in {state}");
    }
    assert!(
        state.contains(
            "\"derives\":{\"selectedId\":\"mv\",\"skyOn\":true,\"canvasColor\":\"#05081a\"}"
        ),
        "{state}"
    );
    assert!(
        state.contains("\"station\":{\"id\":\"mv\",\"name\":\"Mountain View\",\"zone\":"),
        "records carry their field names: {state}"
    );
    assert!(
        state.contains("\"northBoard\":[{\"id\":"),
        "a list of records: {state}"
    );
}

#[test]
fn logs_journal_boot_events_refusals_and_the_clock() {
    let mut host = boot();
    let logs = host.agent(r#"{"op":"logs"}"#);
    assert!(
        logs.starts_with("{\"next\":1,\"from\":0,\"lines\":[\"t=0 boot: "),
        "{logs}"
    );
    let id = view_with_test_id(&host, "change-station");
    host.dispatch_at(id, Event::Press, 5.0);
    host.dispatch_at(9999, Event::Press, 5.0);
    host.advance(2500.0);
    let logs = host.agent(r#"{"op":"logs","since":1}"#);
    assert!(
        !logs.contains("boot: "),
        "since skips what was read: {logs}"
    );
    assert!(
        logs.contains(&format!("press view {id} (openStations) → epoch")),
        "{logs}"
    );
    assert!(
        logs.contains("press view 9999 refused: UnknownView(9999)"),
        "{logs}"
    );
    assert!(
        logs.contains("t=2500 advance → 2 timers fired, epoch"),
        "{logs}"
    );
    let next = host.runner().journal_start() + host.runner().journal().count();
    let again = host.agent(r#"{"op":"logs","since":99}"#);
    assert_eq!(
        again,
        format!("{{\"next\":{next},\"from\":{next},\"lines\":[]}}"),
        "a cursor past the end clamps to it"
    );
    assert_eq!(
        host.agent(r#"{"op":"logs","since":"3"}"#),
        "{\"error\":\"since must be a non-negative number\"}"
    );
    // An unknown action is journaled as a refusal too.
    let _ = host.runner_mut().act("fly", Vec::new());
    let last = host.agent(&format!("{{\"op\":\"logs\",\"since\":{next}}}"));
    assert!(last.contains("act fly refused: NoHandler"), "{last}");
}

#[test]
fn an_advance_batch_marks_each_timers_time_and_says_where_the_clock_landed() {
    let mut host = boot();
    // Two timer fires in one seek: each commit's ops sit behind an `at`
    // marker with the timer's due time, and the batch says where the
    // runner's clock ended (LLP 1002 D3: a page that owns time attributes
    // what each commit started to that instant).
    let batch = host.advance(2500.0);
    let first = batch
        .find("{\"op\":\"at\",\"ms\":1000}")
        .expect("first timer's marker");
    let second = batch
        .find("{\"op\":\"at\",\"ms\":2000}")
        .expect("second timer's marker");
    assert!(first < second, "markers in due order");
    assert!(
        batch.ends_with(",\"timers\":true,\"clock\":2500,\"error\":null}"),
        "{}",
        &batch[batch.len() - 60..]
    );
    // A press carries the clock it was delivered at (the runner's, which an
    // agent's page keeps equal to its own).
    let id = view_with_test_id(&host, "change-station");
    let press = host.dispatch_at(id, Event::Press, 2500.0);
    assert!(
        press.contains("{\"op\":\"at\",\"ms\":2500}"),
        "{}",
        &press[..80]
    );
    assert!(
        press.contains(",\"clock\":2500,"),
        "{}",
        &press[press.len() - 60..]
    );
    // A seek backwards moves nothing and reports the clock as it is.
    let back = host.advance(1000.0);
    assert_eq!(
        back,
        "{\"ops\":[],\"timer_due_ms\":3000,\"timers\":true,\"clock\":2500,\"error\":null}"
    );
}

#[test]
fn settle_is_null_with_no_spring_in_flight_and_unknown_ops_are_errors() {
    let host = boot();
    assert_eq!(host.agent(r#"{"op":"settle"}"#), "{\"settle\":null}");
    assert_eq!(
        host.agent(r#"{"op":"fly"}"#),
        "{\"error\":\"unknown op: fly\"}"
    );
    assert_eq!(host.agent(r#"{"since":3}"#), "{\"error\":\"no op\"}");
}

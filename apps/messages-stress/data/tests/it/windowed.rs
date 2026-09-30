//! Actual Messages Contract with full records and a bounded mounted transcript.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{CollectionFeedback, Event, Runner};
use messages_stress_data::MessagesStress;

fn boot() -> Runner<MessagesStress> {
    Runner::boot(
        contract::compile(include_str!("../../../app.contract")).unwrap(),
        MessagesStress,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn viewport(runner: &mut Runner<MessagesStress>, top: f64, interaction: Option<u32>) {
    let snapshot = runner.collections().remove(0);
    runner
        .collection_feedback(CollectionFeedback {
            view: snapshot.view,
            revision: snapshot.revision,
            scroll_sequence: snapshot.scroll_sequence + 1,
            offset: top,
            port_cross: 640.0,
            port_main: 320.0,
            cross: 640.0,
            measurements: vec![],
            focus_view: None,
            interaction_view: interaction,
        })
        .unwrap();
}

#[test]
fn full_history_is_windowed_and_an_active_row_survives_until_release() {
    let mut runner = boot();
    runner.act("toggleWindowed", vec![]).unwrap();
    runner
        .act("chooseCount", vec![Value::Number(10_000.0)])
        .unwrap();
    let first = runner.collections().remove(0);
    assert_eq!(first.count, 10_000);
    assert!(first.rows.len() <= 16);
    let held = first.rows[0].root;
    viewport(&mut runner, 0.0, Some(held));
    let top_nodes = runner.kernel().arena().live_count();
    for top in [16_000.0, 160_000.0, 300_000.0, 2_000.0] {
        viewport(&mut runner, top, Some(held));
        let snapshot = runner.collections().remove(0);
        assert_eq!(snapshot.count, 10_000);
        assert!(snapshot.rows.len() <= 34, "{} rows", snapshot.rows.len());
        assert!(runner.kernel().node(held).is_some());
        assert!(runner.kernel().arena().live_count() <= top_nodes * 2 + 100);
    }
    runner
        .act(
            "editDraft",
            vec![Value::str("typing while a distant row is held")],
        )
        .unwrap();
    assert_eq!(
        runner.slot("draft"),
        Some(&Value::str("typing while a distant row is held"))
    );
    assert!(runner.kernel().node(held).is_some());
    viewport(&mut runner, 160_000.0, None);
    assert!(runner.kernel().node(held).is_none());
    runner.act("reset", vec![]).unwrap();
    assert!(runner.collections().is_empty());
}

#[test]
fn eager_and_manual_page_controls_remain_available_beside_windowing() {
    let mut runner = boot();
    assert!(runner.collections().is_empty());
    runner
        .act("chooseCount", vec![Value::Number(1_000.0)])
        .unwrap();
    assert!(runner
        .kernel()
        .find_by_test_id("message-m-000000")
        .is_empty());
    assert_eq!(runner.kernel().find_by_test_id("message-m-000999").len(), 1);
    runner.act("toggleWindowed", vec![]).unwrap();
    assert_eq!(runner.collections()[0].count, 1_000);
    runner.act("toggleEager", vec![]).unwrap();
    assert!(runner.collections().is_empty());
    assert_eq!(runner.kernel().find_by_test_id("message-m-000000").len(), 1);
    assert_eq!(runner.kernel().find_by_test_id("message-m-000999").len(), 1);
    runner.act("toggleEager", vec![]).unwrap();
    assert!(runner
        .kernel()
        .find_by_test_id("message-m-000000")
        .is_empty());
}

#[test]
fn reply_events_and_keyboard_alternative_keep_the_logical_message_identity() {
    let mut runner = boot();
    runner.act("toggleWindowed", vec![]).unwrap();
    let id = |runner: &Runner<MessagesStress>, name: &str| {
        let key = runner.kernel().find_by_test_id(name)[0];
        runner.kernel().node_by_key(key).unwrap().id
    };
    let bubble = id(&runner, "reply-hit-m-000003");
    runner.dispatch(bubble, Event::Swiperight).unwrap();
    assert_eq!(runner.slot("replying"), Some(&Value::str("m-000003")));
    runner.act("step", vec![]).unwrap();
    assert_eq!(runner.slot("replying"), Some(&Value::str("m-000003")));
    let cancel = id(&runner, "cancel-reply");
    runner.dispatch(cancel, Event::Press).unwrap();
    assert_eq!(runner.slot("replying"), Some(&Value::str("")));
    let alternative = id(&runner, "reply-m-000005");
    runner.dispatch(alternative, Event::Press).unwrap();
    assert_eq!(runner.slot("replying"), Some(&Value::str("m-000005")));
    runner
        .act("editDraft", vec![Value::str("A local reply")])
        .unwrap();
    runner.act("sendDraft", vec![]).unwrap();
    assert_eq!(runner.slot("replying"), Some(&Value::str("")));
    assert_eq!(runner.slot("echo"), Some(&Value::str("A local reply")));
}

#[test]
fn transcript_follows_new_tail_only_when_reading_at_the_end() {
    let mut runner = boot();
    runner.act("toggleWindowed", vec![]).unwrap();
    runner
        .act("chooseCount", vec![Value::Number(1_000.0)])
        .unwrap();
    let end = runner.collections()[0].total_extent - 320.0;
    viewport(&mut runner, end, None);
    runner
        .act("editDraft", vec![Value::str("An appended local message")])
        .unwrap();
    runner.act("sendDraft", vec![]).unwrap();
    let snapshot = &runner.collections()[0];
    assert_eq!(snapshot.count, 1_001);
    assert_eq!(snapshot.rows.last().unwrap().index, 1_000);
    assert!((snapshot.correction.unwrap().offset - (snapshot.total_extent - 320.0)).abs() < 0.01);

    viewport(&mut runner, 2_000.0, None);
    let first = runner.collections()[0].rows[0].root;
    runner.act("step", vec![]).unwrap();
    let snapshot = &runner.collections()[0];
    assert!(snapshot.rows.iter().any(|row| row.root == first));
    assert!(snapshot.rows.last().unwrap().index < 100);
}

/// Past 10,000 records the virtualized transcript asks 200-record windows:
/// the whole history (about 23 MB at 100,000) is more than one answer over a
/// Rust module's seam carries (16 MiB, LLP 1071 §7).
#[test]
fn past_ten_thousand_the_virtualized_transcript_asks_windows() {
    let mut runner = boot();
    runner.act("toggleWindowed", vec![]).unwrap();
    runner
        .act("chooseCount", vec![Value::Number(100_000.0)])
        .unwrap();
    let snapshot = runner.collections().remove(0);
    assert_eq!(snapshot.count, 200);
    let Some(Value::Record(history)) = runner.resource("history") else {
        panic!("history is a record");
    };
    assert_eq!(history[1], Value::Number(200.0), "materialized");
    runner
        .act("chooseCount", vec![Value::Number(10_000.0)])
        .unwrap();
    assert_eq!(runner.collections().remove(0).count, 10_000);
}

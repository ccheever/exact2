//! The real Contract/runner path, beyond the synthetic generator.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataSource, Event, Runner};
use messages_stress_data::MessagesStress;

fn boot() -> Runner<MessagesStress> {
    let source = include_str!("../../app.contract");
    let plan = contract::compile(source).unwrap();
    let baked = contract::bake(plan, MessagesStress).unwrap();
    Runner::boot(
        baked,
        MessagesStress,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn press(r: &mut Runner<MessagesStress>, name: &str) {
    let key = r.kernel().find_by_test_id(name)[0];
    let node = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(node, Event::Press).unwrap();
}

#[test]
fn controls_drive_the_real_resource_and_stop_at_the_run_cap() {
    let mut r = boot();
    assert_eq!(r.slot("count"), Some(&Value::Number(100.0)));
    assert_eq!(r.kernel().find_by_test_id("message-m-000099").len(), 1);
    press(&mut r, "toggle-eager");
    press(&mut r, "history-1000");
    assert_eq!(r.kernel().find_by_test_id("message-m-000999").len(), 1);
    press(&mut r, "history-100");
    assert!(r.kernel().find_by_test_id("message-m-000999").is_empty());
    press(&mut r, "batch-32");
    assert_eq!(r.slot("batch"), Some(&Value::Number(32.0)));
    press(&mut r, "start");
    r.advance(250.0).unwrap();
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.0)));
    press(&mut r, "pause");
    r.advance(500.0).unwrap();
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.0)));
    press(&mut r, "start");
    for i in 3..=122 {
        r.advance(i as f64 * 250.0).unwrap();
    }
    assert_eq!(r.slot("revision"), Some(&Value::Number(120.0)));
    assert_eq!(r.slot("running"), Some(&Value::Bool(false)));
    press(&mut r, "reset");
    assert_eq!(r.slot("count"), Some(&Value::Number(100.0)));
    assert_eq!(r.slot("revision"), Some(&Value::Number(0.0)));
}

#[test]
fn typing_does_not_change_history_and_one_local_echo_is_retained() {
    let mut r = boot();
    r.act("editDraft", vec![Value::str("Hello 🦀")]).unwrap();
    assert_eq!(r.slot("revision"), Some(&Value::Number(0.0)));
    assert!(r.kernel().find_by_test_id("message-local-echo").is_empty());
    press(&mut r, "send");
    assert_eq!(r.slot("draft"), Some(&Value::str("")));
    assert_eq!(r.kernel().find_by_test_id("message-local-echo").len(), 1);
    r.act("editDraft", vec![Value::str(&"x".repeat(513))])
        .unwrap();
    assert_eq!(r.slot("draft"), Some(&Value::str("")));
    assert_eq!(r.slot("draftTooLong"), Some(&Value::Bool(true)));
    r.act("editDraft", vec![Value::str("second")]).unwrap();
    press(&mut r, "send");
    assert_eq!(r.kernel().find_by_test_id("message-local-echo").len(), 1);
}

#[test]
fn data_seam_rejects_malformed_inputs_without_allocating_a_large_history() {
    for number in [f64::NAN, f64::INFINITY, -1.0, 100.5, 100_001.0] {
        assert!(MessagesStress
            .query(
                "history",
                &[
                    Value::Number(number),
                    Value::Number(0.0),
                    Value::Number(1.0),
                    Value::str(""),
                    Value::Number(0.0),
                    Value::Bool(true)
                ]
            )
            .is_err());
    }
    assert!(MessagesStress.query("history", &[]).is_err());
    assert!(MessagesStress.query("unknown", &[]).is_err());
    let answer = MessagesStress
        .query(
            "history",
            &[
                Value::Number(100.0),
                Value::Number(2.0),
                Value::Number(8.0),
                Value::str(""),
                Value::Number(0.0),
                Value::Bool(true),
            ],
        )
        .unwrap();
    let Value::Record(fields) = answer else {
        panic!("history shape")
    };
    assert_eq!(fields[1], Value::Number(100.0));
    assert_eq!(fields[2], Value::Number(2.0));
    assert_eq!(fields[3], Value::Number(8.0));
    assert!(fields[4].as_number().unwrap() > 0.0);
}

//! `showNotification(title=, body=, tag=, showTrigger=)` and
//! `closeNotification(tag)` (rules/DEFERRED.md, 2026-10-04): named arguments
//! lower as `(title, body, tag, showTrigger)`; every host's arm refuses
//! without `device.notifications`, lists under the agent (a tag replacing
//! its older one, `closeNotification` taking it away), or presents.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::notify::{arm, close, Arm, Notice};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

#[derive(Default)]
struct Grants(&'static str);

impl DataSource for Grants {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        self.0
    }
}

fn boot(grants: &'static str) -> Runner<Grants> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/notify.contract");
    let plan = contract::compile(&std::fs::read_to_string(path).unwrap()).unwrap();
    let plan = contract::bake(plan, Grants(grants)).unwrap();
    Runner::boot(
        plan,
        Grants(grants),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn press(r: &mut Runner<Grants>, test_id: &str) -> Vec<Value> {
    let key = r.kernel().find_by_test_id(test_id)[0];
    let id = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
    r.take_commands().pop().unwrap().args
}

#[test]
fn named_arguments_lower_in_the_notification_api_order() {
    let mut r = boot("");
    assert_eq!(
        press(&mut r, "remind"),
        vec![
            Value::str("Stretch"),
            Value::str("Five minutes, at your desk"),
            Value::str("stretch"),
            Value::Number(1.8e12)
        ]
    );
    let none = Value::Option(None);
    assert_eq!(
        press(&mut r, "now"),
        vec![Value::str("Price alert"), none.clone(), none.clone(), none]
    );
    assert_eq!(press(&mut r, "stop"), vec![Value::str("stretch")]);
}

#[test]
fn every_host_arm_refuses_lists_or_presents_by_one_rule() {
    let mut r = boot("");
    let args = press(&mut r, "remind");
    assert_eq!(
        arm(&mut r, Notice::from_args(&args), true, true),
        Arm::Refused("the grants name no device.notifications".into())
    );
    let mut r = boot("device.notifications purpose.notifications");
    let args = press(&mut r, "remind");
    // Linux: none outside the agent.
    assert_eq!(
        arm(&mut r, Notice::from_args(&args), false, false),
        Arm::Refused("unavailable".into())
    );
    assert_eq!(
        arm(&mut r, Notice::from_args(&args), false, true),
        Arm::Present
    );
    // Under the agent: listed, the newer of a tag replacing the older.
    for _ in 0..2 {
        assert_eq!(
            arm(&mut r, Notice::from_args(&args), true, false),
            Arm::Listed
        );
    }
    let now = press(&mut r, "now");
    arm(&mut r, Notice::from_args(&now), true, false);
    let state = exact_runner::agent::state(&r);
    assert!(
        state.contains(r#""notifications":[{"title":"Stretch","body":"Five minutes, at your desk","tag":"stretch","showTrigger":1800000000000},{"title":"Price alert","body":null,"tag":null,"showTrigger":null}]"#),
        "{state}"
    );
    assert_eq!(close(&mut r, "stretch", true), Arm::Listed);
    assert_eq!(r.notifications().len(), 1);
    let logs = exact_runner::agent::logs(&r, 0);
    for line in [
        "showNotification: refused: unavailable",
        "showNotification: listed (stretch)",
        "showNotification: listed",
    ] {
        assert!(logs.contains(line), "{line} in {logs}");
    }
}

//! `share(title=, text=, url=)` (LLP 1069.003): named arguments lower as
//! `(title, text, url)`; the command carries the node whose press ran it
//! (D3); under the agent it is a held request whose answer is a journal line
//! (D2, D6).

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::share::{arm, Arm, Share};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot() -> Runner<NoData> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/share.contract");
    let plan = contract::compile(&std::fs::read_to_string(path).unwrap()).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn press(r: &mut Runner<NoData>, test_id: &str) -> u32 {
    let key = r.kernel().find_by_test_id(test_id)[0];
    let id = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
    id
}

#[test]
fn named_arguments_lower_in_the_web_share_order_with_the_pressed_node() {
    let mut r = boot();
    let pressed = press(&mut r, "share-link");
    let c = r.take_commands().pop().unwrap();
    assert_eq!(c.name, "share");
    assert_eq!(
        c.args,
        vec![
            Value::str("A post"),
            Value::Option(None),
            Value::str("https://example.com/post/1")
        ]
    );
    assert_eq!(c.source, Some(pressed));
    let pressed = press(&mut r, "share-text");
    let c = r.take_commands().pop().unwrap();
    assert_eq!(c.args[1], Value::str("hello from exact2"));
    assert_eq!(
        (c.args[2].clone(), c.source),
        (Value::Option(None), Some(pressed))
    );
}

#[test]
fn every_host_arm_refuses_holds_or_presents_by_one_rule() {
    let mut r = boot();
    press(&mut r, "share-relative");
    let c = r.take_commands().pop().unwrap();
    let refused = arm(&mut r, Share::from_args(&c.args), c.source, false, true);
    assert!(matches!(refused, Arm::Refused(_)), "{refused:?}");
    let pressed = press(&mut r, "share-link");
    let c = r.take_commands().pop().unwrap();
    // Linux: no sheet outside the agent.
    let none = arm(&mut r, Share::from_args(&c.args), c.source, false, false);
    assert_eq!(none, Arm::Refused("unavailable".into()));
    assert!(matches!(
        arm(&mut r, Share::from_args(&c.args), c.source, false, true),
        Arm::Present(_)
    ));
    let Arm::Held(ticket) = arm(&mut r, Share::from_args(&c.args), c.source, true, false) else {
        panic!("the agent holds a share on every host");
    };
    let hold = &r.device_holds()[0];
    assert_eq!(
        hold.args,
        format!(
            r#"{{"title":"A post","text":null,"url":"https://example.com/post/1","anchor":{pressed}}}"#
        )
    );
    let reply = exact_runner::agent::answer(
        &mut r,
        &format!(r#"{{"op":"tap","ticket":{ticket},"choice":"shared"}}"#),
    )
    .unwrap()
    .0;
    assert!(reply.contains(r#""delivery":"substituted""#), "{reply}");
    let logs = exact_runner::agent::logs(&r, 0);
    for line in [
        "share: refused: url is not an absolute",
        "share: refused: unavailable",
        "device share",
        "share: shared",
    ] {
        assert!(logs.contains(line), "{line} in {logs}");
    }
    // The menu row closes the menu with its press; the hold outlives it.
    press(&mut r, "menu-share");
    let c = r.take_commands().pop().unwrap();
    let Arm::Held(ticket) = arm(&mut r, Share::from_args(&c.args), c.source, true, false) else {
        panic!("held");
    };
    assert_eq!(r.device_holds().len(), 1);
    exact_runner::agent::answer(
        &mut r,
        &format!(r#"{{"op":"tap","ticket":{ticket},"choice":"cancel"}}"#),
    )
    .unwrap();
    assert!(exact_runner::agent::logs(&r, 0).contains("share: dismissed"));
}

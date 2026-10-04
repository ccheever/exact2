//! LLP 1069.011 §9: a control's viewless contents (a native button's face, a
//! select's options) cross as no op on any view, so the batch that changes
//! them says `"controls":true` and the presenter configures its controls;
//! a batch that changes nothing a control reads does not.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

#[test]
fn a_change_to_a_controls_viewless_contents_says_controls() {
    let src = "component App\n  state on = false\n  state other = false\n  action flip\n    on = not on\n  action poke\n    other = not other\n  view\n    column\n      button appearance=\"auto\" buttonStyle=\"bordered\" press=poke testId=\"native\"\n        text (on ? \"Sent\" : \"Send\")\n      select testId=\"choice\" value=\"a\"\n        option value=\"a\" disabled=on\n          text \"A\"\n        option value=\"b\"\n          text \"B\"\n      button press=flip testId=\"flip\"\n        text \"Flip\"\n      text (other ? \"Yes\" : \"No\") testId=\"other\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    // The native button's title text and the option's `disabled` change.
    let flip = view(&host, "flip");
    let changed = host.dispatch_at(flip, Event::Press, 0.0);
    assert!(changed.contains("\"controls\":true"), "{changed}");
    // A change no control reads says nothing of controls.
    let native = view(&host, "native");
    let other = host.dispatch_at(native, Event::Press, 1.0);
    assert!(!other.contains("\"controls\":true"), "{other}");
}

/// Each producer on its own: a button's face text alone, an option's
/// `disabled` alone, and a `when` that swaps a button's face (its children).
#[test]
fn each_viewless_change_alone_says_controls() {
    for (label, src) in [
        ("face text", "component App\n  state on = false\n  action flip\n    on = not on\n  view\n    column\n      button appearance=\"auto\" buttonStyle=\"bordered\" testId=\"native\"\n        text (on ? \"Sent\" : \"Send\")\n      button press=flip testId=\"flip\"\n        text \"Flip\"\n"),
        ("option", "component App\n  state on = false\n  action flip\n    on = not on\n  view\n    column\n      select testId=\"choice\" value=\"a\"\n        option value=\"a\" disabled=on\n          text \"A\"\n        option value=\"b\"\n          text \"B\"\n      button press=flip testId=\"flip\"\n        text \"Flip\"\n"),
        ("face swap", "component App\n  state on = false\n  action flip\n    on = not on\n  view\n    column\n      button appearance=\"auto\" buttonStyle=\"bordered\" aria-label=\"Go\" testId=\"native\"\n        when on\n          image \"symbol:sf/checkmark\"\n        else\n          text \"Go\"\n      button press=flip testId=\"flip\"\n        text \"Flip\"\n"),
    ] {
        let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
        let (mut host, _) = Host::boot(
            &plan.encode(),
            NoData,
            Box::new(MonospaceMeasurer::default()),
            402.0,
            874.0,
        )
        .unwrap();
        let flip = view(&host, "flip");
        let changed = host.dispatch_at(flip, Event::Press, 0.0);
        assert!(changed.contains("\"controls\":true"), "{label}: {changed}");
    }
}

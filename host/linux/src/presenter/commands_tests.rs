//! The commands Linux refuses say so in the journal, where an agent reads them.
use super::*;
use crate::presenter::PainterChoice;
use std::path::PathBuf;

const APP: &str = r#"component App
  action start
    startTrace_EXPERIMENTAL()
  action stop
    stopTrace_EXPERIMENTAL(7500)
  view
    column
      button press=start testId="start" height=30
        text "Start"
      button press=stop testId="stop" height=30
        text "Stop"
"#;

fn tap(p: &mut Presenter<()>, name: &str) {
    let kernel = p.host().kernel();
    let view = kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id;
    p.tap(view).unwrap();
    p.run_commands(|| ());
}

#[test]
fn a_trace_command_is_refused_in_the_journal() {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    tap(&mut p, "start");
    tap(&mut p, "stop");
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    for name in ["startTrace_EXPERIMENTAL", "stopTrace_EXPERIMENTAL"] {
        assert!(
            journal.contains(&format!(
                "{name} refused: trace recording is on Apple hosts only"
            )),
            "{journal}"
        );
    }
}

// `openURL(url, id)`: `change` at `id` hears whether the URL opened; the
// headless host opens nothing, so it hears `failed`.
#[test]
fn an_open_is_answered_at_the_named_element() {
    const OPEN: &str = r#"component App
  state heard = ""
  action open
    openURL("https://example.com/", "opened")
  action answered(value: string)
    heard = value
  view
    column
      button press=open testId="open" height=30
        text "Open"
      input id="opened" display="none" change=answered
      text heard testId="heard"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(OPEN),
        (),
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    tap(&mut p, "open");
    let tree = crate::agent::handle(&mut p, r#"{"op":"tree"}"#);
    assert!(tree.contains("\"failed\""), "{tree}");
}

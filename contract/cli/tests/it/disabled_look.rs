//! @ref LLP 1115.000 D6 (amended 2026-10-13, Charlie's ruling): Contract marks
//! a node that styles its own disabled state (`disabledStyled`), and the
//! hosts that dim a disabled custom pressable leave it as its author drew it.
//! The rule (`contract/lower/src/disabled_look.rs`): `disabled`, and an
//! `opacity` written, or a paint style that reads a name `disabled` reads.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = r##"style Faint
  opacity=0.4

component App
  state busy = false
  state dirty = false
  state blue = "#06f"
  action go
    busy = true
  view
    column
      button press=go disabled=busy opacity=(busy ? 0.6 : 1) testId="opacity"
        text "a"
      button press=go disabled=(busy or not dirty) background-color=(dirty ? "#06f" : "#eee") testId="reads"
        text "a"
      button press=go disabled=busy color=`${busy ? "gray" : "black"}` testId="template"
        text "a"
      button press=go disabled=busy class=Faint testId="class"
        text "a"
      button press=go disabled=true opacity=0.5 testId="literal-opacity"
        text "a"
      button press=go disabled=busy background-color="#eee" testId="static-paint"
        text "a"
      button press=go disabled=busy background-color=blue testId="other-name"
        text "a"
      button press=go disabled=true background-color="#eee" testId="literal-paint"
        text "a"
      button press=go opacity=0.5 testId="never-disabled"
        text "a"
      button press=go disabled=busy width=(busy ? 10 : 20) testId="no-paint"
        text "a"
      button press=go disabled=busy border=(busy ? "1px solid #ccc" : "1px solid #06f") testId="border"
        text "a"
      button press=go disabled=busy color=join(map(["red"], busy => busy), "") testId="shadowed"
        text "a"
"##;

#[test]
fn a_node_that_styles_its_own_disabled_state_is_marked_and_no_other() {
    let plan = contract::compile(APP).unwrap_or_else(|e| panic!("{e}"));
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let marked = |id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .bool(PropId::DisabledStyled)
            == Some(true)
    };
    for id in [
        "opacity",
        "reads",
        "template",
        "class",
        "literal-opacity",
        "border",
    ] {
        assert!(marked(id), "{id}: marked");
    }
    for id in [
        "static-paint",
        "other-name",
        "literal-paint",
        "never-disabled",
        "no-paint",
        // A callback's parameter spelled like the state reads nothing of it.
        "shadowed",
    ] {
        assert!(!marked(id), "{id}: not marked");
    }
}

//! `-exact-return-presses` (LLP 1115.000 D7.3): a container's opt-in to
//! Chromium's Return on a focused button, which the Mac host reads.

use exact_kernel::{Kernel, PropId};
use exact_runner::Runner;

fn app(value: &str) -> String {
    format!(
        "component App\n  state on = \"descendants\"\n  view\n    column -exact-return-presses={value} testId=\"transcript\"\n      button \"Copy\"\n"
    )
}

#[test]
fn the_container_carries_the_word_to_the_host() {
    let plan = contract::compile(&app("\"descendants\"")).unwrap_or_else(|e| panic!("{e}"));
    let r = Runner::boot(plan, (), Kernel::with_monospace(), Default::default(), "/").unwrap();
    let key = r.kernel().find_by_test_id("transcript")[0];
    let id = r.kernel().node_by_key(key).unwrap().id;
    assert_eq!(
        r.kernel()
            .node(id)
            .unwrap()
            .props
            .str(PropId::ReturnPresses),
        Some("descendants")
    );
}

#[test]
fn it_takes_descendants_or_none() {
    for value in ["\"descendants\"", "\"none\"", "on"] {
        contract::compile(&app(value)).unwrap_or_else(|e| panic!("{value}: {e}"));
    }
    for value in ["\"\"", "\"all\"", "\"children\""] {
        let error = contract::compile(&app(value)).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value") && error.contains("`-exact-return-presses` takes"),
            "{value}: {error}"
        );
    }
}

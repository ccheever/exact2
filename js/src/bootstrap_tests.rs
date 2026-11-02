//! The prelude's bootstrap: hardening and what it installs.
use super::Module;

#[test]
fn hardening_refuses_a_reachable_abort_hook_handoff() {
    let mut module = Module::new(
        include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
        "test.pure",
        "",
    );
    let error = match module.load_engine_with_prelude(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/prelude-with-abort-hook.hbc"
    ))) {
        Ok(_) => panic!("a reachable abort hook handoff must refuse hardening"),
        Err(error) => error,
    };
    assert!(error.contains("refusing to harden"), "{error}");
    assert!(error.contains("abort hooks global"), "{error}");
    assert!(error.contains("__exact_ibex2_abort_hooks"), "{error}");
}

#[test]
fn hardening_refuses_abort_hooks_a_proxy_get_trap_hides() {
    let mut module = Module::new(
        include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
        "test.pure",
        "",
    );
    let error = match module.load_engine_with_prelude(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/prelude-with-hidden-abort-hook.hbc"
    ))) {
        Ok(_) => panic!("abort hooks behind a Proxy must refuse hardening"),
        Err(error) => error,
    };
    assert!(error.contains("refusing to harden"), "{error}");
    assert!(error.contains("still reachable"), "{error}");
}

// What the freeze leaves open after a bootstrap that hides a payload
// behind a Proxy's `get` trap, another behind a symbol it makes
// Reflect.ownKeys skip, and plants a property on Object.prototype
// (js/tests/fixtures/harden-probe.js).
#[test]
fn hardening_freezes_what_descriptors_reach() {
    let mut module = Module::new(
        include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
        "test.pure",
        "",
    );
    let mut engine = module
        .load_engine_with_prelude(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/prelude-with-harden-probe.hbc"
        )))
        .expect("the probe prelude hardens");
    let report: serde_json::Value =
        serde_json::from_str(&engine.call("__exact_probe", ["", "", ""]).unwrap()).unwrap();
    assert_eq!(report["open"], 0, "{report}");
    assert!(report["objects"].as_u64().unwrap() > 800, "{report}");
    assert_eq!(report["payload"], true, "{report}");
    // Published under a symbol after bootstrap made Reflect.ownKeys list
    // only names.
    assert_eq!(report["symbolic"], true, "{report}");
    // The week tables, built after hardening, take a name planted on the
    // frozen Object.prototype.
    assert_eq!(report["firstDay"], 7, "{report}");
}

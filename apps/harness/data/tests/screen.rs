//! The real harness screen boots against this data module (LLP 1101.002
//! P2): `terminal.contract`, compiled as the terminal host compiles it,
//! takes `session()` and `animation(...)` through the shape check.

use exact_kernel::Kernel;
use exact_runner::Runner;
use harness_data::shapes::{ContractValue, Session};
use harness_data::{Harness, Options};

#[test]
fn the_real_screen_boots_and_reads_the_session() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../terminal.contract");
    let plan = contract::compile_path_terminal(&path).unwrap_or_else(|errors| {
        panic!(
            "terminal.contract does not compile: {}",
            errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let dir = std::env::temp_dir().join(format!("exact-harness-screen-{}", std::process::id()));
    let harness = Harness::with_options(Options::offline(Some(dir.clone())));
    let mut r = Runner::boot(
        plan,
        harness,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap_or_else(|e| panic!("the screen refused to boot: {e:?}"));
    r.data_ready().unwrap();
    let session = Session::from_value(r.resource("session").expect("a session resource"))
        .expect("the session is a Session");
    assert_eq!(session.entries[0].kind, "banner");
    assert!(session.models.iter().any(|m| m.id == "mock"));
    let _ = std::fs::remove_dir_all(dir);
}

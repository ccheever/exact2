//! Verify the actual build-script output, not a second selection recipe.

use exact_logic::{exact_plan::Value, exact_runner::DataSource};
use std::rc::Rc;

mod dev_factory {
    include!(concat!(env!("OUT_DIR"), "/factory.rs"));

    pub fn source() -> impl exact_logic::exact_runner::DataSource {
        app_data()
    }

    pub fn default_source() -> impl exact_logic::exact_runner::DataSource {
        fn construct<D: Default>() -> D {
            D::default()
        }
        construct::<AppData>()
    }
}

#[test]
fn generated_runtime_factory_matches_compile_selected_mode() {
    let (source, constructor) = match option_env!("EXACT_MESSAGES_SOURCE").unwrap_or("reuse") {
        "reuse" => (
            "messages_stress_data::ReusableMessagesStress",
            "messages_stress_data::ReusableMessagesStress::default()",
        ),
        "stateless" => (
            "messages_stress_data::MessagesStress",
            "messages_stress_data::MessagesStress",
        ),
        mode => panic!("unvalidated compile-time selector {mode:?}"),
    };
    let entry = include_str!(concat!(env!("OUT_DIR"), "/entry.rs"));
    assert!(entry.starts_with(include_str!(concat!(env!("OUT_DIR"), "/factory.rs"))));
    assert!(
        entry.contains(&format!("type AppData = {source};")),
        "{entry}"
    );
    assert!(entry.contains(&format!("fn app_data() -> AppData {{ {constructor} }}")));
}

#[test]
fn compiled_factory_and_dev_default_preserve_values_and_select_row_ownership() {
    fn check(source: &mut impl DataSource) {
        let mut control = messages_stress_data::MessagesStress;
        assert_eq!(source.app_id(), control.app_id());
        assert_eq!(source.grants(), control.grants());
        let args = |revision| {
            vec![
                Value::Number(10_000.0),
                Value::Number(revision),
                Value::Number(32.0),
                Value::str(""),
                Value::Number(0.0),
                Value::Bool(true),
            ]
        };
        let before = source.query("history", &args(0.0)).unwrap();
        let after = source.query("history", &args(1.0)).unwrap();
        assert_eq!(before, control.query("history", &args(0.0)).unwrap());
        assert_eq!(after, control.query("history", &args(1.0)).unwrap());
        let rows = |value: Value| {
            let Value::Record(fields) = value else {
                panic!()
            };
            let Value::List(rows) = &fields[0] else {
                panic!()
            };
            rows.clone()
        };
        let before = rows(before);
        let after = rows(after);
        assert_eq!(before.len(), 10_000);
        assert_eq!(after.len(), 10_000);
        let shared = before
            .iter()
            .zip(after.iter())
            .filter(
                |(a, b)| matches!((a, b), (Value::Record(a), Value::Record(b)) if Rc::ptr_eq(a, b)),
            )
            .count();
        let expected = match option_env!("EXACT_MESSAGES_SOURCE").unwrap_or("reuse") {
            "reuse" => 9_968,
            "stateless" => 0,
            _ => unreachable!(),
        };
        assert_eq!(shared, expected);
    }
    check(&mut dev_factory::source());
    check(&mut dev_factory::default_source());
}

#[test]
fn built_dev_ignores_runtime_selector_and_writes_the_same_baked_plan() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let path = std::env::temp_dir().join(format!(
        "exact-messages-factory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../app.contract");
    let plan = scratch.0.join("app.plan");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_dev"))
        .args([
            source.as_os_str(),
            plan.as_os_str(),
            std::ffi::OsStr::new("--once"),
        ])
        .env("EXACT_MESSAGES_SOURCE", "invalid-at-runtime")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        std::fs::read(plan).unwrap(),
        include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"))
    );
}

//! A pinned TypeScript module behind the snapshot runner, with real file reads
//! and a host-supplied fetch response. No UI boot or writes (LLP 1126.000 WP5).
#![cfg(exact_js_engine)]
use exact_js::Module;
use exact_kernel::Kernel;
use exact_runner::{DataSource, Outcome, Response, Runner, Work};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/snapshot.hbc"));
const APP: &str = "test.widget-js";
const GRANTS: &str =
    "net.fetch https://widget.test\nfs.read app:/data\nfs.write app:/data\nsecret.keep session\n";
fn receipt() -> String {
    serde_json::json!({"version":1,"abi":1,"bytecodeVersion":exact_js::BYTECODE_VERSION,
        "appId":APP,"grants":GRANTS,"module":{"file":"app.hbc","bytes":HBC.len(),
        "sha256":format!("{:x}",Sha256::digest(HBC))}})
    .to_string()
}
fn plan() -> exact_plan::Plan {
    contract::snapshot::compile("shape Props\n  title: string\ncomponent Card\n  props\n    title: string\n  view\n    text title\n", &contract::Widget {
        kind:"card".into(), title:None, description:None, families:vec!["systemSmall".into()],
        component:"Card".into(), props:"Props".into(), refresh:"refresh".into(),
        preview:serde_json::json!({"title":"preview"})
    }).unwrap()
}
fn runner(root: &super::storage::Root) -> Runner<Module> {
    let mut module = Module::new(Vec::new(), APP, GRANTS);
    module.set_budget_ms(f64::INFINITY);
    module
        .prepare_snapshot(Some((&receipt(), HBC.to_vec())))
        .unwrap();
    for name in ["data", "cache", "tmp"] {
        std::fs::create_dir_all(root.0.join(name)).unwrap();
    }
    module
        .configure_storage(
            root.0.join("data"),
            root.0.join("cache"),
            root.0.join("tmp"),
        )
        .unwrap();
    Runner::boot(
        plan(),
        module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn refresh(runner: &mut Runner<Module>, date: f64) -> Result<String, String> {
    runner.snapshot_timeline(
        date,
        vec![("session".into(), b"kept secret".to_vec())],
        Instant::now() + Duration::from_secs(30),
        |request, work| {
            if let Some(Work::Now(work)) = work {
                return Ok(work());
            }
            assert!(work.is_none());
            assert_eq!(request.url, "https://widget.test/value");
            Ok(Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: b"from fetch".to_vec(),
            }))
        },
    )
}
#[test]
fn now_and_fetch_and_file_read_return_typed_timelines() {
    let root = super::storage::Root::new();
    let mut r = runner(&root);
    std::fs::write(root.0.join("data/value"), b"saved value").unwrap();
    for (date, title) in [(1., "kept secret"), (2., "from fetch"), (3., "saved value")] {
        let result = refresh(&mut r, date).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap(),
            serde_json::json!([{"date":date as i64,"props":{"title":title}}])
        );
    }
    assert!(!r.data().take_storage_changed());
}
#[test]
fn snapshot_cannot_write_files_or_secrets_and_can_refresh_after_a_refusal() {
    let root = super::storage::Root::new();
    let mut r = runner(&root);
    std::fs::write(root.0.join("data/value"), b"saved value").unwrap();
    assert!(refresh(&mut r, 4.).is_err());
    assert_eq!(
        std::fs::read(root.0.join("data/value")).unwrap(),
        b"saved value"
    );
    assert!(refresh(&mut r, 5.).unwrap_err().contains("cannot write"));
    assert!(refresh(&mut r, 1.).unwrap().contains("kept secret"));
}
#[test]
fn snapshot_requires_the_pinned_bytes_and_identity() {
    let mut m = Module::new(HBC.to_vec(), APP, GRANTS);
    assert!(m.prepare_snapshot(None).is_err());
    let mut bytes = HBC.to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    assert!(m.prepare_snapshot(Some((&receipt(), bytes))).is_err());
    let wrong = receipt().replace(APP, "another.app");
    assert!(m.prepare_snapshot(Some((&wrong, HBC.to_vec()))).is_err());
}
#[test]
fn expired_refresh_never_dispatches_io() {
    let root = super::storage::Root::new();
    let mut r = runner(&root);
    let error = r.snapshot_timeline(2., vec![], Instant::now(), |_, _| panic!("deadline passed"));
    assert!(error.unwrap_err().contains("timed out"));
}

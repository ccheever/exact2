//! The resident dev driver: a save is observed, compiled, baked, and on
//! disk; an unchanged save is nothing; a broken save is a named refusal.

use exact_runner::{DataError, DataSource, Value};
use exact_web::dev::Session;

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

const GOOD: &str = "component App\n  view\n    column testId=\"root\"\n      text \"one\"\n";

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("exact-dev-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_save_becomes_a_plan_and_an_identical_save_is_nothing() {
    let dir = scratch("save");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    std::fs::write(&src, GOOD).unwrap();
    let mut s = Session::new(&src, &out);
    let built = s.poll::<NoData>().expect("first look builds").unwrap();
    assert!(built.compile_ms < 100.0 && built.bake_ms < 100.0);
    assert_eq!(std::fs::read(&out).unwrap(), built.bytes);
    assert!(exact_plan::Plan::decode(&built.bytes).is_ok());
    assert!(s.poll::<NoData>().is_none(), "nothing changed");
    // Same bytes, new mtime: not a change.
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&src, GOOD).unwrap();
    assert!(
        s.poll::<NoData>().is_none(),
        "identical content is not an edit"
    );
    // A real edit.
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&src, GOOD.replace("\"one\"", "\"two\"")).unwrap();
    let again = s.poll::<NoData>().expect("an edit builds").unwrap();
    assert_ne!(again.bytes, built.bytes);
    assert!(again.saved_ms > built.saved_ms);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_broken_save_is_a_named_refusal_and_the_last_plan_stays() {
    let dir = scratch("broken");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    std::fs::write(&src, GOOD).unwrap();
    let mut s = Session::new(&src, &out);
    let good = s.poll::<NoData>().unwrap().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(
        &src,
        "component App\n  view\n    column\n      text one two\n",
    )
    .unwrap();
    let err = s.poll::<NoData>().unwrap().err().unwrap();
    assert!(err.contains("app.contract:"), "{err}");
    assert_eq!(
        std::fs::read(&out).unwrap(),
        good.bytes,
        "the page keeps the last good plan"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_bridge_boots_from_bytes_in_its_input_buffer() {
    let plan = contract::compile(GOOD).unwrap().encode();
    let mut bridge: exact_web::abi::Bridge<NoData> = exact_web::abi::Bridge::new();
    let ptr = bridge.input(plan.len());
    assert!(!ptr.is_null());
    // Natively the test writes through the safe path the glue's write is
    // equivalent to: the buffer is the bridge's own Vec.
    let n = bridge.input_write(&plan);
    let len = bridge.boot_plan(n, NoData);
    let batch = String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap();
    assert!(batch.starts_with("{\"ops\":[{\"op\":\"create\""), "{batch}");
    assert!(batch.contains("\"text\":\"one\""), "{batch}");
}

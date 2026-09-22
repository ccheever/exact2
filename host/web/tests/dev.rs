//! The resident dev driver: a save is observed, compiled, baked, and on
//! disk; an unchanged save is nothing; a broken save is a named refusal.

use exact_runner::{DataError, DataSource, Value};
use exact_web::dev::Session;
use exact_web::Host;
use std::io::{Seek, Write};

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
fn a_failed_save_is_re_read_until_its_bytes_stabilize() {
    let dir = scratch("failed-stamp");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    let bad = GOOD.replace("text \"one\"", "text =one=");
    assert_eq!(bad.len(), GOOD.len());
    std::fs::write(&src, bad).unwrap();
    let stamp = std::fs::metadata(&src).unwrap().modified().unwrap();
    let mut s = Session::new(&src, &out);
    assert!(s.poll::<NoData>().unwrap().is_err());

    // Reproduce a save whose final bytes changed without changing the
    // watcher's (mtime, length) signature.
    let mut file = std::fs::OpenOptions::new().write(true).open(&src).unwrap();
    file.rewind().unwrap();
    file.write_all(GOOD.as_bytes()).unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(stamp))
        .unwrap();
    drop(file);
    assert_eq!(std::fs::metadata(&src).unwrap().modified().unwrap(), stamp);

    assert!(
        s.poll::<NoData>()
            .expect("the same stamp is re-read")
            .is_ok(),
        "the stable source replaces the transient compile error"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn surface_declarations_recheck_an_unchanged_source_and_preserve_the_last_plan() {
    let dir = scratch("surface-declaration");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    let source = "component App\n  view\n    canvas surface=world(seed=7)\n";
    std::fs::write(&src, source).unwrap();
    let mut session = Session::new(&src, &out);
    let first = session.poll::<NoData>().unwrap().unwrap();
    std::fs::create_dir_all(dir.join(".shells")).unwrap();
    let declaration = dir.join(".shells/surfaces.json");
    std::fs::write(&declaration, r#"{"world":[{"name":"seeds"}]}"#).unwrap();
    let error = session.poll::<NoData>().unwrap().unwrap_err();
    assert!(
        error.contains("analyze-surface-arguments") && error.contains("`seed`"),
        "{error}"
    );
    assert_eq!(std::fs::read(&out).unwrap(), first.bytes);
    assert!(
        session.poll::<NoData>().is_none(),
        "the same error is not repeated"
    );
    std::fs::write(&declaration, r#"{"world":[{"name":"seed"}]}"#).unwrap();
    let fixed = session
        .poll::<NoData>()
        .expect("only the interface changed")
        .unwrap();
    assert_eq!(fixed.bytes, first.bytes);
    assert!(session.poll::<NoData>().is_none());

    std::fs::write(&src, source.replace("seed=", "typo=")).unwrap();
    assert!(session
        .poll::<NoData>()
        .unwrap()
        .unwrap_err()
        .contains("`typo`"));
    assert_eq!(std::fs::read(&out).unwrap(), first.bytes);
    std::fs::write(&declaration, "{").unwrap();
    let error = session.poll::<NoData>().unwrap().unwrap_err();
    assert!(
        error.contains("surfaces.json:") && error.contains("analyze-surface-declaration"),
        "{error}"
    );
    std::fs::remove_file(&declaration).unwrap();
    assert!(
        session.poll::<NoData>().unwrap().is_ok(),
        "removing the optional interface restores runtime binding"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_source_snapshot_keeps_its_path_for_declared_fonts() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/fixtures/fonts/app.contract");
    let dir = scratch("font-path");
    let out = dir.join("app.plan");
    let mut session = Session::new(&source, &out);
    let built = session
        .poll::<NoData>()
        .expect("first look builds")
        .expect("the snapshot resolves assets beside its source path");
    assert!(exact_plan::Plan::decode(&built.bytes).is_ok());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_bridge_boots_from_bytes_in_its_input_buffer() {
    let plan = contract::compile(GOOD).unwrap().encode();
    let mut bridge: exact_web::abi::Bridge<NoData> = exact_web::abi::Bridge::new();
    let len = bridge.baked_plan(&plan);
    assert_eq!(bridge.output_bytes(len as usize), plan);
    let ptr = bridge.input(plan.len());
    assert!(!ptr.is_null());
    // Natively the test writes through the safe path the glue's write is
    // equivalent to: the buffer is the bridge's own Vec.
    let n = bridge.input_write(&plan);
    let len = bridge.boot_plan(n, NoData, 390.0, 844.0, "/");
    let batch = String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap();
    assert!(batch.starts_with("{\"ops\":[{\"op\":\"create\""), "{batch}");
    assert!(batch.contains("\"text\":\"one\""), "{batch}");
}

const COUNTER: &str = "component App\n  state n = 1\n  action inc writes n\n    n = n + 1\n  view\n    column testId=\"root\"\n      button press=inc aria-label=\"Inc\" testId=\"inc\"\n        text \"Inc\"\n      text `${n}` testId=\"n\"\n";

fn view_of(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

fn text_of(host: &Host<NoData>, test_id: &str) -> String {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .str(exact_kernel::PropId::Text)
        .unwrap()
        .to_string()
}

#[test]
fn a_reload_carries_state_by_name_where_the_type_still_fits() {
    let plan = contract::compile(COUNTER).unwrap().encode();
    let (mut host, _) = Host::boot(&plan, NoData, Default::default(), "/").unwrap();
    let inc = view_of(&host, "inc");
    for _ in 0..3 {
        host.dispatch(inc, exact_runner::Event::Press);
    }
    assert_eq!(text_of(&host, "n"), "4");
    let carried = host.carry();

    // The same shape: the count survives.
    let (host, batch) =
        Host::boot_with(&plan, NoData, Some(&carried), Default::default(), "/").unwrap();
    assert_eq!(text_of(&host, "n"), "4");
    assert!(
        batch.contains("\"text\":\"4\""),
        "the first batch already shows it: {batch}"
    );

    // The slot's type changed: the carried number does not fit a string,
    // so the slot starts from its new initializer.
    let restrung = contract::compile(
        &COUNTER
            .replace("state n = 1", "state n = \"x\"")
            .replace("n = n + 1", "n = n + \"!\""),
    )
    .unwrap()
    .encode();
    let (host, _) =
        Host::boot_with(&restrung, NoData, Some(&carried), Default::default(), "/").unwrap();
    assert_eq!(text_of(&host, "n"), "x");

    // The slot was renamed: nothing carries to it.
    let renamed = contract::compile(
        &COUNTER
            .replace("state n = 1", "state m = 1")
            .replace("writes n", "writes m")
            .replace("n = n + 1", "m = m + 1")
            .replace("${n}", "${m}"),
    )
    .unwrap()
    .encode();
    let (host, _) =
        Host::boot_with(&renamed, NoData, Some(&carried), Default::default(), "/").unwrap();
    assert_eq!(text_of(&host, "n"), "1");
}

#[test]
fn the_bridge_carries_state_across_boots_from_bytes() {
    let plan = contract::compile(COUNTER).unwrap().encode();
    let mut bridge: exact_web::abi::Bridge<NoData> = exact_web::abi::Bridge::new();
    let n = bridge.input_write(&plan);
    let len = bridge.boot_plan(n, NoData, 390.0, 844.0, "/");
    let batch = String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap();
    // The button's view id, from its create op.
    let at = batch.find("\"data-testid\":\"inc\"").unwrap();
    let head = &batch[..at];
    let marker = "\"op\":\"create\",\"id\":";
    let id_at = head.rfind(marker).unwrap() + marker.len();
    let inc: u32 = head[id_at..].split(',').next().unwrap().parse().unwrap();
    for _ in 0..2 {
        bridge.dispatch(inc, 0, 0, 0.0);
    }
    let n = bridge.input_write(&plan);
    let len = bridge.boot_plan(n, NoData, 390.0, 844.0, "/");
    let batch = String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap();
    assert!(
        batch.contains("\"text\":\"3\""),
        "the second boot carried the count: {batch}"
    );
}

#[test]
fn static_serving_falls_back_to_an_interrupted_previous_build() {
    let dir = scratch("serve-previous");
    let dist = dir.join("dist");
    let previous = dir.join("dist.previous");
    std::fs::create_dir_all(&previous).unwrap();
    std::fs::write(previous.join("index.html"), "previous").unwrap();
    std::fs::write(previous.join("gpu.js"), "stale gpu").unwrap();
    let module = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("serve.mjs");
    let js = r#"
import { pathToFileURL } from 'node:url';
import { mkdirSync, writeFileSync } from 'node:fs';
const [modulePath, dist] = process.argv.slice(2);
const { readStaticFile } = await import(pathToFileURL(modulePath));
const text = (found) => found?.body.toString();
if (text(readStaticFile(dist, '/')) !== 'previous') throw new Error('missing previous fallback');
if (text(readStaticFile(dist, '/gpu.js')) !== 'stale gpu') throw new Error('previous tree is incomplete');
mkdirSync(dist);
writeFileSync(dist + '/index.html', 'current');
if (text(readStaticFile(dist, '/')) !== 'current') throw new Error('current build did not win');
if (readStaticFile(dist, '/gpu.js') !== null) throw new Error('current build borrowed stale GPU');
"#;
    let output = std::process::Command::new("bun")
        .args(["--input-type=module", "--eval", js])
        .arg("serve-fallback-test")
        .arg(module)
        .arg(&dist)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "bun: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_refused_bridge_reload_keeps_the_running_host() {
    let plan = contract::compile(COUNTER).unwrap().encode();
    let mut bridge: exact_web::abi::Bridge<NoData> = exact_web::abi::Bridge::new();
    let n = bridge.input_write(&plan);
    let len = bridge.boot_plan(n, NoData, 390.0, 844.0, "/");
    let inc: u32 = {
        let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
        let at = batch.find("\"data-testid\":\"inc\"").unwrap();
        let marker = "\"op\":\"create\",\"id\":";
        let id_at = batch[..at].rfind(marker).unwrap() + marker.len();
        batch[id_at..].split(',').next().unwrap().parse().unwrap()
    };

    // Establish state in the live Host, then offer bytes that cannot decode.
    bridge.dispatch(inc, 0, 0, 0.0);
    let bad = b"not an Exact plan";
    let n = bridge.input_write(bad);
    let len = bridge.boot_plan(n, NoData, 390.0, 844.0, "/");
    let refusal = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(refusal.contains("\"error\":\"boot:"), "{refusal}");

    // The same view id still dispatches into the old Host and advances the
    // state it held. Before candidate boot was transactional this said
    // `not booted` because `boot_plan` had taken and dropped the old Host.
    let len = bridge.dispatch(inc, 0, 0, 0.0);
    let after = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(!after.contains("not booted"), "{after}");
    assert!(after.contains("\"text\":\"3\""), "{after}");
}

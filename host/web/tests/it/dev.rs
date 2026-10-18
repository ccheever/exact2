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

/// Writes `text` stamped `n` seconds ahead: a later save reads as new
/// without waiting on the filesystem clock's resolution.
fn save(path: &std::path::Path, text: &str, n: u64) {
    std::fs::write(path, text).unwrap();
    let at = std::time::SystemTime::now() + std::time::Duration::from_secs(n);
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(at).unwrap();
}

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
    assert_eq!(std::fs::read(&out).unwrap(), built.bytes);
    assert!(exact_plan::Plan::decode(&built.bytes).is_ok());
    let map_path = dir.join("app.plan.map.json");
    let map = std::fs::read_to_string(&map_path).unwrap();
    assert_eq!(
        exact_runner::agent::field_str(&map, "digest"),
        Some(contract::plan_digest(&built.bytes))
    );
    assert_eq!(
        exact_runner::agent::field_str(map.split_once("\"nodes\":[").unwrap().1, "file"),
        Some(src.to_str().unwrap().to_owned())
    );
    assert!(s.poll::<NoData>().is_none(), "nothing changed");
    // Same bytes, new mtime: not a change.
    save(&src, GOOD, 1);
    assert!(
        s.poll::<NoData>().is_none(),
        "identical content is not an edit"
    );
    // A real edit.
    save(&src, &GOOD.replace("\"one\"", "\"two\""), 2);
    let again = s.poll::<NoData>().expect("an edit builds").unwrap();
    assert_ne!(again.bytes, built.bytes);
    let map = std::fs::read_to_string(&map_path).unwrap();
    assert_eq!(
        exact_runner::agent::field_str(&map, "digest"),
        Some(contract::plan_digest(&again.bytes))
    );
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
    let good_map = std::fs::read(dir.join("app.plan.map.json")).unwrap();
    save(
        &src,
        "component App\n  view\n    column\n      text one two\n",
        1,
    );
    let err = s.poll::<NoData>().unwrap().err().unwrap();
    assert!(err.contains("app.contract:"), "{err}");
    assert_eq!(
        std::fs::read(&out).unwrap(),
        good.bytes,
        "the page keeps the last good plan"
    );
    assert_eq!(
        std::fs::read(dir.join("app.plan.map.json")).unwrap(),
        good_map
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_save_reports_every_independent_refusal() {
    let dir = scratch("every-refusal");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    std::fs::write(
        &src,
        "component App\n  view\n    column\n      text missing0\n      text missing1\n",
    )
    .unwrap();
    let err = Session::new(&src, &out)
        .poll::<NoData>()
        .unwrap()
        .unwrap_err();
    assert!(err.contains("app.contract:4:"), "{err}");
    assert!(err.contains("app.contract:5:"), "{err}");
    assert_eq!(err.lines().count(), 2, "{err}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn static_build_omits_the_map_and_dev_bake_errors_name_imported_sources() {
    let dir = scratch("static-map");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    std::fs::write(&src, GOOD).unwrap();
    let good = Session::static_build(&src, &out)
        .poll::<NoData>()
        .unwrap()
        .unwrap();
    assert!(!dir.join("app.plan.map.json").exists());
    let dev = Session::new(&src, &out).poll::<NoData>().unwrap().unwrap();
    assert_eq!(good.bytes, dev.bytes);
    std::fs::write(
        &src,
        "use Child from \"./child.contract\"\ncomponent App\n  view\n    column\n      Child()\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("child.contract"),
        "component Child\n  state width = 0\n  view\n    button width=width height=0\n      text \"hidden\"\n",
    )
    .unwrap();
    let error = Session::new(&src, &out)
        .poll::<NoData>()
        .unwrap()
        .unwrap_err();
    assert!(error.contains("child.contract:4:"), "{error}");
    assert!(error.contains("app.contract:5:"), "{error}");
    assert_eq!(std::fs::read(&out).unwrap(), good.bytes);
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

    save(&src, &source.replace("seed=", "typo="), 2);
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

const COUNTER: &str = "component App\n  state n = 1\n  action inc\n    n = n + 1\n  view\n    column testId=\"root\"\n      button press=inc aria-label=\"Inc\" testId=\"inc\"\n        text \"Inc\"\n      text `${n}` testId=\"n\"\n";

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
#[ignore = "async lane: launches Bun, whose static serving builds the filesystem helper; bun scripts/async.mjs runs it"]
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
    let output =
        std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
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
    bridge.set_links(exact_web::HostLinks::ALL);
    let n = bridge.input_write(&plan);
    let len = bridge.boot_plan(n, NoData, 390.0, 844.0, "/");
    let inc: u32 = {
        let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
        let at = batch.find("\"data-testid\":\"inc\"").unwrap();
        let marker = "\"op\":\"create\",\"id\":";
        let id_at = batch[..at].rfind(marker).unwrap() + marker.len();
        batch[id_at..].split(',').next().unwrap().parse().unwrap()
    };
    let inspect = |bridge: &mut exact_web::abi::Bridge<NoData>, include: bool| {
        let request = format!("{{\"op\":\"node\",\"id\":{inc},\"plan\":{include}}}");
        let len = bridge.input_write(request.as_bytes());
        let len = bridge.agent(len);
        String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap()
    };
    assert!(exact_runner::agent::field_str(&inspect(&mut bridge, false), "planDigest").is_none());
    assert_eq!(
        exact_runner::agent::field_str(&inspect(&mut bridge, true), "planDigest"),
        Some(contract::plan_digest(&plan))
    );

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
    assert_eq!(
        exact_runner::agent::field_str(&inspect(&mut bridge, true), "planDigest"),
        Some(contract::plan_digest(&plan)),
        "state changes and refused candidates keep the accepted plan identity"
    );

    let replacement = contract::compile(&COUNTER.replace("Inc", "Add"))
        .unwrap()
        .encode();
    let len = bridge.input_write(&replacement);
    let len = bridge.boot_plan(len, NoData, 390.0, 844.0, "/");
    let response = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(
        exact_runner::agent::field_str(&response, "error").is_none(),
        "{response}"
    );
    assert_eq!(
        exact_runner::agent::field_str(&inspect(&mut bridge, true), "planDigest"),
        Some(contract::plan_digest(&replacement)),
        "a replacement may reuse the same node id but must expose its own plan"
    );
}

#[test]
fn display_mode_bits_preserve_fold_and_preferences_at_boot_and_without_a_resize() {
    let source = "shape Viewport\n  displayMode: string\n  horizontalViewportSegments: number\n  verticalViewportSegments: number\n  devicePosture: string\n  colorGamut: string\n  dynamicRange: string\ncomponent App\n  resource viewport = exactViewport() as shape Viewport\n  view\n    text viewport.displayMode\n";
    let plan = contract::bake(contract::compile(source).unwrap(), NoData)
        .unwrap()
        .encode();
    let mut bridge = exact_web::abi::Bridge::new();
    bridge.set_links(exact_web::HostLinks::ALL);
    for (i, (code, expected, cols, rows, folded, gamut, hdr)) in [
        (1, "browser", 1, 1, false, 0, false),
        (2, "fullscreen", 4, 1, true, 3, true),
        (0, "standalone", 4, 2, false, 1, false),
        (3, "minimal-ui", 2, 4, true, 2, true),
        (4, "window-controls-overlay", 255, 255, false, 0, true),
        (5, "picture-in-picture", 1, 1, true, 1, false),
        (6, "tabbed", 1, 1, false, 3, true),
    ]
    .into_iter()
    .enumerate()
    {
        let bits = 0xff
            | ((folded as u32) << 8)
            | (cols << 9)
            | (rows << 17)
            | (gamut << 26)
            | ((hdr as u32) << 28)
            | (code << 29);
        if i == 0 {
            bridge.set_preferences(bits);
            let len = bridge.boot(&plan, NoData, 600.0, 400.0, "/");
            let first = std::str::from_utf8(bridge.output_bytes(len as usize)).unwrap();
            assert!(first.contains("\"text\":\"browser\""), "{first}");
        } else {
            bridge.resize(600.0, 400.0, bits, 0.0);
        }
        let n = bridge.input_write(br#"{"op":"state"}"#);
        let len = bridge.agent(n);
        let state: serde_json::Value =
            serde_json::from_slice(bridge.output_bytes(len as usize)).unwrap();
        for facts in [&state["resources"]["viewport"], &state["device"]] {
            assert_eq!(facts["displayMode"], expected);
            assert_eq!(facts["horizontalViewportSegments"], cols);
            assert_eq!(facts["verticalViewportSegments"], rows);
            assert_eq!(
                facts["devicePosture"],
                if folded { "folded" } else { "continuous" }
            );
            assert_eq!(
                facts["colorGamut"],
                ["srgb", "p3", "rec2020", "rec2020"][gamut as usize]
            );
            assert_eq!(facts["dynamicRange"], if hdr { "high" } else { "standard" });
        }
        assert_eq!(state["device"]["prefersReducedMotion"], true);
        assert_eq!(state["device"]["prefersReducedTransparency"], true);
        assert_eq!(state["device"]["prefersContrast"], "custom");
        assert_eq!(state["device"]["prefersColorScheme"], "dark");
    }
}

/// LLP 1121 stage 4, commit 3: a value-only edit of the app's design
/// publishes no plan (its file is written, for a host that watches it); its
/// batch says what changed. An edit that changes the plan's
/// structure, or the Contract, still writes one. A write by rename, as the
/// Tamber server makes it, is the same case as one in place, as an editor
/// saves.
#[test]
fn a_value_only_design_edit_writes_no_plan_and_sends_a_batch() {
    let dir = scratch("design");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    let design = dir.join("app.tamber");
    let tamber = "tamber t\n\ntheme\n  ink = #111111\n  page = #ffffff\n";
    std::fs::write(&design, tamber).unwrap();
    let view = "component App\n  view\n    column background-color=\"page\"\n      text \"one\" color=\"ink\"\n";
    std::fs::write(&src, view).unwrap();
    let mut s = Session::new(&src, &out);
    let first = s.poll::<NoData>().expect("first look builds").unwrap();
    assert!(first.live.is_none());
    let on_disk = std::fs::read(&out).unwrap();
    // A value, saved in place.
    save(&design, &tamber.replace("#111111", "#222222"), 1);
    let edit = s.poll::<NoData>().expect("a design edit").unwrap();
    // Dated by the design's save, not the older root's.
    let design_ms = std::fs::metadata(&design)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    assert!(
        edit.saved_ms >= design_ms - 1.0,
        "{} < {design_ms}",
        edit.saved_ms
    );
    let live = edit.live.expect("value-only");
    assert_eq!(std::fs::read(&out).unwrap(), edit.bytes, "the file follows");
    assert_ne!(edit.bytes, on_disk);
    let names: Vec<&str> = live.batch.tokens.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["ink"]);
    assert_eq!(live.batch.tokens[0].value, "#222222ff");
    assert_eq!(live.snapshot.tokens.len(), 2);
    assert_eq!(live.batch.design, live.snapshot.design);
    // Another, written by rename: its base is the last batch's design.
    let tmp = dir.join(".app.tamber.tmp");
    std::fs::write(&tmp, tamber.replace("#111111", "#333333")).unwrap();
    std::fs::rename(&tmp, &design).unwrap();
    let edit = s.poll::<NoData>().expect("a renamed design").unwrap();
    let next = edit.live.expect("value-only");
    assert_eq!(next.batch.base.as_deref(), Some(live.batch.design.as_str()));
    // A token no view reads is in no plan: an empty batch, which the loop
    // does not send. Naming it in the view is structure.
    save(
        &design,
        &format!("{}  edge = #000000\n", tamber.replace("#111111", "#333333")),
        3,
    );
    let quiet = s.poll::<NoData>().expect("rebuilt").unwrap();
    assert!(quiet
        .live
        .as_ref()
        .is_some_and(|l| l.batch.tokens.is_empty()));
    save(
        &src,
        &view.replace("column ", "column border-color=\"edge\" "),
        4,
    );
    let structural = s.poll::<NoData>().expect("a structure edit").unwrap();
    assert!(structural.live.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

/// The wasm loop reloads for what the plan keeps (theme2's ruling, LLP
/// 1121 stage 5, D7's retention): in an app that can name a token outside
/// its views (a data crate beside it) every token is kept, so an added one
/// publishes a plan; and a first `when` on a kept token that did not vary
/// does too, wherever it is (the plan gains its slices). `propose` classes
/// both `rebuild` by the same retention.
#[test]
fn a_kept_token_added_or_starting_to_vary_publishes_a_plan() {
    let dir = scratch("retained");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    let design = dir.join("app.tamber");
    let tamber = "tamber t\n\naxes\n  tone = warm | cool\n\ntheme\n  ink = #111111\n";
    std::fs::write(&design, tamber).unwrap();
    std::fs::write(
        &src,
        "component App\n  view\n    text \"one\" color=\"ink\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(
        dir.join("data/Cargo.toml"),
        "[package]\nname = \"t-data\"\n",
    )
    .unwrap();
    let mut s = Session::new(&src, &out);
    assert!(s
        .poll::<NoData>()
        .expect("first look builds")
        .unwrap()
        .live
        .is_none());
    save(&design, &format!("{tamber}  spare = #222222\n"), 1);
    let added = s.poll::<NoData>().expect("an added token").unwrap();
    assert!(added.live.is_none(), "kept, so the plan's structure moved");
    save(
        &design,
        &format!("{tamber}  spare = #222222\n").replace(
            "ink = #111111\n",
            "ink = #111111\n    when tone=cool: #0a7aff\n",
        ),
        2,
    );
    let varies = s.poll::<NoData>().expect("a first when").unwrap();
    assert!(varies.live.is_none(), "the plan gains the token's slices");
    let _ = std::fs::remove_dir_all(dir);
}

/// LLP 1121 stage 4, commit 6: a scrub's preview compiles the design text
/// in place of the file's and writes nothing; each value's batch is based
/// on the last; restore goes back to the committed design; a structure
/// change is refused; and a save while one previews batches from it.
#[test]
fn a_scrub_previews_without_writing_and_restores_the_committed_design() {
    let dir = scratch("scrub");
    let src = dir.join("app.contract");
    let out = dir.join("app.plan");
    let design = dir.join("app.tamber");
    let tamber = "tamber t\n\ntheme\n  ink = #111111\n  page = #ffffff\n";
    std::fs::write(&design, tamber).unwrap();
    std::fs::write(
        &src,
        "component App\n  view\n    column background-color=\"page\"\n      text \"one\" color=\"ink\"\n",
    )
    .unwrap();
    let mut s = Session::new(&src, &out);
    s.poll::<NoData>().expect("first look builds").unwrap();
    let plan = std::fs::read(&out).unwrap();
    let (batch, snapshot) = s
        .scrub::<NoData>(tamber.replace("#111111", "#222222"))
        .unwrap();
    let batch = batch.expect("ink moved");
    assert_eq!(batch.tokens.len(), 1);
    assert_eq!(batch.tokens[0].value, "#222222ff");
    assert_eq!(snapshot.design, batch.design);
    assert_eq!(std::fs::read(&out).unwrap(), plan, "no plan written");
    assert_eq!(
        std::fs::read_to_string(&design).unwrap(),
        tamber,
        "no design written"
    );
    let (next, _) = s
        .scrub::<NoData>(tamber.replace("#111111", "#333333"))
        .unwrap();
    assert_eq!(next.unwrap().base.as_deref(), Some(batch.design.as_str()));
    // An axis added is structure: refused, the preview kept.
    assert!(s
        .scrub::<NoData>(tamber.replace("theme\n", "axes\n  mode = a | b\n\ntheme\n"))
        .is_err());
    let (back, committed) = s.restore().expect("a scrub previewed");
    let back = back.expect("back to #111111");
    assert_eq!(back.tokens[0].value, "#111111ff");
    assert_eq!(back.design, committed.design);
    assert!(s.restore().is_none(), "restored once");
    // A save while a scrub previews: the builds stay on the preview (an
    // empty batch), and its end restores the saved design.
    let (shown, _) = s
        .scrub::<NoData>(tamber.replace("#111111", "#444444"))
        .unwrap();
    save(&design, &tamber.replace("#111111", "#555555"), 1);
    let saved = s.poll::<NoData>().expect("a design edit").unwrap();
    let live = saved.live.expect("value-only");
    assert!(live.batch.tokens.is_empty());
    let (back, _) = s.restore().expect("still previewing");
    let back = back.expect("to the saved design");
    assert_eq!(back.base.as_deref(), Some(shown.unwrap().design.as_str()));
    assert_eq!(back.tokens[0].value, "#555555ff");
    // A released scrub: the build of what it wrote goes from it, and ends it.
    let (shown, _) = s
        .scrub::<NoData>(tamber.replace("#111111", "#666666"))
        .unwrap();
    s.release();
    save(&design, &tamber.replace("#111111", "#666666"), 2);
    let written = s.poll::<NoData>().expect("the release's write").unwrap();
    let live = written.live.expect("value-only");
    assert_eq!(
        live.batch.base.as_deref(),
        Some(shown.unwrap().design.as_str())
    );
    assert!(live.batch.tokens.is_empty(), "the builds already show it");
    assert!(s.restore().is_none(), "ended");
    // A scrub that previews before a released one's write builds keeps its
    // preview: that build changes only what its end restores.
    s.scrub::<NoData>(tamber.replace("#111111", "#777777"))
        .unwrap();
    s.release();
    let (next, _) = s
        .scrub::<NoData>(tamber.replace("#111111", "#888888"))
        .unwrap();
    save(&design, &tamber.replace("#111111", "#777777"), 3);
    let written = s.poll::<NoData>().expect("the first's write").unwrap();
    assert!(written.live.expect("value-only").batch.tokens.is_empty());
    let (back, _) = s.restore().expect("the second still previews");
    let back = back.expect("to the written design");
    assert_eq!(
        back.base.as_deref(),
        next.as_ref().map(|b| b.design.as_str())
    );
    assert_eq!(back.tokens[0].value, "#777777ff");
    let _ = std::fs::remove_dir_all(dir);
}

/// LLP 1121 stage 4, commit 4: a running web build applies a batch in
/// place (no reload: its state stays), and answers stale for one whose base
/// it has passed.
#[test]
fn a_running_web_build_applies_a_batch_in_place() {
    use exact_runner::design;
    let dir = scratch("live-host");
    let view = "component App\n  state n = 0\n  action bump\n    n = n + 1\n  view\n    column background-color=\"page\"\n      button press=bump testId=\"bump\"\n        text `${n}` color=\"ink\" testId=\"count\"\n";
    let plan = |tamber: &str| {
        std::fs::write(dir.join("app.tamber"), tamber).unwrap();
        std::fs::write(dir.join("app.contract"), view).unwrap();
        contract::compile_path(&dir.join("app.contract")).unwrap()
    };
    let a = plan("tamber t\n\ntheme\n  ink = #111111\n  page = #ffffff\n");
    let b = plan("tamber t\n\ntheme\n  ink = #ff0000\n  page = #ffffff\n");
    let structure = design::shape(&a);
    let batch = design::batch(&structure, &design::tokens(&a), &design::tokens(&b)).unwrap();
    let (mut host, _) = Host::boot(&a.encode(), NoData, Default::default(), "/").unwrap();
    let bump = {
        let k = host.runner().kernel();
        k.node_by_key(k.find_by_test_id("bump")[0]).unwrap().id
    };
    host.dispatch(bump, exact_runner::Event::Press);
    let out = host.apply_live(&batch.json());
    assert!(out.starts_with("{\"live\":\"applied\""), "{out}");
    assert!(
        out.contains("rgba(255,0,0,1)"),
        "the new colour reaches the page: {out}"
    );
    let count = {
        let k = host.runner().kernel();
        let n = k.node_by_key(k.find_by_test_id("count")[0]).unwrap();
        n.props.str(exact_kernel::PropId::Text).map(str::to_string)
    };
    assert_eq!(count.as_deref(), Some("1"), "no reload: the state stays");
    assert!(host
        .apply_live(&batch.json())
        .starts_with("{\"live\":\"stale\""));
    let _ = std::fs::remove_dir_all(dir);
}

/// A scrub's frames (LLP 1121 D13, *1.10*): a build applies one newer than
/// the last of that scrub it applied, drops an older one or its snapshot
/// (which carries no base), and takes another scrub's frames as they come.
#[test]
fn a_running_web_build_drops_a_scrub_frame_older_than_one_it_showed() {
    use exact_runner::design;
    let dir = scratch("live-scrub");
    let view = "component App\n  view\n    column background-color=\"page\"\n      text \"a\" color=\"ink\"\n";
    let plan = |tamber: &str| {
        std::fs::write(dir.join("app.tamber"), tamber).unwrap();
        std::fs::write(dir.join("app.contract"), view).unwrap();
        contract::compile_path(&dir.join("app.contract")).unwrap()
    };
    let a = plan("tamber t\n\ntheme\n  ink = #111111\n  page = #ffffff\n");
    let b = plan("tamber t\n\ntheme\n  ink = #ff0000\n  page = #ffffff\n");
    let c = plan("tamber t\n\ntheme\n  ink = #00ff00\n  page = #ffffff\n");
    let shape = design::shape(&a);
    let (ta, tb, tc) = (design::tokens(&a), design::tokens(&b), design::tokens(&c));
    let mark = |mut e: design::Edit, id: &str, rev| {
        e.scrub = Some(design::Scrub { id: id.into(), rev });
        e
    };
    let (mut host, _) = Host::boot(&a.encode(), NoData, Default::default(), "/").unwrap();
    let applied =
        |out: &str| out.starts_with("{\"live\":\"applied\"") && out.contains("\"design\"");
    let out = host.apply_live(&mark(design::batch(&shape, &ta, &tb).unwrap(), "scrub-1", 2).json());
    assert!(applied(&out) && out.contains("rgba(255,0,0,1)"), "{out}");
    // The snapshot of an earlier frame, fetched late: dropped.
    let out = host.apply_live(&mark(design::snapshot(&shape, &ta), "scrub-1", 1).json());
    assert!(
        !out.contains("rgba(17,17,17,1)"),
        "an older frame is not shown: {out}"
    );
    assert_eq!(host.runner().design(), design::digest(&tb));
    // A newer one is.
    let out = host.apply_live(&mark(design::batch(&shape, &tb, &tc).unwrap(), "scrub-1", 3).json());
    assert!(out.contains("rgba(0,255,0,1)"), "{out}");
    // Another scrub's, whatever its revision.
    let out = host.apply_live(&mark(design::batch(&shape, &tc, &ta).unwrap(), "scrub-2", 1).json());
    assert!(out.contains("rgba(17,17,17,1)"), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}

/// A scrub's frame patched (LLP 1121 D13): the committed design with the
/// folded rows set, as the compile of the same text gives it; a row that
/// would gain slices is not patched.
#[test]
fn a_patched_scrub_frame_is_the_compiled_one() {
    let dir = scratch("patch");
    let design = dir.join("app.tamber");
    let tamber = "tamber t\n\naxes\n  dark-style = dim | black  under scheme=dark\n\ntheme\n  ink = #111111\n  page = light-dark(#ffffff, #151d28)\n    when dark-style=black: #000000\n";
    std::fs::write(&design, tamber).unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  view\n    column background-color=\"page\"\n      text \"a\" color=\"ink\"\n").unwrap();
    let mut s = Session::new(dir.join("app.contract"), dir.join("out.plan"));
    s.poll::<NoData>().expect("built").unwrap();
    let (patched, _) = s
        .patch(&[
            ("ink".into(), vec!["#ff0000ff".into(); 2]),
            (
                "page".into(),
                vec![
                    "light-dark(#ffffffff, #222222ff)".into(),
                    "light-dark(#ffffffff, #000000ff)".into(),
                ],
            ),
        ])
        .unwrap()
        .expect("patched");
    let edited = tamber
        .replace("#111111", "#ff0000")
        .replace("#151d28", "#222222");
    let (compiled, _) = s.scrub::<NoData>(edited.clone()).unwrap();
    // The compile shows the same design as the patch: no batch between.
    assert!(compiled.is_none(), "{compiled:?}");
    assert!(patched.expect("a batch").tokens.len() == 2);
    let compiled = s.verify_job(edited)().unwrap();
    assert_eq!(s.verified(&compiled), None);
    // `ink` is one value in every tuple: two would make it vary.
    assert!(s
        .patch(&[("ink".into(), vec!["#ff0000ff".into(), "#00ff00ff".into()])])
        .unwrap()
        .is_none());
    let _ = std::fs::remove_dir_all(dir);
}

/// LLP 1121 stage 5, commit 2: a role override's value edit reaches a
/// running web build as a batch: its role sheet is written again in the
/// batch (`theme`), which unsaid text and the accent read.
#[test]
fn a_running_web_build_takes_a_role_override_edit_in_place() {
    use exact_runner::design;
    let dir = scratch("live-role");
    let view = "component App\n  view\n    column\n      text \"a\" testId=\"text\"\n";
    let plan = |tamber: &str| {
        std::fs::write(dir.join("app.tamber"), tamber).unwrap();
        std::fs::write(dir.join("app.contract"), view).unwrap();
        contract::compile_path(&dir.join("app.contract")).unwrap()
    };
    let a = plan("tamber t\n\ntheme\n  AccentColor = #0a7aff\n  CanvasText = #111111\n");
    let b = plan("tamber t\n\ntheme\n  AccentColor = #ff0000\n  CanvasText = #222222\n");
    let batch =
        design::batch(&design::shape(&a), &design::tokens(&a), &design::tokens(&b)).unwrap();
    let (mut host, boot) = Host::boot(&a.encode(), NoData, Default::default(), "/").unwrap();
    assert!(boot.contains("--exact-accent:#0a7aff"), "{boot}");
    let out = host.apply_live(&batch.json());
    assert!(out.starts_with("{\"live\":\"applied\""), "{out}");
    assert!(out.contains(r#"{"op":"theme","css":"#), "{out}");
    assert!(out.contains("--exact-accent:#ff0000"), "{out}");
    // Unsaid text reads the sheet's `color`, so the sheet is the edit.
    assert!(out.contains("--exact-canvas-text:#222222ff"), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Two web sessions on one thread, each with its own role overrides (LLP
/// 1121 stage 5): an event on one sends its rows under its own roles, with
/// no role resolved unbound beside the other (a debug build would assert).
#[test]
fn an_event_beside_another_session_crosses_under_its_own_roles() {
    let dir = scratch("two-sessions");
    let view = "component App\n  state n = 0\n  action bump\n    n = n + 1\n  view\n    column\n      button press=bump testId=\"bump\"\n        text `${n}` color=\"#ff0000\" testId=\"count\"\n";
    let plan = |tamber: &str| {
        std::fs::write(dir.join("app.tamber"), tamber).unwrap();
        std::fs::write(dir.join("app.contract"), view).unwrap();
        contract::compile_path(&dir.join("app.contract")).unwrap()
    };
    let a = plan("tamber t\n\ntheme\n  CanvasText = #111111\n");
    let b = plan("tamber t\n\ntheme\n  CanvasText = #222222\n");
    let (mut first, _) = Host::boot(&a.encode(), NoData, Default::default(), "/").unwrap();
    let (_second, _) = Host::boot(&b.encode(), NoData, Default::default(), "/").unwrap();
    let bump = {
        let k = first.runner().kernel();
        k.node_by_key(k.find_by_test_id("bump")[0]).unwrap().id
    };
    let out = first.dispatch(bump, exact_runner::Event::Press);
    assert!(out.contains("\"1\""), "{out}");
    let _ = std::fs::remove_dir_all(dir);
}

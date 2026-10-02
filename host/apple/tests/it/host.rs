//! The Apple host over the Caltrain app, headless: the first batch creates,
//! places, and sizes the whole tree with the kernel's layout; later batches
//! carry only what changed; motion arrives as presentation values; text is
//! measured through the registered callback.

use exact_apple::abi::Bridge;
use exact_apple::measure::{CFontCatalog, CMetrics, CRequest, MAX_CONTENT};
use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};
use std::ffi::c_void;
use std::sync::Mutex;

#[derive(Debug, PartialEq)]
struct RecordedFace {
    family: String,
    source: String,
    stack: u16,
    weight: u16,
    italic: bool,
}

static FONT_CATALOG: Mutex<Vec<RecordedFace>> = Mutex::new(Vec::new());
static FONT_RUNS: Mutex<Vec<(u16, u16, bool)>> = Mutex::new(Vec::new());
static RELOAD_FONT_SOURCES: Mutex<Vec<String>> = Mutex::new(Vec::new());

extern "C" fn record_fonts(_ctx: *mut c_void, catalog: *const CFontCatalog) {
    let catalog = unsafe { &*catalog };
    let rows = unsafe { std::slice::from_raw_parts(catalog.faces, catalog.count) };
    let mut recorded = FONT_CATALOG.lock().unwrap();
    recorded.clear();
    for row in rows {
        let family = unsafe { std::slice::from_raw_parts(row.family, row.family_len) };
        let source = unsafe { std::slice::from_raw_parts(row.source, row.source_len) };
        recorded.push(RecordedFace {
            family: String::from_utf8(family.to_vec()).unwrap(),
            source: String::from_utf8(source.to_vec()).unwrap(),
            stack: row.stack,
            weight: row.weight,
            italic: row.italic != 0,
        });
    }
}

extern "C" fn record_reload_fonts(_ctx: *mut c_void, catalog: *const CFontCatalog) {
    let catalog = unsafe { &*catalog };
    let rows = unsafe { std::slice::from_raw_parts(catalog.faces, catalog.count) };
    let mut recorded = RELOAD_FONT_SOURCES.lock().unwrap();
    recorded.clear();
    recorded.extend(rows.iter().map(|row| {
        let source = unsafe { std::slice::from_raw_parts(row.source, row.source_len) };
        String::from_utf8(source.to_vec()).unwrap()
    }));
}

extern "C" fn record_font_runs(_ctx: *mut c_void, request: *const CRequest) -> CMetrics {
    let request = unsafe { &*request };
    let runs = unsafe { std::slice::from_raw_parts(request.runs, request.count) };
    FONT_RUNS.lock().unwrap().extend(
        runs.iter()
            .map(|run| (run.font_family, run.font_weight, run.italic != 0)),
    );
    CMetrics {
        width: runs
            .iter()
            .map(|run| run.len as f32 * run.font_size * 0.5)
            .sum(),
        height: runs.first().map_or(0.0, |run| run.font_size * 1.2),
        baseline: runs.first().map_or(-1.0, |run| run.font_size),
    }
}

fn boot() -> (Host<caltrain_data::Caltrain>, String) {
    let plan = caltrain::build().unwrap();
    Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap()
}

fn view<D: DataSource>(host: &Host<D>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

/// The last `frame` op for `id`: (x, y, w, h).
fn frame_of(batch: &str, id: u32) -> (f32, f32, f32, f32) {
    let marker = format!("\"op\":\"frame\",\"id\":{id},");
    let at = batch
        .rfind(&marker)
        .unwrap_or_else(|| panic!("no frame for {id}"));
    let rest = &batch[at + marker.len()..];
    let field = |k: &str| -> f32 {
        let s = &rest[rest.find(&format!("\"{k}\":")).unwrap() + k.len() + 3..];
        s[..s.find([',', '}']).unwrap()].parse().unwrap()
    };
    (field("x"), field("y"), field("w"), field("h"))
}

/// The last `content` op for `id`: (w, h).
fn content_of(batch: &str, id: u32) -> (f32, f32) {
    let marker = format!("\"op\":\"content\",\"id\":{id},");
    let at = batch
        .rfind(&marker)
        .unwrap_or_else(|| panic!("no content for {id}"));
    let rest = &batch[at + marker.len()..];
    let field = |k: &str| -> f32 {
        let s = &rest[rest.find(&format!("\"{k}\":")).unwrap() + k.len() + 3..];
        s[..s.find([',', '}']).unwrap()].parse().unwrap()
    };
    (field("w"), field("h"))
}

fn count(batch: &str, op: &str) -> usize {
    batch.matches(&format!("\"op\":\"{op}\"")).count()
}

#[test]
fn inherited_text_color_reaches_untouched_descendants_as_an_unresolved_pair() {
    let src = r##"component Ink
  state changed = false
  action toggle
    changed = not changed
  view
    column color=(changed ? "light-dark(#123456, #abcdef)" : "light-dark(#000000, #ffffff)")
      button press=toggle testId="toggle"
        text "Toggle"
      column
        text "Inherited" testId="inherited"
        input testId="field" value="Input"
        column color="#ff0000"
          text "Override" testId="override"
"##;
    let plan = contract::compile(src).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let inherited = view(&host, "inherited");
    let field = view(&host, "field");
    let overridden = view(&host, "override");
    let op = |batch: &str, id: u32| -> String {
        batch
            .split("{\"op\":")
            .find(|part| part.contains(&format!("\"id\":{id},")))
            .unwrap_or_else(|| panic!("missing {id} in {batch}"))
            .to_owned()
    };
    for id in [inherited, field] {
        assert!(op(&first, id).contains("\"text_color\":[[0,0,0,255],[255,255,255,255]]"));
    }
    assert!(op(&first, overridden).contains("\"text_color\":[255,0,0,255]"));
    let toggle = view(&host, "toggle");
    let changed = host.dispatch_at(toggle, Event::Press, 0.0);
    for id in [inherited, field] {
        assert!(op(&changed, id).contains("\"text_color\":[[18,52,86,255],[171,205,239,255]]"));
    }
    assert!(!changed.contains(&format!("\"op\":\"style\",\"id\":{overridden},")));
    assert_eq!(count(&host.resize(402.0, 874.0), "style"), 0);
}

#[test]
fn the_first_batch_creates_places_and_sizes_the_whole_tree() {
    let (host, batch) = boot();
    assert!(batch.starts_with("{\"ops\":["));
    assert!(
        batch.ends_with(
            ",\"timers\":true,\"motion\":false,\"canvas\":false,\"clock\":0,\"error\":null}"
        ),
        "{}",
        &batch[batch.len() - 80..]
    );
    let live = host.runner().kernel().live_count();
    assert_eq!(count(&batch, "create"), live);
    assert_eq!(count(&batch, "frame"), live, "every node is placed");
    assert!(batch.contains("\"op\":\"create\",\"id\":1,\"kind\":\"view\""));
    assert!(
        batch.contains("\"semanticTag\":\"main\""),
        "props by their own names"
    );
    assert!(batch.contains("\"kind\":\"button\"") && batch.contains("\"handlers\":[\"press\"]"));
    assert!(
        batch.contains("\"kind\":\"scroll\"") && count(&batch, "content") == 1,
        "one scroll container, one content size"
    );
    assert!(
        batch.contains("\"font_size\":24,\"font_weight\":700"),
        "typed style rows: {}",
        &batch[..400]
    );
    assert!(batch.contains("\"background_color\":[238,238,238,255]"));
    // The root is a block as wide as the viewport (LLP 1008 §1) and, since
    // the app lives inside a sky canvas that fills the window (LLP 1014
    // §1a), as tall as it: its `scroll` child holds the page.
    let root = frame_of(&batch, 1);
    assert_eq!(
        (root.0, root.1, root.2, root.3),
        (0.0, 0.0, 390.0, 844.0),
        "{root:?}"
    );
    let scroll = content_of(&batch, 3);
    assert!(
        scroll.1 > 844.0,
        "the page is taller than the viewport: {scroll:?}"
    );
    assert_eq!(
        scroll.0, 390.0,
        "border-box: nothing overflows sideways: {scroll:?}"
    );
    assert!(batch.contains("\"op\":\"roots\",\"ids\":[1]"));
    assert!(
        !batch.contains("\"op\":\"present\""),
        "nothing moves at boot"
    );
    assert!(
        !batch.contains("\"translate\":") && !batch.contains("\"opacity\":"),
        "motion targets are never style"
    );
}

#[test]
fn later_batches_carry_only_what_changed_and_frames_follow() {
    let (mut host, _) = boot();
    let tick = host.advance(61_000.0);
    assert!(count(&tick, "props") > 0, "countdowns changed: {tick}");
    assert_eq!(count(&tick, "create"), 0);
    let swap = host.dispatch_at(view(&host, "change-station"), Event::Press, 61_000.0);
    assert!(
        count(&swap, "destroy") > 0 && count(&swap, "create") > 0,
        "{}",
        &swap[..200]
    );
    assert!(count(&swap, "frame") > 0, "new nodes are placed");
    assert_eq!(
        count(&swap, "present"),
        0,
        "a new view starts at its presentation's identity: {swap}"
    );
    let nothing = host.resize(390.0, 844.0);
    assert_eq!(
        count(&nothing, "frame"),
        0,
        "the same viewport moves nothing"
    );
    let wider = host.resize(600.0, 844.0);
    assert_eq!(frame_of(&wider, 1).2, 600.0);
    assert!(count(&wider, "frame") > 1, "children reflow");
}

#[test]
fn an_iframe_batch_and_its_events_match_the_web_arm() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/iframe.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (mut host, batch) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let iframe = view(&host, "deck");
    assert!(
        batch.contains(&format!(
            "\"op\":\"create\",\"id\":{iframe},\"kind\":\"iframe\""
        )),
        "{batch}"
    );
    assert!(batch.contains("\"sandbox\":\"allow-scripts\""));
    assert!(batch.contains("\"src\":\"/deck/index.html\""));
    assert!(
        batch.contains("\"handlers\":[\"load\",\"message\"]"),
        "{batch}"
    );
    host.dispatch_at(iframe, Event::Load, 0.0);
    host.dispatch_at(iframe, Event::Message("deck-ready".into()), 0.0);
    assert_eq!(host.runner().slot("loaded"), Some(&Value::Bool(true)));
    assert_eq!(
        host.runner().slot("received"),
        Some(&Value::str("deck-ready"))
    );
}

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

struct NamedData;
impl DataSource for NamedData {
    fn app_id(&self) -> &str {
        "com.example.apple-transaction"
    }

    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

#[test]
fn a_spring_arrives_as_presentation_values_frame_by_frame() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/spring.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    assert!(first.contains("\"motion\":false"));
    let toggle = view(&host, "toggle");
    let hello = view(&host, "hello");
    let batch = host.dispatch_at(toggle, Event::Press, 0.0);
    assert!(
        batch.contains("\"motion\":true"),
        "a spring and an easing started: {batch}"
    );
    let mut last = 1.0;
    let mut rising = 0;
    for i in 1..=30 {
        let tick = host.tick(i as f64 * 16.0);
        let marker = format!("\"op\":\"present\",\"id\":{hello},\"property\":\"scale\",\"x\":");
        if let Some(at) = tick.find(&marker) {
            let x: f64 = tick[at + marker.len()..]
                .split(',')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            if x > last {
                rising += 1;
            }
            last = x;
        }
        assert!(
            tick.contains("\"property\":\"opacity\"") || i > 13,
            "the 200 ms easing presents too: {tick}"
        );
    }
    assert!(
        rising > 5 && last > 1.0,
        "the spring moved toward 1.5: {last}"
    );
    let settled = host.tick(20_000.0);
    assert!(settled.contains("\"motion\":false"));
    assert!(settled.contains(&format!(
        "\"op\":\"present\",\"id\":{hello},\"property\":\"scale\",\"x\":1.5"
    )));
}

extern "C" fn wide_glyphs(_ctx: *mut c_void, request: *const CRequest) -> CMetrics {
    // Every glyph is one em wide; a single line; the width offer caps nothing.
    let r = unsafe { &*request };
    let runs = unsafe { std::slice::from_raw_parts(r.runs, r.count) };
    let mut width = 0.0f32;
    let mut size = 0.0f32;
    for run in runs {
        let chars = unsafe { std::slice::from_raw_parts(run.text, run.len) }.len() as f32;
        width += chars * run.font_size;
        size = size.max(run.font_size);
    }
    assert!(r.width > 0.0 || r.width == MAX_CONTENT || r.width == -2.0);
    CMetrics {
        width,
        height: size * 1.25,
        baseline: size,
    }
}

#[test]
fn text_is_measured_through_the_registered_callback() {
    let plan = caltrain::build().unwrap().encode();
    let mut bridge: Bridge<caltrain_data::Caltrain> = Bridge::new();
    let len = bridge.boot(
        &plan,
        caltrain_data::Caltrain,
        exact_apple::abi::Hooks {
            measure: Some(wide_glyphs),
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
        },
        390.0,
        844.0,
    );
    let batch = String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap();
    assert!(
        batch.contains("\"error\":null"),
        "{}",
        &batch[batch.len() - 100..]
    );
    // "Caltrain" at 13 pt, one em per glyph: 104 wide, 16.25 tall.
    let at = batch.find("\"text\":\"Caltrain\"").unwrap();
    let head = &batch[..at];
    let id: u32 = head[head.rfind("\"id\":").unwrap() + 5..]
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    // Its height is the callback's; its width is its flex column's (a text
    // item stretches, as a div does).
    let f = frame_of(&batch, id);
    assert!(
        (f.3 - 16.25).abs() < 1e-3,
        "16.25 exactly: the kernel keeps fractional frames (LLP 1001 §5): {f:?}"
    );
    assert!(f.2 >= 104.0, "{f:?}");
    // The bridge's other calls answer too.
    let len = bridge.tick(16.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("\"motion\":false"));
    let len = bridge.resize(500.0, 844.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("\"w\":500"));
}

#[test]
fn a_refused_plan_reload_keeps_the_running_host() {
    let plan = caltrain::build().unwrap().encode();
    let mut bridge: Bridge<caltrain_data::Caltrain> = Bridge::new();
    let len = bridge.boot(
        &plan,
        caltrain_data::Caltrain,
        exact_apple::abi::Hooks::none(),
        390.0,
        844.0,
    );
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("\"error\":null"));

    let bad = b"not an Exact plan";
    bridge.input_write(bad);
    let len = bridge.boot_plan(
        bad.len(),
        caltrain_data::Caltrain,
        exact_apple::abi::Hooks::none(),
        390.0,
        844.0,
    );
    let refusal = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(refusal.contains("\"error\":\"boot:"), "{refusal}");

    // The next operation still reaches the old host. Before the reload was
    // transactional this was the bridge's `not booted` batch.
    let len = bridge.resize(500.0, 844.0);
    let after = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(!after.contains("not booted"), "{after}");
    assert!(after.contains("\"w\":500"), "{after}");
}

#[test]
fn a_runner_refusal_never_installs_the_candidate_font_catalog() {
    RELOAD_FONT_SOURCES.lock().unwrap().clear();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/fixtures/fonts/app.contract");
    let plan = contract::compile_path(&path).unwrap().encode();
    let mut bridge: Bridge<NamedData> = Bridge::new();
    bridge.set_fonts(Some(record_reload_fonts), std::ptr::null_mut());
    let len = bridge.boot(
        &plan,
        NamedData,
        exact_apple::abi::Hooks {
            measure: Some(wide_glyphs),
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
        },
        390.0,
        844.0,
    );
    let first = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(first.contains("\"error\":null"), "{first}");
    let old_sources = RELOAD_FONT_SOURCES.lock().unwrap().clone();
    assert_eq!(old_sources.len(), 2);

    let mut candidate =
        contract::compile("component Candidate\n  view\n    text \"candidate\"\n").unwrap();
    candidate.app_id = "com.example.somewhere-else".into();
    let bytes = candidate.encode();
    bridge.input_write(&bytes);
    let len = bridge.boot_plan(
        bytes.len(),
        NamedData,
        exact_apple::abi::Hooks {
            measure: Some(wide_glyphs),
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
        },
        390.0,
        844.0,
    );
    let refusal = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(refusal.contains("AppMismatch"), "{refusal}");
    assert_eq!(*RELOAD_FONT_SOURCES.lock().unwrap(), old_sources);
}

#[test]
fn an_invalid_viewport_plan_boot_keeps_the_running_host() {
    // @ref LLP 1039 D2 — refuse the size before layout; keep the old host.
    let plan = contract::compile("component Running\n  view\n    text \"running\"\n")
        .unwrap()
        .encode();
    let mut bridge: Bridge<NoData> = Bridge::new();
    let len = bridge.boot(
        &plan,
        NoData,
        exact_apple::abi::Hooks {
            measure: Some(wide_glyphs),
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
        },
        390.0,
        844.0,
    );
    let first = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(first.contains("\"error\":null"), "{first}");

    let candidate = contract::compile("component Candidate\n  view\n    text \"candidate\"\n")
        .unwrap()
        .encode();
    bridge.input_write(&candidate);
    let len = bridge.boot_plan(
        candidate.len(),
        NoData,
        exact_apple::abi::Hooks {
            measure: Some(wide_glyphs),
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
        },
        f32::NAN,
        844.0,
    );
    let refusal = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(refusal.contains("InvalidViewport"), "{refusal}");

    let len = bridge.resize(500.0, 844.0);
    let after = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(after.contains("\"error\":null"), "{after}");
    assert!(after.contains("\"w\":500"), "{after}");
}

#[test]
fn an_invalid_initial_viewport_publishes_no_host() {
    let plan = contract::compile("component Candidate\n  view\n    text \"candidate\"\n")
        .unwrap()
        .encode();
    let mut bridge: Bridge<NoData> = Bridge::new();
    let len = bridge.boot(
        &plan,
        NoData,
        exact_apple::abi::Hooks {
            measure: Some(wide_glyphs),
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
        },
        f32::NAN,
        844.0,
    );
    let refusal = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(refusal.contains("InvalidViewport"), "{refusal}");
    let len = bridge.resize(500.0, 844.0);
    let after = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(after.contains("not booted"), "{after}");
}

#[test]
fn a_refused_fresh_boot_keeps_the_running_host() {
    let running = contract::compile("component Running\n  view\n    text \"running\"\n")
        .unwrap()
        .encode();
    let candidate = contract::compile("component Candidate\n  view\n    text \"candidate\"\n")
        .unwrap()
        .encode();
    let hooks = exact_apple::abi::Hooks {
        measure: Some(wide_glyphs),
        ctx: std::ptr::null_mut(),
        wake: None,
        wake_ctx: std::ptr::null_mut(),
        canvas_text: None,
    };
    let mut bridge: Bridge<NoData> = Bridge::new();
    let len = bridge.boot(&running, NoData, hooks, 390.0, 844.0);
    let first = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(first.contains("\"error\":null"), "{first}");

    let len = bridge.boot(&candidate, NoData, hooks, f32::NAN, 844.0);
    let refusal = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(refusal.contains("InvalidViewport"), "{refusal}");

    let len = bridge.resize(500.0, 844.0);
    let after = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(after.contains("\"error\":null"), "{after}");
    assert!(after.contains("\"w\":500"), "{after}");
}

#[test]
fn two_prepared_sessions_keep_their_live_hosts_until_both_accept() {
    let running = contract::compile("component App\n  view\n    text \"running\"\n")
        .unwrap()
        .encode();
    let candidate = contract::compile("component App\n  view\n    text \"candidate\"\n")
        .unwrap()
        .encode();
    let hooks = exact_apple::abi::Hooks {
        measure: None,
        ctx: std::ptr::null_mut(),
        wake: None,
        wake_ctx: std::ptr::null_mut(),
        canvas_text: None,
    };
    let mut a: Bridge<NoData> = Bridge::new();
    let mut b: Bridge<NoData> = Bridge::new();
    a.boot(&running, NoData, hooks, 390.0, 844.0);
    b.boot(&running, NoData, hooks, 390.0, 844.0);
    for bridge in [&mut a, &mut b] {
        bridge.input_write(&candidate);
    }
    let n = a.prepare_plan(candidate.len(), NoData, hooks, 390.0, 844.0);
    assert!(String::from_utf8_lossy(a.output_bytes(n as usize)).contains("\"error\":null"));
    let n = b.prepare_plan(candidate.len(), NoData, hooks, f32::NAN, 844.0);
    assert!(String::from_utf8_lossy(b.output_bytes(n as usize)).contains("InvalidViewport"));
    a.discard_plan();
    b.discard_plan();
    for bridge in [&mut a, &mut b] {
        let n = bridge.input_write(br#"{"op":"tree"}"#);
        let n = bridge.agent(n);
        let tree = String::from_utf8_lossy(bridge.output_bytes(n as usize));
        assert!(tree.contains("running"));
        assert!(!tree.contains("candidate"));
        bridge.input_write(&candidate);
        bridge.prepare_plan(candidate.len(), NoData, hooks, 390.0, 844.0);
    }
    for bridge in [&mut a, &mut b] {
        bridge.commit_plan();
    }
    for bridge in [&mut a, &mut b] {
        let n = bridge.input_write(br#"{"op":"tree"}"#);
        let n = bridge.agent(n);
        let tree = String::from_utf8_lossy(bridge.output_bytes(n as usize));
        assert!(tree.contains("candidate"));
        assert!(!tree.contains("running"));
    }
}

#[test]
fn the_plan_font_catalog_and_family_runs_cross_the_host_seam_before_layout() {
    FONT_CATALOG.lock().unwrap().clear();
    FONT_RUNS.lock().unwrap().clear();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/fixtures/fonts/app.contract");
    let plan = contract::compile_path(&path).unwrap().encode();
    let mut bridge: Bridge<NoData> = Bridge::new();
    bridge.set_fonts(Some(record_fonts), std::ptr::null_mut());
    let len = bridge.boot(
        &plan,
        NoData,
        exact_apple::abi::Hooks {
            measure: Some(record_font_runs),
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
        },
        390.0,
        844.0,
    );
    let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(batch.contains("\"error\":null"), "{batch}");
    assert_eq!(
        *FONT_CATALOG.lock().unwrap(),
        vec![
            RecordedFace {
                family: "Fixture Sans".into(),
                source: "assets/DejaVuSans.ttf".into(),
                stack: 8,
                weight: 400,
                italic: false,
            },
            RecordedFace {
                family: "Fixture Sans".into(),
                source: "assets/DejaVuSans-Bold.ttf".into(),
                stack: 8,
                weight: 700,
                italic: false,
            },
        ]
    );
    let runs = FONT_RUNS.lock().unwrap();
    assert!(runs.contains(&(8, 400, false)), "{runs:?}");
    assert!(runs.contains(&(8, 600, false)), "{runs:?}");
    assert!(runs.contains(&(8, 700, false)), "{runs:?}");
}

#[test]
fn an_image_lays_out_from_the_intrinsic_size_the_presenter_reports() {
    let (mut host, first) = boot();
    let logo = view(&host, "logo");
    assert!(
        first.contains(&format!(
            "\"op\":\"frame\",\"id\":{logo},\"x\":0,\"y\":0,\"w\":96,\"h\":0"
        )),
        "nothing until it loads: {}",
        &first[..300]
    );
    let batch = host.set_intrinsic(logo, Some((320.0, 120.0)));
    assert!(
        batch.contains(&format!(
            "\"op\":\"frame\",\"id\":{logo},\"x\":0,\"y\":0,\"w\":96,\"h\":36"
        )),
        "width 96 by ratio → 36: {batch}"
    );
    let name = view(&host, "station-name");
    assert!(
        batch.contains(&format!("\"op\":\"frame\",\"id\":{name},")),
        "the station name below it moved down: {batch}"
    );
    let again = host.set_intrinsic(logo, Some((320.0, 120.0)));
    assert_eq!(
        again.matches("\"op\":\"frame\"").count(),
        0,
        "the same size moves nothing"
    );
    let cleared = host.set_intrinsic(logo, None);
    assert!(
        cleared.contains(&format!(
            "\"op\":\"frame\",\"id\":{logo},\"x\":0,\"y\":0,\"w\":96,\"h\":0"
        )),
        "a source that failed or was cleared is 96×0 again: {cleared}"
    );
}

#[test]
fn an_intrinsic_size_is_refused_for_a_non_image_and_for_a_bad_value() {
    let (mut host, _) = boot();
    let name = view(&host, "station-name");
    let batch = host.set_intrinsic(name, Some((320.0, 120.0)));
    assert!(batch.contains("NotAnImage"), "{batch}");
    let logo = view(&host, "logo");
    let batch = host.set_intrinsic(logo, Some((f32::INFINITY, 120.0)));
    assert!(batch.contains("InvalidIntrinsicSize"), "{batch}");
    assert!(!batch.contains("\"op\":\"frame\""), "{batch}");
}

#[test]
fn several_intrinsic_sizes_lay_out_once_and_a_refusal_keeps_the_rest() {
    let (mut host, _) = boot();
    let (logo, name) = (view(&host, "logo"), view(&host, "station-name"));
    let batch = host.set_intrinsics(&[(name, Some((320.0, 120.0))), (logo, Some((320.0, 120.0)))]);
    assert!(batch.contains("NotAnImage"), "{batch}");
    assert!(
        batch.contains(&format!(
            "\"op\":\"frame\",\"id\":{logo},\"x\":0,\"y\":0,\"w\":96,\"h\":36"
        )),
        "the image's size still lands: {batch}"
    );
    let again = host.set_intrinsics(&[(logo, Some((320.0, 120.0)))]);
    assert_eq!(again.matches("\"op\":\"frame\"").count(), 0, "{again}");
}

/// The insets fixture (`contract/corpus/insets.contract`), booted here.
fn boot_insets() -> (Host<NoData>, String) {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contract/corpus/insets.contract"),
    )
    .unwrap();
    let plan = contract::bake(contract::compile(&src).unwrap(), NoData).unwrap();
    Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap()
}

#[test]
fn the_insets_re_send_the_styles_that_read_them_and_move_what_they_pad() {
    let (mut host, first) = boot_insets();
    let root = view(&host, "root");
    let content = view(&host, "content");
    // At boot the insets are zero: the root's padding is 0 in points, and
    // its `viewportFit` prop is on the wire for the presenter to read.
    assert!(
        first.contains("\"viewportFit\":\"cover\""),
        "{}",
        &first[..300]
    );
    assert!(first.contains("\"padding_top\":0,"), "{}", &first[..400]);
    assert_eq!(frame_of(&first, content).1, 0.0);
    // The phone's insets under `viewport-fit=cover`: the root's dictionary
    // comes again with the points, the content moves down, nothing else is
    // re-sent (the child's own style does not read the insets).
    let batch = host.set_insets(62.0, 0.0, 34.0, 0.0);
    assert_eq!(count(&batch, "style"), 1, "{batch}");
    assert!(
        batch.contains(&format!("\"op\":\"style\",\"id\":{root},")),
        "{batch}"
    );
    assert!(
        batch.contains("\"padding_top\":62,") && batch.contains("\"padding_bottom\":34,"),
        "{batch}"
    );
    assert_eq!(frame_of(&batch, content), (0.0, 62.0, 402.0, 778.0));
    assert!(batch.contains("\"error\":null"));
    // The same insets again: nothing to say.
    let again = host.set_insets(62.0, 0.0, 34.0, 0.0);
    assert_eq!(
        count(&again, "style") + count(&again, "frame"),
        0,
        "{again}"
    );
    // A non-finite inset is refused by the kernel, as an error on the batch.
    let bad = host.set_insets(f32::NAN, 0.0, 0.0, 0.0);
    assert!(bad.contains("\"error\":\"insets: "), "{bad}");
    // An app that reads no inset (Caltrain) gets an empty batch.
    let (mut caltrain, _) = boot();
    let none = caltrain.set_insets(62.0, 0.0, 34.0, 0.0);
    assert_eq!(count(&none, "style") + count(&none, "frame"), 0, "{none}");
}

// ---------------------------------------------------------------- the handle (LLP 1031 D2)

/// The exports for a fixture app: two runtimes in one thread, each with its
/// own plan and clock; a destroyed or invented handle refused by name.
mod handles {
    use exact_runner::{DataError, DataSource, Value};
    use std::cell::Cell;
    use std::sync::OnceLock;

    thread_local! {
        pub static ANALYSIS: Cell<bool> = const { Cell::new(false) };
        // constructor, grants, selection, started, candidate facts, fonts,
        // delivery expression. Thread-local so other ABI tests stay independent.
        pub static CALLS: Cell<[usize; 7]> = const { Cell::new([0; 7]) };
    }
    fn count(index: usize) {
        CALLS.with(|calls| {
            let mut next = calls.get();
            next[index] += 1;
            calls.set(next);
        });
    }
    pub struct Fixture;
    impl Default for Fixture {
        fn default() -> Self {
            count(0);
            Self
        }
    }
    impl DataSource for Fixture {
        fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(s.into()))
        }
        fn grants(&self) -> &'static str {
            count(1);
            ""
        }
    }
    pub extern "C" fn fonts(
        _: *mut std::ffi::c_void,
        _: *const exact_apple::measure::CFontCatalog,
    ) {
        count(5);
    }
    pub static DELIVERY: exact_apple::delivery::Hooks = exact_apple::delivery::Hooks {
        selected_module: || Ok(None),
        selected_plan: || {
            count(2);
            Some(("selected".into(), baked().to_vec()))
        },
        candidate_delivery: |_, _| {
            count(4);
            Some(exact_runner::Delivery::default())
        },
        boot_started: || count(3),
        entry_refused: |_, _| {},
        status_into: |_| {},
        take_note: || None,
        last_line: || None,
    };
    fn delivery() -> Option<&'static exact_apple::delivery::Hooks> {
        count(6);
        ANALYSIS.with(|analysis| analysis.get().then_some(&DELIVERY))
    }

    pub fn baked() -> &'static [u8] {
        static PLAN: OnceLock<Vec<u8>> = OnceLock::new();
        PLAN.get_or_init(|| {
            contract::compile("component Baked\n  view\n    text \"baked\"\n")
                .unwrap()
                .encode()
        })
    }

    /// The archive's `compat.json` (LLP 1030 D3a) as this fixture's binary
    /// would carry it: the exports hand it to the runner at every boot.
    pub const COMPAT: &str =
        r#"{"id":"fixture00000000","inputs":{"executors":["native"],"store":{"L":"0"}}}"#;
    pub const ANALYSIS_COMPAT: &str = r#"{"inputs":{"store":{"L":"A"},"trust":"production"},"embedded":{"analysis":true,"seq":null}}"#;
    fn compat() -> &'static str {
        ANALYSIS.with(|analysis| {
            if analysis.get() {
                ANALYSIS_COMPAT
            } else {
                COMPAT
            }
        })
    }

    exact_apple::host!(Fixture, baked(), compat(), delivery(), std::ptr::null());
}

fn out(rt: u32, len: u32) -> String {
    let p = handles::exact_out(rt);
    assert!(!p.is_null());
    String::from_utf8(unsafe { std::slice::from_raw_parts(p, len as usize) }.to_vec()).unwrap()
}

fn put(rt: u32, bytes: &[u8]) -> usize {
    let p = handles::exact_in(rt, bytes.len());
    assert!(!p.is_null(), "runtime {rt} has no input buffer");
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len()) };
    bytes.len()
}

#[test]
fn analysis_exports_and_direct_bridges_refuse_before_app_or_adapter_calls() {
    handles::ANALYSIS.set(true);
    handles::CALLS.set([0; 7]);
    let rt = handles::exact_create();
    handles::exact_set_fonts(rt, Some(handles::fonts), std::ptr::null_mut());
    let plan = handles::baked();
    for operation in 0..3 {
        let len = put(rt, plan);
        let n = match operation {
            0 => handles::exact_boot(rt, 390.0, 844.0),
            1 => handles::exact_boot_plan(rt, len, 390.0, 844.0),
            _ => handles::exact_prepare_plan(rt, 41, len, 390.0, 844.0),
        };
        let refusal = out(rt, n);
        assert!(
            refusal.contains("compatibility analysis artifact"),
            "{refusal}"
        );
        assert!(refusal.contains("\"ops\":[]"), "{refusal}");
        assert_eq!(handles::CALLS.get(), [0; 7], "export {operation}");
    }
    assert_eq!(
        out(rt, handles::exact_baked_compat(rt)),
        handles::ANALYSIS_COMPAT
    );
    assert_eq!(handles::CALLS.get(), [0; 7]);

    let mut bridge = Bridge::new();
    bridge.set_compat(handles::ANALYSIS_COMPAT);
    bridge.set_delivery(Some(&handles::DELIVERY));
    bridge.set_fonts(Some(handles::fonts), std::ptr::null_mut());
    let hooks = exact_apple::abi::Hooks::none();
    let n = bridge.boot_selected(plan, handles::Fixture::default, hooks, 390.0, 844.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize))
        .contains("compatibility analysis artifact"));
    let n = bridge.boot(plan, handles::Fixture, hooks, 390.0, 844.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize))
        .contains("compatibility analysis artifact"));
    bridge.input_write(plan);
    let n = bridge.prepare_plan(plan.len(), handles::Fixture, hooks, 390.0, 844.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize))
        .contains("compatibility analysis artifact"));
    assert_eq!(handles::CALLS.get(), [0; 7], "direct Bridge calls");

    // Prove the same instrumented export runs the ordinary release path.
    handles::ANALYSIS.set(false);
    let booted = out(rt, handles::exact_boot(rt, 390.0, 844.0));
    assert!(booted.contains("\"error\":null"), "{booted}");
    let calls = handles::CALLS.get();
    assert!(calls[0] > 0 && calls[1] > 0 && calls[5] > 0 && calls[6] > 0);
    handles::exact_destroy(rt);
}

#[test]
fn two_runtimes_live_apart_in_one_thread_and_a_dead_handle_is_refused() {
    let a = handles::exact_create();
    let b = handles::exact_create();
    assert!(
        a != 0 && b != 0 && a != b,
        "handles are distinct and never 0"
    );
    assert_eq!(out(a, handles::exact_baked_compat(a)), handles::COMPAT);
    // Each boots its own plan: the baked fixture, and a second plan from bytes.
    let first = out(a, handles::exact_boot(a, 390.0, 844.0));
    assert!(first.contains("\"text\":\"baked\""), "{first}");
    let other = contract::compile("component Other\n  view\n    text \"other\"\n")
        .unwrap()
        .encode();
    let n = put(b, &other);
    let second = out(b, handles::exact_boot_plan(b, n, 200.0, 100.0));
    assert!(second.contains("\"text\":\"other\""), "{second}");
    assert!(!second.contains("baked"));
    // Their clocks are their own.
    let ta = out(a, handles::exact_advance(a, 5_000.0, 0));
    assert!(ta.contains("\"clock\":5000"), "{ta}");
    let tb = out(b, handles::exact_tick(b, 16.0));
    assert!(tb.contains("\"clock\":0"), "{tb}");
    // Their viewports are their own.
    let ra = out(a, handles::exact_resize(a, 500.0, 844.0));
    assert!(ra.contains("\"w\":500"), "{ra}");
    let rb = out(b, handles::exact_resize(b, 200.0, 100.0));
    assert_eq!(
        rb.matches("\"op\":\"frame\"").count(),
        0,
        "b did not move: {rb}"
    );
    // Destroying one leaves the other; the dead handle refuses every call
    // by name, and its buffers are gone (exact_in answers null).
    handles::exact_destroy(a);
    let dead = out(a, handles::exact_resize(a, 300.0, 300.0));
    assert!(dead.contains("no such runtime"), "{dead}");
    assert!(handles::exact_in(a, 4).is_null());
    handles::exact_destroy(a); // idempotent
    let alive = out(b, handles::exact_resize(b, 210.0, 100.0));
    assert!(
        alive.contains("\"error\":null") && alive.contains("\"w\":210"),
        "{alive}"
    );
    // A handle nobody was given is refused the same way, and handles are
    // never reused: the next create is a new number — from a process-wide
    // counter, so a handle made on another thread is a stranger here too.
    let c = handles::exact_create();
    assert!(c > b, "never reused: {a} {b} {c}");
    let elsewhere = std::thread::spawn(|| handles::exact_create())
        .join()
        .unwrap();
    assert!(elsewhere > c, "process-wide, never per thread: {elsewhere}");
    let stranger = out(elsewhere, handles::exact_boot(elsewhere, 1.0, 1.0));
    assert!(stranger.contains("no such runtime"), "{stranger}");
    let invented = out(c + 1000, handles::exact_boot(c + 1000, 1.0, 1.0));
    assert!(invented.contains("no such runtime"), "{invented}");
    handles::exact_destroy(b);
    handles::exact_destroy(c);
}

#[test]
fn a_setter_on_a_runtime_takes_effect_at_its_boot() {
    let rt = handles::exact_create();
    handles::exact_set_measure(rt, Some(wide_glyphs), std::ptr::null_mut());
    let batch = out(rt, handles::exact_boot(rt, 390.0, 844.0));
    // "baked" at 16 pt, one em per glyph: 80 wide by the callback, 20 tall.
    let at = batch.find("\"text\":\"baked\"").unwrap();
    let head = &batch[..at];
    let id: u32 = head[head.rfind("\"id\":").unwrap() + 5..]
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let f = frame_of(&batch, id);
    assert!((f.3 - 20.0).abs() <= 0.5, "{f:?}");
    handles::exact_destroy(rt);
}

#[test]
fn a_core_only_archive_refuses_compatibility_facts_that_require_an_adapter() {
    let plan = caltrain::build().unwrap().encode();
    let mut bridge = Bridge::new();
    bridge.set_compat(r#"{"inputs":{"store":{"L":"A"}}}"#);
    let n = bridge.boot(
        &plan,
        caltrain_data::Caltrain,
        exact_apple::abi::Hooks::none(),
        390.0,
        844.0,
    );
    let refused = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(
        refused.contains("does not match the linked delivery adapter"),
        "{refused}"
    );
    bridge.set_compat(r#"{"inputs":{"store":{"L":"0"}}}"#);
    let n = bridge.boot(
        &plan,
        caltrain_data::Caltrain,
        exact_apple::abi::Hooks::none(),
        390.0,
        844.0,
    );
    let booted = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(
        booted.contains("\"error\":null"),
        "a matching core composition must boot"
    );
}

#[test]
fn an_analysis_archive_refuses_both_boot_and_prepare_but_exposes_its_receipt() {
    let plan = contract::compile("component App\n  view\n    text \"analysis\"\n")
        .unwrap()
        .encode();
    for compat in [
        r#"{"inputs":{"store":{"L":"0"}},"embedded":{"analysis":true,"seq":null}}"#,
        r#"{"inputs":{"store":{"L":"A"}},"embedded":{"analysis":true,"seq":null}}"#,
    ] {
        let mut bridge = Bridge::new();
        bridge.set_compat(compat);
        let n = bridge.baked_compat(compat);
        assert_eq!(bridge.output_bytes(n as usize), compat.as_bytes());
        let hooks = exact_apple::abi::Hooks::none();
        let n = bridge.boot(&plan, NoData, hooks, 390.0, 844.0);
        let refusal = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
        assert!(
            refusal.contains("compatibility analysis artifact"),
            "{refusal}"
        );
        assert!(refusal.contains("\"ops\":[]"), "{refusal}");
        bridge.input_write(&plan);
        let n = bridge.prepare_plan(plan.len(), NoData, hooks, 390.0, 844.0);
        let refusal = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
        assert!(
            refusal.contains("compatibility analysis artifact"),
            "{refusal}"
        );
        let n = bridge.commit_plan();
        let refusal = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
        assert!(refusal.contains("no prepared plan"), "{refusal}");
    }
}

#[test]
fn a_multi_timer_advance_creates_children_before_attaching_them() {
    let plan = contract::compile(
        r#"component App
  state phase = 0
  action tick
    phase = phase + 1
  task clock mount
    every(100, tick)
  view
    column testId="parent"
      when phase == 1
        text "typing"
      when phase >= 2
        text "reply" testId="reply"
"#,
    )
    .unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let advance = host.advance(200.0);
    let kernel = host.runner().kernel();
    let reply = kernel
        .node_by_key(kernel.find_by_test_id("reply")[0])
        .unwrap()
        .id;
    let parent = kernel
        .node_by_key(kernel.find_by_test_id("parent")[0])
        .unwrap()
        .id;
    let creation = advance
        .find(&format!("\"op\":\"create\",\"id\":{reply},"))
        .unwrap();
    let attachment = advance
        .find(&format!(
            "\"op\":\"children\",\"id\":{parent},\"ids\":[{reply}]"
        ))
        .unwrap();
    assert!(
        creation < attachment,
        "child attached before creation: {advance}"
    );
}

#[test]
fn inherited_spelling_hint_updates_editors_and_returns_to_default() {
    let plan = contract::compile(
        r#"component App
  state hint = "false"
  action enable
    hint = "true"
  action reset
    hint = "invalid"
  view
    column spellcheck=hint
      button "Enable" press=enable testId="enable"
      button "Reset" press=reset testId="reset"
      column
        input testId="field" value="hello"
        textarea testId="area" value="hello" spellcheck="invalid"
        input testId="override" spellcheck="FALSE"
"#,
    )
    .unwrap();
    let (mut host, initial) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let ids = [view(&host, "field"), view(&host, "area")];
    let overridden = view(&host, "override");
    for id in ids {
        let op = initial
            .split("{\"op\":")
            .find(|part| part.contains(&format!("\"id\":{id},")))
            .unwrap();
        assert!(op.contains("\"spellcheck\":\"false\""), "{op}");
    }
    let enable = view(&host, "enable");
    let changed = host.dispatch_at(enable, Event::Press, 0.0);
    for id in ids {
        assert!(
            changed.contains(&format!(
                "\"op\":\"props\",\"id\":{id},\"set\":{{\"spellcheck\":\"true\"}}"
            )),
            "{changed}"
        );
    }
    assert!(!changed.contains(&format!("\"op\":\"props\",\"id\":{overridden},")));
    let reset = view(&host, "reset");
    let cleared = host.dispatch_at(reset, Event::Press, 1.0);
    for id in ids {
        assert!(
            cleared.contains(&format!(
                "\"op\":\"props\",\"id\":{id},\"set\":{{}},\"clear\":[\"spellcheck\"]"
            )),
            "{cleared}"
        );
    }
    assert_eq!(count(&host.resize(402.0, 874.0), "props"), 0);
}

#[test]
fn content_below_the_client_height_keeps_its_extent_and_changes() {
    let plan = contract::compile(
        r#"component Extent
  state large = false
  action grow
    large = true
  view
    column
      button "Grow" press=grow testId="grow"
      scroll testId="scroll" width=200 height=200 padding-top=5 padding-bottom=7
        box width=120 height=(large ? 80 : 40)
"#,
    )
    .unwrap();
    let (mut host, initial) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let scroll = view(&host, "scroll");
    assert_eq!(frame_of(&initial, scroll).3, 212.0);
    assert_eq!(content_of(&initial, scroll).1, 52.0);
    let grow = view(&host, "grow");
    let changed = host.dispatch_at(grow, Event::Press, 0.0);
    assert_eq!(content_of(&changed, scroll).1, 92.0);
    assert!(!changed.contains(&format!("\"op\":\"frame\",\"id\":{scroll},")));
}

#[test]
fn created_listeners_follow_the_final_tree_in_single_and_bulk_batches() {
    let plan = contract::compile(
        r#"component App
  state phase = 0
  action next
    phase = phase + 1
  view
    column
      when phase == 1
        column
          text "next" press=next contextmenu=next testId="go"
          text "plain" testId="plain"
      else
        text "next" press=next testId="go"
"#,
    )
    .unwrap();
    let (mut host, mut batch) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let mut old = None;
    for phase in 0..3 {
        let id = view(&host, "go");
        let prefix = format!("\"create\",\"id\":{id},");
        let create = batch
            .split("{\"op\":")
            .find(|part| part.starts_with(&prefix))
            .unwrap();
        let expected = if phase == 1 {
            "\"handlers\":[\"press\",\"contextmenu\"]"
        } else {
            "\"handlers\":[\"press\"]"
        };
        assert!(create.contains(expected), "{create}");
        let tree = host.agent(r#"{"op":"tree"}"#);
        assert_eq!(tree.matches(expected).count(), 1);
        let empty = if phase == 1 { 3 } else { 1 };
        assert_eq!(tree.matches("\"handlers\":[]").count(), empty);
        if phase == 1 {
            let plain = view(&host, "plain");
            let prefix = format!("\"create\",\"id\":{plain},");
            let create = batch
                .split("{\"op\":")
                .find(|part| part.starts_with(&prefix))
                .unwrap();
            assert!(create.contains("\"handlers\":[]"), "{create}");
        }
        if let Some(gone) = old {
            assert!(!tree.contains(&format!("{{\"id\":{gone},")));
            assert!(host.runner().handlers_of(gone).is_empty());
            assert!(!host.runner().handlers().contains_key(&gone));
        }
        old = Some(id);
        if phase < 2 {
            batch = host.dispatch_at(id, Event::Press, 0.0);
        }
    }
    assert_eq!(count(&batch, "create"), 1, "single-create path");
}

#[test]
fn resolved_language_is_published_at_boot_and_after_switching() {
    let mut plan = contract::compile("component App\n  state language = \"en\"\n  view\n    column width=300\n      text \"123\" testId=\"text\"\n").unwrap();
    plan.locale = Some(exact_plan::SlotsId(0));
    for rtl in [false, true] {
        let name = exact_plan::StrId(plan.strings.len() as u32);
        plan.strings.push(if rtl { "ar" } else { "en" }.into());
        plan.locales.push(exact_plan::LocalesRow {
            name,
            rtl,
            texts: Default::default(),
        });
    }
    let plan = contract::bake(plan, NoData).unwrap();
    let (mut host, batch) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.0,
        300.0,
    )
    .unwrap();
    assert!(batch.contains(r#"{"op":"language","lang":"en","dir":"ltr"}"#));
    let batch = host.set_place("ar-EG", "UTC", None);
    assert!(
        batch.contains(r#"{"op":"language","lang":"ar","dir":"rtl"}"#),
        "{batch}"
    );
    assert!(batch.contains(r#""direction":"rtl""#), "{batch}");
    assert!(!host
        .set_place("ar-SA", "UTC", None)
        .contains(r#""op":"language""#));
    let batch = host.set_place("de", "UTC", None);
    assert!(batch.contains(r#"{"op":"language","lang":"en","dir":"ltr"}"#));
}

// @ref LLP 1073 D2, D4, D5 — a presented frame fires the frame task once, at
// the frame's time; once frames are presented the timeout fires none; a stall
// is not caught up; the agent's clock takes them back as virtual frames.
#[test]
fn a_frame_task_fires_once_per_presented_frame() {
    let plan = contract::compile(
        r#"component App
  state frames = 0
  state at = 0
  action step
    frames = frames + 1
    at = now()
  task ticker mount
    every(frame, step)
  view
    text `${frames}`
"#,
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    assert!(first.contains("\"frames\":true"), "{first}");
    let slot = |host: &Host<NoData>, name| host.runner().slot(name).cloned();
    let frame = host.frame(10.0);
    assert!(frame.contains("\"clock\":10"), "{frame}");
    assert!(frame.contains("\"frames\":true"), "{frame}");
    assert!(!frame.contains("timer_due_ms"), "{frame}");
    assert_eq!(slot(&host, "frames"), Some(Value::Number(1.0)));
    host.advance(1_000.0);
    assert_eq!(slot(&host, "frames"), Some(Value::Number(1.0)));
    let at = slot(&host, "at");
    host.frame(5_000.0);
    assert_eq!(slot(&host, "frames"), Some(Value::Number(2.0)));
    assert_ne!(slot(&host, "at"), at);
    // The agent's clock: a second of seek is sixty virtual frames.
    host.present_frames(false);
    host.advance_until_request(6_000.0);
    assert_eq!(slot(&host, "frames"), Some(Value::Number(62.0)));
}

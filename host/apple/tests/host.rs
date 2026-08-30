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

extern "C" fn record_fonts(catalog: *const CFontCatalog) {
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
fn the_first_batch_creates_places_and_sizes_the_whole_tree() {
    let (host, batch) = boot();
    assert!(batch.starts_with("{\"ops\":["));
    assert!(
        batch.ends_with(",\"timers\":true,\"motion\":false,\"clock\":0,\"error\":null}"),
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

struct NoData;
impl DataSource for NoData {
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
        (f.3 - 16.25).abs() <= 0.5,
        "16.25, rounded to the point grid: {f:?}"
    );
    assert!(f.2 >= 104.0, "{f:?}");
    // The bridge's other calls answer too.
    let len = bridge.tick(16.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("\"motion\":false"));
    let len = bridge.resize(500.0, 844.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("\"w\":500"));
}

#[test]
fn the_plan_font_catalog_and_family_runs_cross_the_host_seam_before_layout() {
    FONT_CATALOG.lock().unwrap().clear();
    FONT_RUNS.lock().unwrap().clear();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/fixtures/fonts/app.contract");
    let plan = contract::compile_path(&path).unwrap().encode();
    let mut bridge: Bridge<NoData> = Bridge::new();
    bridge.set_fonts(Some(record_fonts));
    let len = bridge.boot(
        &plan,
        NoData,
        exact_apple::abi::Hooks {
            measure: Some(record_font_runs),
            ctx: std::ptr::null_mut(),
            wake: None,
            wake_ctx: std::ptr::null_mut(),
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
fn url_descriptors_survive_process_registration_name_collisions() {
    let source = include_str!("../swift/Text.swift");
    let install = source
        .split("static func install")
        .nth(1)
        .unwrap()
        .split("private static func fontURL")
        .next()
        .unwrap();
    let descriptor = install
        .find("CTFontManagerCreateFontDescriptorsFromURL")
        .unwrap();
    let registration = install.find("if !register(url)").unwrap();
    assert!(
        descriptor < registration,
        "URL identity must be acquired before best-effort process registration"
    );

    let register = source
        .split("private static func register")
        .nth(1)
        .unwrap()
        .split("private static func matched")
        .next()
        .unwrap();
    assert!(register.contains("CTFontManagerError.alreadyRegistered"));
    assert!(register.contains("CTFontManagerError.duplicatedName"));
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

//! Native speed metrics for the v1 app, as one JSON line.
//!
//! Run by `scripts/metrics.mjs`; every number is a p50 over repeated runs
//! (or a size), measured on the real pipeline: compile → bake → boot →
//! layout → update → tick, plus the web host's batches.

use exact_kernel::{Kernel, Offer, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};
use exact_web::Host;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

// Diagnostic-only allocation accounting. Production hosts never link this
// binary. The memory run counts net requested bytes after a baseline, not
// allocator capacity, RSS, or a claim that every allocation belongs to a node.
struct MeasuredAllocator;
static MEASURE_HEAP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static HEAP_DELTA: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static HEAP_PEAK: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

fn allocated(bytes: isize) {
    use std::sync::atomic::Ordering::Relaxed;
    if MEASURE_HEAP.load(Relaxed) {
        let live = HEAP_DELTA.fetch_add(bytes, Relaxed) + bytes;
        HEAP_PEAK.fetch_max(live, Relaxed);
    }
}

// SAFETY: every operation delegates to System with the original pointer and
// layout; accounting uses non-allocating atomics and never changes ownership.
unsafe impl std::alloc::GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::System.alloc(layout) };
        if !p.is_null() {
            allocated(layout.size() as isize);
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::System.alloc_zeroed(layout) };
        if !p.is_null() {
            allocated(layout.size() as isize);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, layout) };
        allocated(-(layout.size() as isize));
    }

    unsafe fn realloc(&self, p: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        let next = unsafe { std::alloc::System.realloc(p, layout, size) };
        if !next.is_null() {
            allocated(size as isize - layout.size() as isize);
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;

fn p50(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let v = f();
    (v, t.elapsed().as_secs_f64() * 1000.0)
}

fn repeat<T>(n: usize, mut f: impl FnMut() -> T) -> (T, f64) {
    let mut samples = Vec::with_capacity(n);
    let mut last = None;
    for _ in 0..n {
        let (v, ms) = time(&mut f);
        samples.push(ms);
        last = Some(v);
    }
    (last.unwrap(), p50(samples))
}

fn view_of(k: &Kernel, test_id: &str) -> u32 {
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

fn main() {
    if let Some(index) = std::env::args().position(|arg| arg == "--collection-memory") {
        let count: usize = std::env::args()
            .nth(index + 1)
            .expect("row count")
            .parse()
            .unwrap();
        let mode = std::env::args()
            .nth(index + 2)
            .expect("eager or virtualized");
        assert!((1..=25000).contains(&count));
        assert!(matches!(mode.as_str(), "eager" | "virtualized"));
        collection_memory(
            count,
            mode == "virtualized",
            std::env::args().any(|arg| arg == "--hold"),
        );
        return;
    }
    if let Some(index) = std::env::args().position(|arg| arg == "--list-memory") {
        let count: usize = std::env::args()
            .nth(index + 1)
            .expect("--list-memory needs a row count")
            .parse()
            .expect("row count must be an integer");
        assert!(count > 0 && count <= 25000, "row count must be 1..=25000");
        list_memory(
            count,
            std::env::args().any(|arg| arg == "--hold"),
            std::env::args().any(|arg| arg == "--windowed"),
        );
        return;
    }
    if std::env::args().any(|arg| arg == "--scaling") {
        scaling();
        return;
    }
    let (plan, compile_ms) = repeat(20, || caltrain::compile().unwrap());
    let plan_bytes = plan.encode().len();
    let (baked, bake_ms) = repeat(5, || {
        contract::bake(plan.clone(), caltrain_data::Caltrain).unwrap()
    });
    let baked_bytes = baked.encode().len();
    let encoded = baked.encode();
    let (_, decode_ms) = repeat(20, || exact_plan::Plan::decode(&encoded).unwrap());

    let (runner, boot_ms) = repeat(10, || {
        Runner::boot(
            baked.clone(),
            caltrain_data::Caltrain,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    });
    let nodes = runner.kernel().live_count();
    let mut runner = runner;
    // The first layout after boot: every node dirty, the real first-frame cost.
    let layout_ms = {
        let mut samples = Vec::new();
        for _ in 0..10 {
            let mut fresh = Runner::boot(
                baked.clone(),
                caltrain_data::Caltrain,
                Kernel::with_monospace(),
                Default::default(),
                "/",
            )
            .unwrap();
            let root = fresh.roots()[0];
            let (_, ms) = time(|| {
                fresh
                    .kernel_mut()
                    .compute_layout(root, Offer::definite(390.0, 844.0))
                    .unwrap()
            });
            samples.push(ms);
        }
        p50(samples)
    };

    // An update that swaps a screen (the heaviest common interaction), alternating.
    let change = view_of(runner.kernel(), "change-station");
    runner.dispatch(change, Event::Press).unwrap();
    let back = view_of(runner.kernel(), "stations-back");
    runner.dispatch(back, Event::Press).unwrap();
    let mut samples = Vec::new();
    for i in 0..20 {
        let id = if i % 2 == 0 {
            view_of(runner.kernel(), "change-station")
        } else {
            view_of(runner.kernel(), "stations-back")
        };
        let (_, ms) = time(|| runner.dispatch(id, Event::Press).unwrap());
        samples.push(ms);
    }
    let update_ms = p50(samples);
    // An inherited row changed on the root (LLP 1035.000 D2): the kernel
    // re-derives the computed value down the tree, stopping under any node
    // that sets its own — the invalidation's whole cost, and how far it
    // reached, alternating two inks so every change is a change.
    let (inherit_ms, inherit_touched) = {
        use exact_kernel::{Color, ColorValue, Op, StyleId, StyleMask, StyleProps};
        let root = runner.roots()[0];
        let mut samples = Vec::new();
        let mut touched = 0;
        for i in 0..20 {
            let mut mask = StyleMask::default();
            mask.set(StyleId::TextColor);
            let patch = StyleProps {
                text_color: ColorValue::Fixed(Color(if i % 2 == 0 {
                    0x112233ff
                } else {
                    0x445566ff
                })),
                mask,
                ..StyleProps::default()
            };
            let op = Op::SetStyle {
                id: root,
                patch: Box::new(patch),
            };
            let (receipt, ms) = time(|| runner.kernel_mut().apply(0, 0, &[op]).unwrap());
            touched = receipt.touched.len();
            samples.push(ms);
        }
        (p50(samples), touched)
    };
    let mut now = runner.now_ms();
    let (_, tick_ms) = repeat(20, || {
        now += 1000.0;
        runner.advance(now).unwrap()
    });
    let text_nodes = {
        let k = runner.kernel();
        let mut n = 0;
        let mut stack = k.roots();
        while let Some(id) = stack.pop() {
            let node = k.node(id).unwrap();
            if node.props.str(PropId::Text).is_some() {
                n += 1;
            }
            stack.extend(node.children());
        }
        n
    };

    // The web host's batches.
    let ((host, first_batch), web_boot_ms) = repeat(5, || {
        Host::boot(&encoded, caltrain_data::Caltrain, Default::default(), "/").unwrap()
    });
    let mut host = host;
    let change = view_of(host.runner().kernel(), "change-station");
    let (batch, web_update_ms) = time(|| host.dispatch(change, Event::Press));

    println!(
        "{{\"compile_ms\":{compile_ms:.3},\"bake_ms\":{bake_ms:.3},\"decode_ms\":{decode_ms:.3},\"plan_bytes\":{plan_bytes},\"baked_bytes\":{baked_bytes},\"boot_ms\":{boot_ms:.3},\"nodes\":{nodes},\"text_nodes\":{text_nodes},\"layout_ms\":{layout_ms:.3},\"update_ms\":{update_ms:.3},\"inherit_ms\":{inherit_ms:.3},\"inherit_touched\":{inherit_touched},\"tick_ms\":{tick_ms:.3},\"web_boot_ms\":{web_boot_ms:.3},\"web_first_batch_bytes\":{},\"web_update_ms\":{web_update_ms:.3},\"web_update_batch_bytes\":{}}}",
        first_batch.len(),
        batch.len()
    );
}

// @ref LLP 1010 §6 — baseline first; these are runner measurements, not
// browser/phone frames. One invocation owns one size and one runner.
fn list_memory(count: usize, hold: bool, windowed: bool) {
    use std::io::Write;
    use std::sync::atomic::Ordering::Relaxed;

    struct MemoryRows(Value);
    impl DataSource for MemoryRows {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(self.0.clone())
        }
    }

    let source = r#"shape Row
  id: number
component App
  resource rows = rows() as shape list<Row>
  view
    scroll height=844 width=390
      column
        each row in rows key=row.id
          text `${row.id}` testId=`row-${row.id}` height=24 flex-shrink=0
"#;
    // Compile outside the measured interval; retain only the encoded input.
    // The decoded plan and source data below are counted separately once,
    // even when runner values share their storage through Rc.
    let source = if windowed {
        source.replace("scroll height=844 width=390\n      column\n        each row in rows key=row.id\n          text", "list item-height=24 height=844 width=390\n      each row in rows key=row.id\n        text")
    } else {
        source.to_string()
    };
    let plan = contract::compile(&source).unwrap();
    let encoded = plan.encode();
    drop(plan);
    MEASURE_HEAP.store(true, Relaxed);
    let data = MemoryRows(Value::list(
        (0..count)
            .map(|id| Value::Record(vec![Value::Number(id as f64)].into()))
            .collect(),
    ));
    let data_bytes = HEAP_DELTA.load(Relaxed);
    let (plan, decode_ms) = time(|| exact_plan::Plan::decode(&encoded).unwrap());
    let plan_bytes = HEAP_DELTA.load(Relaxed) - data_bytes;
    let (mut runner, boot_ms) = time(|| {
        Runner::boot(
            plan,
            data,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    });
    let root = runner.roots()[0];
    let (_, window_ms) = time(|| {
        if windowed {
            runner
                .list_viewport(
                    root,
                    exact_runner::ListViewport {
                        top: (count as f64 * 12.0 - 422.0).max(0.0),
                        height: 844.0,
                        width: 390.0,
                        origin: 0.0,
                        pins: [0, 0],
                        rows: &[],
                        velocity: 0.0,
                    },
                )
                .unwrap();
        }
    });
    let (_, layout_ms) = time(|| {
        runner
            .kernel_mut()
            .compute_layout(root, Offer::definite(390.0, 844.0))
            .unwrap()
    });
    let nodes = runner.kernel().live_count();
    if windowed {
        assert!(nodes <= 216, "three viewports of row instances");
    } else {
        assert_eq!(nodes, count + 2, "the baseline must materialize every row");
    }
    let initial_retained = HEAP_DELTA.load(Relaxed);
    let mut first_traversal_retained = initial_retained;
    if windowed {
        for pass in 0..20 {
            for row in (0..count).step_by(35).chain((0..count).step_by(35).rev()) {
                let top = (row as f64 * 24.0).min((count as f64 * 24.0 - 844.0).max(0.0));
                runner
                    .list_viewport(
                        root,
                        exact_runner::ListViewport {
                            top,
                            height: 844.0,
                            width: 390.0,
                            origin: 0.0,
                            pins: [0, 0],
                            rows: &[],
                            velocity: 0.0,
                        },
                    )
                    .unwrap();
                runner
                    .kernel_mut()
                    .compute_layout(root, Offer::definite(390.0, 844.0))
                    .unwrap();
                assert!(runner.kernel().live_count() <= 216);
            }
            runner
                .list_viewport(
                    root,
                    exact_runner::ListViewport {
                        top: (count as f64 * 12.0 - 422.0).max(0.0),
                        height: 844.0,
                        width: 390.0,
                        origin: 0.0,
                        pins: [0, 0],
                        rows: &[],
                        velocity: 0.0,
                    },
                )
                .unwrap();
            runner
                .kernel_mut()
                .compute_layout(root, Offer::definite(390.0, 844.0))
                .unwrap();
            if pass == 0 {
                first_traversal_retained = HEAP_DELTA.load(Relaxed);
            }
        }
    }
    let retained = HEAP_DELTA.load(Relaxed);
    let peak = HEAP_PEAK.load(Relaxed);
    MEASURE_HEAP.store(false, Relaxed);
    let mode = if windowed { "windowed" } else { "eager" };
    let slots = runner.kernel().arena().slot_count();
    println!(
        "{{\"mode\":\"{mode}\",\"initial_retained_heap_bytes\":{initial_retained},\"first_traversal_retained_heap_bytes\":{first_traversal_retained},\"kernel_slots\":{slots},\"window_ms\":{window_ms:.4},\"rows\":{count},\"live_kernel_nodes\":{nodes},\"encoded_plan_bytes\":{},\"data_heap_bytes\":{data_bytes},\"decoded_plan_heap_bytes\":{plan_bytes},\"retained_heap_delta_bytes\":{retained},\"peak_heap_delta_bytes\":{peak},\"decode_ms\":{decode_ms:.4},\"runner_boot_ms\":{boot_ms:.4},\"layout_ms\":{layout_ms:.4},\"first_frame_ms\":null,\"native_views\":null,\"decoded_image_bytes\":null}}",
        encoded.len()
    );
    // The parent samples RSS while the runner is still alive. It then sends
    // one newline; a direct invocation without --hold finishes immediately.
    if hold {
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
    }
    std::hint::black_box(&runner);
}

// A diagnostic workload, not a blocking benchmark or a runtime dependency graph.
// Every row has one text node; unrelated rows remain present during local edits.
#[derive(Clone)]
struct Rows {
    count: usize,
    requests: Rc<Cell<usize>>,
}

impl DataSource for Rows {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        self.requests.set(self.requests.get() + 1);
        let mut rows: Vec<_> = (0..self.count)
            .map(|id| Value::Record(vec![Value::Number(id as f64)].into()))
            .collect();
        if args == [Value::Bool(true)] {
            rows.reverse();
        }
        Ok(Value::List(rows.into()))
    }
}

fn summary(mut samples: Vec<f64>) -> String {
    samples.sort_by(f64::total_cmp);
    format!(
        "{{\"p50\":{:.4},\"p95\":{:.4}}}",
        samples[samples.len() / 2],
        samples[(samples.len() * 95).div_ceil(100) - 1]
    )
}

fn scaling() {
    let source = r#"shape Row
  id: number
component App
  state count = 0
  state reverse = false
  state shown = true
  resource rows = rows(reverse) as shape list<Row>
  derive total = length(rows)
  action bump writes count
    count = count + 1
  action reorder writes reverse
    reverse = !reverse
  action topology writes shown
    shown = !shown
  view
    column
      button press=bump testId="bump"
        text `${count}`
      button press=reorder testId="reorder"
        text "reorder"
      button press=topology testId="topology"
        text "topology"
      text `${total}`
      when shown
        each row in rows key=row.id
          text `${row.id}` testId=`row-${row.id}`
"#;
    let plan = contract::compile(source).unwrap();
    let mut results = Vec::new();
    for count in [300, 3000, 10000] {
        let data = Rows {
            count,
            requests: Rc::new(Cell::new(0)),
        };
        let encoded = plan.encode();
        for action in ["bump", "reorder", "topology"] {
            let mut runner = Runner::boot(
                plan.clone(),
                data.clone(),
                Kernel::with_monospace(),
                Default::default(),
                "/",
            )
            .unwrap();
            let root = runner.roots()[0];
            runner
                .kernel_mut()
                .compute_layout(root, Offer::definite(390.0, 844.0))
                .unwrap();
            let (mut host, _) =
                Host::boot(&encoded, data.clone(), Default::default(), "/").unwrap();
            let id = view_of(runner.kernel(), action);
            let host_id = view_of(host.runner().kernel(), action);
            let mut updates = Vec::new();
            let mut layouts = Vec::new();
            let mut batches = Vec::new();
            let mut requests = Vec::new();
            let mut touched = Vec::new();
            let mut created = Vec::new();
            let mut destroyed = Vec::new();
            let mut batch_bytes = Vec::new();
            for iteration in 0..44 {
                let before = data.requests.get();
                let (receipt, update_ms) = time(|| runner.dispatch(id, Event::Press).unwrap());
                let requested = data.requests.get() - before;
                let (_, layout_ms) = time(|| {
                    runner
                        .kernel_mut()
                        .compute_layout(root, Offer::definite(390.0, 844.0))
                        .unwrap()
                });
                let (batch, host_ms) = time(|| host.dispatch(host_id, Event::Press));
                assert!(!batch.contains("\"error\":\""), "{batch}");
                if iteration >= 4 {
                    updates.push(update_ms);
                    layouts.push(layout_ms);
                    batches.push(host_ms);
                    requests.push(requested as f64);
                    touched.push(receipt.touched.len() as f64);
                    created.push(receipt.created.len() as f64);
                    destroyed.push(receipt.destroyed.len() as f64);
                    batch_bytes.push(batch.len() as f64);
                }
                if action == "bump" {
                    assert_eq!(receipt.touched.len(), 1);
                    assert_eq!(requested, 0);
                }
                if action == "reorder" {
                    assert!(receipt.created.is_empty() && receipt.destroyed.is_empty());
                    assert_eq!(requested, 1);
                }
            }
            results.push(format!(
                "{{\"rows\":{count},\"nodes\":{},\"action\":\"{action}\",\"samples\":40,\"runner_update_ms\":{},\"layout_ms\":{},\"web_runner_and_batch_ms\":{},\"source_requests\":{},\"touched\":{},\"created\":{},\"destroyed\":{},\"batch_bytes\":{}}}",
                runner.kernel().live_count(), summary(updates), summary(layouts), summary(batches),
                summary(requests), summary(touched), summary(created), summary(destroyed), summary(batch_bytes)
            ));
        }
    }
    println!("{{\"scaling\":[{}],\"scaling_note\":\"release; four warmup updates, 40 measured; runner includes settlement/evaluation/kernel apply; web timings include runner and serialization, not browser; layout uses monospace, no physical device or allocation measurement\"}}", results.join(","));
}

// @ref LLP 1010 §6: paired current-binary eager/windowed controls. Geometry is
// measured by the real kernel; there is no browser/native presenter in this run.
const COLLECTION_HEIGHT: f64 = 800.0;
const COLLECTION_WIDTH: f64 = 390.0;
const COLLECTION_ROW_HEIGHT: f64 = 24.0;

struct CollectionData {
    count: usize,
    data: Value,
    reverse: bool,
    revision: usize,
    queries: usize,
}
impl DataSource for CollectionData {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        self.queries += 1;
        let reverse = args[0] == Value::Bool(true);
        let revision = args[1].as_number().unwrap() as usize;
        if reverse != self.reverse || revision != self.revision {
            self.data = collection_records(self.count, reverse, revision);
            self.reverse = reverse;
            self.revision = revision;
        }
        Ok(self.data.clone())
    }
}
fn collection_records(count: usize, reverse: bool, revision: usize) -> Value {
    Value::list(
        (0..count)
            .map(|index| {
                let index = if reverse { count - index - 1 } else { index };
                Value::record(vec![Value::Number((revision * count + index) as f64)])
            })
            .collect(),
    )
}
struct CollectionSamples {
    runner: Vec<f64>,
    layout: Vec<f64>,
    driver: Vec<f64>,
    feedbacks: Vec<usize>,
    keyed: Vec<usize>,
    max_live: usize,
    max_arena: usize,
}
impl CollectionSamples {
    fn new(count: usize) -> Self {
        // Preallocated before heap baseline; no timed sample can grow this buffer.
        // RSS still includes whichever of these diagnostic pages become resident.
        let capacity = (count.div_ceil(20) + 10) * 40 + 128;
        Self {
            runner: Vec::with_capacity(capacity),
            layout: Vec::with_capacity(capacity),
            driver: Vec::with_capacity(capacity),
            feedbacks: Vec::with_capacity(capacity),
            keyed: Vec::with_capacity(capacity),
            max_live: 0,
            max_arena: 0,
        }
    }
    fn clear(&mut self) {
        self.runner.clear();
        self.layout.clear();
        self.driver.clear();
        self.feedbacks.clear();
        self.keyed.clear();
        self.max_live = 0;
        self.max_arena = 0;
    }
    fn push(
        &mut self,
        runner: &Runner<CollectionData>,
        run: f64,
        layout: f64,
        driver: f64,
        feedbacks: usize,
        keyed: usize,
    ) {
        assert!(
            self.runner.len() < self.runner.capacity(),
            "diagnostic sample buffer exhausted"
        );
        self.runner.push(run);
        self.layout.push(layout);
        self.driver.push(driver);
        self.feedbacks.push(feedbacks);
        self.keyed.push(keyed);
        self.max_live = self.max_live.max(runner.kernel().live_count());
        self.max_arena = self.max_arena.max(runner.kernel().arena().slot_count());
    }
    fn buffer_bytes(&self) -> usize {
        self.runner.capacity() * (3 * std::mem::size_of::<f64>() + 2 * std::mem::size_of::<usize>())
    }
}
#[derive(Default)]
struct CollectionDriver {
    top: f64,
    sequence: u64,
}
fn collection_layout(runner: &mut Runner<CollectionData>) -> f64 {
    let root = runner.roots()[0];
    time(|| {
        runner
            .kernel_mut()
            .compute_layout(root, Offer::definite(390.0, 844.0))
            .unwrap()
    })
    .1
}
fn collection_limit(runner: &Runner<CollectionData>, virtualized: bool, count: usize) -> f64 {
    let extent = if virtualized {
        runner.collections()[0].total_extent
    } else {
        count as f64 * COLLECTION_ROW_HEIGHT
    };
    (extent - COLLECTION_HEIGHT).max(0.0)
}
impl CollectionDriver {
    fn settle(
        &mut self,
        runner: &mut Runner<CollectionData>,
        virtualized: bool,
        count: usize,
        samples: &mut CollectionSamples,
    ) {
        let started = Instant::now();
        let mut run_ms = 0.0;
        let mut layout_ms = 0.0;
        let mut feedbacks = 0;
        let queries = runner.data_ref().queries;
        if virtualized {
            if let Some(correction) = runner.collections()[0].correction {
                if correction.scroll_sequence == self.sequence {
                    self.top = correction.scroll_top;
                }
            }
        }
        self.top = self
            .top
            .clamp(0.0, collection_limit(runner, virtualized, count));
        if virtualized {
            for iteration in 0..12 {
                let snapshot = runner.collections().pop().unwrap();
                let port = runner.kernel().node(snapshot.view).unwrap().frame;
                assert!((port.width as f64 - COLLECTION_WIDTH).abs() < 0.1);
                assert!((port.height as f64 - COLLECTION_HEIGHT).abs() < 0.1);
                let measurements = snapshot
                    .rows
                    .iter()
                    .map(|row| exact_runner::RowMeasurement {
                        view: row.view,
                        epoch: row.epoch,
                        height: runner.kernel().node(row.view).unwrap().frame.height as f64,
                    })
                    .collect();
                self.sequence += 1;
                let facts = exact_runner::CollectionFeedback {
                    view: snapshot.view,
                    revision: snapshot.revision,
                    scroll_sequence: self.sequence,
                    scroll_top: self.top,
                    port_width: port.width as f64,
                    port_height: port.height as f64,
                    row_width: port.width as f64,
                    measurements,
                    focus_view: None,
                    interaction_view: None,
                };
                let (receipt, elapsed) = time(|| runner.collection_feedback(facts).unwrap());
                run_ms += elapsed;
                feedbacks += 1;
                assert_eq!(
                    runner.last_instance_work().rows_keyed,
                    0,
                    "scroll must not key the input list"
                );
                if receipt.receipts.is_empty() {
                    break;
                }
                layout_ms += collection_layout(runner);
                if let Some(correction) = runner.collections()[0].correction {
                    assert_eq!(correction.scroll_sequence, self.sequence);
                    self.top = correction.scroll_top;
                }
                assert!(iteration < 11, "measurement feedback did not converge");
            }
        }
        assert_eq!(
            runner.data_ref().queries,
            queries,
            "geometry reached the data seam"
        );
        samples.push(
            runner,
            run_ms,
            layout_ms,
            started.elapsed().as_secs_f64() * 1000.0,
            feedbacks,
            0,
        );
    }
}
struct CollectionMemoryIdentity {
    count: usize,
    virtualized: bool,
    encoded: usize,
    data: isize,
    plan: isize,
    owned_slots: usize,
    decode_ms: f64,
    buffers: usize,
    hold: bool,
}
fn collection_phase(
    runner: Option<&Runner<CollectionData>>,
    identity: &CollectionMemoryIdentity,
    name: &str,
    samples: &CollectionSamples,
    extra: &str,
) {
    use std::io::Write;
    use std::sync::atomic::Ordering::Relaxed;
    let retained = HEAP_DELTA.load(Relaxed);
    let peak = HEAP_PEAK.load(Relaxed);
    // All census/formatting/stdio temporaries are born and dropped with tracking
    // paused. They never pollute the runtime's retained requested-byte total.
    MEASURE_HEAP.store(false, Relaxed);
    {
        let (nodes, arena, rows, queries) = runner.map_or((0, 0, 0, 0), |r| {
            let kernel = r.kernel();
            let rows = kernel
                .arena()
                .iter_live()
                .filter(|slot| {
                    kernel
                        .node_by_key(kernel.arena().key(*slot))
                        .unwrap()
                        .props
                        .str(PropId::TestId)
                        .is_some_and(|id| id.starts_with("row-"))
                })
                .count();
            (
                kernel.live_count(),
                kernel.arena().slot_count(),
                rows,
                r.data_ref().queries,
            )
        });
        if runner.is_some() && name != "left_list" {
            if identity.virtualized {
                assert!(
                    rows <= 103,
                    "mounted rows exceeded one viewport plus two overscan viewports: {rows}"
                );
            } else {
                assert_eq!(rows, identity.count, "eager control must realize every row");
            }
        }
        let floats = |v: &[f64]| {
            v.iter()
                .map(|x| format!("{x:.6}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let integers = |v: &[usize]| v.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
        let stats = |v: &[f64]| {
            if v.is_empty() {
                "null".to_owned()
            } else {
                summary(v.to_vec())
            }
        };
        println!("{{\"phase\":\"{name}\",\"mode\":\"{}\",\"rows\":{},\"live_kernel_nodes\":{nodes},\"kernel_arena_slots\":{arena},\"live_row_instances\":{rows},\"live_row_local_slots\":{},\"encoded_plan_bytes\":{},\"data_initial_heap_bytes\":{},\"decoded_plan_heap_bytes\":{},\"decode_ms\":{},\"row_slot_count_basis\":\"live authored row roots times one compiled owned slot\",\"retained_heap_delta_bytes\":{retained},\"peak_heap_delta_bytes\":{peak},\"diagnostic_buffer_capacity_bytes\":{},\"source_queries_total\":{queries},\"sample_count\":{},\"max_sample_live_nodes\":{},\"max_sample_arena_slots\":{},\"runner_call_ms\":{},\"kernel_layout_ms\":{},\"driver_ms\":{},\"raw_runner_call_ms\":[{}],\"raw_kernel_layout_ms\":[{}],\"raw_driver_ms\":[{}],\"raw_feedback_calls\":[{}],\"raw_rows_keyed\":[{}],\"first_pixel_ms\":null,\"native_views\":null,\"decoded_image_bytes\":null{extra}}}",
            if identity.virtualized {"virtualized"} else {"eager"}, identity.count, rows * identity.owned_slots, identity.encoded,
            identity.data, identity.plan, identity.decode_ms, identity.buffers, samples.runner.len(), samples.max_live, samples.max_arena,
            stats(&samples.runner), stats(&samples.layout), stats(&samples.driver), floats(&samples.runner), floats(&samples.layout), floats(&samples.driver), integers(&samples.feedbacks), integers(&samples.keyed));
        if identity.hold {
            std::io::stdout().flush().unwrap();
            let mut ack = String::new();
            std::io::stdin().read_line(&mut ack).unwrap();
        }
    }
    MEASURE_HEAP.store(true, Relaxed);
}
fn collection_action(
    runner: &mut Runner<CollectionData>,
    driver: &mut CollectionDriver,
    virtualized: bool,
    count: usize,
    action: &str,
    args: Vec<Value>,
    samples: &mut CollectionSamples,
) {
    let started = Instant::now();
    let (_, mut run_ms) = time(|| runner.act(action, args).unwrap());
    let keyed = runner.last_instance_work().rows_keyed;
    let mut layout_ms = collection_layout(runner);
    let mut feedbacks = 0;
    if virtualized && action != "leave" {
        // Include post-action geometry settlement in the reported CPU path.
        let mut scratch = CollectionSamples {
            runner: Vec::with_capacity(1),
            layout: Vec::with_capacity(1),
            driver: Vec::with_capacity(1),
            feedbacks: Vec::with_capacity(1),
            keyed: Vec::with_capacity(1),
            max_live: 0,
            max_arena: 0,
        };
        driver.settle(runner, virtualized, count, &mut scratch);
        run_ms += scratch.runner[0];
        layout_ms += scratch.layout[0];
        feedbacks = scratch.feedbacks[0];
    }
    samples.push(
        runner,
        run_ms,
        layout_ms,
        started.elapsed().as_secs_f64() * 1000.0,
        feedbacks,
        keyed,
    );
}

fn collection_memory(count: usize, virtualized: bool, hold: bool) {
    use std::sync::atomic::Ordering::Relaxed;
    let source = r#"shape Row
  id: number
component App
  state draft = ""
  state body = 0
  state reverse = false
  state revision = 0
  state shown = true
  resource rows = rows(reverse, revision) as shape list<Row>
  action edit(value) writes draft
    draft = value
  action bump writes body
    body = body + 1
  action reorder writes reverse
    reverse = !reverse
  action replace writes revision
    revision = revision + 1
  action leave writes shown
    shown = false
  view
    column height=844 width=390
      input value=draft change=edit height=24 flex-shrink=0
      when shown
        list virtualized=VIRTUALIZED height=800 width=390
          each row in rows key=row.id
            Item(id=row.id, body=body)
component Item
  props
    id: number
    body: number
  state local = 0
  view
    text `${id} ${body} ${local}` testId=`row-${id}` height=24 flex-shrink=0
"#
    .replace("VIRTUALIZED", if virtualized { "true" } else { "false" });
    let compiled = contract::compile(&source).unwrap();
    let owned_slots = compiled.slots.iter().filter(|s| s.owner.is_some()).count();
    assert_eq!(owned_slots, 1);
    let encoded = compiled.encode();
    drop(compiled);
    drop(source);
    let mut samples = CollectionSamples::new(count);
    let mut identity = CollectionMemoryIdentity {
        count,
        virtualized,
        encoded: encoded.len(),
        data: 0,
        plan: 0,
        owned_slots,
        decode_ms: 0.0,
        buffers: samples.buffer_bytes(),
        hold,
    };
    collection_phase(None, &identity, "baseline", &samples, "");
    let data = CollectionData {
        count,
        data: collection_records(count, false, 0),
        reverse: false,
        revision: 0,
        queries: 0,
    };
    identity.data = HEAP_DELTA.load(Relaxed);
    let (plan, decode_ms) = time(|| exact_plan::Plan::decode(&encoded).unwrap());
    identity.plan = HEAP_DELTA.load(Relaxed) - identity.data;
    identity.decode_ms = decode_ms;
    let (mut runner, boot_ms) = time(|| {
        Runner::boot(
            plan,
            data,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    });
    let layout_ms = collection_layout(&mut runner);
    samples.push(
        &runner,
        boot_ms,
        layout_ms,
        boot_ms + layout_ms,
        0,
        runner.last_instance_work().rows_keyed,
    );
    collection_phase(Some(&runner), &identity, "bootstrap_layout", &samples, "");
    let mut driver = CollectionDriver::default();
    for phase in [
        "settled_top",
        "settled_middle",
        "settled_bottom",
        "return_top",
    ] {
        samples.clear();
        driver.top = match phase {
            "settled_middle" => collection_limit(&runner, virtualized, count) / 2.0,
            "settled_bottom" => collection_limit(&runner, virtualized, count),
            _ => 0.0,
        };
        driver.settle(&mut runner, virtualized, count, &mut samples);
        collection_phase(Some(&runner), &identity, phase, &samples, "");
    }
    samples.clear();
    for _ in 0..20 {
        loop {
            let limit = collection_limit(&runner, virtualized, count);
            if driver.top >= limit - 0.01 {
                break;
            }
            driver.top = (driver.top + COLLECTION_HEIGHT).min(limit);
            driver.settle(&mut runner, virtualized, count, &mut samples);
        }
        while driver.top > 0.01 {
            driver.top = (driver.top - COLLECTION_HEIGHT).max(0.0);
            driver.settle(&mut runner, virtualized, count, &mut samples);
        }
    }
    if virtualized {
        assert!(
            (runner.collections()[0].total_extent - count as f64 * COLLECTION_ROW_HEIGHT).abs()
                < 0.01,
            "traversals must measure the entire logical document"
        );
    }
    collection_phase(Some(&runner), &identity, "twenty_traversals", &samples,
        ",\"traversals\":20,\"traversal_definition\":\"top to bottom and back to top, steps at most one scrollport height\"");
    for (phase, action) in [("input_echo", "edit"), ("all_row_bodies", "bump")] {
        samples.clear();
        for i in 0..40 {
            let args = if action == "edit" {
                vec![Value::str(&format!("input {i}"))]
            } else {
                vec![]
            };
            collection_action(
                &mut runner,
                &mut driver,
                virtualized,
                count,
                action,
                args,
                &mut samples,
            );
        }
        collection_phase(Some(&runner), &identity, phase, &samples, "");
    }
    for (phase, action) in [
        ("replaced", "replace"),
        ("reordered", "reorder"),
        ("left_list", "leave"),
    ] {
        samples.clear();
        collection_action(
            &mut runner,
            &mut driver,
            virtualized,
            count,
            action,
            vec![],
            &mut samples,
        );
        collection_phase(Some(&runner), &identity, phase, &samples, "");
    }
    drop(runner);
    samples.clear();
    collection_phase(None, &identity, "runner_dropped", &samples, "");
    MEASURE_HEAP.store(false, Relaxed);
}

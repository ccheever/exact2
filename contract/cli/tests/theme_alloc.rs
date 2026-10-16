//! @ref LLP 1095 D4 — a theme costs a themed list nothing per row: the
//! plan's table is built once and bound by pointer, so mounting a 1,000-row
//! list whose rows name theme entries allocates no more than the same list
//! written in literal colours, within noise. Its own binary: it counts every
//! allocation the process makes.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn mount(src: &str) -> usize {
    let plan = contract::compile(src).unwrap();
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let n = ALLOCATIONS.load(Ordering::Relaxed) - before;
    drop(r);
    n
}

#[test]
fn a_themed_list_allocates_as_a_literal_one_does() {
    let rows: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
    let list = |row: &str| {
        format!("component App\n  state rows: number[] = [{}]\n  view\n    column\n      for r in rows key=r\n        text \"x\" {row}\n", rows.join(", "))
    };
    let themed = format!(
        "theme\n  brand = \"#c8102e\"\n  card = \"light-dark(#f1f3f5, #0f1419)\"\n  ink = \"brand\"\n{}\n{}",
        (0..60).map(|i| format!("  extra{i} = \"#{:06x}\"\n", i * 4000)).collect::<String>(),
        list("color=\"brand\" background-color=\"card\" border-color=\"ink\" box-shadow=\"0 1px 2px brand\"")
    );
    let plain = list("color=\"#c8102e\" background-color=\"light-dark(#f1f3f5, #0f1419)\" border-color=\"#c8102e\" box-shadow=\"0 1px 2px #c8102e\"");
    // Warm both (interning, lazy tables), then measure.
    mount(&themed);
    mount(&plain);
    let (themed, plain) = (mount(&themed), mount(&plain));
    eprintln!("themed {themed} allocations, plain {plain}");
    assert!(
        themed as f64 <= plain as f64 * 1.02 + 200.0,
        "themed {themed} allocations, plain {plain}"
    );
}

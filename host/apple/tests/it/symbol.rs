//! A system symbol's box through the Apple host's registered callback.
use crate::host::NoData;
use exact_apple::measure::{CMetrics, CRequest, CallbackMeasurer};
use exact_apple::Host;
use std::ffi::c_void;

extern "C" fn no_text(_ctx: *mut c_void, _request: *const CRequest) -> CMetrics {
    CMetrics {
        width: 0.0,
        height: 0.0,
        baseline: 0.0,
    }
}

/// A symbol a fake host draws 1.5 em wide and 1 em tall, at its weight.
extern "C" fn wide_symbols(
    _ctx: *mut c_void,
    name: *const u8,
    len: usize,
    size: f32,
    weight: u16,
    out: *mut f32,
) -> u8 {
    let name = std::str::from_utf8(unsafe { std::slice::from_raw_parts(name, len) }).unwrap();
    assert_eq!(
        (name, weight),
        ("gearshape", 400),
        "the Apple name a portable role draws"
    );
    unsafe {
        *out = 1.5 * size;
        *out.add(1) = size
    };
    1
}

/// A system symbol has its box in the first batch, measured through the
/// registered callback before layout, with no view to report it yet.
#[test]
fn a_symbol_is_measured_through_the_registered_callback_before_the_first_layout() {
    let src = "component App\n  view\n    row\n      image \"symbol:settings\" font-size=20 testId=\"gear\"\n";
    let plan = contract::compile(src).unwrap();
    let measurer =
        CallbackMeasurer::new(no_text, std::ptr::null_mut(), None).with_symbol(Some(wide_symbols));
    let (host, _) = Host::boot(&plan.encode(), NoData, Box::new(measurer), 402.0, 874.0).unwrap();
    let k = host.runner().kernel();
    let node = k.node_by_key(k.find_by_test_id("gear")[0]).unwrap();
    assert_eq!((node.frame.width, node.frame.height), (30.0, 20.0));
}

/// The exported setter reaches the measurer a runtime boots with: the symbol
/// callback shares the context and the text measurer `exact_set_measure` set.
#[test]
fn the_exported_symbol_setter_measures_a_symbol_at_boot() {
    use crate::host::{frame_of, handles, out, put};
    let rt = handles::exact_create();
    handles::exact_set_measure(rt, Some(no_text), std::ptr::null_mut());
    handles::exact_set_symbol_measure(rt, Some(wide_symbols));
    let plan = contract::compile(
        "component App\n  view\n    row\n      image \"symbol:settings\" font-size=20 testId=\"gear\"\n",
    )
    .unwrap()
    .encode();
    let n = put(rt, &plan);
    let batch = out(rt, handles::exact_boot_plan(rt, n, 402.0, 874.0));
    let at = batch.find("\"testId\":\"gear\"").expect(&batch);
    let head = &batch[..at];
    let id: u32 = head[head.rfind("\"id\":").unwrap() + 5..]
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let f = frame_of(&batch, id);
    assert_eq!((f.2, f.3), (30.0, 20.0), "{f:?}");
    handles::exact_destroy(rt);
}

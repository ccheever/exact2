//! Runtime font registration through the Apple C callback boundary.

use super::host::NoData;
use exact_apple::abi::{Bridge, Hooks};
use exact_apple::measure::{CFontCatalog, CMetrics, CRequest};
use std::cell::RefCell;
use std::ffi::c_void;

struct RecordedFace {
    family: String,
    stack: u16,
    weight: u16,
}

#[derive(Default)]
struct RuntimeFontProbe {
    faces: RefCell<Vec<RecordedFace>>,
    runs: RefCell<Vec<u16>>,
}

extern "C" fn probe_fonts(ctx: *mut c_void, catalog: *const CFontCatalog) {
    let probe = unsafe { &*ctx.cast::<RuntimeFontProbe>() };
    let catalog = unsafe { &*catalog };
    let rows = unsafe { std::slice::from_raw_parts(catalog.faces, catalog.count) };
    let mut faces = probe.faces.borrow_mut();
    faces.clear();
    faces.extend(rows.iter().map(|row| {
        let family = unsafe { std::slice::from_raw_parts(row.family, row.family_len) };
        RecordedFace {
            family: String::from_utf8(family.to_vec()).unwrap(),
            stack: row.stack,
            weight: row.weight,
        }
    }));
}

extern "C" fn probe_font_runs(ctx: *mut c_void, request: *const CRequest) -> CMetrics {
    let probe = unsafe { &*ctx.cast::<RuntimeFontProbe>() };
    let request = unsafe { &*request };
    let runs = unsafe { std::slice::from_raw_parts(request.runs, request.count) };
    probe
        .runs
        .borrow_mut()
        .extend(runs.iter().map(|run| run.font_family));
    CMetrics {
        width: runs
            .iter()
            .map(|run| run.len as f32 * run.font_size * 0.5)
            .sum(),
        height: runs.first().map_or(0.0, |run| run.font_size * 1.2),
        baseline: runs.first().map_or(-1.0, |run| run.font_size),
    }
}

#[test]
fn a_family_interned_by_boot_resize_is_registered_before_its_measure() {
    let probe = RuntimeFontProbe::default();
    let ctx = std::ptr::from_ref(&probe).cast_mut().cast();
    let plan = contract::compile(
        "component App\n  state family = \"Arial\"\n  action resized(w: number, h: number, r: DOMRectReadOnly)\n    family = \"Menlo\"\n  view\n    column width=200 height=80 resize=resized\n      text \"The quick brown fox\" font-family=family testId=\"sample\"\n",
    ).unwrap();
    let menlo_id = plan.stacks.len() as u16 + 1;
    let mut bridge: Bridge<NoData> = Bridge::new();
    bridge.set_fonts(Some(probe_fonts), ctx);
    let len = bridge.boot(
        &plan.encode(),
        NoData,
        Hooks {
            measure: Some(probe_font_runs),
            ctx,
            ..Hooks::none()
        },
        390.0,
        844.0,
    );
    let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(batch.contains("\"error\":null"), "{batch}");
    assert!(probe
        .faces
        .borrow()
        .iter()
        .any(|f| f.stack == menlo_id && f.family == "Menlo"));
    assert!(probe.runs.borrow().contains(&menlo_id));
}

#[test]
fn a_quoted_generic_is_sent_to_apple_as_an_installed_name() {
    let probe = RuntimeFontProbe::default();
    let ctx = std::ptr::from_ref(&probe).cast_mut().cast();
    let plan = contract::compile("component App\n  state family = \"'monospace', Arial\"\n  view\n    text \"hello\" font-family=family\n").unwrap();
    let mut bridge: Bridge<NoData> = Bridge::new();
    bridge.set_fonts(Some(probe_fonts), ctx);
    let len = bridge.boot(&plan.encode(), NoData, Hooks::none(), 390.0, 844.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("\"error\":null"));
    let rows = probe.faces.borrow();
    assert!(rows
        .iter()
        .any(|f| f.family == "monospace" && f.weight == 0));
}

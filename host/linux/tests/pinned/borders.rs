//! LLP 1053 G2: per-side border colours, held to Chrome's pixels. The page
//! is `scripts/fixtures/borders.contract`; Chrome's pictures of it (the web
//! host at 1×, `bun scripts/agent.mjs web --plan … screenshot`) are
//! `borders.web.png` as booted and `borders.web-dark.png` after `flip` and
//! `dark`. Each case's box is compared: rasterizers part only on
//! antialiased edges, so the band is on the box, not a pixel.

use crate::pin_font;
use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;
use exact_runner::{DataError, DataSource, Value};
use std::path::PathBuf;
use tiny_skia::Pixmap;

#[derive(Default)]
pub(crate) struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/fixtures"
    ))
}

/// The twelve cases, as the page lays them out.
const CASES: [&str; 12] = [
    "square4",
    "round4",
    "unequal",
    "unequal-round",
    "radii",
    "ring",
    "transparent",
    "translucent",
    "quote",
    "current",
    "scheme",
    "hairline",
];

/// A parity page from `scripts/fixtures`, booted at 390×460 and 1×.
pub(crate) fn boot(choice: PainterChoice, page: &str) -> Presenter<NoData> {
    pin_font();
    let src = std::fs::read_to_string(fixtures().join(page)).unwrap();
    let plan = contract::compile(&src).unwrap();
    Presenter::boot_with(
        &plan.encode(),
        NoData,
        (390.0, 460.0),
        1.0,
        fixtures(),
        choice,
    )
    .unwrap()
    .0
}

pub(crate) fn view(p: &Presenter<NoData>, test_id: &str) -> u32 {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// Over one box: the mean absolute difference per channel (0–255) and the
/// share of pixels where a channel differs by more than 48.
fn compare(ours: &Pixmap, chrome: &Pixmap, (x, y, w, h): (f32, f32, f32, f32)) -> (f64, f64) {
    let (mut sum, mut over, mut n) = (0u64, 0u64, 0u64);
    for py in y as u32..(y + h) as u32 {
        for px in x as u32..(x + w) as u32 {
            let a = ours.pixel(px, py).unwrap().demultiply();
            let b = chrome.pixel(px, py).unwrap().demultiply();
            let d = [
                (a.red() as i32 - b.red() as i32).unsigned_abs(),
                (a.green() as i32 - b.green() as i32).unsigned_abs(),
                (a.blue() as i32 - b.blue() as i32).unsigned_abs(),
            ];
            sum += d.iter().map(|v| *v as u64).sum::<u64>();
            over += u64::from(d.iter().any(|v| *v > 48));
            n += 1;
        }
    }
    (
        sum as f64 / (3.0 * n as f64),
        100.0 * over as f64 / n as f64,
    )
}

/// Each case's box against Chrome's picture; the failures, named.
pub(crate) fn held_to_chrome(
    p: &mut Presenter<NoData>,
    reference: &str,
    painter: &str,
    cases: &[&str],
) -> Vec<String> {
    let chrome = Pixmap::load_png(fixtures().join(reference)).unwrap();
    let frame = p.frame();
    assert_eq!(
        (frame.width(), frame.height()),
        (chrome.width(), chrome.height())
    );
    let mut failures = Vec::new();
    for &case in cases {
        let id = view(p, case);
        let rect = p.boxes().iter().find(|b| b.id == id).unwrap().rect;
        let (mean, over) = compare(&frame, &chrome, rect);
        eprintln!("{painter} {reference} {case}: mean {mean:.2}/255, {over:.2}% beyond 48");
        // Measured 2026-09-25, both painters: the worst case is a mean of
        // 0.88/255 and 0.99% of pixels beyond the band, on curved edges and
        // joins. The band is that with room.
        if mean > 2.0 || over > 2.0 {
            failures.push(format!(
                "{painter} {case} vs {reference}: mean {mean:.2}/255, {over:.2}% beyond 48"
            ));
        }
    }
    failures
}

#[test]
fn every_border_case_matches_chrome_light_then_flipped_and_dark() {
    let mut choices = vec![PainterChoice::Cpu];
    if exact_linux::gpu::Gpu::new().is_ok() {
        choices.push(PainterChoice::Gpu);
    }
    let mut failures = Vec::new();
    for choice in choices {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "borders.contract");
        failures.extend(held_to_chrome(&mut p, "borders.web.png", &name, &CASES));
        // `currentcolor` follows the new `color`; `light-dark()` the scheme.
        for target in ["flip", "dark"] {
            let id = view(&p, target);
            p.tap(id).unwrap();
        }
        p.run_commands(NoData::default);
        failures.extend(held_to_chrome(
            &mut p,
            "borders.web-dark.png",
            &name,
            &CASES,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

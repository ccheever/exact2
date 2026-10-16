//! LLP 1053 G2: per-side border colours, held to Chrome's pixels. The page
//! is `scripts/fixtures/borders.contract`; Chrome's pictures of it (the web
//! host at 1×, `bun scripts/agent.mjs web --plan … screenshot`) are
//! `borders.web.png` as booted and `borders.web-dark.png` after `flip` and
//! `dark`. Each case's box is compared: rasterizers part only on
//! antialiased edges, so the band is on the box, not a pixel.

use crate::pin_font;
use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;
use std::path::PathBuf;
use tiny_skia::Pixmap;

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
pub(crate) fn boot(choice: PainterChoice, page: &str) -> Presenter<()> {
    pin_font();
    let src = std::fs::read_to_string(fixtures().join(page)).unwrap();
    let plan = contract::compile(&src).unwrap();
    Presenter::boot_with(&plan.encode(), (), (390.0, 460.0), 1.0, fixtures(), choice)
        .unwrap()
        .0
}

pub(crate) fn view(p: &Presenter<()>, test_id: &str) -> u32 {
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

fn underline_has_ink_and_gaps(frame: &Pixmap, (x, y, _, _): (f32, f32, f32, f32)) -> bool {
    let row = y as u32 + 69;
    let greens =
        (x as u32 + 10..x as u32 + 90).map(|px| frame.pixel(px, row).unwrap().demultiply().green());
    let (ink, gaps) = greens.fold((0, 0), |(ink, gaps), green| {
        (
            ink + usize::from(green < 160),
            gaps + usize::from(green > 245),
        )
    });
    ink >= 20 && gaps >= 20
}

fn thin_box_edges_have_ink_and_gaps(frame: &Pixmap, rect: (f32, f32, f32, f32)) -> bool {
    let (x, y, w, h) = rect;
    let edges = [
        (10..90)
            .map(|i| (x as u32 + i, y as u32))
            .collect::<Vec<_>>(),
        (10..60)
            .map(|i| (x as u32 + w as u32 - 1, y as u32 + i))
            .collect(),
        (10..90)
            .map(|i| (x as u32 + i, y as u32 + h as u32 - 1))
            .collect(),
        (10..60).map(|i| (x as u32, y as u32 + i)).collect(),
    ];
    edges.iter().all(|edge| {
        let samples = edge
            .iter()
            .map(|&(px, py)| frame.pixel(px, py).unwrap().demultiply());
        let (ink, gaps) = samples.fold((0, 0), |(ink, gaps), color| {
            let low = color.red().min(color.green()).min(color.blue());
            (ink + usize::from(low < 160), gaps + usize::from(low > 245))
        });
        ink >= 10 && gaps >= 10
    })
}

fn thin_dot_phase_matches_chrome(
    frame: &Pixmap,
    chrome: &Pixmap,
    rect: (f32, f32, f32, f32),
) -> bool {
    let (x, y, w, h) = rect;
    let edges = [
        (10..90)
            .map(|i| (x as u32 + i, y as u32))
            .collect::<Vec<_>>(),
        (10..60)
            .map(|i| (x as u32 + w as u32 - 1, y as u32 + i))
            .collect(),
        (10..90)
            .map(|i| (x as u32 + i, y as u32 + h as u32 - 1))
            .collect(),
        (10..60).map(|i| (x as u32, y as u32 + i)).collect(),
    ];
    edges.iter().all(|edge| {
        let mismatches = edge
            .iter()
            .filter(|&&(px, py)| {
                (frame.pixel(px, py).unwrap().demultiply().green() < 160)
                    != (chrome.pixel(px, py).unwrap().demultiply().green() < 160)
            })
            .count();
        mismatches <= 4
    })
}

/// Each case's box against Chrome's picture; the failures, named.
pub(crate) fn held_to_chrome(
    p: &mut Presenter<()>,
    reference: &str,
    painter: &str,
    cases: &[&str],
) -> Vec<String> {
    held_to_chrome_with_band(p, reference, painter, cases, 2.0)
}

fn held_to_chrome_with_band(
    p: &mut Presenter<()>,
    reference: &str,
    painter: &str,
    cases: &[&str],
    band: f64,
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
        if mean > band || over > band {
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
        p.run_commands(<()>::default);
        failures.extend(held_to_chrome(
            &mut p,
            "borders.web-dark.png",
            &name,
            &CASES,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn dotted_and_dashed_borders_match_chrome() {
    const PATTERNS: [&str; 12] = [
        "dot1",
        "dot2",
        "dot4",
        "dash1",
        "dash2",
        "dash4",
        "round-dot",
        "round-dash",
        "mixed",
        "unequal-dot",
        "underline-dot",
        "underline-dash",
    ];
    let mut choices = vec![PainterChoice::Cpu];
    if exact_linux::gpu::Gpu::new().is_ok() {
        choices.push(PainterChoice::Gpu);
    }
    let mut failures = Vec::new();
    for choice in choices {
        let mut p = boot(choice, "border-patterns.contract");
        // At 1×, tiny-skia/Vello and Skia differ on 1–2 px dots and curved joins.
        // The measured worst case stays below 5/255 mean and 5% beyond 48.
        failures.extend(held_to_chrome_with_band(
            &mut p,
            "border-patterns.web.png",
            &format!("{choice:?}"),
            &PATTERNS,
            5.0,
        ));
        let frame = p.frame();
        let chrome = Pixmap::load_png(fixtures().join("border-patterns.web.png")).unwrap();
        for case in ["dot1", "dash1"] {
            let id = view(&p, case);
            let rect = p.boxes().iter().find(|b| b.id == id).unwrap().rect;
            if !thin_box_edges_have_ink_and_gaps(&frame, rect) {
                failures.push(format!("{choice:?} {case} lacks ink or gaps on an edge"));
            }
            if case == "dot1" && !thin_dot_phase_matches_chrome(&frame, &chrome, rect) {
                failures.push(format!("{choice:?} {case} has the wrong dot phase"));
            }
        }
        let id = view(&p, "underline-dot");
        let rect = p.boxes().iter().find(|b| b.id == id).unwrap().rect;
        if !underline_has_ink_and_gaps(&frame, rect) {
            failures.push(format!(
                "{choice:?} thin dotted underline lacks ink or gaps"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn thin_dotted_underline_band_rejects_missing_ink() {
    let chrome = Pixmap::load_png(fixtures().join("border-patterns.web.png")).unwrap();
    let mut blank = Pixmap::new(chrome.width(), chrome.height()).unwrap();
    blank.fill(tiny_skia::Color::WHITE);
    let rect = (145., 305., 100., 70.);
    assert!(underline_has_ink_and_gaps(&chrome, rect));
    assert!(!underline_has_ink_and_gaps(&blank, rect), "blank passed");
    let row = 374 * blank.width() as usize * 4;
    for x in 155..235 {
        blank.data_mut()[row + x * 4..row + x * 4 + 4].copy_from_slice(&[229, 57, 53, 255]);
    }
    assert!(
        !underline_has_ink_and_gaps(&blank, rect),
        "solid line passed"
    );
}

#[test]
fn thin_box_guards_reject_blank_and_solid_edges() {
    let chrome = Pixmap::load_png(fixtures().join("border-patterns.web.png")).unwrap();
    for rect in [(20., 20., 100., 70.), (20., 115., 100., 70.)] {
        assert!(thin_box_edges_have_ink_and_gaps(&chrome, rect));
        let mut blank = Pixmap::new(chrome.width(), chrome.height()).unwrap();
        blank.fill(tiny_skia::Color::WHITE);
        assert!(!thin_box_edges_have_ink_and_gaps(&blank, rect));
        let (x, y, w, h) = rect;
        for py in y as u32..(y + h) as u32 {
            for px in x as u32..(x + w) as u32 {
                if px == x as u32
                    || px == (x + w) as u32 - 1
                    || py == y as u32
                    || py == (y + h) as u32 - 1
                {
                    let i = (py * blank.width() + px) as usize * 4;
                    blank.data_mut()[i..i + 4].copy_from_slice(&[229, 57, 53, 255]);
                }
            }
        }
        assert!(!thin_box_edges_have_ink_and_gaps(&blank, rect));
    }
}

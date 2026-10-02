//! LLP 1076: `corner-shape` and `mask-image` held to Chrome's pixels, and
//! `text-shadow` painted where CSS puts it. The pages are
//! `scripts/fixtures/visual.contract` and `text-shadow.contract`; Chrome's
//! pictures of the first (the web host at 1×) are `visual.web.png` as booted
//! and `visual.web-dark.png` after `dark`. Text is the pinned font here and
//! the system's in Chrome, so the shadow page is checked by where the
//! shadow's colour lands, not against Chrome's glyphs.

use crate::borders::{boot, held_to_chrome, view, NoData};
use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;

/// Every case Chrome draws as CSS defines it (`apple` and `apple-border`
/// are a declared approximation on the web).
const CASES: [&str; 10] = [
    "squircle",
    "bevel",
    "scoop",
    "bordered",
    "sides",
    "clips",
    "fade",
    "spot",
    "children",
    "masked-round",
];

fn painters() -> Vec<PainterChoice> {
    let mut choices = vec![PainterChoice::Cpu];
    if exact_linux::gpu::Gpu::new().is_ok() {
        choices.push(PainterChoice::Gpu);
    }
    choices
}

#[test]
fn every_corner_and_mask_case_matches_chrome_light_then_dark() {
    let mut failures = Vec::new();
    for choice in painters() {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "visual.contract");
        failures.extend(held_to_chrome(&mut p, "visual.web.png", &name, &CASES));
        let id = view(&p, "dark");
        p.tap(id).unwrap();
        p.run_commands(NoData::default);
        failures.extend(held_to_chrome(&mut p, "visual.web-dark.png", &name, &CASES));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Pixels in a case's box (and `pad` points around it) whose colour is
/// within 40 of `rgb` in every channel.
fn count(p: &mut Presenter<NoData>, case: &str, pad: f32, rgb: [u8; 3]) -> usize {
    count_where(p, case, pad, |c| {
        c.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= 40)
    })
}

/// Pixels in a case's box (and `pad` points around it) that `like` takes.
fn count_where(
    p: &mut Presenter<NoData>,
    case: &str,
    pad: f32,
    like: impl Fn([u8; 3]) -> bool,
) -> usize {
    let id = view(p, case);
    let (x, y, w, h) = p.boxes().iter().find(|b| b.id == id).unwrap().rect;
    let frame = p.frame();
    let mut n = 0;
    for py in ((y - pad).max(0.0) as u32)..((y + h + pad) as u32).min(frame.height()) {
        for px in ((x - pad).max(0.0) as u32)..((x + w + pad) as u32).min(frame.width()) {
            let c = frame.pixel(px, py).unwrap().demultiply();
            if like([c.red(), c.green(), c.blue()]) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn a_text_shadow_paints_its_colour_under_the_glyphs_and_none_paints_none() {
    for choice in painters() {
        let mut p = boot(choice, "text-shadow.contract");
        let red = [0xe5, 0x39, 0x35];
        assert!(
            count(&mut p, "offset", 4.0, red) > 40,
            "{choice:?}: offset shadow"
        );
        // A blurred red shadow under dark text: red over the white page.
        let reddish =
            |[r, g, b]: [u8; 3]| r > 200 && r > g.saturating_add(40) && r > b.saturating_add(40);
        assert!(
            count_where(&mut p, "lines", 8.0, reddish) > 100,
            "{choice:?}: both lines' shadow"
        );
        // currentcolor: the text's own green, so more green than the glyphs.
        let green = [0x43, 0xa0, 0x47];
        let shadowed = count(&mut p, "current", 6.0, green);
        let plain = count(&mut p, "none", 6.0, green);
        assert!(
            shadowed > plain,
            "{choice:?}: currentcolor shadow {shadowed} vs {plain}"
        );
        // A blurred black shadow: grey around black glyphs.
        assert!(
            count(&mut p, "blurred", 8.0, [0x99, 0x99, 0x99]) > 30,
            "{choice:?}: blur"
        );
    }
}

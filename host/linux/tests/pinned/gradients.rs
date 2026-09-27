//! LLP 1066: `background-image` gradients held to Chrome's pixels. The page
//! is `scripts/fixtures/gradients.contract`; Chrome's pictures of it (the web
//! host at 1×) are `gradients.web.png` as booted and `gradients.web-dark.png`
//! after `dark`. Each case's box is compared within the border parity's band.

use crate::borders::{boot, held_to_chrome, view, NoData};
use exact_linux::presenter::PainterChoice;

const CASES: [&str; 12] = [
    "linear", "angle", "corner", "fade", "over", "clip", "radial", "circle", "ellipse", "rounded",
    "bordered", "scheme",
];

#[test]
fn every_gradient_case_matches_chrome_light_then_dark() {
    let mut choices = vec![PainterChoice::Cpu];
    if exact_linux::gpu::Gpu::new().is_ok() {
        choices.push(PainterChoice::Gpu);
    }
    let mut failures = Vec::new();
    for choice in choices {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "gradients.contract");
        failures.extend(held_to_chrome(&mut p, "gradients.web.png", &name, &CASES));
        // `light-dark()` stops follow the scheme, and so does the page
        // under the transparent ones.
        let id = view(&p, "dark");
        p.tap(id).unwrap();
        p.run_commands(NoData::default);
        failures.extend(held_to_chrome(
            &mut p,
            "gradients.web-dark.png",
            &name,
            &CASES,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

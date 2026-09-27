//! LLP 1053.000: CSS `backdrop-filter: blur()` held to Chrome's pixels. The
//! page is `scripts/fixtures/backdrop.contract`; Chrome's picture of it (the
//! web host at 1×) is `backdrop.web.png`. Each glass box is compared within
//! the border parity's band, on both painters.

use crate::borders::{boot, held_to_chrome};
use exact_linux::presenter::PainterChoice;

const CASES: [&str; 5] = ["blur4", "blur20", "glass", "none", "edge"];

#[test]
fn every_backdrop_case_matches_chrome() {
    let mut choices = vec![PainterChoice::Cpu];
    if exact_linux::gpu::Gpu::new().is_ok() {
        choices.push(PainterChoice::Gpu);
    }
    let mut failures = Vec::new();
    for choice in choices {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "backdrop.contract");
        failures.extend(held_to_chrome(&mut p, "backdrop.web.png", &name, &CASES));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

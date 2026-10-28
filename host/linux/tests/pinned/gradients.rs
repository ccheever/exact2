//! LLP 1066: `background-image` gradients held to Chrome's pixels. The page
//! is `scripts/fixtures/gradients.contract`; Chrome's pictures of it (the CSS
//! declarations at 1×) are `gradients.web.png` as booted and `gradients.web-dark.png`
//! after `dark`. Each case's box is compared within the border parity's band.

use crate::borders::{boot, held_to_chrome, view};

const CASES: [&str; 18] = [
    "linear",
    "angle",
    "corner",
    "fade",
    "over",
    "clip",
    "radial",
    "circle",
    "ellipse",
    "rounded",
    "bordered",
    "scheme",
    "hatch",
    "length",
    "mixed",
    "zero-two",
    "zero-three",
    "zero-four",
];

#[test]
fn every_gradient_case_matches_chrome_light_then_dark() {
    let choices = crate::paint::painters();
    let mut failures = Vec::new();
    for choice in choices {
        let name = format!("{choice:?}");
        let cases = CASES;
        let mut p = boot(choice, "gradients.contract");
        let _ = p.resize(390.0, 650.0);
        failures.extend(held_to_chrome(&mut p, "gradients.web.png", &name, &cases));
        // `light-dark()` stops follow the scheme, and so does the page
        // under the transparent ones.
        let id = view(&p, "dark");
        p.tap(id).unwrap();
        p.run_commands(<()>::default);
        failures.extend(held_to_chrome(
            &mut p,
            "gradients.web-dark.png",
            &name,
            &cases,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

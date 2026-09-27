//! LLP 1053.000 D4: named `backgroundMaterial`s as the web approximates
//! them (the schema's blur and tint), held to Chrome's pixels. The page is
//! `scripts/fixtures/materials.contract`, a grey backdrop so the web's
//! `saturate()` (which Linux does not draw) changes nothing; Chrome's
//! picture is `materials.web.png`. An authored background paints over the
//! tint, and a material wins over `backdrop-filter`.

use crate::borders::{boot, held_to_chrome};
use exact_linux::presenter::PainterChoice;

const CASES: [&str; 8] = [
    "ultra-thin",
    "thin",
    "chrome-dark",
    "sidebar",
    "hud-window",
    "prominent",
    "tinted",
    "over-blur",
];

#[test]
fn named_materials_match_chrome() {
    let mut choices = vec![PainterChoice::Cpu];
    if exact_linux::gpu::Gpu::new().is_ok() {
        choices.push(PainterChoice::Gpu);
    }
    let mut failures = Vec::new();
    for choice in choices {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "materials.contract");
        failures.extend(held_to_chrome(&mut p, "materials.web.png", &name, &CASES));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

//! Raster template alpha, fitting and box paint held to Chrome at 1×.
use crate::borders::{boot, held_to_chrome, view, NoData};
use exact_linux::presenter::PainterChoice;
use std::time::Duration;

#[test]
fn raster_tint_matches_chrome_in_both_painters() {
    let cases = [
        "fill", "contain", "cover", "none", "down", "small", "natural", "auto", "rounded",
    ];
    let mut choices = vec![PainterChoice::Cpu];
    if exact_linux::gpu::Gpu::new().is_ok() {
        choices.push(PainterChoice::Gpu);
    }
    let mut failures = Vec::new();
    for choice in choices {
        let mut p = boot(choice, "tint.contract");
        p.wait_images(Duration::from_secs(3));
        for (dark, reference) in [(false, "tint.web.png"), (true, "tint.web-dark.png")] {
            if dark {
                let id = view(&p, "dark");
                p.tap(id).unwrap();
                p.run_commands(NoData::default);
            }
            failures.extend(held_to_chrome(
                &mut p,
                reference,
                &format!("{choice:?}"),
                &cases,
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

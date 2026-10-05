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

/// LLP 1095 D8: an untinted symbol is `AccentColor`, here its fallback pair
/// (`#007aff`, no report on Linux), not the text colour; a tint row wins.
#[test]
fn an_untinted_symbol_paints_the_accent_fallback() {
    crate::pin_font();
    let plan = contract::compile(
        r##"component App
  view
    column width=100 height=100 background-color="#ffffff" color="#000000"
      image "symbol:play-fill" testId="plain" width=48 height=48 font-size=48
      image "symbol:play-fill" testId="red" width=48 height=48 font-size=48 tint-color="#ff0000"
"##,
    )
    .unwrap();
    let assets = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/caltrain/assets"
    ));
    let (mut p, error) =
        exact_linux::Presenter::boot(&plan.encode(), NoData, (100.0, 100.0), 1.0, assets).unwrap();
    assert!(error.is_none(), "{error:?}");
    let frame = p.frame();
    // The most strongly coloured pixel in each 48-pixel row band.
    let ink = |top: u32| {
        (top..top + 48)
            .flat_map(|y| (0..48).map(move |x| (x, y)))
            .filter_map(|(x, y)| frame.pixel(x, y))
            .map(|c| {
                let c = c.demultiply();
                [c.red(), c.green(), c.blue()]
            })
            .min_by_key(|c| u16::from(c[0]) + u16::from(c[1]) + u16::from(c[2]))
            .unwrap()
    };
    assert_eq!(ink(0), [0x00, 0x7a, 0xff]);
    assert_eq!(ink(48), [0xff, 0x00, 0x00]);
}

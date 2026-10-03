//! A colour a platform can't show is refused when the app is built for it
//! (LLP 1100 D1): the platform manages colour, and Exact makes up for none.
//! Linux draws sRGB only; no browser ships CSS Color HDR's `rec2100-*`;
//! Apple shows every space CSS names.

use exact_color::{Parsed, Wide};
use std::path::Path;

/// Why `wide` is not available on `platform`, or `None` when it is.
pub fn unavailable(wide: &Wide, platform: &str) -> Option<&'static str> {
    match platform {
        "linux" if !wide.in_srgb() => Some("is outside sRGB, and Linux draws sRGB only"),
        "web" if wide.space.is_hdr() => {
            Some("is in a CSS Color HDR space, which no browser draws yet")
        }
        _ => None,
    }
}

/// Every modern colour written in `text` — alone, or inside a gradient, a
/// shadow or a `light-dark()` — as written and parsed.
pub fn modern_colors(text: &str) -> Vec<(&str, Wide)> {
    let mut out = Vec::new();
    let lower = text.to_ascii_lowercase();
    for name in ["color(", "lab(", "lch(", "oklab(", "oklch("] {
        let mut from = 0;
        while let Some(at) = lower[from..].find(name).map(|i| i + from) {
            from = at + name.len();
            // `lab(` inside `oklab(` is that function's, not another.
            if at > 0 && lower.as_bytes()[at - 1].is_ascii_alphanumeric() {
                continue;
            }
            let Some(close) = text[at..].find(')').map(|i| at + i + 1) else {
                break;
            };
            if let Some(Parsed::Wide(w)) = exact_color::parse(&text[at..close]) {
                out.push((&text[at..close], w));
            }
        }
    }
    out
}

/// Every `color(--name …)` in `text`: as written, the name and the
/// number of components.
fn profiled_colors(text: &str) -> Vec<(&str, String, usize)> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find("color(--").map(|i| i + from) {
        from = at + 8;
        let Some(close) = text[at..].find(')').map(|i| at + i + 1) else {
            break;
        };
        if let Some((name, c, _)) = exact_color::parse_profiled(&text[at..close]) {
            out.push((&text[at..close], name, c.len()));
        }
    }
    out
}

/// An ICC profile's channel count from its header's data colour space
/// (ICC.1 §7.2.6), the only part of it Exact reads.
fn icc_channels(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 128 || bytes.len() > 256 * 1024 || &bytes[36..40] != b"acsp" {
        return None;
    }
    match &bytes[16..20] {
        b"RGB " | b"Lab " | b"XYZ " => Some(3),
        b"CMYK" => Some(4),
        b"GRAY" => Some(1),
        _ => None,
    }
}

/// The string a binding's expression is, when it is one literal: a `Str`
/// (opcode 2) and its end.
fn literal<'p>(plan: &'p exact_plan::Plan, b: &exact_plan::BindingsRow) -> Option<&'p str> {
    let code = plan
        .code
        .get(b.expr.offset as usize..(b.expr.offset + b.expr.len) as usize)?;
    (code.len() == 6 && code[0] == 2)
        .then(|| u32::from_le_bytes([code[1], code[2], code[3], code[4]]) as usize)
        .and_then(|i| plan.strings.get(i))
        .map(|t| &**t)
}

/// The app's colour literals checked for `platform`: the first one it
/// can't show, as an error naming it, the platform and the way out.
pub fn check(app_dir: &Path, platform: &str) -> Result<(), String> {
    let source = app_dir.join("app.contract");
    if !source.exists() {
        return Ok(());
    }
    let plan = contract::compile_path(&source).map_err(|e| format!("app.contract: {e}"))?;
    // A row computed by an expression is the runtime's to refuse.
    let literals: Vec<&str> = plan
        .bindings
        .iter()
        .filter(|b| b.kind == exact_plan::BindingKind::Style)
        .filter_map(|b| literal(&plan, b))
        .collect();
    let mut channels = std::collections::BTreeMap::new();
    for row in &plan.profiles {
        let (name, src) = (plan.str(row.name), plan.str(row.src));
        let bytes = std::fs::read(app_dir.join(src))
            .map_err(|e| format!("app.contract: `color-profile {name}`: {src}: {e}"))?;
        channels.insert(name.to_string(), icc_channels(&bytes).ok_or_else(|| {
            format!("app.contract: `color-profile {name}`: {src} is not an ICC profile of at most 256 KiB")
        })?);
    }
    for text in &literals {
        for (written, name, count) in profiled_colors(text) {
            if platform == "linux" || platform == "web" {
                return Err(format!(
                    "app.contract: `{written}` is in a color profile's space, which only the platform converts and {platform} does not \
                     (LLP 1100 D3): write a CSS color, or choose this one where the platform has it"
                ));
            }
            let want = exact_color::PROFILES
                .iter()
                .find(|p| p.0 == name)
                .map(|p| p.2)
                .or_else(|| channels.get(&name).copied());
            match want {
                Some(n) if n == count => {}
                Some(n) => return Err(format!("app.contract: `{written}`: `{name}` takes {n} components, not {count}")),
                None => return Err(format!("app.contract: `{written}`: no `color-profile {name}` is declared, and it is no standard space")),
            }
        }
    }
    for text in &literals {
        for (written, wide) in modern_colors(text) {
            if let Some(why) = unavailable(&wide, platform) {
                return Err(format!(
                    "app.contract: `{written}` {why} (LLP 1100 D1): write a color {platform} shows, or \
                     choose this one where `exactViewport().colorGamut` (or `dynamicRange`) says the display has it"
                ));
            }
        }
    }
    // @ref LLP 1100 D12a, D10 — Linux's canvases are 8-bit sRGB; a setting
    // computed at run time is sRGB there (`Runner::canvas_settings`).
    if platform == "linux" {
        for b in plan
            .bindings
            .iter()
            .filter(|b| b.kind == exact_plan::BindingKind::Prop)
        {
            let (attr, refused) = match b.id {
                id if id == exact_kernel::PropId::ColorSpace as u16 => {
                    ("color-space", "display-p3")
                }
                id if id == exact_kernel::PropId::ColorType as u16 => ("color-type", "float16"),
                _ => continue,
            };
            if literal(&plan, b) == Some(refused) {
                return Err(format!(
                    "app.contract: a canvas's `{attr}=\"{refused}\"`: Linux's canvases are 8-bit sRGB (LLP 1100 D12a)"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(rows: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "exact-colors-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = format!(
            "shape D\n  colorGamut: string\nstyle Brand\n  color=\"color(display-p3 0 0.5 1)\"\ncomponent A\n  resource d = exactViewport() as shape D\n  view\n    column\n      {rows}\n"
        );
        std::fs::write(dir.join("app.contract"), src).unwrap();
        dir
    }

    #[test]
    fn a_platform_refuses_the_literal_colors_it_cannot_show() {
        let p3 = app("box width=4 height=4 background-color=\"color(display-p3 1 0 0)\"");
        let e = check(&p3, "linux").unwrap_err();
        assert!(
            e.contains("`color(display-p3 1 0 0)` is outside sRGB, and Linux draws sRGB only"),
            "{e}"
        );
        assert!(e.contains("exactViewport().colorGamut"), "{e}");
        for shows in ["ios", "macos", "web"] {
            assert_eq!(check(&p3, shows), Ok(()), "{shows}");
        }
        // Inside a gradient and a shadow, too.
        let gradient = app(
            "box width=4 height=4 background-image=\"linear-gradient(red, oklch(0.7 0.35 145))\"",
        );
        assert!(check(&gradient, "linux").is_err());
        let shadow = app("box width=4 height=4 box-shadow=\"0 1px 2px color(display-p3 0 1 0)\"");
        assert!(check(&shadow, "linux").is_err());
        // A named style's row, as the node that takes it.
        let named = app("text \"hi\" class=Brand");
        assert!(check(&named, "linux").is_err(), "a style's literal row");
        // Inside sRGB, a modern colour is fine everywhere.
        let inside = app("box width=4 height=4 background-color=\"oklch(0.7 0.05 200)\"");
        assert_eq!(check(&inside, "linux"), Ok(()));
        // The web has no CSS Color HDR.
        let hdr = app("box width=4 height=4 background-color=\"color(rec2100-linear 2 2 2)\"");
        assert!(check(&hdr, "web").unwrap_err().contains("CSS Color HDR"));
        assert_eq!(check(&hdr, "macos"), Ok(()));
        // A colour chosen by the display is the runtime's.
        let chosen = app("box width=4 height=4 background-color=(d.colorGamut == \"srgb\" ? \"#ff0000\" : \"color(display-p3 1 0 0)\")");
        assert_eq!(check(&chosen, "linux"), Ok(()));
    }

    /// LLP 1100 D3: a profile's colour — a standard space or an app's
    /// `color-profile` — is Apple's to draw, checked against its profile.
    #[test]
    fn a_profiles_colour_is_checked_and_only_apple_draws_it() {
        let dir = app("box width=4 height=4 background-color=\"color(--dci-p3 1 0 0)\"");
        assert_eq!(check(&dir, "ios"), Ok(()));
        assert!(check(&dir, "linux")
            .unwrap_err()
            .contains("color profile's space"));
        assert!(check(&dir, "web").is_err());
        let wrong = app("box width=4 height=4 background-color=\"color(--dci-p3 1 0)\"");
        assert!(check(&wrong, "macos").is_err(), "--dci-p3 takes three");
        // An app's profile: an ICC header saying CMYK (four components).
        let declared =
            app("box width=4 height=4 background-color=\"color(--brand 0.1 0.8 0.2 0.05)\"");
        let src = std::fs::read_to_string(declared.join("app.contract")).unwrap();
        std::fs::write(
            declared.join("app.contract"),
            format!("color-profile --brand src=\"assets/brand.icc\"\n{src}"),
        )
        .unwrap();
        assert!(
            check(&declared, "ios")
                .unwrap_err()
                .contains("assets/brand.icc"),
            "the file must be there"
        );
        let mut icc = vec![0u8; 132];
        icc[16..20].copy_from_slice(b"CMYK");
        icc[36..40].copy_from_slice(b"acsp");
        std::fs::create_dir_all(declared.join("assets")).unwrap();
        std::fs::write(declared.join("assets/brand.icc"), &icc).unwrap();
        assert_eq!(check(&declared, "ios"), Ok(()));
        icc[16..20].copy_from_slice(b"RGB ");
        std::fs::write(declared.join("assets/brand.icc"), &icc).unwrap();
        assert!(check(&declared, "ios")
            .unwrap_err()
            .contains("takes 3 components, not 4"));
    }

    #[test]
    fn linux_refuses_a_wide_or_deep_canvas() {
        let p3 = app("canvas surface=dot() width=4 height=4 color-space=\"display-p3\"");
        assert!(check(&p3, "linux")
            .unwrap_err()
            .contains("color-space=\"display-p3\""));
        assert_eq!(check(&p3, "macos"), Ok(()));
        assert_eq!(check(&p3, "web"), Ok(()));
    }
}

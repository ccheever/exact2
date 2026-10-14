//! Box-dependent gradient stops for both Apple presenters (LLP 1066).
#![allow(unsafe_code)]
use exact_kernel::gradient::{BackgroundImage, Geometry};

/// Resolve one gradient into the existing presenter ramp plus its geometry.
///
/// # Safety
/// `css` is valid for `len` reads; `out`, when non-null, for `cap` writes.
pub unsafe fn resolve(
    css: *const u8,
    len: usize,
    size: [f32; 2],
    dark: bool,
    out: *mut u8,
    cap: usize,
) -> usize {
    if css.is_null() || !size.iter().all(|n| n.is_finite() && *n >= 0.0) {
        return 0;
    }
    // SAFETY: the caller owns this byte range.
    let bytes = unsafe { std::slice::from_raw_parts(css, len) };
    let Some(image) = std::str::from_utf8(bytes)
        .ok()
        .and_then(BackgroundImage::parse)
    else {
        return 0;
    };
    let Some(g) = image.gradient() else {
        return 0;
    };
    let (ramp, geometry) = g.placed(size[0], size[1], dark);
    let placement = match geometry {
        Geometry::Linear { start, end } => {
            format!("\"line\":[{},{},{},{}]", start.0, start.1, end.0, end.1)
        }
        Geometry::Radial { center, radii } => format!(
            "\"ellipse\":[{},{},{},{}]",
            center.0, center.1, radii.0, radii.1
        ),
        Geometry::Conic { .. } => return 0,
    };
    let mut json = gradient_json(&ramp);
    json.pop();
    json.push(',');
    json.push_str(&placement);
    json.push('}');
    if !out.is_null() && cap >= json.len() {
        // SAFETY: the caller supplies at least `cap` writable bytes.
        unsafe { std::ptr::copy_nonoverlapping(json.as_ptr(), out, json.len()) };
    }
    json.len()
}

/// Export size-dependent gradient resolution from an app's archive.
#[macro_export]
macro_rules! gradient_exports {
    () => {
        /// Resolve a CSS gradient.
        ///
        /// # Safety
        /// Input/output buffers have their stated lengths.
        #[no_mangle]
        pub unsafe extern "C" fn exact_gradient(
            css: *const u8,
            len: usize,
            width: f32,
            height: f32,
            dark: u8,
            out: *mut u8,
            cap: usize,
        ) -> usize {
            // SAFETY: forwarded from the caller's buffer contract.
            unsafe { $crate::gradient::resolve(css, len, [width, height], dark != 0, out, cap) }
        }
    };
}

use crate::style::num;
/// A gradient for the presenter (LLP 1066): its shape — `linear` degrees,
/// a `to <corner>` as `[right, bottom]`, or `radial` as `[circle, extent,
/// x%, xpx, y%, ypx]` (extent: closest-side, closest-corner, farthest-side,
/// farthest-corner) — and `stops` flat as `t, r, g, b, a`, positions 0–1.
/// The box, so the placement, is the presenter's. Stops are already expanded
/// to mix as CSS's premultiplied ones do (Core Graphics and Core Animation
/// mix unpremultiplied); a `light-dark()` gradient also carries `dark`, and
/// the view picks by its own appearance (LLP 1034 D2).
pub(crate) fn gradient_json(g: &exact_kernel::gradient::Gradient) -> String {
    use exact_kernel::gradient::{premultiplied_ramp, Direction, GradientKind, Length};
    if g.needs_box() {
        let mut css = String::new();
        exact_runner::agent::quote(&g.wire(), &mut css);
        return format!("{{\"css\":{css}}}");
    }
    // LLP 1100 D2: interpolated outside sRGB, the ramp crosses as unclipped
    // extended linear sRGB floats, alpha 0–1.
    let stops = |dark: bool| {
        if let Some(ramp) = g.interpolated(dark) {
            let parts: Vec<String> = ramp
                .into_iter()
                .map(|(at, c, a)| {
                    format!(
                        "{},{},{},{},{}",
                        num(at),
                        c[0] as f32,
                        c[1] as f32,
                        c[2] as f32,
                        a as f32
                    )
                })
                .collect();
            return format!("[{}]", parts.join(","));
        }
        let parts: Vec<String> = premultiplied_ramp(&g.resolved(dark))
            .into_iter()
            .map(|(at, c)| format!("{},{},{},{},{}", num(at), c.r(), c.g(), c.b(), c.a()))
            .collect();
        format!("[{}]", parts.join(","))
    };
    let shape = match g.kind {
        GradientKind::Linear(Direction::Angle(deg)) => format!("\"linear\":{}", num(deg)),
        GradientKind::Linear(Direction::Corner { right, bottom }) => {
            format!("\"corner\":[{},{}]", u8::from(right), u8::from(bottom))
        }
        // @ref LLP 1077 D5 — `[from, x%, xpx, y%, ypx]`.
        GradientKind::Conic { from, at } => {
            let axis = |l: Length| match l {
                Length::Percent(p) => format!("{},0", num(p)),
                Length::Px(px) => format!("0,{}", num(px)),
            };
            format!("\"conic\":[{},{},{}]", num(from), axis(at[0]), axis(at[1]))
        }
        GradientKind::Radial { circle, extent, at } => {
            let axis = |l: Length| match l {
                Length::Percent(p) => format!("{},0", num(p)),
                Length::Px(px) => format!("0,{}", num(px)),
            };
            format!(
                "\"radial\":[{},{},{},{}]",
                u8::from(circle),
                extent as u8,
                axis(at[0]),
                axis(at[1])
            )
        }
    };
    let dark = if g.is_scheme_aware() {
        format!(",\"dark\":{}", stops(true))
    } else {
        String::new()
    };
    let space = if g.interpolation(false).is_some() {
        ",\"space\":\"srgb-linear\""
    } else {
        ""
    };
    let dark_space = if g.is_scheme_aware() && g.interpolation(true).is_some() {
        ",\"darkSpace\":\"srgb-linear\""
    } else {
        ""
    };
    format!(
        "{{{shape},\"stops\":{}{dark}{space}{dark_space}}}",
        stops(false)
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn lengths_cross_the_apple_seam_after_the_box_is_known() {
        let css = b"linear-gradient(to right, red 20px, blue 80%)";
        // SAFETY: input is readable; the first call asks only for its size.
        let n = unsafe {
            super::resolve(
                css.as_ptr(),
                css.len(),
                [200.0, 50.0],
                false,
                std::ptr::null_mut(),
                0,
            )
        };
        assert!(n > 0);
        let mut out = vec![0; n];
        // SAFETY: the output has the exact capacity just requested.
        assert_eq!(
            unsafe {
                super::resolve(
                    css.as_ptr(),
                    css.len(),
                    [200.0, 50.0],
                    false,
                    out.as_mut_ptr(),
                    n,
                )
            },
            n
        );
        let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!((value["stops"][5].as_f64().unwrap() - 0.1).abs() < 0.0001);
        assert_eq!(value["line"][2], 200.0);
        assert!(value.get("css").is_none());
    }
}

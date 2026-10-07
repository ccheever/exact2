//! `corner-shape`'s outline for the Swift presenters (LLP 1077 D1): the
//! kernel's geometry, so every host draws one shape for one name.
// The C seam reads the caller's arrays and writes its buffer.
#![allow(unsafe_code)]

use exact_kernel::corner::{outline, Corner, CornerShape};
use exact_kernel::svg::Seg;

/// The outline of `rect` with `radii` (top-left first, horizontal then
/// vertical, already reduced as CSS reduces them) and `shape` (each corner's
/// K; NaN is `-exact-continuous`) as points of one closed polygon, `x, y`
/// pairs into `out`. Returns the number of points; a `cap` too small writes
/// nothing and returns the number needed.
///
/// # Safety
/// `shape` is valid for 4 reads, `radii` for 8, `out` for `cap` × 2 writes.
pub unsafe fn outline_points(
    shape: *const f32,
    rect: [f32; 4],
    radii: *const f32,
    out: *mut f32,
    cap: usize,
) -> usize {
    if shape.is_null() || radii.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees both arrays' lengths.
    let (k, r) = unsafe {
        (
            std::slice::from_raw_parts(shape, 4),
            std::slice::from_raw_parts(radii, 8),
        )
    };
    let corner = |k: f32| {
        if k.is_nan() {
            Corner::AppleContinuous
        } else {
            Corner::Superellipse(k)
        }
    };
    let shape = CornerShape([corner(k[0]), corner(k[1]), corner(k[2]), corner(k[3])]);
    let path = outline(
        (rect[0], rect[1], rect[2], rect[3]),
        [(r[0], r[1]), (r[2], r[3]), (r[4], r[5]), (r[6], r[7])],
        &shape,
    );
    let points: Vec<(f32, f32)> = path
        .0
        .iter()
        .filter_map(|s| match *s {
            Seg::Move(x, y) | Seg::Line(x, y) => Some((x, y)),
            _ => None,
        })
        .collect();
    if out.is_null() || cap < points.len() {
        return points.len();
    }
    // SAFETY: the caller guarantees `out` holds `cap` pairs.
    let out = unsafe { std::slice::from_raw_parts_mut(out, cap * 2) };
    for (i, (x, y)) in points.iter().enumerate() {
        out[2 * i] = *x;
        out[2 * i + 1] = *y;
    }
    points.len()
}

/// Export the corner outline from the application's static archive.
#[macro_export]
macro_rules! corner_exports {
    () => {
        /// A box outline with shaped corners (LLP 1077 D1).
        ///
        /// # Safety
        /// As [`$crate::corner::outline_points`].
        #[no_mangle]
        #[allow(clippy::too_many_arguments)]
        pub unsafe extern "C" fn exact_corner_outline(
            shape: *const f32,
            x: f32,
            y: f32,
            width: f32,
            height: f32,
            radii: *const f32,
            out: *mut f32,
            cap: usize,
        ) -> usize {
            // SAFETY: forwarded from this function's own contract.
            unsafe { $crate::corner::outline_points(shape, [x, y, width, height], radii, out, cap) }
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_squircle_outline_is_counted_then_written() {
        let shape = [2.0f32; 4];
        let radii = [10.0f32; 8];
        // SAFETY: the arrays are the lengths the seam reads.
        let n = unsafe {
            super::outline_points(
                shape.as_ptr(),
                [0.0, 0.0, 50.0, 40.0],
                radii.as_ptr(),
                std::ptr::null_mut(),
                0,
            )
        };
        assert!(n > 20);
        let mut out = vec![0f32; n * 2];
        // SAFETY: `out` holds `n` pairs.
        let wrote = unsafe {
            super::outline_points(
                shape.as_ptr(),
                [0.0, 0.0, 50.0, 40.0],
                radii.as_ptr(),
                out.as_mut_ptr(),
                n,
            )
        };
        assert_eq!(wrote, n);
        assert_eq!((out[0], out[1]), (0.0, 10.0));
    }
}

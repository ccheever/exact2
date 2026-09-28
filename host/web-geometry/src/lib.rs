//! `frame` and `measure` on the web (LLP 1051.000 D4): the page answers.
//!
//! The browser lays out, so the kernel's layout there is not the page's: both
//! reads ask the page through one import, `exact_geometry.read`, which
//! `host/web/geometry-glue.js` answers from the live DOM within the call.
//! Until that piece has loaded (after first paint), every read answers
//! `unavailable`.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

use exact_kernel::{Kernel, ViewId};
use exact_runner::geometry::{GeometryAnswer, GeometryLinks};

/// The page's answers, for [`exact_runner::RunnerLinks::geometry`].
pub static PAGE: GeometryLinks = GeometryLinks::new(page_frame, page_measure);

/// The import's `op`: a layout box (0) or its `height: auto` measure (1).
const FRAME: u32 = 0;
const MEASURE: u32 = 1;

/// Bits of the import's reply: answered, and provisional.
const ANSWERED: u32 = 1;
const PROVISIONAL: u32 = 2;

fn page_frame(_: &Kernel, view: ViewId) -> GeometryAnswer {
    read(FRAME, view)
}

fn page_measure(_: &mut Kernel, view: ViewId) -> GeometryAnswer {
    read(MEASURE, view)
}

/// The page's reply as an answer: four numbers when answered.
fn answer(flags: u32, [x, y, width, height]: [f64; 4]) -> GeometryAnswer {
    let finite = [x, y, width, height].iter().all(|v| v.is_finite());
    if flags & ANSWERED == 0 || !finite || width < 0.0 || height < 0.0 {
        return GeometryAnswer::UNAVAILABLE;
    }
    GeometryAnswer {
        x,
        y,
        width,
        height,
        provisional: flags & PROVISIONAL != 0,
        unavailable: false,
    }
}

#[cfg(target_arch = "wasm32")]
fn read(op: u32, view: ViewId) -> GeometryAnswer {
    #[link(wasm_import_module = "exact_geometry")]
    extern "C" {
        fn read(op: u32, view: u32, out: *mut f64) -> u32;
    }
    let mut out = [0.0f64; 4];
    // SAFETY: the page writes at most four f64s into this owned, aligned
    // buffer and calls nothing back into the module.
    let flags = unsafe { read(op, view, out.as_mut_ptr()) };
    answer(flags, out)
}

/// Off the web there is no page to ask: a reply without the answered bit.
#[cfg(not(target_arch = "wasm32"))]
fn read(_: u32, _: ViewId) -> GeometryAnswer {
    answer(0, [0.0; 4])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_without_the_answered_bit_or_with_a_bad_number_is_unavailable() {
        assert!(answer(0, [1.0, 2.0, 3.0, 4.0]).unavailable);
        assert!(answer(ANSWERED, [f64::NAN, 2.0, 3.0, 4.0]).unavailable);
        assert!(answer(ANSWERED, [1.0, 2.0, -3.0, 4.0]).unavailable);
        let a = answer(ANSWERED | PROVISIONAL, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!((a.x, a.y, a.width, a.height), (1.0, 2.0, 3.0, 4.0));
        assert!(a.provisional && !a.unavailable);
    }
}

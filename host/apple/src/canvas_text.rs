//! Canvas 2D text measurement on Apple (LLP 1056 D8): the recorder's
//! `measureText` and `fillText` measure with Core Text, through the app's
//! callback, on the thread the draw runs on — never a hop to another
//! thread. The callback's context is the session's text engine, the one the
//! Core Graphics replayer draws with, so what was measured is what is drawn.

use exact_runner::exact_canvas::{RawMetrics, TextEngine, TextRun};
use std::ffi::c_void;

/// One run, as the callback receives it. The strings live for the call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CCanvasText {
    /// UTF-8 text, spaces already normalised.
    pub text: *const u8,
    /// Its byte length.
    pub len: usize,
    /// The family list, names joined by `,`.
    pub families: *const u8,
    /// Its byte length.
    pub families_len: usize,
    /// CSS px.
    pub size: f64,
    /// A percentage (100 normal).
    pub stretch: f64,
    /// `letterSpacing`, px.
    pub letter_spacing: f64,
    /// `wordSpacing`, px.
    pub word_spacing: f64,
    /// 1–1000.
    pub weight: u16,
    /// 0 normal, 1 italic, 2 oblique.
    pub style: u8,
    /// Index into `exact_canvas::font::CAPS`.
    pub caps: u8,
    /// 0 auto, 1 normal, 2 none.
    pub kerning: u8,
    /// 1 for a right-to-left base direction.
    pub rtl: u8,
}

/// The run's eleven raw metrics, in `RawMetrics::to_array`'s order.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CCanvasMetrics {
    /// width, left, right, ascent, descent, font ascent, font descent, em
    /// ascent, em descent, hanging, ideographic.
    pub v: [f64; 11],
}

/// The callback's type.
pub type CanvasTextFn = extern "C" fn(ctx: *mut c_void, run: *const CCanvasText) -> CCanvasMetrics;

/// The app's Core Text measurer for one runtime. The context is kept as an
/// address: the runtime's thread is the only one that calls it.
pub struct CallbackText {
    f: CanvasTextFn,
    ctx: usize,
}

impl CallbackText {
    /// Wrap the callback with the context it is called with.
    pub fn new(f: CanvasTextFn, ctx: *mut c_void) -> CallbackText {
        CallbackText {
            f,
            ctx: ctx as usize,
        }
    }
}

impl TextEngine for CallbackText {
    fn measure(&self, run: &TextRun<'_>) -> RawMetrics {
        let families = run.font.family_list();
        let c = CCanvasText {
            text: run.text.as_ptr(),
            len: run.text.len(),
            families: families.as_ptr(),
            families_len: families.len(),
            size: run.font.size,
            stretch: run.font.stretch,
            letter_spacing: run.letter_spacing,
            word_spacing: run.word_spacing,
            weight: run.font.weight,
            style: run.font.style,
            caps: run.font.caps,
            kerning: run.kerning,
            rtl: u8::from(run.rtl),
        };
        // The one foreign call, with `c`'s strings alive for it.
        let m = (self.f)(self.ctx as *mut c_void, &c);
        RawMetrics::from_slice(&m.v)
    }
}

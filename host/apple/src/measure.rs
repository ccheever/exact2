//! Text measurement through a callback the app registers.
//!
//! @ref LLP 1008 §3; LLP 1001 §6 (a per-kernel injected measurer, never a
//! process-global callback)
//!
//! The kernel never embeds a platform text API. The app hands `exact_boot` a
//! C function and a context; this module wraps them as the kernel's
//! [`TextMeasurer`], flattening each request into C structs the function
//! reads. Text is passed as UTF-8 bytes with a length; nothing is
//! NUL-terminated. Widths and heights are points; an unconstrained offer is
//! negative ([`MAX_CONTENT`], [`MIN_CONTENT`]).

use exact_kernel::{AxisOffer, TextAlign, TextMeasureRequest, TextMeasurer, TextMetrics};
use std::ffi::c_void;

/// Offer value meaning "as wide/tall as the content wants".
pub const MAX_CONTENT: f32 = -1.0;
/// Offer value meaning "as narrow as the content can be".
pub const MIN_CONTENT: f32 = -2.0;

/// One run, as the callback sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CRun {
    /// UTF-8 bytes, not NUL-terminated.
    pub text: *const u8,
    /// Byte length.
    pub len: usize,
    /// Points.
    pub font_size: f32,
    /// CSS 100–900.
    pub font_weight: u16,
    /// 1 for italic.
    pub italic: u8,
    /// Points; 0 means the font's natural line height.
    pub line_height: f32,
    /// Points per glyph.
    pub letter_spacing: f32,
}

/// One request, as the callback sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CRequest {
    /// The runs.
    pub runs: *const CRun,
    /// How many.
    pub count: usize,
    /// Width offer in points, or [`MAX_CONTENT`] / [`MIN_CONTENT`].
    pub width: f32,
    /// Height offer in points, or [`MAX_CONTENT`] / [`MIN_CONTENT`].
    pub height: f32,
    /// 0 start, 1 center, 2 end, 3 justify.
    pub align: u8,
    /// Maximum lines; 0 means unlimited.
    pub line_clamp: u32,
}

/// What the callback returns.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CMetrics {
    /// Points.
    pub width: f32,
    /// Points.
    pub height: f32,
    /// Top to first alphabetic baseline, points; negative when unknown.
    pub baseline: f32,
}

/// The callback's type.
pub type MeasureFn = extern "C" fn(ctx: *mut c_void, request: *const CRequest) -> CMetrics;

/// A kernel measurer backed by the app's callback.
pub struct CallbackMeasurer {
    f: MeasureFn,
    ctx: *mut c_void,
}

impl CallbackMeasurer {
    /// Wrap `f` with its context.
    pub fn new(f: MeasureFn, ctx: *mut c_void) -> CallbackMeasurer {
        CallbackMeasurer { f, ctx }
    }
}

fn offer(a: AxisOffer) -> f32 {
    match a {
        AxisOffer::Definite(v) => v,
        AxisOffer::MaxContent => MAX_CONTENT,
        AxisOffer::MinContent => MIN_CONTENT,
    }
}

impl TextMeasurer for CallbackMeasurer {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        let runs: Vec<CRun> = request
            .runs
            .iter()
            .map(|r| CRun {
                text: r.text.as_ptr(),
                len: r.text.len(),
                font_size: r.style.font_size,
                font_weight: r.style.font_weight,
                italic: u8::from(r.style.font_style != exact_kernel::FontStyle::Normal),
                line_height: r.style.line_height,
                letter_spacing: r.style.letter_spacing,
            })
            .collect();
        let c = CRequest {
            runs: runs.as_ptr(),
            count: runs.len(),
            width: offer(request.width),
            height: offer(request.height),
            align: match request.paragraph.text_align {
                TextAlign::Left => 0,
                TextAlign::Center => 1,
                TextAlign::Right => 2,
                TextAlign::Justify => 3,
            },
            line_clamp: request.paragraph.line_clamp,
        };
        // The one foreign call: the app's function, with the structs above
        // alive for its duration and read-only.
        let m = (self.f)(self.ctx, &c);
        TextMetrics {
            width: if m.width.is_finite() {
                m.width.max(0.0)
            } else {
                0.0
            },
            height: if m.height.is_finite() {
                m.height.max(0.0)
            } else {
                0.0
            },
            first_baseline: (m.baseline.is_finite() && m.baseline >= 0.0).then_some(m.baseline),
        }
    }
}

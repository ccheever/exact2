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
use exact_plan::{Plan, StackMemberKind};
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
    /// Plan font stack id.
    pub font_family: u16,
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

/// One declared face in the synchronous boot catalog callback. The UTF-8
/// strings live for the duration of the callback and are not NUL-terminated.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CFontFace {
    /// The Contract alias.
    pub family: *const u8,
    /// Alias byte length.
    pub family_len: usize,
    /// App-relative source path.
    pub source: *const u8,
    /// Source byte length.
    pub source_len: usize,
    /// Plan stack id.
    pub stack: u16,
    /// CSS weight.
    pub weight: u16,
    /// 1 for italic.
    pub italic: u8,
}

/// Every declared face in one validated plan.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CFontCatalog {
    /// Face rows.
    pub faces: *const CFontFace,
    /// Face row count.
    pub count: usize,
}

/// Installs a complete plan-scoped catalog before the first text layout,
/// with the context the runtime was given (LLP 1031 D12: the catalog is
/// the session's, so the callback needs to know which session).
pub type FontsFn = extern "C" fn(ctx: *mut c_void, catalog: *const CFontCatalog);

/// Project the validated plan's tables across the host-only font seam. This
/// is deliberately separate from `exact_out()`, whose payload remains ops.
pub fn install_fonts(plan: &Plan, callback: FontsFn, ctx: *mut c_void) {
    let mut faces = Vec::new();
    for (stack_index, stack) in plan.stacks.iter().enumerate() {
        let member = plan.stack_member(stack.members.iter().next().expect("validated stack"));
        if member.kind != StackMemberKind::Family {
            continue;
        }
        let family = plan.familie(member.family.expect("validated family member"));
        let name = plan.str(family.name);
        for face_id in family.faces.iter() {
            let face = plan.face(face_id);
            let source = plan.str(face.source);
            faces.push(CFontFace {
                family: name.as_ptr(),
                family_len: name.len(),
                source: source.as_ptr(),
                source_len: source.len(),
                stack: stack_index as u16,
                weight: face.weight,
                italic: u8::from(face.italic),
            });
        }
    }
    let catalog = CFontCatalog {
        faces: faces.as_ptr(),
        count: faces.len(),
    };
    callback(ctx, &catalog);
}

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
                font_family: r.style.font_family,
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

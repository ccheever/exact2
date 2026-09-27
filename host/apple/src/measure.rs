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

use exact_kernel::{
    AxisOffer, ParagraphStamp, TextAlign, TextMeasureRequest, TextMeasurer, TextMetrics,
};

mod identified;
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
    /// Whether line_height is an explicit used length, including zero.
    pub has_line_height: u8,
    /// Logical points when has_line_height is 1.
    pub line_height: f32,
    /// Points per glyph.
    pub letter_spacing: f32,
    /// CSS `font-variant-numeric` bits: 1 is `tabular-nums` (LLP 1053 G4).
    pub font_variant_numeric: u8,
}

/// One request, as the callback sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CRequest {
    /// Identified paragraph allocation/source; zero view means synchronous only.
    pub view: u32,
    /// Generation-checked kernel slot.
    pub node_index: u32,
    /// Allocation generation.
    pub node_generation: u32,
    /// Source/metric revision within this measurer lifetime.
    pub revision: u64,
    /// The runs.
    pub runs: *const CRun,
    /// How many.
    pub count: usize,
    /// Paragraph minimum line box font/style, with no text.
    pub strut: CRun,
    /// Width offer in points, or [`MAX_CONTENT`] / [`MIN_CONTENT`].
    pub width: f32,
    /// Height offer in points, or [`MAX_CONTENT`] / [`MIN_CONTENT`].
    pub height: f32,
    /// 0 start, 1 center, 2 end, 3 justify.
    pub align: u8,
    /// Maximum lines; 0 means unlimited.
    pub line_clamp: u32,
    /// CSS overflow-wrap: normal, break-word, anywhere.
    pub overflow_wrap: u8,
    /// CSS white-space: normal, pre-wrap, nowrap, pre-line. Runs arrive collapsed
    /// unless it preserves (LLP 1053 G5).
    pub white_space: u8,
    /// CSS direction: ltr, rtl.
    pub direction: u8,
    /// Resolved exclusions, borrowed for this callback.
    pub exclusions: *const crate::textflow::Shape,
    /// Exclusion count.
    pub exclusion_count: usize,
    /// 1 when the one run is Markdown source the host expands (LLP 1045 D3).
    pub markup: u8,
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
    memo: identified::Memo,
}

impl CallbackMeasurer {
    /// Wrap `f` with its context for one installed metric catalog.
    /// Construct a new measurer when that catalog changes; Apple boot and
    /// candidate preparation already do so before invoking the font hook.
    pub fn new(f: MeasureFn, ctx: *mut c_void) -> CallbackMeasurer {
        CallbackMeasurer {
            f,
            ctx,
            memo: identified::Memo::default(),
        }
    }
}

fn offer(a: AxisOffer) -> f32 {
    match a {
        AxisOffer::Definite(v) => v,
        AxisOffer::MaxContent => MAX_CONTENT,
        AxisOffer::MinContent => MIN_CONTENT,
    }
}

fn c_run(text: &str, style: exact_kernel::TextStyle) -> CRun {
    CRun {
        text: text.as_ptr(),
        len: text.len(),
        font_size: style.font_size,
        font_weight: style.font_weight,
        font_family: style.font_family,
        italic: u8::from(style.font_style != exact_kernel::FontStyle::Normal),
        has_line_height: u8::from(style.line_height.is_some()),
        line_height: style.line_height.unwrap_or(0.0),
        letter_spacing: style.letter_spacing,
        font_variant_numeric: style.font_variant_numeric,
    }
}

impl CallbackMeasurer {
    fn foreign_measure(
        &mut self,
        request: &TextMeasureRequest<'_>,
        stamp: Option<&ParagraphStamp>,
    ) -> CMetrics {
        // CSS collapsing before CoreText, as the browser does (LLP 1053 G5);
        // Markdown source keeps its own lines. The strings live for the call.
        let collapsed = (request.paragraph.markup == exact_kernel::Markup::None)
            .then(|| exact_textflow::collapse(request.runs, request.paragraph.white_space.model()))
            .flatten();
        let text = |i: usize| -> &str {
            collapsed
                .as_ref()
                .map_or(&*request.runs[i].text, |c| c.runs[i].as_str())
        };
        let single;
        let owned;
        let runs: &[CRun] = if let [run] = request.runs {
            single = c_run(text(0), run.style);
            std::slice::from_ref(&single)
        } else {
            owned = request
                .runs
                .iter()
                .enumerate()
                .map(|(i, r)| c_run(text(i), r.style))
                .collect::<Vec<CRun>>();
            &owned
        };
        let shapes = crate::textflow::Shapes::new(request.exclusions);
        let c = CRequest {
            view: stamp.map_or(0, ParagraphStamp::view),
            node_index: stamp.map_or(0, |s| s.owner().index),
            node_generation: stamp.map_or(0, |s| s.owner().generation),
            revision: stamp.map_or(0, ParagraphStamp::metric_revision),
            runs: runs.as_ptr(),
            count: runs.len(),
            strut: c_run("", request.paragraph.strut),
            width: offer(request.width),
            height: offer(request.height),
            // Physical already (`Paragraph::from_style`); `start`/`end` never arrive.
            align: match request
                .paragraph
                .text_align
                .physical(request.paragraph.direction)
            {
                TextAlign::Left | TextAlign::Start => 0,
                TextAlign::Center => 1,
                TextAlign::Right | TextAlign::End => 2,
                TextAlign::Justify => 3,
            },
            line_clamp: request.paragraph.line_clamp,
            overflow_wrap: request.paragraph.overflow_wrap as u8,
            white_space: request.paragraph.white_space as u8,
            direction: request.paragraph.direction as u8,
            exclusions: shapes.flat.as_ptr(),
            exclusion_count: shapes.flat.len(),
            markup: u8::from(request.paragraph.markup == exact_kernel::Markup::Markdown),
        };
        // The one foreign call: the app's function, with the structs above
        // alive for its duration and read-only.
        (self.f)(self.ctx, &c)
    }
}

fn sanitize(m: CMetrics) -> TextMetrics {
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

impl TextMeasurer for CallbackMeasurer {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        sanitize(self.foreign_measure(request, None))
    }

    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        request: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        // @ref LLP 1043.000 §3 D4 — moving geometry is not a paragraph identity.
        if !request.exclusions.is_empty() {
            return self.measure(request);
        }
        if let Some(metrics) = self.memo.get(stamp, request.width, request.height) {
            return metrics;
        }
        let raw = self.foreign_measure(request, Some(stamp));
        if raw.baseline == -2.0 {
            self.memo.defer_to_owner(stamp);
        }
        let valid = raw.width.is_finite()
            && raw.width >= 0.0
            && raw.height.is_finite()
            && raw.height >= 0.0
            && raw.baseline.is_finite()
            && raw.baseline != -2.0; // pending worker metrics are never memoized here
        let metrics = sanitize(raw);
        // A negative finite baseline is the existing C "unknown" sentinel.
        // Invalid raw output keeps its existing sanitized return behavior, but
        // must retry the callback next time instead of caching a synthetic zero.
        if valid {
            self.memo.put(stamp, request.width, request.height, metrics);
        }
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_zero_and_normal_are_distinct_at_the_c_seam() {
        let mut style = exact_kernel::StyleProps::default();
        let normal = c_run("", exact_kernel::TextStyle::from_style(&style));
        assert_eq!(normal.has_line_height, 0);
        style.line_height = exact_kernel::LineHeight::Number(0.0);
        let zero = c_run("", exact_kernel::TextStyle::from_style(&style));
        assert_eq!(zero.has_line_height, 1);
        assert_eq!(zero.line_height, 0.0);
        style.font_size = 20.0;
        style.line_height = exact_kernel::LineHeight::Number(1.5);
        let ratio = c_run("", exact_kernel::TextStyle::from_style(&style));
        assert_eq!(ratio.has_line_height, 1);
        assert_eq!(ratio.line_height, 30.0);
    }
}

#[cfg(test)]
mod identified_tests;

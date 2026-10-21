use super::*;

/// What the presenter hands the library at boot: the text measurer (LLP
/// 1008 §3) and the wake for a request's reply (LLP 1016 D2), each with an
/// opaque context the library passes back untouched.
#[derive(Clone, Copy)]
pub struct Hooks {
    /// Measures a paragraph; `None` for the monospace reference measurer.
    pub measure: Option<MeasureFn>,
    /// Passed back to `measure` and `lines`.
    pub ctx: *mut c_void,
    /// A paragraph's line boxes (LLP 1093 D6); `None` keeps paragraphs whole.
    pub lines: Option<crate::measure::LinesFn>,
    /// Called on the executor's thread when a reply is queued; `None`: replies wait for `exact_pump`.
    pub wake: Option<crate::executor::WakeFn>,
    /// Passed back to `wake`.
    pub wake_ctx: *mut c_void,
    /// Measures a Canvas 2D run with Core Text (LLP 1056 D8), with `ctx`.
    pub canvas_text: Option<crate::canvas_text::CanvasTextFn>,
    /// Measures a system symbol (`TextMeasurer::symbol_size`), with `ctx`.
    pub symbol: Option<crate::measure::SymbolFn>,
    /// Platform time formatting on the runner clock.
    pub time_format: Option<crate::time::TimeFormatFn>,
}
impl Hooks {
    /// No callbacks: the reference measurer, and replies on `pump` only.
    pub const fn none() -> Hooks {
        Hooks {
            measure: None,
            ctx: std::ptr::null_mut(),
            lines: None,
            wake: None,
            wake_ctx: std::ptr::null_mut(),
            canvas_text: None,
            symbol: None,
            time_format: None,
        }
    }
}

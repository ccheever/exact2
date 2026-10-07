//! Lowering's refusals.

use contract_syntax::Span;

/// A typed rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    /// Stable id.
    pub id: &'static str,
    /// What went wrong.
    pub message: String,
    /// Where.
    pub span: Span,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] {}", self.span, self.id, self.message)
    }
}

/// One refusal, as the plural result lowering returns.
pub(crate) fn err_one(id: &'static str, message: impl Into<String>, span: Span) -> Vec<LowerError> {
    vec![LowerError {
        id,
        message: message.into(),
        span,
    }]
}

pub(crate) fn err<T>(
    id: &'static str,
    message: impl Into<String>,
    span: Span,
) -> Result<T, LowerError> {
    Err(LowerError {
        id,
        message: message.into(),
        span,
    })
}

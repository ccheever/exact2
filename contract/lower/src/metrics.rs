//! The metrics of a look Exact draws itself (LLP 1115.002 D6, D8.4): rows
//! of the kernel's schema (`metrics`), a value a platform. A sheet the
//! compiler writes names one where it used to hold a number, and the kernel
//! reads the name as its platform's value when the row is set. The names are
//! the compiler's own: nothing an author writes may be one, so the
//! vocabulary of Contract does not grow by them.
//! @ref LLP 1115.002 D6, D8.4

use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, Span};
use exact_kernel::style::metrics::named;

/// The grouped list's metric `name` as its sheet writes it: the name of a
/// row of the kernel's `metrics`, which is the only place its value is.
pub(crate) fn grouped(name: &str) -> String {
    let written = format!("-exact-grouped-{name}");
    debug_assert!(
        named(&written).is_some(),
        "{written} is no metric of the schema"
    );
    written
}

/// Whether a value's literal results name a metric, whichever branch of a
/// choice is taken: a string that every host reads as its platform's value,
/// so a row that takes only numbers takes it (`check_style_value`).
pub(crate) fn names(e: &Expr) -> bool {
    match e {
        Expr::Str(s, _) => named(s).is_some(),
        Expr::Ternary(_, yes, no, _) => names(yes) || names(no),
        _ => false,
    }
}

/// Refuse a metric's name where an author wrote it: `value` is one literal
/// result of `a`'s value.
pub(crate) fn refuse_authored(a: &Attr, value: &Expr, span: Span) -> Result<(), LowerError> {
    match value {
        Expr::Str(s, _) if a.written && named(s).is_some() => err(
            "lower-attr-value",
            format!("`{}=\"{s}\"`: `{s}` is a metric of the platform's own look, which the compiler writes and an author does not; write the value you mean (a number, a text style such as `-exact-body`, a colour role)", a.name),
            span,
        ),
        _ => Ok(()),
    }
}

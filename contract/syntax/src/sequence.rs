//! `sequence NAME when COND [key=EXPR]` holding one `play(tracks, bpm=…)`
//! (LLP 1125 D2): what should play, declared as a value the runner expands
//! into its voice table a second ahead.

use crate::ast::Expr;
use crate::Span;

/// A `sequence` declaration: a task's gate grammar and one `play(...)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    /// Name.
    pub name: String,
    /// `when cond`: the sequence plays while `cond` holds, armed from the
    /// commit that made it true; `None` for `mount` and for `key=` alone.
    pub gate: Option<Expr>,
    /// `key=expr`: a new key restarts it from `from`.
    pub key: Option<Expr>,
    /// `play`'s first argument, the tracks.
    pub tracks: Expr,
    /// `play`'s named arguments but `ended=`, in written order:
    /// `bpm`, `from`, `countIn`, `clock`, `clockOut` (the type pass refuses
    /// any other, and a repeat).
    pub options: Vec<(String, Expr, Span)>,
    /// `ended=action`: run when the last finite track finishes.
    pub ended: Option<(String, Span)>,
    /// Where.
    pub span: Span,
}

impl Sequence {
    /// The named argument `name`, if written.
    pub fn option(&self, name: &str) -> Option<&Expr> {
        self.options
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, e, _)| e)
    }
}

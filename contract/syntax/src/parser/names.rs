//! Which words a name may be (@ref LLP 1085 D5).
//!
//! Sixteen words are reserved, and only where a name is bound: a later
//! expression reads or calls what a binder names, and each of these words
//! heads an expression, a statement or a view node there (`none` is the
//! literal, so `shape none` could never be built). Every other keyword is
//! contextual: a keyword where its construct starts (`state` at the head of
//! a section line, `refresh` before a name in a body) and a name everywhere
//! else. A shape field, a named argument, a member after `.` and an
//! attribute name can never head an expression, so they admit every word.

use super::*;

/// The words no binder may be.
pub(super) const RESERVED: [&str; 16] = [
    "when", "if", "else", "each", "in", "match", "case", "as", "fn", "and", "or", "not", "true",
    "false", "none", "some",
];

/// Whether `w` is one of the sixteen reserved words.
pub(super) fn is_reserved(w: &str) -> bool {
    RESERVED.contains(&w)
}

/// The refusal of a reserved word where a name goes.
pub(super) fn reserved_message(w: &str) -> String {
    let why = if matches!(w, "true" | "false" | "none" | "some") {
        "it is a literal"
    } else {
        "it shapes an expression"
    };
    format!("`{w}` is reserved in Contract ({why}); choose another name")
}

impl Parser {
    /// A name a binder introduces, or one that refers to a binder's name: any
    /// word but the sixteen reserved ones.
    pub(super) fn ident(&mut self) -> R<(String, Span)> {
        match self.peek_kind().clone() {
            TokenKind::Ident(w) if !is_reserved(&w) => {
                let t = self.next();
                Ok((w, t.span))
            }
            TokenKind::Ident(w) => self.err("syntax-expected-name", reserved_message(&w)),
            other => self.err(
                "syntax-expected-name",
                format!("expected a name, found {}", describe(&other)),
            ),
        }
    }

    /// A name where the grammar can never read an expression: a shape field,
    /// a named argument, a member after `.`. Every word is one.
    pub(super) fn field_name(&mut self) -> R<(String, Span)> {
        match self.peek_kind().clone() {
            TokenKind::Ident(w) => {
                let t = self.next();
                Ok((w, t.span))
            }
            _ => self.ident(),
        }
    }
}

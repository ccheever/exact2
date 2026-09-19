//! Contract syntax: the lexer, the AST, and the parser.
//!
//! @ref LLP 1004 D3 (the language basis: Contract v1 Edition 1, scoped to the
//! v1 app's constructs) / LLP 0508 (research)
//!
//! The grammar is indentation-structured. A file is a sequence of `shape` and
//! `component` declarations; a component holds `props`, `state`, `derive`,
//! `resource`, `action`, `task`, and `view` sections; a view is a tree of
//! elements, component uses, and the three region constructs `when`/`else`,
//! `each … in … key=…`, and `match … case some(x) / case none`. Expressions
//! are closed: literals, template strings, names, member access, calls,
//! arithmetic, comparison, boolean logic, the conditional operator, `some`,
//! `none`, and inline `match`.
//!
//! Every node carries a [`Span`]; every rejection is a [`SyntaxError`] with
//! one stable id and a span. Nothing here knows about types or the plan.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod ast;
pub mod fmt;
pub mod inline;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use inline::{expand, inline, Expanded};
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::{parse, SyntaxError};

/// A source position, 1-based: the token starts at `line:col` and ends
/// before `end_col`, so `col..end_col` is the identifier (LLP 1035.005 D2).
/// Columns count bytes, as `col` always has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct Span {
    /// Line.
    pub line: u32,
    /// Column of the first byte.
    pub col: u32,
    /// Column after the last byte.
    pub end_col: u32,
}

/// A second place a diagnostic names — the other side of a mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    /// Where.
    pub span: Span,
    /// What is there.
    pub note: String,
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

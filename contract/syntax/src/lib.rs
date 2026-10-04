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
mod clock;
pub mod fmt;
pub mod idioms;
pub mod inline;
pub mod lexer;
pub mod parser;
pub mod scope;
mod share;
mod spans;

pub use ast::*;
pub use clock::{resolve_clock_timelines, resolve_clock_timelines_in};
pub use inline::{expand, expand_all, expand_mapped, inline, Expanded, Instance, Owner};
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::{parse, parse_source, parse_source_all, SyntaxError};
pub use share::share_calls;
pub use spans::VisitSpans;

/// A token range in one source file. Lines and byte columns are 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct Span {
    /// Line.
    pub line: u32,
    /// Inclusive byte column.
    pub col: u32,
    /// Exclusive byte column; equal to `col` for a structural position.
    pub end_col: u32,
    /// File identity assigned by the loader; zero for a standalone source.
    pub source_id: u32,
}

impl Span {
    /// An empty position in a standalone source.
    pub const fn point(line: u32, col: u32) -> Self {
        Self {
            line,
            col,
            end_col: col,
            source_id: 0,
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

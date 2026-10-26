//! `tool NAME reads|writes [additive]|destroys [idempotent] [closed]
//! "DESCRIPTION" = ACTION [returns EXPR] [fails when EXPR] [available when
//! EXPR]` (LLP 1111 D2): a root action offered to a caller that is not the
//! person, with the author's word for what it does. The clauses may go on
//! deeper-indented lines, in that order.
//!
//! The parser gives each clause's expression to the component as a derive
//! named [`Tool::derive`], so that every later pass checks and lowers it as
//! it does any derive, and the plan's tool row names the derives.

use crate::ast::Expr;
use crate::Span;

/// What a tool does, in the author's word (LLP 1111 D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consequence {
    /// `reads`: changes nothing the person keeps.
    Reads,
    /// `writes`: changes or adds to what the person keeps.
    Writes,
    /// `destroys`: removes what the person keeps.
    Destroys,
}

impl Consequence {
    /// The word as written.
    pub fn word(self) -> &'static str {
        match self {
            Consequence::Reads => "reads",
            Consequence::Writes => "writes",
            Consequence::Destroys => "destroys",
        }
    }
}

/// A `tool` line.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// Its name, as a caller calls it.
    pub name: String,
    /// `reads`, `writes` or `destroys`.
    pub consequence: Consequence,
    /// `additive` (after `writes` only): adds, never overwrites.
    pub additive: bool,
    /// `idempotent`: a repeat has no further effect.
    pub idempotent: bool,
    /// `closed`: touches only the app's own data.
    pub closed: bool,
    /// The description, as a caller reads it.
    pub description: String,
    /// The root action it runs, and where its name is.
    pub action: (String, Span),
    /// `returns EXPR`: the call's value.
    pub returns: Option<Expr>,
    /// `fails when EXPR`: the call failed.
    pub fails: Option<Expr>,
    /// `available when EXPR`: while false, the tool is not offered.
    pub available: Option<Expr>,
    /// Where.
    pub span: Span,
}

impl Tool {
    /// The name of the derive holding clause `clause` (`returns`, `fails`,
    /// `available`) of tool `tool`: a name no author can write.
    pub fn derive(tool: &str, clause: &str) -> String {
        format!("tool:{tool}:{clause}")
    }
}

//! Tamber, Exact's design system (LLP 1121): the `.tamber` grammar (§3),
//! its checker (D3–D5) and the build's one evaluator (D7), which folds a
//! design to the IR every generator and the editor read (D2).
//!
//! Stage 2 builds colour tokens and app axes; every other kind is parsed
//! where the grammar has it and refused with the stage that brings it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod check;
mod color_fn;
mod css;
mod edit;
mod fmt;
mod fold;
pub mod history;
pub mod import;
mod ir;
mod lex;
mod session;
mod syntax;

pub use check::{Checked, MAX_TUPLES};
pub use edit::{apply, apply_all, undo, Edited, Op, WhenLine};
pub use fmt::format;
pub use fold::{Folded, Half, Host};
pub use ir::FORMAT;
pub use session::{plan, Class, Planned, Refused};
pub use syntax::{parse, At, Axis, Design, Dial, DialKind, Token, When};

/// A refusal, at a line and column of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable id (`tamber-dial-inert`, `tamber-under-light` …).
    pub id: &'static str,
    /// What and why.
    pub message: String,
    /// The line, 1-based.
    pub line: u32,
    /// The column, 1-based.
    pub col: u32,
}

impl Diagnostic {
    pub(crate) fn new(
        id: &'static str,
        message: impl Into<String>,
        line: u32,
        col: u32,
    ) -> Diagnostic {
        Diagnostic {
            id,
            message: message.into(),
            line,
            col,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {}: {}",
            self.line, self.col, self.id, self.message
        )
    }
}

/// A colour role, as the kernel's schema lists it (LLP 1095 D2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    /// The schema's name: CSS's (`Canvas`) or Exact's (`secondary-label`).
    pub name: String,
    /// A CSS system colour, written by its CSS name; else `-exact-<name>`.
    pub system: bool,
    /// The light and dark pair every host shows until it reports its own.
    pub fallback: [String; 2],
}

/// What a design is checked against: the schema's vocabulary, which the
/// caller reads from `kernel/tables/schema.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vocab {
    /// The colour roles, in the schema's order.
    pub roles: Vec<Role>,
}

/// A checked, folded design.
#[derive(Debug, Clone, PartialEq)]
pub struct Folding {
    /// The design as parsed.
    pub design: Design,
    /// Its tokens, in declaration order.
    pub tokens: Vec<Checked>,
    /// How many full app tuples its axes make.
    pub tuples: usize,
    fallbacks: Vec<Option<[String; 2]>>,
    dials: Vec<String>,
}

/// Parse, check and fold a `.tamber` source.
pub fn check(src: &str, vocab: &Vocab) -> Result<Folding, Vec<Diagnostic>> {
    check_design(parse(src)?, vocab)
}

/// Check and fold a design already parsed, or built by a caller: the
/// compiler's inline `theme` (LLP 1095 D4) and its synthetic tokens (D3).
pub fn check_design(design: Design, vocab: &Vocab) -> Result<Folding, Vec<Diagnostic>> {
    let (tokens, tuples, dials) = check::check(&design, vocab)?;
    let fallbacks = tokens
        .iter()
        .map(|t| fallback(t.values.first()?, vocab))
        .collect();
    Ok(Folding {
        design,
        tokens,
        tuples,
        fallbacks,
        dials: dials.iter().map(ir::dial_text).collect(),
    })
}

/// A value's fallback pair (D7): light and dark, roles at theirs.
pub fn fallback(value: &Folded, vocab: &Vocab) -> Option<[String; 2]> {
    match value {
        Folded::One(h) => Some([h.clip(), h.clip()]),
        Folded::Pair(l, d) => Some([l.clip(), d.clip()]),
        Folded::Host(Host::Role(row)) => vocab
            .roles
            .iter()
            .find(|r| {
                let spelled = if r.system {
                    r.name.clone()
                } else {
                    format!("-exact-{}", r.name)
                };
                spelled.eq_ignore_ascii_case(row)
            })
            .map(|r| {
                r.fallback.clone().map(|c| match exact_color::parse(&c) {
                    Some(exact_color::Parsed::Legacy(c)) => Half::legacy(c).clip(),
                    _ => c,
                })
            }),
        Folded::Host(Host::Platform(_, pair)) => Some(pair.clone()),
    }
}

impl Folding {
    /// The IR, `tamber.json` (D2): generated, never committed.
    pub fn ir(&self) -> String {
        ir::write(self)
    }

    /// A token's value for one tuple, by name.
    pub fn value(&self, name: &str, tuple: usize) -> Option<&Folded> {
        self.tokens
            .iter()
            .find(|t| t.name == name)
            .and_then(|t| t.values.get(tuple))
    }

    /// The tuple for `axis=value` pairs, the rest at their defaults.
    pub fn tuple(&self, modes: &[(&str, &str)]) -> Option<usize> {
        let mut tuple = 0;
        for axis in &self.design.axes {
            let chosen = modes
                .iter()
                .find(|(a, _)| *a == axis.name.value)
                .map_or(Some(0), |(_, v)| axis.values.iter().position(|x| x == v))?;
            tuple = tuple * axis.values.len() + chosen;
        }
        modes
            .iter()
            .all(|(a, _)| self.design.axes.iter().any(|x| x.name.value == *a))
            .then_some(tuple)
    }
}

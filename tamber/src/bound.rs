//! The design checker's own bounds (rt1's finding; theme2's approval of the
//! checker bound), beside a value's bytes and nesting (`css::over_limit`)
//! and a length's residual (`residual::MAX_RESIDUAL_ATOMS`): how deep a chain
//! of names goes, how much a kind walk infers, and the design's values
//! checked against them all before anything reads one ([`preflight`]).

use crate::measure::Kind;
use crate::syntax::{At, Design};
use crate::Diagnostic;

/// One kind walk over a design's tokens (`Checker::kind_of`): which are on
/// its path, each kind found, the cycles met, and its budget and depth.
pub(crate) struct Walk {
    pub(crate) on: Vec<bool>,
    pub(crate) known: Vec<Option<Kind>>,
    pub(crate) cycles: usize,
    /// Kinds inferred, which only a cycle repeats (a kind found without
    /// meeting one is kept): past [`Walk::budget`] the rest are colours,
    /// as a cycle's are, so a small cycle read twice a level is no 2^n.
    pub(crate) steps: usize,
    /// The names followed to here: never past [`MAX_REFERENCE_DEPTH`].
    pub(crate) depth: usize,
}

impl Walk {
    /// The kinds a design's walk may infer: every token's once, four times
    /// over, which no design without a cycle reaches.
    pub(crate) fn budget(&self) -> usize {
        4 * self.on.len() + 64
    }
}

/// The deepest a chain of tokens naming one another may go (the checker's
/// own bound, beside a value's bytes and nesting): each name followed is a
/// level of recursion, so a longer one is refused before it would overflow
/// the stack (`tamber-reference-depth`).
pub const MAX_REFERENCE_DEPTH: usize = 256;

/// Every value, name and word a design holds, past `css::over_limit` or a
/// name's bytes: each refused where it is written, before aliasing, kind
/// inference or any reader sees one (a token's, `when` line's, dial's,
/// range end's, icon's, param's, component param's, metadata's, axis's or
/// export's). The work is the design's text, once.
pub(crate) fn preflight(design: &Design) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut value = |text: &str, at: (u32, u32)| {
        if let Some((id, why)) = crate::css::over_limit(text) {
            out.push(Diagnostic::new(id, why, at.0, at.1));
        }
    };
    let pos = |a: &At<String>| (a.line, a.col);
    for t in design
        .tokens
        .iter()
        .chain(&design.icons)
        .chain(&design.params)
    {
        value(&t.name.value, pos(&t.name));
        value(&t.value.value, pos(&t.value));
        for w in &t.whens {
            value(&w.value.value, pos(&w.value));
            for (a, v) in &w.conds {
                value(a, pos(&w.value));
                value(v, pos(&w.value));
            }
        }
        for (k, v) in &t.meta {
            value(k, pos(&t.value));
            value(v, pos(&t.value));
        }
    }
    for d in &design.dials {
        value(&d.name.value, pos(&d.name));
        value(&d.value.value, pos(&d.value));
        if let Some((lo, hi)) = &d.range {
            value(lo, pos(&d.value));
            value(hi, pos(&d.value));
        }
        for (k, v) in &d.meta {
            value(k, pos(&d.value));
            value(v, pos(&d.value));
        }
    }
    for c in &design.components {
        value(&c.name.value, pos(&c.name));
        for p in &c.params {
            value(&p.name.value, pos(&p.name));
            value(&p.value.value, pos(&p.value));
            if let Some((lo, hi)) = &p.range {
                value(lo, pos(&p.value));
                value(hi, pos(&p.value));
            }
            for (k, v) in &p.meta {
                value(k, pos(&p.value));
                value(v, pos(&p.value));
            }
        }
    }
    for a in &design.axes {
        value(&a.name.value, pos(&a.name));
        for v in &a.values {
            value(v, pos(&a.name));
        }
    }
    for e in design.exports.iter().flatten() {
        value(&e.value, pos(e));
    }
    out
}

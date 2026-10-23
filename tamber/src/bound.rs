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
    /// The names followed to here: never past [`MAX_REFERENCE_DEPTH`].
    pub(crate) depth: usize,
    /// The first token reached past [`MAX_REFERENCE_DEPTH`], which the design
    /// is refused at.
    pub(crate) deep: Option<usize>,
}

impl Walk {
    /// One level deeper, for token `i`'s walk: a name followed or a function
    /// entered. Past [`MAX_REFERENCE_DEPTH`] it isn't (`deep` names `i`); a
    /// caller that enters leaves with `depth -= 1`.
    pub(crate) fn enter(&mut self, i: usize) -> bool {
        if self.depth >= MAX_REFERENCE_DEPTH {
            self.deep.get_or_insert(i);
            return false;
        }
        self.depth += 1;
        true
    }
}

/// The deepest a chain of tokens naming one another may go (the checker's
/// own bound, beside a value's bytes and nesting): each name followed is a
/// level of recursion, so a longer one is refused before it would overflow
/// the stack (`tamber-reference-depth`). Past the importers' own 256 links,
/// so every chain they write folds.
pub const MAX_REFERENCE_DEPTH: usize = 512;

/// Every value, name and word a design holds, past `css::over_limit` or a
/// name's bytes: each refused where it is written, before aliasing, kind
/// inference or any reader sees one (a token's, `when` line's, dial's,
/// range end's, icon's, param's, component param's, metadata's, axis's or
/// export's). The work is the design's text, once.
pub(crate) fn preflight(design: &Design) -> Vec<Diagnostic> {
    use crate::css::{over_bytes, over_limit};
    let mut out = Vec::new();
    let mut said = |over: Option<(&'static str, String)>, at: (u32, u32)| {
        if let Some((id, why)) = over {
            out.push(Diagnostic::new(id, why, at.0, at.1));
        }
    };
    let pos = |a: &At<String>| (a.line, a.col);
    let meta = |k: &str, v: &str| over_bytes("a metadata word", &format!("{k}={v}"));
    said(over_bytes("a name", &design.name), (1, 1));
    for t in design
        .tokens
        .iter()
        .chain(&design.icons)
        .chain(&design.params)
    {
        said(over_bytes("a name", &t.name.value), pos(&t.name));
        if let Some(k) = &t.kind {
            said(over_bytes("a kind", k), pos(&t.name));
        }
        said(over_limit(&t.value.value), pos(&t.value));
        for w in &t.whens {
            said(over_limit(&w.value.value), pos(&w.value));
            for (a, v) in &w.conds {
                said(over_bytes("a name", a), pos(&w.value));
                said(over_bytes("a name", v), pos(&w.value));
            }
        }
        for (k, v) in &t.meta {
            said(meta(k, v), pos(&t.value));
        }
    }
    for d in &design.dials {
        said(over_bytes("a name", &d.name.value), pos(&d.name));
        said(over_limit(&d.value.value), pos(&d.value));
        if let Some((lo, hi)) = &d.range {
            said(over_limit(lo), pos(&d.value));
            said(over_limit(hi), pos(&d.value));
        }
        for (k, v) in &d.meta {
            said(meta(k, v), pos(&d.value));
        }
    }
    for c in &design.components {
        said(over_bytes("a name", &c.name.value), pos(&c.name));
        for p in &c.params {
            said(over_bytes("a name", &p.name.value), pos(&p.name));
            said(over_bytes("a kind", &p.kind), pos(&p.name));
            said(over_limit(&p.value.value), pos(&p.value));
            if let Some((lo, hi)) = &p.range {
                said(over_limit(lo), pos(&p.value));
                said(over_limit(hi), pos(&p.value));
            }
            for (k, v) in &p.meta {
                said(meta(k, v), pos(&p.value));
            }
        }
    }
    for a in &design.axes {
        said(over_bytes("a name", &a.name.value), pos(&a.name));
        for v in &a.values {
            said(over_bytes("a name", v), pos(&a.name));
        }
    }
    for e in design.exports.iter().flatten() {
        said(over_bytes("a name", &e.value), pos(e));
    }
    out
}

/// A chain of names past [`MAX_REFERENCE_DEPTH`], the functions its
/// values nest counted with it: its id and words.
pub(crate) fn too_deep() -> (&'static str, String) {
    (
        "tamber-reference-depth",
        format!("a chain of tokens naming one another, with the functions their values nest, is at most {MAX_REFERENCE_DEPTH} deep"),
    )
}

thread_local! {
    /// The folds on this thread's stack now: each token followed
    /// (`Checker::value`) and each function folded (`measure::func`,
    /// `fold::func`), which compose (Astra's and Grok's round 2: 512 tokens
    /// each nesting 32 functions inside both bounds overflowed a 2 MB stack).
    static DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One level of the fold held while it is alive, or none past
/// [`MAX_REFERENCE_DEPTH`] (the caller refuses with [`too_deep`]).
pub(crate) struct Deeper(());

impl Drop for Deeper {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// A level more of the fold, if the bound has one left.
pub(crate) fn deeper() -> Option<Deeper> {
    DEPTH.with(|d| {
        (d.get() < MAX_REFERENCE_DEPTH).then(|| {
            d.set(d.get() + 1);
            Deeper(())
        })
    })
}

//! Re-import's dials (LLP 1121 D14; §5 stage 10b's gate): a token whose
//! every changed context sits on a line that is one colour dial alone, moved
//! to one literal, and whose every context on such a line moved alike, is
//! that dial's change, every token value it moves listed per tuple, so
//! accepting it is seen to reach past the one variable. A changed value whose
//! line followed dials or tokens by an expression is a literal override,
//! marked with what it no longer follows, so a dial never goes inert unseen.
//! Inverting an expression (a lightened accent back to the accent) is not
//! attempted: it stays a marked override.

use crate::edit::Op;
use crate::syntax::{DialKind, Token};
use crate::{Folding, Value, Vocab};

/// A dial the import moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialChange {
    /// The dial.
    pub dial: String,
    /// Its new value, CSS text.
    pub value: String,
    /// The tokens the file changed whose lines are the dial alone.
    pub from: Vec<String>,
    /// Every token value this change moves, beside the import's others.
    pub impact: Vec<Impact>,
}

/// One token's value for one tuple, before and after a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impact {
    /// The token.
    pub token: String,
    /// The tuple, `axis=value, …` (`default` with no axes).
    pub tuple: String,
    /// Its value without the change, as the plan carries it.
    pub before: String,
    /// Its value with it.
    pub after: String,
}

/// A literal the import writes over a line that followed others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// The token.
    pub token: String,
    /// The line's conditions; none is the default.
    pub when: Vec<(String, String)>,
    /// The dials and tokens it no longer follows there.
    pub follows: Vec<String>,
}

/// A token re-import would move by its dial: the token, the dial, the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    token: String,
    dial: String,
    value: String,
}

/// The token's literal operations, for when its dial cannot take the change,
/// or the line and why they are refused too.
pub(crate) type Literal = Result<Vec<Op>, (String, String)>;

/// Two colours' texts the same colour (both parse, as candidates' do).
fn alike(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
        || exact_color::parse(a).is_some_and(|x| exact_color::parse(b) == Some(x))
}

/// The dial a changed token moves by, if it does: every context whose
/// winning line (`branch`) is one colour dial's name alone changed, all to
/// one literal (`want`), one dial, and no other context changed.
pub(crate) fn candidate(
    cur: &Folding,
    decl: &Token,
    written: &str,
    tuples: &[(Vec<(String, String)>, [String; 2])],
    changed: &dyn Fn(&str) -> bool,
    want: &dyn Fn(&str) -> Option<String>,
    branch: &dyn Fn(&Vec<(String, String)>, bool) -> Option<usize>,
) -> Option<Candidate> {
    let dial_of = |b: Option<usize>| {
        let text = b
            .map_or(&decl.value.value, |w| &decl.whens[w].value.value)
            .trim();
        cur.design
            .dials
            .iter()
            .find(|d| d.name.value == text && d.kind == DialKind::Color)
            .map(|d| d.name.value.clone())
    };
    let mut found: Option<(String, String)> = None;
    for (modes, sides) in tuples {
        for dark in [false, true] {
            let context = &sides[usize::from(dark)];
            if !changed(context) {
                continue;
            }
            let (dial, value) = (dial_of(branch(modes, dark))?, want(context)?);
            if value.starts_with("light-dark(") || exact_color::parse(&value).is_none() {
                return None;
            }
            match &found {
                Some((d, v)) if *d != dial || !alike(v, &value) => return None,
                Some(_) => {}
                None => found = Some((dial, value)),
            }
        }
    }
    // Every context on a line of that dial moved, to that value: a context
    // the dial reaches that the file left alone is no change of the dial's.
    let (dial, value) = found?;
    for (modes, sides) in tuples {
        for dark in [false, true] {
            let context = &sides[usize::from(dark)];
            if dial_of(branch(modes, dark)).as_deref() == Some(dial.as_str())
                && !(changed(context) && want(context).is_some_and(|w| alike(&w, &value)))
            {
                return None;
            }
        }
    }
    Some(Candidate {
        token: written.to_string(),
        dial,
        value,
    })
}

/// What re-import's dial candidates come to.
pub(crate) struct Dialed {
    /// The dials' operations, first in the transaction.
    pub ops: Vec<Op>,
    /// The changes proposed.
    pub changes: Vec<DialChange>,
    /// The tokens whose dial already has the value: mapped, nothing to do.
    pub kept: usize,
    /// The tokens reported: the token, the line, why.
    pub refused: Vec<(String, String, String)>,
    /// The tokens whose dial cannot take their change, by their literals:
    /// pinned overrides, marked, as any changed folded value is.
    pub fallback: Vec<(String, Vec<Op>)>,
}

impl Dialed {
    /// A candidate its dial cannot take (`why`): its literals, else reported.
    fn fall(&mut self, token: String, literal: Literal, why: String) {
        match literal {
            Ok(ops) => self.fallback.push((token, ops)),
            Err((block, w)) => {
                self.refused
                    .push((token, block, format!("{why}; as a literal, {w}")))
            }
        }
    }
}

/// The candidates, by dial: one value a dial, already the dial's (mapped,
/// no operation), or a set the design takes alone and together. A dial its
/// tokens would set to different values, or the design refuses, moves not:
/// each token's literals are its change, pinned and marked.
pub(crate) fn dials(
    cur: &Folding,
    current: &str,
    candidates: Vec<(Candidate, Literal)>,
    vocab: &Vocab,
) -> Dialed {
    let mut out = Dialed {
        ops: Vec::new(),
        changes: Vec::new(),
        kept: 0,
        refused: Vec::new(),
        fallback: Vec::new(),
    };
    let mut by: Vec<(String, Vec<(Candidate, Literal)>)> = Vec::new();
    for c in candidates {
        match by.iter_mut().find(|(d, _)| *d == c.0.dial) {
            Some((_, all)) => all.push(c),
            None => by.push((c.0.dial.clone(), vec![c])),
        }
    }
    let mut held: Vec<Vec<(Candidate, Literal)>> = Vec::new();
    for (dial, all) in by {
        let tokens: Vec<String> = all.iter().map(|c| c.0.token.clone()).collect();
        let value = all[0].0.value.clone();
        if all.iter().any(|c| !alike(&c.0.value, &value)) {
            let values: Vec<String> = all
                .iter()
                .map(|c| format!("{} {}", c.0.token, c.0.value))
                .collect();
            let why = format!("tokens that follow the dial `{dial}` moved to different values ({}), and one dial takes one value", values.join(", "));
            for (c, literal) in all {
                out.fall(c.token, literal, why.clone());
            }
            continue;
        }
        let now = cur
            .design
            .dials
            .iter()
            .find(|d| d.name.value == dial)
            .map(|d| d.value.value.trim().to_string());
        if now.as_deref() == Some(value.as_str()) {
            out.kept += tokens.len();
            continue;
        }
        let op = Op::SetDial {
            name: dial.clone(),
            value: value.clone(),
        };
        match crate::apply_all(current, std::slice::from_ref(&op), vocab) {
            // A spelling of the value the dial has: nothing to do.
            Ok(edited)
                if crate::check(&edited.text, vocab).is_ok_and(|f| f.tokens == cur.tokens) =>
            {
                out.kept += tokens.len();
            }
            Ok(_) => {
                out.ops.push(op);
                out.changes.push(DialChange {
                    dial,
                    value,
                    from: tokens,
                    impact: Vec::new(),
                });
                held.push(all);
            }
            Err(diags) => {
                let first = diags
                    .first()
                    .map_or(String::new(), |d| format!("{}: {}", d.id, d.message));
                let why = format!("its dial `{dial}` set to {value} is refused ({first})");
                for (c, literal) in all {
                    out.fall(c.token, literal, why.clone());
                }
            }
        }
    }
    if !out.ops.is_empty() && crate::apply_all(current, &out.ops, vocab).is_err() {
        out.changes.clear();
        out.ops.clear();
        for (c, literal) in held.into_iter().flatten() {
            out.fall(
                c.token,
                literal,
                "the dials it moves are refused together".into(),
            );
        }
    }
    out
}

/// The pins `ops` (the transaction as it stands) write, and each dial
/// change's impact: what it moves beside the transaction's other operations.
pub(crate) fn marks(
    cur: &Folding,
    current: &str,
    ops: &[Op],
    changes: &mut [DialChange],
    vocab: &Vocab,
    under: &[&str],
    branch: &dyn Fn(&Token, &[&str], &Vec<(String, String)>, bool) -> Option<usize>,
) -> Vec<Pin> {
    let mut pins = Vec::new();
    for op in ops {
        let Op::SetToken {
            name, when, value, ..
        } = op
        else {
            continue;
        };
        let Some((checked, decl)) = cur
            .tokens
            .iter()
            .zip(&cur.design.tokens)
            .find(|(_, t)| t.name.value == *name)
        else {
            continue;
        };
        // The lines that won where it writes, side by side: its own line,
        // or for a new `when`, each tuple and scheme it holds in, the line
        // that won there before it.
        let text = |b: Option<usize>| b.map_or(&decl.value.value, |w| &decl.whens[w].value.value);
        let old: Vec<(&String, usize)> = match decl.whens.iter().position(|w| w.conds == *when) {
            Some(w) => vec![(text(Some(w)), 0), (text(Some(w)), 1)],
            None => (0..cur.tokens.first().map_or(1, |t| t.values.len()))
                .map(|k| modes_of(cur, k))
                .filter(|modes| when.iter().all(|c| modes.contains(c)))
                .flat_map(|modes| {
                    [false, true]
                        .map(|dark| (text(branch(decl, under, &modes, dark)), usize::from(dark)))
                })
                .collect(),
        };
        // What it followed there and no longer does: a side the literal
        // keeps (`light-dark(accent, #f00)`) follows as before.
        let new = sides(value);
        let follows: Vec<String> = checked
            .dials
            .iter()
            .chain(&checked.names)
            .filter(|n| {
                old.iter()
                    .any(|(text, i)| names(&sides(text)[*i], n) && !names(&new[*i], n))
            })
            .cloned()
            .collect();
        if !follows.is_empty() {
            pins.push(Pin {
                token: name.clone(),
                when: when.clone(),
                follows,
            });
        }
    }
    let fold = |ops: &[Op]| {
        crate::apply_all(current, ops, vocab)
            .ok()
            .and_then(|e| crate::check(&e.text, vocab).ok())
    };
    // Each dial alone against the design as it is: what accepting it moves,
    // a sibling pin in the same import masking none of it.
    for change in changes.iter_mut() {
        let set = Op::SetDial {
            name: change.dial.clone(),
            value: change.value.clone(),
        };
        let Some(after) = fold(std::slice::from_ref(&set)) else {
            continue;
        };
        for t in &cur.tokens {
            let Some(moved) = after.tokens.iter().find(|a| a.name == t.name) else {
                continue;
            };
            for (k, (b, a)) in t.values.iter().zip(&moved.values).enumerate() {
                if b != a {
                    change.impact.push(Impact {
                        token: t.name.clone(),
                        tuple: label(cur, k),
                        before: css(b),
                        after: css(a),
                    });
                }
            }
        }
    }
    pins
}

/// The marks `ops` make on the design as it is (`current`): each dial they
/// set, with every token value it moves, and the pins they write. A held
/// re-import recomputed (files moved since, or some of its items) is marked
/// again so, never by the design it was planned on.
pub fn marks_of(current: &str, ops: &[Op], vocab: &Vocab) -> Option<(Vec<DialChange>, Vec<Pin>)> {
    let cur = crate::check(current, vocab).ok()?;
    let mut changes: Vec<DialChange> = ops
        .iter()
        .filter_map(|op| match op {
            Op::SetDial { name, value } => Some(DialChange {
                dial: name.clone(),
                value: value.clone(),
                from: Vec::new(),
                impact: Vec::new(),
            }),
            _ => None,
        })
        .collect();
    let under: Vec<&str> = cur
        .design
        .axes
        .iter()
        .filter(|a| a.under_dark)
        .map(|a| a.name.value.as_str())
        .collect();
    let pins = marks(
        &cur,
        current,
        ops,
        &mut changes,
        vocab,
        &under,
        &crate::export::branch,
    );
    Some((changes, pins))
}

/// A value's light and dark sides as written: a top-level `light-dark(a,
/// b)`'s, else the whole value on both.
fn sides(text: &str) -> [String; 2] {
    let t = text.trim();
    if let Some(inner) = t
        .strip_prefix("light-dark(")
        .and_then(|r| r.strip_suffix(')'))
    {
        let mut depth = 0;
        for (i, c) in inner.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    return [
                        inner[..i].trim().to_string(),
                        inner[i + 1..].trim().to_string(),
                    ]
                }
                _ => {}
            }
        }
    }
    [t.to_string(), t.to_string()]
}

/// Whether `text` names `name` as a whole word (a dial or token reference),
/// in any case, as names are read.
fn names(text: &str, name: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_';
    let (text, name) = (text.to_ascii_lowercase(), name.to_ascii_lowercase());
    text.match_indices(&name).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(word)
            && !text[at + name.len()..].chars().next().is_some_and(word)
    })
}

/// Tuple `k`'s modes, as the design orders its axes.
fn modes_of(f: &Folding, mut k: usize) -> Vec<(String, String)> {
    let mut modes = Vec::new();
    for axis in f.design.axes.iter().rev() {
        let n = axis.values.len().max(1);
        modes.push((axis.name.value.clone(), axis.values[k % n].to_string()));
        k /= n;
    }
    modes.reverse();
    modes
}

/// Tuple `k`'s name, `axis=value, …` (`default` with no axes).
fn label(f: &Folding, k: usize) -> String {
    let parts: Vec<String> = modes_of(f, k)
        .iter()
        .map(|(a, v)| format!("{a}={v}"))
        .collect();
    if parts.is_empty() {
        "default".into()
    } else {
        parts.join(", ")
    }
}

fn css(v: &Value) -> String {
    v.css()
}

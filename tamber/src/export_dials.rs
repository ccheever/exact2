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
use std::collections::HashMap;

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

impl Candidate {
    /// The token, as the design spells it (what `dials` asks a literal by).
    pub(crate) fn token(&self) -> &str {
        &self.token
    }
}

/// The token's literal operations, for when its dial cannot take the change,
/// or the line and why they are refused too.
pub(crate) type Literal = Result<Vec<Op>, (String, String)>;

/// A candidate's literal, settled only when its dial falls back (theme2's
/// ruling on the counts' round 2: the dial decided first): its operations, a
/// refusal of them (said beside the dial's), or a report that leaves it out
/// of its dial's change (said as the token's own, as before).
pub(crate) enum Fallback {
    Literal(Literal),
    Report(String, String),
}

/// A tuple: every axis and its value.
type Modes = Vec<(String, String)>;
/// Which line of a token wins in a tuple.
type Branch<'a> = &'a dyn Fn(&Modes, bool) -> Option<usize>;
/// The same, for the token named and the contexts it is read under.
type TokenBranch<'a> = &'a dyn Fn(&Token, &[&str], &Modes, bool) -> Option<usize>;

/// Two colours' texts the same colour, to a reprint's rounding (`#ff0000`,
/// `red`, `color(srgb 1 0 0)`), as re-import compares values.
fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
        || matches!((read_css(a), read_css(b)), (Some(x), Some(y)) if alike(&x, &y))
}

/// A CSS colour as the colour it reads as.
pub(crate) fn read_css(text: &str) -> Option<exact_color::Wide> {
    match exact_color::parse(text)? {
        exact_color::Parsed::Legacy(c) => Some(crate::Half::legacy(c).wide()),
        exact_color::Parsed::Wide(w) => Some(w),
        _ => None,
    }
}

/// Two colours the same, to a reprint's rounding: a millionth in linear
/// sRGB, well inside an 8-bit step.
pub(crate) fn alike(x: &exact_color::Wide, y: &exact_color::Wide) -> bool {
    let (p, q) = (x.linear_srgb(), y.linear_srgb());
    p.iter().zip(&q).all(|(m, n)| (m - n).abs() < 1e-6) && (x.alpha - y.alpha).abs() < 1e-6
}

/// The dial a changed token moves by, if it does: every context whose
/// winning line (`branch`) is one colour dial's name alone changed, all to
/// one literal (`want`), one dial, and no other context changed.
pub(crate) fn candidate(
    cur: &Folding,
    decl: &Token,
    written: &str,
    tuples: &[(Modes, [String; 2])],
    changed: &dyn Fn(&str) -> bool,
    want: &dyn Fn(&str) -> Option<String>,
    branch: Branch,
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
                Some((d, v)) if *d != dial || !same(v, &value) => return None,
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
                && !(changed(context) && want(context).is_some_and(|w| same(&w, &value)))
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
    /// The tokens whose dial fell back and whose literal lines can't be made:
    /// reported as the token's own (the token, the line, why).
    pub reported: Vec<(String, String, String)>,
}

impl Dialed {
    /// A candidate its dial cannot take (`why`): its literals, else reported.
    fn fall(&mut self, token: String, literal: Fallback, why: String) {
        match literal {
            Fallback::Literal(Ok(ops)) => self.fallback.push((token, ops)),
            Fallback::Literal(Err((block, w))) => {
                self.refused
                    .push((token, block, format!("{why}; as a literal, {w}")))
            }
            Fallback::Report(block, w) => self.reported.push((token, block, w)),
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
    candidates: Vec<Candidate>,
    literal: &mut dyn FnMut(&str) -> Fallback,
    vocab: &Vocab,
) -> Dialed {
    let mut out = Dialed {
        ops: Vec::new(),
        changes: Vec::new(),
        kept: 0,
        refused: Vec::new(),
        fallback: Vec::new(),
        reported: Vec::new(),
    };
    // (each a candidate and nothing of its literal, which is settled only if
    // its dial falls back: the dial's decision reads no literal)
    let candidates: Vec<(Candidate, ())> = candidates.into_iter().map(|c| (c, ())).collect();
    let mut by: Vec<(String, Vec<(Candidate, ())>)> = Vec::new();
    for c in candidates {
        match by.iter_mut().find(|(d, _)| *d == c.0.dial) {
            Some((_, all)) => all.push(c),
            None => by.push((c.0.dial.clone(), vec![c])),
        }
    }
    let mut held: Vec<Vec<(Candidate, ())>> = Vec::new();
    for (dial, all) in by {
        // Past the re-import's budget (a literal's settle spent it), no more
        // dials are checked: the re-import is refused.
        if crate::budget::spent().is_some() {
            return out;
        }
        let tokens: Vec<String> = all.iter().map(|c| c.0.token.clone()).collect();
        let value = all[0].0.value.clone();
        if all.iter().any(|c| !same(&c.0.value, &value)) {
            let values: Vec<String> = all
                .iter()
                .map(|c| format!("{} {}", c.0.token, c.0.value))
                .collect();
            let why = format!("tokens that follow the dial `{dial}` moved to different values ({}), and one dial takes one value", values.join(", "));
            for (c, ()) in all {
                let l = literal(&c.token);
                out.fall(c.token, l, why.clone());
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
                for (c, ()) in all {
                    let l = literal(&c.token);
                    out.fall(c.token, l, why.clone());
                }
            }
        }
    }
    if crate::budget::spent().is_some() {
        return out;
    }
    if !out.ops.is_empty() && crate::apply_all(current, &out.ops, vocab).is_err() {
        out.changes.clear();
        out.ops.clear();
        for (c, ()) in held.into_iter().flatten() {
            let l = literal(&c.token);
            out.fall(c.token, l, "the dials it moves are refused together".into());
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
    branch: TokenBranch,
) -> Vec<Pin> {
    let mut pins = Vec::new();
    // Each token by its name, then by its name in any ASCII case: the first
    // so named, read once for all the operations.
    let (mut exact, mut folded) = (HashMap::new(), HashMap::new());
    for (k, t) in cur.design.tokens.iter().enumerate().take(cur.tokens.len()) {
        exact.entry(t.name.value.as_str()).or_insert(k);
        folded.entry(t.name.value.to_ascii_lowercase()).or_insert(k);
    }
    for op in ops {
        let Op::SetToken {
            name, when, value, ..
        } = op
        else {
            continue;
        };
        let Some(k) = exact
            .get(name.as_str())
            .or_else(|| folded.get(&name.to_ascii_lowercase()))
        else {
            continue;
        };
        let (checked, decl) = (&cur.tokens[*k], &cur.design.tokens[*k]);
        // The lines that won where it writes, side by side: its own line
        // (the default for no `when`), or for a new `when`, each tuple and
        // scheme where it wins, the line that won there before it (not where
        // a narrower line still wins).
        let text = |b: Option<usize>| b.map_or(&decl.value.value, |w| &decl.whens[w].value.value);
        let own = match when.is_empty() {
            true => Some(None),
            false => decl.whens.iter().position(|w| w.conds == *when).map(Some),
        };
        let old: Vec<(&String, usize)> = match own {
            Some(w) => vec![(text(w), 0), (text(w), 1)],
            None => (0..cur.tokens.first().map_or(1, |t| t.values.len()))
                .map(|k| modes_of(cur, k))
                .filter(|modes| when.iter().all(|c| modes.contains(c)))
                .flat_map(|modes| {
                    [false, true].map(|dark| (branch(decl, under, &modes, dark), usize::from(dark)))
                })
                .filter(|(b, _)| b.is_none_or(|w| decl.whens[w].conds.len() <= when.len()))
                .map(|(b, i)| (text(b), i))
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
                // A role's row (`-exact-background`) is named bare too (`background`).
                let named = |text: &str| {
                    names(text, n)
                        || n.strip_prefix("-exact-")
                            .is_some_and(|bare| names(text, bare))
                };
                old.iter()
                    .any(|(text, i)| named(&sides(text)[*i]) && !named(&new[*i]))
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
        let mut by_name = HashMap::new();
        for a in &after.tokens {
            by_name.entry(&a.name).or_insert(a);
        }
        for t in &cur.tokens {
            let Some(moved) = by_name.get(&t.name) else {
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

/// Each context's tokens by name, the last of a name (the resolver's winner).
pub(crate) type Lasts<'a> = HashMap<&'a str, HashMap<&'a str, &'a serde_json::Value>>;
pub(crate) fn last_by_name(tokens: &crate::export::ByContext) -> Lasts<'_> {
    tokens
        .iter()
        .map(|(c, all)| {
            (
                c.as_str(),
                all.iter().map(|(n, t)| (n.as_str(), t)).collect(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{dials, Candidate, Fallback};
    use crate::Vocab;

    fn candidate(token: &str, value: &str) -> Candidate {
        Candidate {
            token: token.into(),
            dial: "c".into(),
            value: value.into(),
        }
    }

    /// The dial is decided on its candidates' values alone (theme2's
    /// conditions on the counts' round 2): a dial that takes the change asks
    /// for no literal, and a candidate whose literal can't be made swings
    /// nothing; when the dial falls back, each candidate's literal is asked
    /// for then, and one that can't be made is reported as its own.
    #[test]
    fn a_dial_is_decided_before_any_literal() {
        let current =
            "tamber t\n\ndials\n  c: color = #000000\n\ntheme\n  a = c\n  b = c\n  bad = c\n";
        let cur = crate::check(current, &Vocab::default()).unwrap();
        let mut asked = Vec::new();
        let mut never = |t: &str| -> Fallback {
            asked.push(t.to_string());
            Fallback::Report(String::new(), "unbuildable".into())
        };
        let alike = vec![
            candidate("a", "#ffffff"),
            candidate("b", "#ffffff"),
            candidate("bad", "#ffffff"),
        ];
        let taken = dials(&cur, current, alike, &mut never, &Vocab::default());
        assert_eq!((taken.ops.len(), asked.len()), (1, 0), "{asked:?}");
        // The same decision without the bad candidate.
        let without = vec![candidate("a", "#ffffff"), candidate("b", "#ffffff")];
        let mut none = |_: &str| -> Fallback { unreachable!("no literal is asked for") };
        assert_eq!(
            dials(&cur, current, without, &mut none, &Vocab::default()).ops,
            taken.ops
        );
        // Falling back (values differ): each asked for, the bad one reported.
        let differ = vec![
            candidate("a", "#ffffff"),
            candidate("b", "#ff0000"),
            candidate("bad", "#ffffff"),
        ];
        let mut literal = |t: &str| -> Fallback {
            match t {
                "bad" => Fallback::Report("light".into(), "unbuildable".into()),
                _ => Fallback::Literal(Ok(Vec::new())),
            }
        };
        let fell = dials(&cur, current, differ, &mut literal, &Vocab::default());
        assert!(fell.ops.is_empty());
        assert_eq!(
            fell.fallback
                .iter()
                .map(|(t, _)| t.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            fell.reported,
            [(
                "bad".to_string(),
                "light".to_string(),
                "unbuildable".to_string()
            )]
        );
    }
}

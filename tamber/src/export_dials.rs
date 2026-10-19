//! Re-import's dials (LLP 1121 D14; §5 stage 10b's gate): a value a file
//! changed where the design's line is one dial alone is offered as that
//! dial's change, every token it moves listed per tuple, so accepting it is
//! seen to reach past the one variable; a changed value whose line followed
//! dials or tokens by an expression is a literal override, marked with what
//! it no longer follows, so a dial never goes inert unseen. Inverting an
//! expression (a lightened accent back to the accent) is not attempted: it
//! stays a marked override.

use crate::edit::Op;
use crate::syntax::DialKind;
use crate::{Folding, Value, Vocab};

/// A dial the import moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialChange {
    /// The dial.
    pub dial: String,
    /// Its new value, CSS text.
    pub value: String,
    /// The tokens the file changed whose line is the dial alone.
    pub from: Vec<String>,
    /// Every token value the change moves.
    pub impact: Vec<Impact>,
}

/// One token's value for one tuple, before and after a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impact {
    /// The token.
    pub token: String,
    /// The tuple, `axis=value, …` (`default` with no axes).
    pub tuple: String,
    /// Its value now, as the plan carries it.
    pub before: String,
    /// Its value after the change.
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

/// The dial changes among `accepted` (each token's operations), taken out
/// of their tokens' operations: a token whose one operation sets its default
/// line, that line being a colour dial's name alone, to one value. Tokens
/// that would set one dial to different values keep their operations (and
/// are pinned). The dials' operations come first in the transaction; a set
/// the design refuses beside the rest is no dial change.
pub(crate) fn dials(
    cur: &Folding,
    current: &str,
    accepted: &mut [(String, Vec<Op>)],
    vocab: &Vocab,
) -> (Vec<Op>, Vec<DialChange>) {
    let mut found: Vec<(String, String, Vec<usize>)> = Vec::new();
    let mut split: Vec<String> = Vec::new();
    for (i, (_, ops)) in accepted.iter().enumerate() {
        let [Op::SetToken {
            name, when, value, ..
        }] = ops.as_slice()
        else {
            continue;
        };
        if !when.is_empty() {
            continue;
        }
        let Some(decl) = cur.design.tokens.iter().find(|t| t.name.value == *name) else {
            continue;
        };
        let line = decl.value.value.trim();
        // A dial takes a colour of its own: a literal, one in both schemes.
        if !cur
            .design
            .dials
            .iter()
            .any(|d| d.name.value == line && d.kind == DialKind::Color)
            || exact_color::parse(value).is_none()
        {
            continue;
        }
        match found.iter_mut().find(|(d, ..)| d == line) {
            Some((_, v, at)) if v == value => at.push(i),
            Some(_) => split.push(line.to_string()),
            None => found.push((line.to_string(), value.clone(), vec![i])),
        }
    }
    // Each set the design takes alone, then together.
    found.retain(|(d, v, _)| {
        !split.contains(d)
            && crate::apply_all(
                current,
                &[Op::SetDial {
                    name: d.clone(),
                    value: v.clone(),
                }],
                vocab,
            )
            .is_ok()
    });
    let ops: Vec<Op> = found
        .iter()
        .map(|(d, v, _)| Op::SetDial {
            name: d.clone(),
            value: v.clone(),
        })
        .collect();
    if ops.is_empty() || crate::apply_all(current, &ops, vocab).is_err() {
        return (Vec::new(), Vec::new());
    }
    let mut changes = Vec::new();
    for (dial, value, at) in found {
        let from = at
            .iter()
            .map(|&i| match &accepted[i].1[0] {
                Op::SetToken { name, .. } => name.clone(),
                _ => unreachable!("a dial's token sets its default"),
            })
            .collect();
        for &i in &at {
            accepted[i].1.clear();
        }
        changes.push(DialChange {
            dial,
            value,
            from,
            impact: Vec::new(),
        });
    }
    (ops, changes)
}

/// The pins `ops` (the transaction as it stands) write, and each dial
/// change's impact: every token value it moves, by tuple.
pub(crate) fn marks(
    cur: &Folding,
    current: &str,
    ops: &[Op],
    changes: &mut [DialChange],
    vocab: &Vocab,
) -> Vec<Pin> {
    let mut pins = Vec::new();
    for op in ops {
        let Op::SetToken { name, when, .. } = op else {
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
        // The line it writes over (a new `when`'s tuples followed the token's other lines).
        let replaced: Vec<&str> = match when.is_empty() {
            true => vec![decl.value.value.as_str()],
            false => match decl.whens.iter().find(|w| w.conds == *when) {
                Some(w) => vec![w.value.value.as_str()],
                None => std::iter::once(decl.value.value.as_str())
                    .chain(decl.whens.iter().map(|w| w.value.value.as_str()))
                    .collect(),
            },
        };
        let follows: Vec<String> = checked
            .dials
            .iter()
            .chain(&checked.names)
            .filter(|n| replaced.iter().any(|text| names(text, n)))
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
    if changes.is_empty() {
        return pins;
    }
    let Ok(edited) = crate::apply_all(current, ops, vocab) else {
        return pins;
    };
    let Ok(after) = crate::check(&edited.text, vocab) else {
        return pins;
    };
    for change in changes.iter_mut() {
        for t in cur.tokens.iter().filter(|t| t.dials.contains(&change.dial)) {
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

/// Whether `text` names `name` as a whole word (a dial or token reference).
fn names(text: &str, name: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_';
    text.match_indices(name).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(word)
            && !text[at + name.len()..].chars().next().is_some_and(word)
    })
}

/// Tuple `k`'s name, `axis=value, …`, as the design orders its axes.
fn label(f: &Folding, mut k: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    for axis in f.design.axes.iter().rev() {
        let n = axis.values.len().max(1);
        parts.push(format!("{}={}", axis.name.value, axis.values[k % n]));
        k /= n;
    }
    parts.reverse();
    if parts.is_empty() {
        "default".into()
    } else {
        parts.join(", ")
    }
}

fn css(v: &Value) -> String {
    v.css()
}

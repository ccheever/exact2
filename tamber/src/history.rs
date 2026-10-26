//! A design's history read as edits (LLP 1122 D9, T1): two versions of a
//! `.tamber` file compared by what they declare, so a commit reads as
//! "`radius.md` 8px → 12px", and a past commit's inverse planned against the
//! file as it is now.
//!
//! Versions are compared parsed, not checked: a version the checker refuses
//! still reads as edits. Values are compared as written (trimmed), so an
//! edit that only reformats a line is no edit.

use crate::edit::{meta_word, Op};
use crate::syntax::{conds_key, same_conds, Axis, Design, Dial, Token};
use std::collections::{HashMap, HashSet};

/// What an edit is to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    /// A token's default (`when` empty) or one `when` line.
    Token,
    /// A dial's value.
    Dial,
    /// An axis's values, as `a | b`, and `under scheme=dark` after two
    /// spaces when it has it.
    Axis,
    /// Anything else declared: a token's or dial's kind, range or metadata,
    /// the design's name, its exports. Read, never reverted.
    Other,
}

impl What {
    /// As the wire spells it.
    pub fn name(self) -> &'static str {
        match self {
            What::Token => "token",
            What::Dial => "dial",
            What::Axis => "axis",
            What::Other => "other",
        }
    }
}

/// One edit between two versions: a branch of a token, a dial, an axis, or
/// another declaration, with its text before and after (`None`: absent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// What it is to.
    pub what: What,
    /// The token, dial or axis; for [`What::Other`], what changed
    /// (`surface.card meta`, `exports`).
    pub name: String,
    /// The `when` line's conditions; none is the default.
    pub when: Vec<(String, String)>,
    /// The text before.
    pub before: Option<String>,
    /// The text after.
    pub after: Option<String>,
}

/// The edits from `old` to `new`; a version that is `None` (the file did not
/// exist) declares nothing. In the newer version's order, then what it
/// removed. A version that declares a name twice, or a token's `when` twice,
/// has no one reading by name: refused, naming it.
pub fn edits(old: Option<&Design>, new: Option<&Design>) -> Result<Vec<Edit>, String> {
    let empty = Design::default();
    let (a, b) = (old.unwrap_or(&empty), new.unwrap_or(&empty));
    for (d, which) in [(a, "the version before"), (b, "the version after")] {
        if let Some(why) = ambiguous(d) {
            return Err(format!("{which} {why}"));
        }
    }
    let mut out = Vec::new();
    let other =
        |out: &mut Vec<Edit>, name: String, before: Option<String>, after: Option<String>| {
            if before != after {
                out.push(Edit {
                    what: What::Other,
                    name,
                    when: Vec::new(),
                    before,
                    after,
                });
            }
        };
    let named = |d: &Design| (!d.name.is_empty()).then(|| d.name.clone());
    other(&mut out, "name".into(), named(a), named(b));
    for (axis, x, y) in union(&a.axes, &b.axes, |x| &x.name.value) {
        let (before, after) = (x.map(axis_text), y.map(axis_text));
        if before != after {
            out.push(Edit {
                what: What::Axis,
                name: axis.to_string(),
                when: Vec::new(),
                before,
                after,
            });
        }
    }
    for (dial, x, y) in union(&a.dials, &b.dials, |d| &d.name.value) {
        let value = |d: &Dial| d.value.value.trim().to_string();
        if x.map(value) != y.map(value) {
            out.push(Edit {
                what: What::Dial,
                name: dial.to_string(),
                when: Vec::new(),
                before: x.map(value),
                after: y.map(value),
            });
        }
        if let (Some(x), Some(y)) = (x, y) {
            other(
                &mut out,
                format!("{dial} declaration"),
                Some(dial_shape(x)),
                Some(dial_shape(y)),
            );
        }
    }
    for (token, x, y) in union(&a.tokens, &b.tokens, |t| &t.name.value) {
        for when in branches(x, y) {
            let (before, after) = (branch(x, &when), branch(y, &when));
            if before != after {
                out.push(Edit {
                    what: What::Token,
                    name: token.to_string(),
                    when,
                    before,
                    after,
                });
            }
        }
        if let (Some(x), Some(y)) = (x, y) {
            other(
                &mut out,
                format!("{token} declaration"),
                Some(token_shape(x)),
                Some(token_shape(y)),
            );
        }
    }
    let exports = |d: &Design| {
        d.exports.as_ref().map(|e| {
            e.iter()
                .map(|n| n.value.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
    };
    other(&mut out, "exports".into(), exports(a), exports(b));
    // An icon is no operation's to write (D10): a change to one is reported.
    for (icon, x, y) in union(&a.icons, &b.icons, |t| &t.name.value) {
        let value = |t: &crate::Token| t.value.value.trim().to_string();
        other(&mut out, format!("icon {icon}"), x.map(value), y.map(value));
    }
    Ok(out)
}

/// What a design declares twice, in words, if anything: the checker refuses
/// it, but a past version is read unchecked.
fn ambiguous(d: &Design) -> Option<String> {
    let twice = |names: Vec<&String>, what: &str| {
        let mut seen = HashSet::new();
        names
            .into_iter()
            .find(|n| !seen.insert(*n))
            .map(|n| format!("declares the {what} `{n}` twice"))
    };
    twice(d.axes.iter().map(|x| &x.name.value).collect(), "axis")
        .or_else(|| twice(d.dials.iter().map(|x| &x.name.value).collect(), "dial"))
        .or_else(|| twice(d.tokens.iter().map(|x| &x.name.value).collect(), "token"))
        .or_else(|| twice(d.icons.iter().map(|x| &x.name.value).collect(), "icon"))
        .or_else(|| {
            // A `when` naming an axis twice reads as no set of conditions.
            d.tokens.iter().find_map(|t| {
                let twice = t.whens.iter().find(|w| {
                    let mut axes = HashSet::new();
                    !w.conds.iter().all(|(a, _)| axes.insert(a))
                });
                twice.map(|_| format!("names an axis twice in one `when` of `{}`", t.name.value))
            })
        })
        .or_else(|| {
            d.tokens.iter().find_map(|t| {
                let mut seen = HashSet::new();
                t.whens
                    .iter()
                    .any(|w| !seen.insert(conds_key(&w.conds)))
                    .then(|| format!("gives the token `{}` one `when` twice", t.name.value))
            })
        })
}

/// Why one edit of a past commit cannot be reverted on the file as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// The commit's edit.
    pub edit: Edit,
    /// What the file has there now (`None`: absent).
    pub now: Option<String>,
    /// Why, in words.
    pub why: String,
}

/// A past commit's inverse, planned on the file as it is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Revert {
    /// The operations that put each edit's `before` back, in an order each
    /// applies in: axis values and tokens added first, removed last.
    pub ops: Vec<Op>,
    /// The commit's edits the file already has as they were before it.
    pub already: Vec<Edit>,
    /// The edits it cannot revert. Any at all and the revert is refused.
    pub conflicts: Vec<Conflict>,
}

/// The inverse of the commit that took a file from `old` to `new`, on `now`,
/// the file as it is. Each edit is put back where `now` still has the
/// commit's `after`; one `now` has as `before` already is left; anything
/// else conflicts, as does an edit no operation can invert (a dial or an
/// axis added or removed, a kind or metadata changed).
pub fn revert(old: &Design, new: &Design, now: &Design) -> Revert {
    let mut out = Revert::default();
    let (mut first, mut middle, mut last) = (Vec::new(), Vec::new(), Vec::new());
    let read = edits(Some(old), Some(new)).and_then(|all| match ambiguous(now) {
        Some(why) => Err(format!("the file now {why}")),
        None => Ok(all),
    });
    let all = match read {
        Ok(all) => all,
        Err(why) => {
            out.conflicts.push(Conflict {
                edit: Edit {
                    what: What::Other,
                    name: "declarations".into(),
                    when: Vec::new(),
                    before: None,
                    after: None,
                },
                now: None,
                why,
            });
            return out;
        }
    };
    // Each version's entries by name, made once (linear: the checker
    // bound's sweep).
    let new_axes = ByName::new(&new.axes, |x| &x.name.value);
    let new_tokens = ByName::new(&new.tokens, |x| &x.name.value);
    let now_axes = ByName::new(&now.axes, |x| &x.name.value);
    let now_dials = ByName::new(&now.dials, |x| &x.name.value);
    let now_tokens = ByName::new(&now.tokens, |x| &x.name.value);
    let old_axes = ByName::new(&old.axes, |x| &x.name.value);
    let old_dials = ByName::new(&old.dials, |x| &x.name.value);
    let old_tokens = ByName::new(&old.tokens, |x| &x.name.value);
    let old_at: HashMap<&str, usize> = old
        .tokens
        .iter()
        .enumerate()
        .rev()
        .map(|(i, t)| (t.name.value.as_str(), i))
        .collect();
    let old_next = next_present(&old.tokens, &now_tokens);
    for edit in all {
        let conflict = |out: &mut Revert, now: Option<String>, why: &str| {
            out.conflicts.push(Conflict {
                edit: edit.clone(),
                now,
                why: why.to_string(),
            })
        };
        match edit.what {
            What::Other => conflict(
                &mut out,
                None,
                "no operation changes a kind, a range, metadata, the design's name or its exports",
            ),
            What::Dial => {
                let current = now_dials
                    .get(&edit.name)
                    .map(|d| d.value.value.trim().to_string());
                match (&edit.before, &edit.after) {
                    (Some(before), Some(_)) if current == edit.after => middle.push(Op::SetDial {
                        name: edit.name.clone(),
                        value: before.clone(),
                    }),
                    (Some(_), Some(_)) if current == edit.before => out.already.push(edit.clone()),
                    (Some(_), Some(_)) => conflict(&mut out, current, "changed since"),
                    (None, Some(_)) if current.is_none() => out.already.push(edit.clone()),
                    // Removed by the commit and there again: as it was, or
                    // with another declaration, which no operation changes.
                    (Some(_), None)
                        if current == edit.before
                            && now_dials.get(&edit.name).map(dial_shape)
                                == old_dials.get(&edit.name).map(dial_shape) =>
                    {
                        out.already.push(edit.clone())
                    }
                    (Some(_), None) if current.is_some() => conflict(
                        &mut out,
                        current,
                        "a dial by that name is there again, with another declaration",
                    ),
                    _ => conflict(&mut out, current, "no operation adds or removes a dial"),
                }
            }
            What::Axis => {
                let current = now_axes.get(&edit.name);
                let (x, y) = (old_axes.get(&edit.name), new_axes.get(&edit.name));
                if current.map(axis_text) == edit.before {
                    out.already.push(edit.clone());
                    continue;
                }
                match (x, y, current) {
                    (Some(x), None, None) => first.push(Op::AddAxis {
                        name: edit.name.clone(),
                        values: x.values.clone(),
                        under_dark: x.under_dark,
                    }),
                    (None, Some(_), _) => conflict(
                        &mut out,
                        current.map(axis_text),
                        "no operation removes an axis",
                    ),
                    (Some(x), Some(y), Some(c)) if !same_order(&x.values, &y.values) => conflict(
                        &mut out,
                        Some(axis_text(c)),
                        "no operation reorders an axis's values (the first is its default)",
                    ),
                    (Some(x), Some(y), Some(c))
                        if x.under_dark == y.under_dark && c.under_dark == y.under_dark =>
                    {
                        // The values the commit left, in the order it left
                        // them, or the file reordered them since (a later
                        // default the revert would undo): refused.
                        if !same_order(&y.values, &c.values) {
                            conflict(
                                &mut out,
                                Some(axis_text(c)),
                                "its values have moved since; reverting would leave them in another order",
                            );
                            continue;
                        }
                        // Values the commit added go, where they still are;
                        // values it removed come back, before the next of
                        // the old ones the axis has now.
                        let gone: Vec<&String> = y
                            .values
                            .iter()
                            .filter(|v| !x.values.contains(v) && c.values.contains(v))
                            .collect();
                        let mut list: Vec<&String> =
                            c.values.iter().filter(|v| !gone.contains(v)).collect();
                        let mut back = Vec::new();
                        for (i, v) in x.values.iter().enumerate() {
                            if !y.values.contains(v) && !c.values.contains(v) {
                                let before =
                                    x.values[i + 1..].iter().find(|n| c.values.contains(n));
                                let at = before
                                    .and_then(|b| list.iter().position(|l| *l == b))
                                    .unwrap_or(list.len());
                                list.insert(at, v);
                                back.push(Op::AddAxisValue {
                                    axis: edit.name.clone(),
                                    value: v.clone(),
                                    before: before.cloned(),
                                });
                            }
                        }
                        // The old values in their old order (the first the
                        // default), or the file moved them since: refused.
                        let kept: Vec<&String> =
                            list.into_iter().filter(|v| x.values.contains(v)).collect();
                        if kept != x.values.iter().collect::<Vec<_>>() {
                            conflict(
                                &mut out,
                                Some(axis_text(c)),
                                "its values have moved since; reverting would leave them in another order",
                            );
                            continue;
                        }
                        first.extend(back);
                        last.extend(gone.into_iter().map(|v| Op::RemoveAxisValue {
                            axis: edit.name.clone(),
                            value: v.clone(),
                        }));
                    }
                    (Some(_), Some(_), Some(_)) => conflict(
                        &mut out,
                        current.map(axis_text),
                        "no operation changes an axis's `under`",
                    ),
                    _ => conflict(&mut out, current.map(axis_text), "changed since"),
                }
            }
            What::Token => {
                let (x, y) = (old_tokens.get(&edit.name), new_tokens.get(&edit.name));
                let current = now_tokens.get(&edit.name);
                match (x, y) {
                    // The commit removed the token: it comes back whole, once
                    // (its `when` lines are this one op's).
                    (Some(x), None) => {
                        if !edit.when.is_empty() {
                            continue;
                        }
                        match current {
                            None => first.push(Op::AddToken {
                                name: x.name.value.clone(),
                                kind: x.kind.clone(),
                                value: x.value.value.trim().to_string(),
                                meta: x.meta.iter().map(|(k, v)| meta_word(k, v)).collect(),
                                whens: x
                                    .whens
                                    .iter()
                                    .map(|w| {
                                        (w.conds.clone(), w.value.value.trim().to_string(), None)
                                    })
                                    .collect(),
                                comment: None,
                                before: old_at
                                    .get(edit.name.as_str())
                                    .and_then(|&i| old_next[i])
                                    .map(str::to_string),
                            }),
                            Some(c) if same_token(c, x) => out.already.push(edit.clone()),
                            Some(c) => conflict(
                                &mut out,
                                branch(Some(c), &edit.when),
                                "a token by that name is there again, with other values",
                            ),
                        }
                    }
                    // The commit added it: it goes, if it is as the commit
                    // left it.
                    (None, Some(y)) => {
                        if !edit.when.is_empty() {
                            continue;
                        }
                        match current {
                            None => out.already.push(edit.clone()),
                            Some(c) if same_token(c, y) => last.push(Op::RemoveToken {
                                name: edit.name.clone(),
                            }),
                            Some(c) => {
                                conflict(&mut out, branch(Some(c), &edit.when), "changed since")
                            }
                        }
                    }
                    _ => {
                        let Some(c) = current else {
                            conflict(&mut out, None, "the token has been removed since");
                            continue;
                        };
                        let now_value = branch(Some(c), &edit.when);
                        if now_value == edit.before {
                            out.already.push(edit.clone());
                        } else if now_value != edit.after {
                            conflict(&mut out, now_value, "changed since");
                        } else {
                            middle.push(match &edit.before {
                                Some(v) => Op::SetToken {
                                    name: edit.name.clone(),
                                    when: edit.when.clone(),
                                    value: v.clone(),
                                    at: None,
                                    comment: None,
                                },
                                None => Op::UnsetTokenWhen {
                                    name: edit.name.clone(),
                                    when: edit.when.clone(),
                                },
                            });
                        }
                    }
                }
            }
        }
    }
    out.ops = first.into_iter().chain(middle).chain(last).collect();
    out
}

/// Names in `a`'s order, then those only `b` has, each with its entry in
/// either (the first of the name), by maps (linear: the checker bound's
/// sweep).
type Paired<'a, T> = Vec<(&'a str, Option<&'a T>, Option<&'a T>)>;

fn union<'a, T>(a: &'a [T], b: &'a [T], name: impl Fn(&T) -> &String) -> Paired<'a, T> {
    let (x, y) = (ByName::new(a, &name), ByName::new(b, &name));
    let mut seen = HashSet::new();
    a.iter()
        .chain(b)
        .map(|e| name(e).as_str())
        .filter(|n| seen.insert(*n))
        .map(|n| (n, x.get(n), y.get(n)))
        .collect()
}

/// A list's entries by name, the first of each.
struct ByName<'a, T>(HashMap<&'a str, &'a T>);

impl<'a, T> ByName<'a, T> {
    fn new(all: &'a [T], name: impl Fn(&T) -> &String) -> Self {
        ByName(all.iter().rev().map(|e| (name(e).as_str(), e)).collect())
    }

    fn get(&self, n: &str) -> Option<&'a T> {
        self.0.get(n).copied()
    }
}

/// The default (no conditions) and every `when` either version has, in
/// order; conditions the same in any order are one branch.
fn branches(a: Option<&Token>, b: Option<&Token>) -> Vec<Vec<(String, String)>> {
    let mut out = vec![Vec::new()];
    let mut seen = HashSet::from([Vec::new()]);
    for w in a.into_iter().chain(b).flat_map(|t| &t.whens) {
        if seen.insert(conds_key(&w.conds)) {
            out.push(w.conds.clone());
        }
    }
    out
}

/// A branch's value, as written: the default when `when` is empty.
fn branch(t: Option<&Token>, when: &[(String, String)]) -> Option<String> {
    let t = t?;
    if when.is_empty() {
        return Some(t.value.value.trim().to_string());
    }
    t.whens
        .iter()
        .find(|w| same_conds(&w.conds, when))
        .map(|w| w.value.value.trim().to_string())
}

/// The same declaration: kind, metadata and the value on every branch.
fn same_token(a: &Token, b: &Token) -> bool {
    token_shape(a) == token_shape(b)
        && branches(Some(a), Some(b))
            .iter()
            .all(|w| branch(Some(a), w) == branch(Some(b), w))
}

/// Whether the values two lists share come in the same order.
fn same_order(a: &[String], b: &[String]) -> bool {
    let shared = |x: &[String], y: &[String]| -> Vec<String> {
        x.iter().filter(|v| y.contains(v)).cloned().collect()
    };
    shared(a, b) == shared(b, a)
}

/// For each of `old`'s tokens, the first after it that `now` has: where a
/// removed token goes back. One pass from the end.
fn next_present<'a>(old: &'a [Token], now: &ByName<'_, Token>) -> Vec<Option<&'a str>> {
    let mut out = vec![None; old.len()];
    let mut next = None;
    for (i, t) in old.iter().enumerate().rev() {
        out[i] = next;
        if now.get(&t.name.value).is_some() {
            next = Some(t.name.value.as_str());
        }
    }
    out
}

fn axis_text(a: &Axis) -> String {
    let values = a.values.join(" | ");
    match a.under_dark {
        true => format!("{values}  under scheme=dark"),
        false => values,
    }
}

/// What a dial declares besides its value.
fn dial_shape(d: &Dial) -> String {
    let mut s = d.kind.name().to_string();
    if let Some((lo, hi)) = &d.range {
        s.push_str(&format!("  in {lo}..{hi}"));
    }
    for (k, v) in &d.meta {
        s.push_str(&format!("  {}", meta_word(k, v)));
    }
    s
}

/// What a token declares besides its values.
fn token_shape(t: &Token) -> String {
    let mut s = t.kind.clone().unwrap_or_default();
    for (k, v) in &t.meta {
        s.push_str(&format!("  {}", meta_word(k, v)));
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apply_all, parse, Vocab};

    const V1: &str = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  surface = #080808\n  ink = light-dark(#000000, #ffffff)\n    when dark-style=dim: light-dark(#000000, #eeeeee)\n  edge = #101010\n";

    fn d(src: &str) -> Design {
        parse(src).unwrap()
    }

    /// An icon is no operation's to write: its change is reported (D10).
    #[test]
    fn an_icon_change_is_reported() {
        let v1 = format!("{V1}\nicons\n  icon.reply = icon(sf \"bubble.left\")\n");
        let v2 = v1.replace("bubble.left", "arrowshape.turn.up.left");
        let all = edits(Some(&d(&v1)), Some(&d(&v2))).unwrap();
        assert_eq!(all.len(), 1, "{all:?}");
        assert_eq!(all[0].what, What::Other);
        assert_eq!(all[0].name, "icon icon.reply");
        let twice = v1.replace("icons\n", "icons\n  icon.reply = icon(sf \"b\")\n");
        assert!(edits(Some(&d(&twice)), Some(&d(&v2))).is_err());
    }

    #[test]
    fn a_commit_reads_as_token_edits() {
        let v2 = V1
            .replace("#eeeeee", "#dddddd")
            .replace("  edge = #101010\n", "  edge = #202020\n  card = #303030\n");
        let all = edits(Some(&d(V1)), Some(&d(&v2))).unwrap();
        let dim = vec![("dark-style".to_string(), "dim".to_string())];
        assert_eq!(
            all,
            vec![
                Edit {
                    what: What::Token,
                    name: "ink".into(),
                    when: dim,
                    before: Some("light-dark(#000000, #eeeeee)".into()),
                    after: Some("light-dark(#000000, #dddddd)".into())
                },
                Edit {
                    what: What::Token,
                    name: "edge".into(),
                    when: vec![],
                    before: Some("#101010".into()),
                    after: Some("#202020".into())
                },
                Edit {
                    what: What::Token,
                    name: "card".into(),
                    when: vec![],
                    before: None,
                    after: Some("#303030".into())
                },
            ]
        );
        // A new file: everything it declares, from nothing.
        let fresh = edits(None, Some(&d(V1))).unwrap();
        assert!(fresh
            .iter()
            .any(|e| e.what == What::Axis && e.before.is_none()));
        assert_eq!(fresh.iter().filter(|e| e.what == What::Token).count(), 4);
        // Reformatting is no edit.
        assert!(edits(
            Some(&d(V1)),
            Some(&d(&V1.replace("= #101010", "=   #101010")))
        )
        .unwrap()
        .is_empty());
        // A version that declares a token twice has no reading by name.
        let twice = format!("{V1}  edge = #202020\n");
        assert!(edits(Some(&d(V1)), Some(&d(&twice)))
            .unwrap_err()
            .contains("`edge` twice"));
        assert!(revert(&d(V1), &d(&twice), &d(&twice)).conflicts[0]
            .why
            .contains("twice"));
    }

    #[test]
    fn a_reorder_or_a_later_declaration_change_is_refused_not_lost() {
        // The commit reordered an axis (its default changed) and set a token.
        let v2 = V1
            .replace("black | dim", "dim | black")
            .replace("#101010", "#202020");
        let r = revert(&d(V1), &d(&v2), &d(&v2));
        assert!(r
            .conflicts
            .iter()
            .any(|c| c.edit.what == What::Axis && c.why.contains("reorders")));
        // The commit added a token, and a later edit marked it: removing it
        // would lose that.
        let v2 = format!("{V1}  card = #303030\n");
        let later = format!("{V1}  card = #303030  scheme=fixed\n");
        let r = revert(&d(V1), &d(&v2), &d(&later));
        assert!(r.ops.is_empty());
        assert_eq!(r.conflicts[0].edit.name, "card");
        // A value the commit removed comes back in its place, unless the
        // values moved since.
        let three = V1.replace("black | dim", "black | dim | navy");
        let v2 = three.replace("black | dim | navy", "black | navy");
        let back = revert(&d(&three), &d(&v2), &d(&v2));
        let text = apply_all(&v2, &back.ops, &Vocab::default()).unwrap().text;
        assert!(text.contains("black | dim | navy"), "{text}");
        let moved = v2.replace("black | navy", "navy | black");
        assert!(revert(&d(&three), &d(&v2), &d(&moved)).conflicts[0]
            .why
            .contains("moved since"));
        // The commit added a value, and a later edit made it the default:
        // removing it would undo that.
        let later = three.replace("black | dim | navy", "navy | black | dim");
        let r = revert(&d(V1), &d(&three), &d(&later));
        assert!(r.ops.is_empty() && r.conflicts[0].why.contains("moved since"));
    }

    #[test]
    fn a_revert_puts_back_what_the_file_still_has() {
        let vocab = Vocab::default();
        let v2 = V1
            .replace("#eeeeee", "#dddddd")
            .replace("  edge = #101010\n", "  edge = #202020\n  card = #303030\n");
        // The file as the commit left it: every edit reverts, and the result
        // declares what the commit's parent did.
        let r = revert(&d(V1), &d(&v2), &d(&v2));
        assert!(r.conflicts.is_empty(), "{:?}", r.conflicts);
        let back = apply_all(&v2, &r.ops, &vocab).unwrap().text;
        assert!(
            edits(Some(&d(V1)), Some(&d(&back))).unwrap().is_empty(),
            "{back}"
        );
        // Moved since, elsewhere: recomputed on today's file, the later edit kept.
        let today = v2.replace("#080808", "#0a0a0a");
        let r = revert(&d(V1), &d(&v2), &d(&today));
        assert!(r.conflicts.is_empty());
        let back = apply_all(&today, &r.ops, &vocab).unwrap().text;
        assert!(
            back.contains("surface = #0a0a0a")
                && back.contains("edge = #101010")
                && !back.contains("card")
        );
        // Moved since, on an edit of the commit's: refused, naming it.
        let clash = v2.replace("#202020", "#2a2a2a");
        let r = revert(&d(V1), &d(&v2), &d(&clash));
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(r.conflicts[0].edit.name, "edge");
        assert_eq!(r.conflicts[0].now.as_deref(), Some("#2a2a2a"));
        // Already as it was: nothing to do for that edit.
        let r = revert(&d(V1), &d(&v2), &d(&v2.replace("#202020", "#101010")));
        assert!(r.already.iter().any(|e| e.name == "edge"));
    }

    #[test]
    fn removals_come_back_and_dials_added_cannot_go() {
        let vocab = Vocab::default();
        // A token removed with its `when`, and an axis value added.
        let v2 = V1
            .replace("  ink = light-dark(#000000, #ffffff)\n    when dark-style=dim: light-dark(#000000, #eeeeee)\n", "")
            .replace("black | dim", "black | dim | navy");
        let r = revert(&d(V1), &d(&v2), &d(&v2));
        assert!(r.conflicts.is_empty(), "{:?}", r.conflicts);
        let back = apply_all(&v2, &r.ops, &vocab).unwrap().text;
        assert!(
            edits(Some(&d(V1)), Some(&d(&back))).unwrap().is_empty(),
            "{back}"
        );
        // A dial the commit added has no inverse.
        let with = "tamber t\n\ndials\n  accent: color = #1083fe\n\ntheme\n  ink = accent\n";
        let r = revert(
            &d("tamber t\n\ntheme\n  ink = #000000\n"),
            &d(with),
            &d(with),
        );
        assert!(r.conflicts.iter().any(|c| c.edit.what == What::Dial));
        // A dial the commit removed, back with another declaration, is no
        // "already".
        let without = "tamber t\n\ntheme\n  ink = #111111\n";
        let back = "tamber t\n\ndials\n  accent: color = #1083fe  label=\"Other\"\n\ntheme\n  ink = accent\n";
        let r = revert(&d(with), &d(without), &d(back));
        assert!(r
            .conflicts
            .iter()
            .any(|c| c.edit.what == What::Dial && c.why.contains("another declaration")));
    }
}

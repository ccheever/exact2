//! An import checked against the design it lands in (LLP 1121 D14, after
//! stage 6 H): the tokens it adds or sets are laid into the design as
//! `reconcile` writes them (a set default, a set or added `when`, a `when` on
//! the importer's own axes it no longer gives taken out), and the design
//! folded. A token the design has of another kind is refused before that,
//! a colour too. Then each refusal the fold makes that the design did not
//! make before is charged to what caused it: a laid token's own line; a
//! design token's value (through the design tokens it reads) to the laid
//! token whose absence clears it; a dial left unused to the token that no
//! longer reads it; anything else to every token laid. A token naming a
//! refused one is refused with it. Until the design folds.

use super::{Mapped, Unmapped};
use crate::syntax::{At, Axis, Token, When};
use crate::{Design, Diagnostic, Vocab};
use std::collections::HashMap;

/// Where a laid token's lines are said to be: past any design's own.
const LAID: u32 = 10_000_000;

/// Whether `w`, a `when` of a design token the import sets, stays: one on
/// the importer's own axes alone that the source no longer gives is taken
/// back.
fn kept(w: &When, t: &Mapped, owned: &[&str]) -> bool {
    !w.conds.iter().all(|(a, _)| owned.contains(&a.as_str()))
        || t.whens.iter().any(|(c, _)| same(c, &w.conds))
}

fn same(a: &[(String, String)], b: &[(String, String)]) -> bool {
    a.len() == b.len() && a.iter().all(|c| b.contains(c))
}

/// `design` with each of `tokens` not in `skip` laid in as `reconcile`
/// writes it: its default and its own `when` lines on line [`LAID`] + its
/// index, the design's kept `when` lines at their own; and each axis of
/// `axes`.
fn laid(
    design: &Design,
    tokens: &[Mapped],
    skip: &[bool],
    axes: &[(String, Vec<String>)],
    owned: &[&str],
) -> Design {
    let mut out = design.clone();
    for (name, values) in axes {
        match out.axes.iter_mut().find(|a| a.name.value == *name) {
            Some(a) => {
                for v in values {
                    if !a.values.contains(v) {
                        a.values.push(v.clone());
                    }
                }
            }
            None => out.axes.push(Axis {
                name: At {
                    value: name.clone(),
                    line: LAID - 1,
                    col: 1,
                },
                values: values.clone(),
                under_dark: false,
            }),
        }
    }
    let index: HashMap<String, usize> = out
        .tokens
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name.value.clone(), i))
        .collect();
    for (i, t) in tokens.iter().enumerate().filter(|(i, _)| !skip[*i]) {
        let line = LAID + i as u32;
        let at = |value: &str| At {
            value: value.to_string(),
            line,
            col: 1,
        };
        let when = |conds: &[(String, String)], value: &str| When {
            conds: conds.to_vec(),
            value: at(value),
            line,
            col: 1,
        };
        match index.get(&t.name) {
            Some(&d) => {
                let d = &mut out.tokens[d];
                d.name.line = line;
                d.value = at(&t.value);
                d.whens.retain(|w| kept(w, t, owned));
                for (conds, value) in &t.whens {
                    match d.whens.iter_mut().find(|w| same(&w.conds, conds)) {
                        Some(w) => *w = when(conds, value),
                        None => d.whens.push(when(conds, value)),
                    }
                }
            }
            None => out.tokens.push(Token {
                name: at(&t.name),
                kind: t.kind.map(str::to_string),
                value: at(&t.value),
                whens: t.whens.iter().map(|(c, v)| when(c, v)).collect(),
                meta: Vec::new(),
            }),
        }
    }
    out
}

/// The names a value may hold: its words outside strings (a quoted family
/// name is text, not a reference).
fn words(value: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut quote, mut start) = (None, None);
    let name = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
    for (i, c) in value.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => {
                if let Some(s) = start.take() {
                    out.push(&value[s..i]);
                }
                quote = Some(c);
            }
            None if name(c) => {
                start.get_or_insert(i);
            }
            None => {
                if let Some(s) = start.take() {
                    out.push(&value[s..i]);
                }
            }
        }
    }
    if let (None, Some(s)) = (quote, start) {
        out.push(&value[s..]);
    }
    out
}

/// A refusal as compared across folds: its id, its line, its message.
type Seen = (&'static str, u32, String);

fn seen(d: &Diagnostic) -> Seen {
    (d.id, d.line, d.message.clone())
}

/// A token's values, its default and its `when` lines'.
fn values(t: &Token) -> impl Iterator<Item = &str> {
    std::iter::once(t.value.value.as_str()).chain(t.whens.iter().map(|w| w.value.value.as_str()))
}

/// Each design token's kind: as the design folds, or, where it does not,
/// as what does fold once the lines it refuses are left out, and as
/// written for those left out.
fn kinds(design: &Design, vocab: &Vocab) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = design
        .tokens
        .iter()
        .filter_map(|t| Some((t.name.value.clone(), t.kind.clone()?)))
        .collect();
    let mut probe = design.clone();
    for _ in 0..16 {
        match crate::check_design(probe.clone(), vocab) {
            Ok(f) => {
                for c in f.tokens {
                    out.insert(c.name.clone(), c.kind.name().to_string());
                }
                return out;
            }
            Err(all) => {
                let lines: Vec<u32> = all.iter().map(|d| d.line).collect();
                let before = (probe.tokens.len(), probe.dials.len());
                probe.tokens.retain(|t| {
                    !lines.contains(&t.name.line)
                        && !lines.contains(&t.value.line)
                        && !t.whens.iter().any(|w| lines.contains(&w.line))
                });
                probe.dials.retain(|d| !lines.contains(&d.name.line));
                if (probe.tokens.len(), probe.dials.len()) == before {
                    return out;
                }
            }
        }
    }
    out
}

/// Why each of `tokens` would not hold once the import lands in `design`
/// (`None` where it holds). `axes` the import adds; `owned` the axes whose
/// `when` lines are the importer's to take back. Refused whole when the
/// axes alone would leave the design unfolded.
pub(super) fn landing(
    tokens: &[Mapped],
    axes: &[(String, Vec<String>)],
    owned: &[&str],
    design: &Design,
    vocab: &Vocab,
) -> Result<Vec<Option<String>>, Diagnostic> {
    let mut why: Vec<Option<String>> = vec![None; tokens.len()];
    let have = kinds(design, vocab);
    for (t, why) in tokens.iter().zip(why.iter_mut()) {
        let kind = t.kind.unwrap_or("color");
        if let Some(have) = have.get(&t.name).filter(|h| *h != kind) {
            let what = if kind == "color" { "colour" } else { kind };
            *why = Some(format!(
                "the design's `{}` is a {have} token, not a {what}",
                t.name
            ));
            continue;
        }
        // A line the edit grammar takes (a name of identifiers, a value
        // with no unquoted `in`, metadata or comment).
        let op = crate::edit::Op::AddToken {
            name: t.name.clone(),
            kind: t.kind.map(str::to_string),
            value: t.value.clone(),
            meta: Vec::new(),
            whens: t
                .whens
                .iter()
                .map(|(c, v)| (c.clone(), v.clone(), None))
                .collect(),
            comment: None,
            before: None,
        };
        if let Err(all) = crate::edit::valid(&op) {
            *why = all.first().map(|d| d.message.clone());
        }
    }
    // What the design refused before the import is not the import's.
    let old: Vec<Seen> = crate::check_design(design.clone(), vocab)
        .err()
        .unwrap_or_default()
        .iter()
        .map(seen)
        .collect();
    let fold = |skip: &[bool]| -> Vec<Diagnostic> {
        match crate::check_design(laid(design, tokens, skip, axes, owned), vocab) {
            Ok(_) => Vec::new(),
            Err(all) => all
                .into_iter()
                .filter(|d| !old.contains(&seen(d)))
                .collect(),
        }
    };
    if let Some(d) = fold(&vec![true; tokens.len()]).first() {
        return Err(Diagnostic::new(
            "tamber-import-axis",
            format!(
                "the import's axes would leave the design unfolded: {}",
                d.message
            ),
            1,
            1,
        ));
    }
    // A name's text as laid: an import token's value and `when` lines with
    // the design's kept ones, or a design token's.
    let text = |n: &str, skip: &[bool]| -> Vec<String> {
        let found = design.tokens.iter().find(|d| d.name.value == n);
        match tokens
            .iter()
            .position(|t| t.name == n)
            .filter(|i| !skip[*i])
        {
            Some(i) => {
                let t = &tokens[i];
                std::iter::once(t.value.clone())
                    .chain(t.whens.iter().map(|(_, v)| v.clone()))
                    .chain(found.into_iter().flat_map(|d| {
                        d.whens
                            .iter()
                            .filter(|w| kept(w, t, owned))
                            .map(|w| w.value.value.clone())
                    }))
                    .collect()
            }
            None => found
                .map(|d| values(d).map(str::to_string).collect())
                .unwrap_or_default(),
        }
    };
    // Every name a value reads, through the tokens it names, as laid.
    let reads = |value: &str, skip: &[bool]| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut stack: Vec<String> = words(value).into_iter().map(str::to_string).collect();
        while let Some(n) = stack.pop() {
            if out.contains(&n) {
                continue;
            }
            for v in text(&n, skip) {
                stack.extend(words(&v).into_iter().map(str::to_string));
            }
            out.push(n);
        }
        out
    };
    for _ in 0..=tokens.len() + 1 {
        // A token naming a refused one goes with it (the reference would read
        // the design's token of that name instead).
        loop {
            let refused: Vec<&str> = tokens
                .iter()
                .zip(&why)
                .filter(|(_, w)| w.is_some())
                .map(|(t, _)| t.name.as_str())
                .collect();
            let named: Vec<(usize, String)> = tokens
                .iter()
                .enumerate()
                .filter(|(i, _)| why[*i].is_none())
                .filter_map(|(i, t)| {
                    std::iter::once(t.value.as_str())
                        .chain(t.whens.iter().map(|(_, v)| v.as_str()))
                        .flat_map(words)
                        .find(|w| refused.contains(w))
                        .map(|r| (i, r.to_string()))
                })
                .collect();
            if named.is_empty() {
                break;
            }
            for (i, r) in named {
                why[i] = Some(format!("it names `{r}`, which this import does not set"));
            }
        }
        let skip: Vec<bool> = why.iter().map(Option::is_some).collect();
        let errors = fold(&skip);
        if errors.is_empty() {
            return Ok(why);
        }
        let mut charged = false;
        for d in &errors {
            if d.line >= LAID {
                let i = (d.line - LAID) as usize;
                if i < tokens.len() && why[i].is_none() {
                    why[i] = Some(d.message.clone());
                    charged = true;
                }
                continue;
            }
            // A design token's line (its default, or a `when` of it): what
            // it and its default read of the laid tokens, the token itself if
            // laid, and of those the ones that cause it.
            let own = design.tokens.iter().find_map(|o| {
                if o.name.line == d.line || o.value.line == d.line {
                    Some((o, None))
                } else {
                    o.whens
                        .iter()
                        .find(|w| w.line == d.line)
                        .map(|w| (o, Some(w)))
                }
            });
            if let Some((own, branch)) = own {
                let laid_own = tokens
                    .iter()
                    .position(|t| t.name == own.name.value)
                    .filter(|i| !skip[*i]);
                let mut read: Vec<String> = match branch {
                    // A `when` is compared with its default (the scheme's
                    // sides, a kind): both are read.
                    Some(w) => std::iter::once(w.value.value.clone())
                        .chain(text(&own.name.value, &skip).into_iter().take(1))
                        .collect(),
                    None => text(&own.name.value, &skip),
                };
                if laid_own.is_none() && branch.is_some() {
                    read.push(own.value.value.clone());
                }
                let mut candidates: Vec<usize> = laid_own.into_iter().collect();
                for v in &read {
                    for r in reads(v, &skip) {
                        if let Some(i) = tokens.iter().position(|t| t.name == r) {
                            if !skip[i] && !candidates.contains(&i) {
                                candidates.push(i);
                            }
                        }
                    }
                }
                let clears = |without: &[usize]| {
                    let mut s = skip.clone();
                    for &c in without {
                        s[c] = true;
                    }
                    !fold(&s).iter().any(|e| e.id == d.id && e.line == d.line)
                };
                // Those whose absence alone clears it; else those that keep
                // it when they are all that is laid of them.
                let mut culprits: Vec<usize> = candidates
                    .iter()
                    .copied()
                    .filter(|&c| clears(&[c]))
                    .collect();
                if culprits.is_empty() {
                    culprits = candidates
                        .iter()
                        .copied()
                        .filter(|&c| {
                            let others: Vec<usize> =
                                candidates.iter().copied().filter(|&o| o != c).collect();
                            !clears(&others)
                        })
                        .collect();
                }
                if culprits.is_empty() {
                    culprits = candidates;
                }
                for i in culprits {
                    charged = true;
                    if why[i].is_none() {
                        why[i] = Some(format!(
                            "the design's `{}` would not fold once it lands: {}",
                            own.name.value, d.message
                        ));
                    }
                }
                continue;
            }
            // A dial no token reads once the import lands: the tokens that
            // read it before and no longer do.
            if let Some(dial) = design.dials.iter().find(|x| x.name.line == d.line) {
                for (i, t) in tokens.iter().enumerate() {
                    let read_it = design.tokens.iter().any(|o| {
                        o.name.value == t.name
                            && values(o).flat_map(words).any(|w| w == dial.name.value)
                    });
                    if why[i].is_none() && read_it {
                        why[i] = Some(format!(
                            "the design's dial `{}` would be left unused once it lands: {}",
                            dial.name.value, d.message
                        ));
                        charged = true;
                    }
                }
            }
        }
        // Nothing to charge a refusal to: every token laid goes with it.
        if !charged {
            let message = errors[0].message.clone();
            for w in why.iter_mut().filter(|w| w.is_none()) {
                *w = Some(format!(
                    "the design would not fold once the import lands: {message}"
                ));
            }
            return Ok(why);
        }
    }
    Ok(why)
}

/// The tokens of `tokens` that would not hold where they land
/// ([`landing`]), taken out; for each, its source values (`sources`, by
/// token name) reported with why. The number of source values so moved from
/// mapped to reported (a `(derived)` one is outside the count). The import
/// is refused whole when its axes alone would not hold.
pub(super) fn settle(
    tokens: &mut Vec<Mapped>,
    sources: &[(String, Vec<Unmapped>)],
    (axes, owned): (&[(String, Vec<String>)], &[&str]),
    design: &Design,
    vocab: &Vocab,
    unmapped: &mut Vec<Unmapped>,
) -> Result<usize, Vec<Diagnostic>> {
    let why = landing(tokens, axes, owned, design, vocab).map_err(|d| vec![d])?;
    let mut moved = 0;
    for (t, why) in tokens.iter().zip(&why) {
        let Some(why) = why else { continue };
        for (_, from) in sources.iter().filter(|(n, _)| *n == t.name) {
            for u in from {
                unmapped.push(Unmapped {
                    why: why.clone(),
                    token: Some(t.name.clone()),
                    ..u.clone()
                });
                // A derived value (shadcn's radii) is outside the count.
                if u.block != "(derived)" {
                    moved += 1;
                }
            }
        }
    }
    let mut at = 0;
    tokens.retain(|_| {
        at += 1;
        why[at - 1].is_none()
    });
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(name: &str, kind: Option<&'static str>, value: &str) -> Mapped {
        Mapped {
            name: name.into(),
            value: value.into(),
            whens: Vec::new(),
            kind,
        }
    }

    fn why(design: &str, tokens: &[Mapped], owned: &[&str]) -> Vec<Option<String>> {
        landing(
            tokens,
            &[],
            owned,
            &crate::parse(design).unwrap(),
            &Vocab::default(),
        )
        .unwrap()
    }

    #[test]
    fn a_token_that_would_not_hold_where_it_lands_says_why() {
        let w = why(
            "tamber app\n\ntheme\n  t.radius: length = 4px\n  bridge: length = t.radius\n  derived: length = calc(bridge + 1px)\n  t.gap: length = 8px\n  t.b: length = 1px\n  both: length = calc(t.radius + t.b)\n",
            &[
                token("t.radius", Some("length"), "1rem"),
                token("t.gap", None, "#ffffff"),
                token("t.new", Some("length"), "2px"),
                token("t.alias", None, "t.gap"),
                token("t.b", Some("length"), "2px"),
            ],
            &[],
        );
        // Through `bridge`; of the two `both` reads, the one that breaks it.
        assert!(
            w[0].as_deref().unwrap().starts_with("the design's `"),
            "{w:?}"
        );
        assert_eq!(
            w[1].as_deref(),
            Some("the design's `t.gap` is a length token, not a colour")
        );
        assert_eq!(w[2], None);
        assert_eq!(
            w[3].as_deref(),
            Some("it names `t.gap`, which this import does not set")
        );
        assert_eq!(w[4], None, "{w:?}");
    }

    #[test]
    fn a_when_the_importer_takes_back_or_keeps_is_laid_as_it_will_be() {
        let design = "tamber app\n\naxes\n  theme = default | ocean\n\ntheme\n  s.primary: length = 1rem\n    when theme=default: 4px\n    when theme=ocean: 8px\n  derived: length = calc(s.primary + 1px)\n";
        let w = why(
            design,
            &[token("s.primary", Some("length"), "1rem")],
            &["theme"],
        );
        assert!(
            w[0].is_some(),
            "its `when` lines taken back, 1rem is read: {w:?}"
        );
        let design = "tamber app\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  t.brand = #ffffff\n    when dark-style=dim: light-dark(#ffffff, #222222)\n";
        let w = why(design, &[token("t.brand", None, "#000000")], &[]);
        assert!(
            w[0].is_some(),
            "a kept `when` is charged to the token: {w:?}"
        );
        let design = "tamber app\n\ndials\n  brand: color = #ffffff\n\ntheme\n  t.ink = brand\n";
        let w = why(design, &[token("t.ink", None, "#000000")], &[]);
        assert!(w[0].as_deref().unwrap().contains("dial `brand`"), "{w:?}");
    }
}

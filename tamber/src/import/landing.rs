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

/// `design` with each of `tokens` not in `skip` laid in as `reconcile`
/// writes it, every line of a token it sets (its kept `when` lines too) on
/// line [`LAID`] + its index; and each axis of `axes`.
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
        let same = |a: &[(String, String)], b: &[(String, String)]| {
            a.len() == b.len() && a.iter().all(|c| b.contains(c))
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
                // A `when` on the importer's own axes alone that the source
                // no longer gives is taken back; the rest are kept.
                d.whens.retain(|w| {
                    !w.conds.iter().all(|(a, _)| owned.contains(&a.as_str()))
                        || t.whens.iter().any(|(c, _)| same(c, &w.conds))
                });
                for w in &mut d.whens {
                    w.line = line;
                    w.value.line = line;
                }
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

/// The names a value may hold.
fn words(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .filter(|w| !w.is_empty())
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

/// Why each of `tokens` would not hold once the import lands in `design`
/// (`None` where it holds). `axes` the import adds; `owned` the axes whose
/// `when` lines are the importer's to take back.
pub(super) fn landing(
    tokens: &[Mapped],
    axes: &[(String, Vec<String>)],
    owned: &[&str],
    design: &Design,
    vocab: &Vocab,
) -> Vec<Option<String>> {
    let mut why: Vec<Option<String>> = vec![None; tokens.len()];
    let before = crate::check_design(design.clone(), vocab);
    // Its kinds as it folds, else as written.
    for (t, why) in tokens.iter().zip(why.iter_mut()) {
        let kind = t.kind.unwrap_or("color");
        let have = match &before {
            Ok(f) => f
                .tokens
                .iter()
                .find(|c| c.name == t.name)
                .map(|c| c.kind.name().to_string()),
            Err(_) => design
                .tokens
                .iter()
                .find(|d| d.name.value == t.name)
                .and_then(|d| d.kind.clone()),
        };
        if let Some(have) = have.filter(|h| h != kind) {
            let what = if kind == "color" { "colour" } else { kind };
            *why = Some(format!(
                "the design's `{}` is a {have} token, not a {what}",
                t.name
            ));
        }
    }
    // What the design refused before the import is not the import's.
    let old: Vec<Seen> = before.err().unwrap_or_default().iter().map(seen).collect();
    let fold = |skip: &[bool]| -> Vec<Diagnostic> {
        match crate::check_design(laid(design, tokens, skip, axes, owned), vocab) {
            Ok(_) => Vec::new(),
            Err(all) => all
                .into_iter()
                .filter(|d| !old.contains(&seen(d)))
                .collect(),
        }
    };
    // Every name `name` reads, through the tokens it names, as laid.
    let reads = |name: &str, skip: &[bool]| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut stack = vec![name.to_string()];
        while let Some(n) = stack.pop() {
            let laid_at = tokens
                .iter()
                .position(|t| t.name == n)
                .filter(|i| !skip[*i]);
            let text: Vec<String> = match laid_at {
                Some(i) => std::iter::once(tokens[i].value.clone())
                    .chain(tokens[i].whens.iter().map(|(_, v)| v.clone()))
                    .collect(),
                None => design
                    .tokens
                    .iter()
                    .find(|d| d.name.value == n)
                    .map(|d| values(d).map(str::to_string).collect())
                    .unwrap_or_default(),
            };
            for w in text.iter().flat_map(|v| words(v)) {
                if !out.iter().any(|o| o == w) {
                    out.push(w.to_string());
                    stack.push(w.to_string());
                }
            }
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
            return why;
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
            // A design token's value: the laid tokens it reads, and of
            // those, the ones whose absence clears this refusal.
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
                let value = branch.map_or(own.value.value.as_str(), |w| w.value.value.as_str());
                let mut candidates: Vec<usize> = Vec::new();
                for w in words(value) {
                    for r in std::iter::once(w.to_string()).chain(reads(w, &skip)) {
                        if let Some(i) = tokens.iter().position(|t| t.name == r) {
                            if !skip[i] && !candidates.contains(&i) {
                                candidates.push(i);
                            }
                        }
                    }
                }
                let culprits: Vec<usize> = match candidates.len() {
                    0 | 1 => candidates.clone(),
                    _ => candidates
                        .iter()
                        .copied()
                        .filter(|&c| {
                            let mut without = skip.clone();
                            without[c] = true;
                            !fold(&without)
                                .iter()
                                .any(|e| e.id == d.id && e.line == d.line)
                        })
                        .collect(),
                };
                let culprits = if culprits.is_empty() {
                    candidates
                } else {
                    culprits
                };
                for i in culprits {
                    charged = true;
                    if why[i].is_some() {
                        continue;
                    }
                    why[i] = Some(format!(
                        "the design's `{}` reads it, and would not fold once it lands: {}",
                        own.name.value, d.message
                    ));
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
            return why;
        }
    }
    why
}

/// The tokens of `tokens` that would not hold where they land
/// ([`landing`]), taken out; for each, its source values (`sources`, by
/// token name) reported with why. The number of source values so moved from
/// mapped to reported (a `(derived)` one is outside the count).
pub(super) fn settle(
    tokens: &mut Vec<Mapped>,
    sources: &[(String, Vec<Unmapped>)],
    (axes, owned): (&[(String, Vec<String>)], &[&str]),
    design: &Design,
    vocab: &Vocab,
    unmapped: &mut Vec<Unmapped>,
) -> usize {
    let why = landing(tokens, axes, owned, design, vocab);
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
    moved
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

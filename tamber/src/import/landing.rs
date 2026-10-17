//! An import checked against the design it lands in (LLP 1121 D14, after
//! stage 6 H): each token the import adds or sets is laid into the design as
//! `reconcile` would write it, and the design folded. A token the design has
//! of another kind is refused before that (a colour too), and so is one whose
//! landing leaves a token of the design that reads it unfolded.

use super::Mapped;
use crate::syntax::{At, Axis, Token, When};
use crate::{Design, Vocab};

/// Where a laid token's lines are said to be: past any design's own.
const LAID: u32 = 10_000_000;

/// `design` with each of `tokens` (by index, `skip` left out) and each axis
/// of `axes` laid in: an added token, a set default, a set `when` line, each
/// on the line [`LAID`] + its index.
fn laid(
    design: &Design,
    tokens: &[Mapped],
    skip: &[bool],
    axes: &[(String, Vec<String>)],
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
        match out.tokens.iter_mut().find(|d| d.name.value == t.name) {
            Some(d) => {
                d.name.line = line;
                d.value = at(&t.value);
                for (conds, value) in &t.whens {
                    let same = |w: &&mut When| {
                        w.conds.len() == conds.len() && w.conds.iter().all(|c| conds.contains(c))
                    };
                    match d.whens.iter_mut().find(same) {
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

/// Why each of `tokens` would not hold once the import lands in `design`
/// (`None` where it holds): the design's token of that name is of another
/// kind (as it folds against `vocab`, else as written), or, laid in with
/// the rest, it or a design token that reads it does not fold. A design that
/// does not fold before the import is checked for kinds alone.
pub(super) fn landing(
    tokens: &[Mapped],
    axes: &[(String, Vec<String>)],
    design: &Design,
    vocab: &Vocab,
) -> Vec<Option<String>> {
    let mut why: Vec<Option<String>> = vec![None; tokens.len()];
    let before = crate::check_design(design.clone(), vocab).ok();
    for (t, why) in tokens.iter().zip(why.iter_mut()) {
        let kind = t.kind.unwrap_or("color");
        let have = match &before {
            Some(f) => f
                .tokens
                .iter()
                .find(|c| c.name == t.name)
                .map(|c| c.kind.name().to_string()),
            None => design
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
    if before.is_none() {
        return why;
    }
    // Laid in, the ones not refused yet, again without each it refuses.
    for _ in 0..8 {
        let skip: Vec<bool> = why.iter().map(Option::is_some).collect();
        let Err(all) = crate::check_design(laid(design, tokens, &skip, axes), vocab) else {
            return why;
        };
        let mut refused = false;
        for d in &all {
            if d.line >= LAID {
                let i = (d.line - LAID) as usize;
                if i < tokens.len() && why[i].is_none() {
                    why[i] = Some(d.message.clone());
                    refused = true;
                }
                continue;
            }
            // A design token's line: what it reads of the import.
            let Some(own) = design.tokens.iter().find(|o| {
                o.name.line == d.line
                    || o.value.line == d.line
                    || o.whens.iter().any(|w| w.line == d.line)
            }) else {
                continue;
            };
            let read: Vec<&str> = std::iter::once(own.value.value.as_str())
                .chain(own.whens.iter().map(|w| w.value.value.as_str()))
                .flat_map(words)
                .collect();
            for (i, t) in tokens.iter().enumerate() {
                if why[i].is_none() && read.contains(&t.name.as_str()) {
                    why[i] = Some(format!(
                        "the design's `{}` reads it, and would not fold once it lands: {}",
                        own.name.value, d.message
                    ));
                    refused = true;
                }
            }
        }
        if !refused {
            return why;
        }
    }
    why
}

/// The colour tokens of `tokens` that would not hold where they land
/// ([`landing`]), taken out: each one's name and why.
pub(super) fn colours_refused(
    tokens: &mut Vec<Mapped>,
    axes: &[(String, Vec<String>)],
    design: &Design,
    vocab: &Vocab,
) -> Vec<(String, String)> {
    let why = landing(tokens, axes, design, vocab);
    let refused: Vec<(String, String)> = tokens
        .iter()
        .zip(why)
        .filter(|(t, _)| t.kind.is_none())
        .filter_map(|(t, why)| why.map(|w| (t.name.clone(), w)))
        .collect();
    tokens.retain(|t| !refused.iter().any(|(n, _)| *n == t.name));
    refused
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

    #[test]
    fn a_token_that_would_not_hold_where_it_lands_says_why() {
        let design = crate::parse(
            "tamber app\n\ntheme\n  t.radius: length = 4px\n  derived: length = calc(t.radius + 1px)\n  t.gap: length = 8px\n",
        )
        .unwrap();
        let tokens = [
            token("t.radius", Some("length"), "1rem"),
            token("t.gap", None, "#ffffff"),
            token("t.new", Some("length"), "2px"),
        ];
        let why = landing(&tokens, &[], &design, &Vocab::default());
        assert!(
            why[0]
                .as_deref()
                .unwrap()
                .starts_with("the design's `derived` reads it"),
            "{why:?}"
        );
        assert_eq!(
            why[1].as_deref(),
            Some("the design's `t.gap` is a length token, not a colour")
        );
        assert_eq!(why[2], None);
    }
}

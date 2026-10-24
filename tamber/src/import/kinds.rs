//! The kinds besides colour an importer proposes (LLP 1121 D14, stage 6 H):
//! lengths, numbers, type and families, shadows, durations and easings. A
//! source's value is its token's when it folds as its kind beside the
//! import's other tokens, checked as one design (which also says why one
//! does not); a `var(--x)` in it is the token `--x` becomes, named bare
//! (D3). A value of these kinds has no dark side: only a colour's
//! `light-dark()` has one.

use super::Mapped;
use crate::Vocab;
use std::collections::{HashMap, HashSet};

/// The kind `add-token` writes for an importer's kind (a ring is a box
/// shadow, `inset` in its value).
pub(super) fn written(kind: &str) -> &'static str {
    match kind {
        "length" => "length",
        "number" => "number",
        "type" => "type",
        "family" => "family",
        "text-shadow" => "text-shadow",
        "duration" => "duration",
        "easing" => "easing",
        _ => "box-shadow",
    }
}

/// Why a value of these kinds that differs between the scheme's sides is
/// none.
pub(super) const SIDED: &str =
    "differs between light and dark, and only a colour has a dark side (`light-dark()`)";

/// `value` on one line with each `var(--x)` written as the token `token`
/// gives `--x`, or why it cannot be. A string (a quoted family name) is kept
/// as written: its spaces, and any `var(` in it, are its text.
pub(super) fn bare(value: &str, token: impl Fn(&str) -> Option<String>) -> Result<String, String> {
    let chars: Vec<char> = value.trim().chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' || c == '\'' {
            // The string to its close, escapes and all.
            out.push(c);
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == '\\' && i + 1 < chars.len() {
                    out.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                i += 1;
                if chars[i - 1] == c {
                    break;
                }
            }
            continue;
        }
        if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(' ');
            continue;
        }
        if chars[i..].starts_with(&['v', 'a', 'r', '(']) {
            let mut depth = 1;
            let mut j = i + 4;
            while j < chars.len() && depth > 0 {
                match chars[j] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            if depth > 0 {
                return Err(format!("`{}` has a `var(` it does not close", value.trim()));
            }
            let var: String = chars[i + 4..j - 1].iter().collect();
            let var = var.trim();
            if var.contains(',') {
                return Err(format!(
                    "`var({var})` has a fallback, which a token never needs"
                ));
            }
            match token(var) {
                Some(name) => out.push_str(&name),
                None => return Err(format!("`{var}` makes no token for it to name")),
            }
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    Ok(out)
}

/// The variables `value`'s `var()`s name, outside its strings.
pub(super) fn vars_in(value: &str) -> Vec<String> {
    let names = std::cell::RefCell::new(Vec::new());
    let _ = bare(value, |var| {
        names.borrow_mut().push(var.to_string());
        Some(String::new())
    });
    names.into_inner()
}

/// `text` folded alone as a `kind` value, as the plan writes it
/// (`calc(1 / 0.75)` as a number is `1.3333333333333333`), or why not.
pub(super) fn folded(kind: &str, text: &str) -> Result<String, String> {
    let design = format!("tamber probe\n\ntheme\n  probe: {kind} = {text}\n");
    match crate::check(&design, &Vocab::default()) {
        Ok(f) => f.tokens[0]
            .values
            .first()
            .map(crate::Value::css)
            .ok_or_else(|| format!("`{text}` has no value")),
        Err(all) => Err(all
            .first()
            .map_or_else(|| format!("`{text}` does not fold"), |d| d.message.clone())),
    }
}

/// [`unfolded`] for the tokens with a kind, then, for those and every
/// colour, why it would not hold where it lands (`landing`): one why a
/// token.
pub(super) fn refused(
    tokens: &[Mapped],
    (axes, owned): (&[(String, Vec<String>)], &[&str]),
    design: &crate::Design,
    vocab: &Vocab,
) -> Result<Vec<Option<String>>, crate::Diagnostic> {
    let mut why = unfolded(tokens, axes, design, vocab);
    // (each of `unfolded`'s probes charged as it was built: one the budget ran out in refuses the import)
    crate::budget::spend(1).map_err(|m| crate::Diagnostic::new("tamber-import-source", m, 1, 1))?;
    let kept: Vec<usize> = (0..tokens.len()).filter(|&i| why[i].is_none()).collect();
    let laid: Vec<Mapped> = kept.iter().map(|&i| tokens[i].clone()).collect();
    let landed = super::landing::landing(&laid, axes, owned, design, vocab)?;
    for (i, w) in kept.into_iter().zip(landed) {
        why[i] = w;
    }
    Ok(why)
}

/// The names a value may hold.
fn words(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .filter(|w| !w.is_empty())
}

/// Why each of `tokens` with a kind does not fold beside the others (`None`
/// where it does, and for a colour): one design of them, with the colours
/// they name and `axes` their `when` lines name, checked again without the
/// ones it refuses until it folds; and one `design` has of another kind.
pub(super) fn unfolded(
    tokens: &[Mapped],
    axes: &[(String, Vec<String>)],
    design: &crate::Design,
    vocab: &Vocab,
) -> Vec<Option<String>> {
    let mut why: Vec<Option<String>> = vec![None; tokens.len()];
    for (t, why) in tokens.iter().zip(why.iter_mut()) {
        let Some(kind) = t.kind else { continue };
        // A value the edit grammar takes on its own line (no unquoted `in`,
        // no `key=value` last word, no comment).
        let op = crate::edit::Op::AddToken {
            name: t.name.clone(),
            kind: Some(kind.to_string()),
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
    // A token the design has with no kind written, whose value this changes:
    // `set-token` keeps the line's kind unsaid, so the new value says it.
    let mut design_named: HashMap<&str, Vec<&crate::syntax::Token>> = HashMap::new();
    for d in &design.tokens {
        design_named
            .entry(d.name.value.as_str())
            .or_default()
            .push(d);
    }
    let untyped: Vec<usize> = (0..tokens.len())
        .filter(|&i| {
            tokens[i].kind.is_some()
                && design_named.get(tokens[i].name.as_str()).is_some_and(|ds| {
                    ds.iter().any(|d| {
                        d.kind.is_none()
                            && super::squash(&d.value.value) != super::squash(&tokens[i].value)
                    })
                })
        })
        .collect();
    let untyped_set: HashSet<usize> = untyped.iter().copied().collect();
    // Each colour token by name, the first so named.
    let mut colour_at: HashMap<&str, usize> = HashMap::new();
    for (j, t) in tokens.iter().enumerate().filter(|(_, t)| t.kind.is_none()) {
        colour_at.entry(t.name.as_str()).or_insert(j);
    }
    let named = |i: usize| {
        let t = &tokens[i];
        std::iter::once(t.value.as_str())
            .chain(t.whens.iter().map(|(_, v)| v.as_str()))
            .flat_map(words)
            .collect::<Vec<_>>()
    };
    // The colours they name, through the colours those name.
    let mut colours: HashSet<usize> = HashSet::new();
    let mut seen: Vec<usize> = (0..tokens.len())
        .filter(|&i| tokens[i].kind.is_some())
        .collect();
    while let Some(i) = seen.pop() {
        for w in named(i) {
            if let Some(&j) = colour_at.get(w) {
                if colours.insert(j) {
                    seen.push(j);
                }
            }
        }
    }
    let mut out: HashSet<usize> = HashSet::new();
    loop {
        let included: Vec<usize> = (0..tokens.len())
            .filter(|&i| {
                (tokens[i].kind.is_some() && why[i].is_none() || colours.contains(&i))
                    && !out.contains(&i)
            })
            .collect();
        if !included.iter().any(|&i| tokens[i].kind.is_some()) {
            return why;
        }
        let used: Vec<&(String, Vec<String>)> = axes
            .iter()
            .filter(|(a, _)| {
                included.iter().any(|&i| {
                    tokens[i]
                        .whens
                        .iter()
                        .any(|(c, _)| c.iter().any(|(x, _)| x == a))
                })
            })
            .collect();
        let mut text = String::from("tamber probe\n\n");
        if !used.is_empty() {
            text.push_str("axes\n");
            for (a, vs) in &used {
                text.push_str(&format!("  {a} = {}\n", vs.join(" | ")));
            }
            text.push('\n');
        }
        text.push_str("theme\n");
        // Each token's first and last line, counted as the text grows.
        let mut lines: Vec<(u32, u32, usize)> = Vec::new();
        let mut count = text.lines().count() as u32;
        let push = |text: &mut String, count: &mut u32, line: String| {
            *count += line.lines().count() as u32;
            text.push_str(&line);
        };
        for &i in &included {
            let t = &tokens[i];
            let start = count + 1;
            match t.kind.filter(|_| !untyped_set.contains(&i)) {
                Some(k) => push(
                    &mut text,
                    &mut count,
                    format!("  {}: {k} = {}\n", t.name, t.value),
                ),
                None => push(
                    &mut text,
                    &mut count,
                    format!("  {} = {}\n", t.name, t.value),
                ),
            }
            for (conds, v) in &t.whens {
                let c: Vec<String> = conds.iter().map(|(a, v)| format!("{a}={v}")).collect();
                push(
                    &mut text,
                    &mut count,
                    format!("    when {}: {v}\n", c.join(" ")),
                );
            }
            lines.push((start, count, i));
        }
        // (a probe a pass, each of them all the tokens left: charged by its text; past the budget, stopped)
        if crate::budget::spend(text.len() as u64).is_err() {
            return why;
        }
        let all = match crate::check(&text, vocab) {
            Ok(f) => {
                let mut kind_of: HashMap<&str, &'static str> = HashMap::new();
                for c in &f.tokens {
                    kind_of.entry(c.name.as_str()).or_insert(c.kind.name());
                }
                for &i in &untyped {
                    let (t, kind) = (&tokens[i], tokens[i].kind.unwrap_or_default());
                    let inferred = kind_of.get(t.name.as_str()).copied();
                    if let Some(other) = inferred.filter(|k| *k != kind) {
                        why[i] = Some(format!("the design's `{}` takes its kind from its value, and `{}` would make it a {other}, not a {kind}", t.name, t.value));
                    }
                }
                return why;
            }
            Err(all) => all,
        };
        let mut refused = false;
        for d in &all {
            // The token whose lines hold it (they run in order, end to end).
            let at = lines.partition_point(|(_, b, _)| *b < d.line);
            match lines
                .get(at)
                .filter(|(a, b, _)| (*a..=*b).contains(&d.line))
            {
                Some(&(_, _, i)) if tokens[i].kind.is_some() => {
                    if why[i].is_none() {
                        why[i] = Some(d.message.chars().take(300).collect());
                        refused = true;
                    }
                }
                // A colour they name that does not fold beside them is left
                // out, and what names it says so.
                Some(&(_, _, i)) if !out.contains(&i) => {
                    out.insert(i);
                    refused = true;
                }
                _ => {}
            }
        }
        // Nothing to point at: the rest share the design's refusal.
        if !refused {
            // (said in every token's why: bounded, one diagnostic no multiplier of them)
            let message: String = all
                .first()
                .map_or_else(String::new, |d| d.message.chars().take(300).collect());
            for &i in included.iter().filter(|&&i| tokens[i].kind.is_some()) {
                why[i] = Some(message.clone());
            }
            return why;
        }
    }
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
    fn each_folds_beside_the_others_or_says_why() {
        let tokens = [
            token("t.radius", Some("length"), "0.5rem"),
            token("t.radius.lg", Some("length"), "t.radius"),
            token("t.radius.sm", Some("length"), "calc(t.radius - 4px)"),
            token("t.ink", None, "#000"),
            token("t.shadow", Some("box-shadow"), "0 1px 2px t.ink"),
            token("t.gap", Some("length"), "12%"),
        ];
        let why = unfolded(
            &tokens,
            &[],
            &crate::parse("tamber t\n").unwrap(),
            &Vocab::default(),
        );
        assert_eq!(why[0], None);
        assert_eq!(why[1], None);
        assert!(why[2].as_deref().unwrap().contains("only px"), "{why:?}");
        assert_eq!(why[3], None);
        assert_eq!(why[4], None, "{why:?}");
        assert!(why[5].is_some());
        assert_eq!(
            bare("calc(var(--radius)  - 4px)", |v| (v == "--radius")
                .then(|| "t.radius".into())),
            Ok("calc(t.radius - 4px)".into())
        );
        assert!(bare("var(--x, 1px)", |_| None).is_err());
        assert_eq!(
            bare("\"A  var(--x)\",   serif", |_| None),
            Ok("\"A  var(--x)\", serif".into())
        );
    }
}

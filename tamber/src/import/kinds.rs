//! The kinds besides colour an importer proposes (LLP 1121 D14, stage 6 H):
//! lengths, numbers, type and families, shadows, durations and easings. A
//! source's value is its token's when it folds as its kind beside the
//! import's other tokens, checked as one design (which also says why one
//! does not); a `var(--x)` in it is the token `--x` becomes, named bare
//! (D3). A value of these kinds has no dark side: only a colour's
//! `light-dark()` has one.

use super::Mapped;
use crate::Vocab;

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
/// gives `--x`, or why it cannot be.
pub(super) fn bare(value: &str, token: impl Fn(&str) -> Option<String>) -> Result<String, String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    let mut rest = value.as_str();
    while let Some(at) = rest.find("var(") {
        out.push_str(&rest[..at]);
        let inner = &rest[at + 4..];
        let mut depth = 1;
        let close = inner.char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        });
        let Some(close) = close else {
            return Err(format!("`{value}` has a `var(` it does not close"));
        };
        let var = inner[..close].trim();
        if var.contains(',') {
            return Err(format!(
                "`var({var})` has a fallback, which a token never needs"
            ));
        }
        match token(var) {
            Some(name) => out.push_str(&name),
            None => return Err(format!("`{var}` makes no token for it to name")),
        }
        rest = &inner[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
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

/// The names a value may hold.
fn words(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .filter(|w| !w.is_empty())
}

/// Why each of `tokens` with a kind does not fold beside the others (`None`
/// where it does, and for a colour): one design of them, with the colours
/// they name and `axes` their `when` lines name, checked again without the
/// ones it refuses until it folds.
pub(super) fn unfolded(
    tokens: &[Mapped],
    axes: &[(String, Vec<String>)],
    vocab: &Vocab,
) -> Vec<Option<String>> {
    let mut why: Vec<Option<String>> = vec![None; tokens.len()];
    let named = |i: usize| {
        let t = &tokens[i];
        std::iter::once(t.value.as_str())
            .chain(t.whens.iter().map(|(_, v)| v.as_str()))
            .flat_map(words)
            .collect::<Vec<_>>()
    };
    // The colours they name, through the colours those name.
    let mut colours: Vec<usize> = Vec::new();
    let mut seen: Vec<usize> = (0..tokens.len())
        .filter(|&i| tokens[i].kind.is_some())
        .collect();
    while let Some(i) = seen.pop() {
        for w in named(i) {
            if let Some(j) = tokens.iter().position(|t| t.kind.is_none() && t.name == w) {
                if !colours.contains(&j) {
                    colours.push(j);
                    seen.push(j);
                }
            }
        }
    }
    let mut out: Vec<usize> = Vec::new();
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
        let mut lines: Vec<(u32, u32, usize)> = Vec::new();
        for &i in &included {
            let t = &tokens[i];
            let start = text.lines().count() as u32 + 1;
            match t.kind {
                Some(k) => text.push_str(&format!("  {}: {k} = {}\n", t.name, t.value)),
                None => text.push_str(&format!("  {} = {}\n", t.name, t.value)),
            }
            for (conds, v) in &t.whens {
                let c: Vec<String> = conds.iter().map(|(a, v)| format!("{a}={v}")).collect();
                text.push_str(&format!("    when {}: {v}\n", c.join(" ")));
            }
            lines.push((start, text.lines().count() as u32, i));
        }
        let Err(all) = crate::check(&text, vocab) else {
            return why;
        };
        let mut refused = false;
        for d in &all {
            match lines.iter().find(|(a, b, _)| (*a..=*b).contains(&d.line)) {
                Some(&(_, _, i)) if tokens[i].kind.is_some() => {
                    if why[i].is_none() {
                        why[i] = Some(d.message.clone());
                        refused = true;
                    }
                }
                // A colour they name that does not fold beside them is left
                // out, and what names it says so.
                Some(&(_, _, i)) if !out.contains(&i) => {
                    out.push(i);
                    refused = true;
                }
                _ => {}
            }
        }
        // Nothing to point at: the rest share the design's refusal.
        if !refused {
            let message = all.first().map_or_else(String::new, |d| d.message.clone());
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
        let why = unfolded(&tokens, &[], &Vocab::default());
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
    }
}

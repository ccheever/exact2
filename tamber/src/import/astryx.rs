//! Astryx (LLP 1121 D14, importer 4; exact1 LLP 0267): a theme package's
//! tokens resolved to light and dark values (`scripts/astryx-tokens.mjs`
//! runs `resolveThemeTokens()` on the pinned package), mapped by name. It is
//! also a probe of Tamber's token kinds: its unmapped report is the
//! evidence for new kinds, each gap a §9 question or a later kind.
//!
//! - `--color-background-*` → `surface.*` (`-exact-background` offered for
//!   `body`, `AccentColor` for `--color-accent`), `--color-text-*` →
//!   `text.*`, `--color-icon-*` → `icon.*`,
//!   `--color-border*` → `border.*`, `--color-overlay*` → `overlay.*`,
//!   `--color-syntax-*` → `syntax.*`, `--color-data-*` → `data.*`, the rest
//!   of `--color-*` (accent, status colours with their muted and on
//!   variants) whole; light and dark values one `light-dark()`.
//! - Spacing, radii, sizes, borders and focus widths are lengths; shadows
//!   (rings as `inset`) box shadows; the three durations durations; easings
//!   easings; families families, font sizes lengths, weights numbers, and a
//!   text style's size, weight and leading one type token (stage 6 H). One
//!   whose sides differ or that does not fold is reported.
//! - A duration's `-min` and `-max` tiers are reported: Tamber has one value
//!   per duration token.
//! - The theme's own tokens (`localTokens`, which `resolveThemeTokens()`
//!   leaves out), a colour each as written (often a `light-dark()`), go under
//!   `local.*`.

use super::{
    colour_literal, counted, reconcile, split_roles, tamber_name, Import, Mapped, Offer, Unmapped,
};
use crate::edit::Op;
use crate::{Design, Diagnostic, Vocab};
use serde_json::Value;

/// What Tamber makes of an Astryx token.
enum Kind {
    /// A colour token, by these segments.
    Colour(Vec<String>),
    /// Another kind, and the token's segments.
    Other(&'static str, Vec<String>),
    /// Not mapped: why.
    Reported(String),
}

/// `blue-5` → `blue`, `5`; `primary` → `primary`.
fn stepped(group: &str, rest: &str) -> Vec<String> {
    let mut out = vec![group.to_string()];
    match rest.rsplit_once('-') {
        Some((name, step)) if !name.is_empty() && step.chars().all(|c| c.is_ascii_digit()) => {
            out.extend(name.split('-').map(str::to_string));
            out.push(step.to_string());
        }
        _ => out.extend(rest.split('-').map(str::to_string)),
    }
    out
}

fn kind(var: &str) -> Kind {
    let one = |group: &str, rest: &str| vec![group.to_string(), rest.to_string()];
    if let Some(c) = var.strip_prefix("--color-") {
        for (prefix, group) in [
            ("background-", "surface"),
            ("text-", "text"),
            ("icon-", "icon"),
            ("syntax-", "syntax"),
        ] {
            if let Some(rest) = c.strip_prefix(prefix) {
                return Kind::Colour(one(group, rest));
            }
        }
        if let Some(rest) = c.strip_prefix("data-") {
            return Kind::Colour(stepped("data", rest));
        }
        for group in ["border", "overlay"] {
            if c == group {
                return Kind::Colour(vec![group.to_string()]);
            }
            if let Some(rest) = c.strip_prefix(group).and_then(|r| r.strip_prefix('-')) {
                return Kind::Colour(one(group, rest));
            }
        }
        return Kind::Colour(vec![c.to_string()]);
    }
    if var == "--focus-outline-color" {
        return Kind::Colour(one("focus", "outline"));
    }
    if let Some(rest) = var.strip_prefix("--duration-") {
        if rest.ends_with("-min") || rest.ends_with("-max") {
            return Kind::Reported(
                "a duration's min or max tier: Tamber has one value per duration token".into(),
            );
        }
        return Kind::Other("duration", one("duration", rest));
    }
    for (prefix, kind, group) in [
        ("--spacing-", "length", "space"),
        ("--radius-", "length", "radius"),
        ("--size-", "length", "size"),
        ("--shadow-inset-", "box-shadow (inset)", "ring"),
        ("--shadow-", "box-shadow", "shadow"),
        ("--ease-", "easing", "ease"),
        ("--font-family-", "family", "family"),
        ("--font-size-", "length", "font-size"),
        ("--font-weight-", "number", "weight"),
    ] {
        if let Some(rest) = var.strip_prefix(prefix) {
            return Kind::Other(kind, one(group, rest));
        }
    }
    // `--text-heading-1-size` and its weight and leading: one text style.
    if let Some(rest) = var.strip_prefix("--text-") {
        let style = rest
            .rsplit_once('-')
            .filter(|(_, part)| matches!(*part, "size" | "weight" | "leading"))
            .map_or(rest, |(s, _)| s);
        return Kind::Other("type", one("type", style));
    }
    match var {
        "--border-width" => Kind::Other("length", one("border", "width")),
        "--focus-outline-width" => Kind::Other("length", one("focus", "outline-width")),
        "--focus-outline-offset" => Kind::Other("length", one("focus", "outline-offset")),
        "--focus-outline-style" => Kind::Reported("no Tamber kind: a line style".into()),
        _ => Kind::Reported("a token Tamber's Astryx mapping does not name".into()),
    }
}

/// Import the resolved theme `json` (`{source, light, dark}`) under
/// `prefix` (`astryx` by default), planned against `design`.
pub fn import(
    json: &str,
    prefix: &str,
    design: &Design,
    vocab: &Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    let refuse = |m: String| vec![Diagnostic::new("tamber-import-source", m, 1, 1)];
    let doc: Value = serde_json::from_str(json).map_err(|e| refuse(e.to_string()))?;
    let side = |k: &str| {
        doc.get(k)
            .and_then(Value::as_object)
            .ok_or_else(|| refuse(format!("the resolved theme has no `{k}` object")))
    };
    let (light, dark) = (side("light")?, side("dark")?);
    let mut unmapped = Vec::new();
    let mut tokens = Vec::new();
    let mut mapped = 0;
    // Each token made, by the variable that made it: a second variable to
    // one token is reported.
    let mut made: std::collections::BTreeMap<String, String> = Default::default();
    let mut others: Vec<Other> = Vec::new();
    let mut names: Vec<String> = light.keys().cloned().collect();
    names.extend(dark.keys().filter(|k| !light.contains_key(*k)).cloned());
    for var in &names {
        let get = |m: &serde_json::Map<String, Value>| {
            m.get(var).and_then(Value::as_str).map(str::to_string)
        };
        let (l, d) = (get(light), get(dark));
        let shown = match (&l, &d) {
            (Some(l), Some(d)) if l != d => format!("{l} | {d}"),
            (Some(v), _) | (None, Some(v)) => v.clone(),
            (None, None) => String::new(),
        };
        let report = |why: String, token: Option<String>| Unmapped {
            block: "light | dark".into(),
            name: var.clone(),
            value: shown.clone(),
            why,
            token,
        };
        match kind(var) {
            Kind::Colour(segs) => {
                let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
                let name = tamber_name(prefix, &refs);
                if let Some(first) = made.get(&name) {
                    unmapped.push(report(format!("its token is `{first}`'s ({name})"), None));
                    continue;
                }
                let lc = l.as_deref().and_then(colour_literal);
                let dc = d.as_deref().and_then(colour_literal);
                match (lc, dc) {
                    (Some(lc), Some(dc)) => {
                        mapped += 1;
                        made.insert(name.clone(), var.clone());
                        let value = match lc == dc {
                            true => lc,
                            false => format!("light-dark({lc}, {dc})"),
                        };
                        tokens.push(Mapped {
                            name,
                            value,
                            whens: Vec::new(),
                            kind: None,
                        });
                    }
                    _ => unmapped.push(report(
                        "not a colour Tamber reads on both sides".into(),
                        Some(name),
                    )),
                }
            }
            Kind::Other(k, segs) => {
                let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
                others.push(Other {
                    var: var.clone(),
                    sides: (l, d),
                    shown: shown.clone(),
                    kind: k,
                    token: tamber_name(prefix, &refs),
                });
            }
            Kind::Reported(why) => unmapped.push(report(why, None)),
        }
    }
    // The theme's own tokens: `--astryx-theme-<name>-color-status-fill-accent`
    // as `local.status-fill-accent`.
    let local = doc
        .get("local")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (var, value) in &local {
        let shown = value.as_str().unwrap_or_default().to_string();
        let rest = var
            .strip_prefix("--astryx-theme-")
            .and_then(|r| r.split_once('-').map(|(_, r)| r))
            .unwrap_or(var.trim_start_matches('-'));
        let rest = rest.strip_prefix("color-").unwrap_or(rest);
        let name = tamber_name(prefix, &["local", rest]);
        let report = |why: String, token: Option<String>| Unmapped {
            block: "localTokens".into(),
            name: var.clone(),
            value: shown.clone(),
            why,
            token,
        };
        if let Some(first) = made.get(&name) {
            unmapped.push(report(format!("its token is `{first}`'s ({name})"), None));
            continue;
        }
        let folds =
            !shown.is_empty() && super::unfolded(&shown).is_none() && !shown.contains("var(");
        match folds {
            true => {
                mapped += 1;
                made.insert(name.clone(), var.clone());
                tokens.push(Mapped {
                    name,
                    value: shown.split_whitespace().collect::<Vec<_>>().join(" "),
                    whens: Vec::new(),
                    kind: None,
                });
            }
            false => unmapped.push(report("not a colour Tamber folds".into(), Some(name))),
        }
    }
    mapped += other_kinds(
        &others,
        &mut made,
        &mut tokens,
        &mut unmapped,
        design,
        vocab,
    );
    // A colour that would not hold where it lands: its variable reported.
    for (name, why) in super::landing::colours_refused(&mut tokens, &[], design, vocab) {
        let Some(var) = made.get(&name) else { continue };
        let (block, value) = match light.get(var).or_else(|| dark.get(var)) {
            Some(v) => ("light | dark", v.as_str().unwrap_or_default().to_string()),
            None => (
                "localTokens",
                local
                    .get(var)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
        };
        mapped -= 1;
        unmapped.push(Unmapped {
            block: block.into(),
            name: var.clone(),
            value,
            why,
            token: Some(name),
        });
    }
    let ops = reconcile(design, &[], &[], &tokens);
    let offers = offers(prefix, design, vocab, &tokens);
    let counted = counted(names.len() + local.len(), mapped, unmapped.len())?;
    Ok(Import {
        ops,
        offers,
        unmapped,
        counted,
    })
}

/// An Astryx token of a kind besides colour, as resolved.
struct Other {
    var: String,
    /// Its light and dark values.
    sides: (Option<String>, Option<String>),
    shown: String,
    kind: &'static str,
    token: String,
}

/// The tokens of the kinds besides colour, beside the colour `tokens`
/// (`made` holds each token's variable): a text style's size, weight and
/// leading one type token, the rest one each. One whose sides differ, whose
/// name another variable has, or that does not fold is reported. The count
/// mapped.
fn other_kinds(
    others: &[Other],
    made: &mut std::collections::BTreeMap<String, String>,
    tokens: &mut Vec<Mapped>,
    unmapped: &mut Vec<Unmapped>,
    design: &Design,
    vocab: &Vocab,
) -> usize {
    use super::kinds;
    let report = |unmapped: &mut Vec<Unmapped>, o: &Other, why: String| {
        unmapped.push(Unmapped {
            block: "light | dark".into(),
            name: o.var.clone(),
            value: o.shown.clone(),
            why,
            token: Some(o.token.clone()),
        });
    };
    // `--text-body-leading` → `leading`.
    let part = |o: &Other| o.var.rsplit_once('-').map_or("", |(_, p)| p).to_string();
    let start = tokens.len();
    let mut carried: Vec<(&'static str, Vec<&Other>)> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for o in others {
        if seen.contains(&o.token.as_str()) {
            continue;
        }
        seen.push(&o.token);
        // The variables one token carries: a text style's size, weight and
        // leading, the first of each; any other to its name is reported.
        let mut group: Vec<&Other> = Vec::new();
        for x in others.iter().filter(|x| x.token == o.token) {
            // A text style's parts share its variables' stem
            // (`--text-body-`): another style whose name becomes this
            // token's is a collision.
            let stem = |o: &Other| o.var.rsplit_once('-').map_or("", |(s, _)| s).to_string();
            let fits = match o.kind {
                "type" => {
                    matches!(part(x).as_str(), "size" | "weight" | "leading")
                        && !group.iter().any(|g| part(g) == part(x))
                        && group.first().is_none_or(|g| stem(g) == stem(x))
                }
                _ => group.is_empty(),
            };
            match (fits, group.first()) {
                (true, _) => group.push(x),
                (false, Some(first)) => report(
                    unmapped,
                    x,
                    format!("its token is `{}`'s ({})", first.var, o.token),
                ),
                (false, None) => report(
                    unmapped,
                    x,
                    "a text style's part Tamber's type token does not name".into(),
                ),
            }
        }
        if group.is_empty() {
            continue;
        }
        if let Some(first) = made.get(&o.token) {
            for x in &group {
                report(
                    unmapped,
                    x,
                    format!("its token is `{first}`'s ({})", o.token),
                );
            }
            continue;
        }
        // Each one's value: its sides' one value, on both.
        let value = |x: &Other| match &x.sides {
            (Some(l), Some(d)) if l.trim() != d.trim() => Err(kinds::SIDED.to_string()),
            (Some(v), Some(_)) => kinds::bare(v, |_| None),
            _ => Err("not a value Tamber reads on both sides".to_string()),
        };
        let text = if o.kind == "type" {
            let find = |p: &str| group.iter().find(|x| part(x) == p).copied();
            let Some(size) = find("size") else {
                for x in &group {
                    report(unmapped, x, "a text style with no size".into());
                }
                continue;
            };
            (|| {
                let mut out = match find("weight") {
                    Some(w) => format!("{} ", value(w)?),
                    None => String::new(),
                };
                out.push_str(&value(size)?);
                if let Some(l) = find("leading") {
                    out.push_str(&format!("/{}", value(l)?));
                }
                Ok(out)
            })()
        } else {
            value(o)
        };
        match text {
            Ok(text) => {
                made.insert(o.token.clone(), o.var.clone());
                tokens.push(Mapped {
                    name: o.token.clone(),
                    value: text,
                    whens: Vec::new(),
                    kind: Some(kinds::written(o.kind)),
                });
                carried.push((o.kind, group));
            }
            Err(why) => {
                for x in &group {
                    report(unmapped, x, why.clone());
                }
            }
        }
    }
    let why = kinds::unfolded(tokens, &[], design, vocab);
    let mut mapped = 0;
    for (k, (kind, group)) in carried.iter().enumerate() {
        match &why[start + k] {
            None => mapped += group.len(),
            Some(why) => {
                made.remove(&group[0].token);
                for x in group {
                    report(unmapped, x, format!("not a {kind} Tamber folds: {why}"));
                }
            }
        }
    }
    let mut at = 0;
    tokens.retain(|_| {
        at += 1;
        why[at - 1].is_none()
    });
    mapped
}

/// The body surface as the platform's background and the accent as the
/// theme's tint (`AccentColor`), each offered alone. Astryx gives a token no
/// `when` line, but the design may keep its own on one (reconcile owns no
/// axis here): a role has one value (LLP 1121 D5), so such a token is
/// offered with no operations, and a note.
fn offers(prefix: &str, design: &Design, vocab: &Vocab, tokens: &[Mapped]) -> Vec<Offer> {
    let roles: Vec<(String, bool)> = vocab
        .roles
        .iter()
        .map(|r| (r.name.clone(), r.system))
        .collect();
    let mut out = Vec::new();
    for (segs, role) in [
        (&["surface", "body"][..], "-exact-background"),
        (&["accent"][..], "AccentColor"),
    ] {
        let token = tamber_name(prefix, segs);
        if !tokens.iter().any(|t| t.name == token) {
            continue;
        }
        if design
            .tokens
            .iter()
            .any(|t| t.name.value == token && !t.whens.is_empty())
        {
            out.push(Offer {
                role: role.into(),
                note: Some(format!("not offered: `{token}` keeps the design's `when` lines, and a role has one value, which the hosts' own surfaces read (LLP 1121 D5)")),
                token,
                ops: Vec::new(),
            });
            continue;
        }
        let existing = crate::edit::token_index(design, &roles, role)
            .ok()
            .map(|i| &design.tokens[i]);
        if let Some(dial) = existing.and_then(|t| super::orphaned_dial(design, t)) {
            out.push(Offer {
                role: role.into(),
                note: Some(format!("not offered: the design's `{}` is the only use of the dial `{dial}`, which rebinding it would leave unused (`tamber-dial-inert`)", existing.map_or(role, |t| t.name.value.as_str()))),
                token,
                ops: Vec::new(),
            });
            continue;
        }
        let ops = match existing {
            Some(t) if t.value.value.trim() == token && t.whens.is_empty() => continue,
            Some(t) => std::iter::once(Op::SetToken {
                name: t.name.value.clone(),
                when: Vec::new(),
                value: token.clone(),
                at: None,
                comment: None,
            })
            .chain(t.whens.iter().map(|w| Op::UnsetTokenWhen {
                name: t.name.value.clone(),
                when: w.conds.clone(),
            }))
            .collect(),
            None => vec![Op::AddToken {
                name: role.into(),
                kind: None,
                value: token.clone(),
                meta: Vec::new(),
                whens: Vec::new(),
                comment: None,
                before: None,
            }],
        };
        out.push(Offer {
            role: role.into(),
            token,
            ops,
            note: None,
        });
    }
    let named = split_roles(design, &roles, tokens, &out);
    out.extend(named);
    out
}

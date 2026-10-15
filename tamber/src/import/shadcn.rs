//! shadcn/ui (LLP 1121 D14, importer 1): the `:root` and `.dark` blocks of a
//! `globals.css`, in v4's `oklch()` or v3's bare HSL channels
//! (`222.2 84% 4.9%`, read as `hsl()`).
//!
//! - Each colour variable becomes `<prefix>.<name>` (`--chart-1` →
//!   `.chart.s1`, `--sidebar-primary` → `.sidebar.primary`): `:root`'s value
//!   the light side, `.dark`'s (or a `:root` inside
//!   `@media (prefers-color-scheme: dark)`) the dark side, one `light-dark()`.
//!   `var(--x)` is a reference to the token `--x` becomes, when it becomes a
//!   colour token.
//! - A further theme class (`.theme-blue`, `[data-theme=blue]`, and its dark
//!   block `.dark .theme-blue`) is a proposed axis, `theme = default | blue`,
//!   with `when theme=blue` lines; `.theme-default` is the default theme.
//! - `--radius` and other lengths are `kind pending (stage 6)`, with the
//!   tokens they would become (`radius`, and `radius.sm/md/lg/xl` as shadcn
//!   derives them).
//! - Role overrides are offered apart: `--background` as
//!   `-exact-background`, `--foreground` as `-exact-label`, `--card` as
//!   `-exact-secondary-background`, `--border` as `-exact-separator`, and
//!   `--primary` as `AccentColor`, the tint every host takes from the theme
//!   (an authored `accent-color` still wins its subtree).
//! - Tailwind's own blocks (`@theme`, `@theme inline`), which map these
//!   variables to utilities, are not the source and are not read.

use super::engine::{self, Decl, Fate, Target};
use super::{counted, css_blocks, reconcile, tamber_name, Import, Mapped, Offer, Unmapped};
use crate::edit::Op;
use crate::{Design, Diagnostic, Vocab};

/// Where a block's declarations go: each target its selector list names,
/// or why none. `dark` is an enclosing `prefers-color-scheme: dark`.
fn targets(selectors: &[String], dark: bool) -> Result<Vec<Target>, String> {
    let (mut out, mut refused) = (Vec::new(), None);
    for selector in selectors {
        match target(selector.trim(), dark) {
            Ok(Some(t)) if !out.contains(&t) => out.push(t),
            Ok(_) => {}
            Err(why) => refused = refused.or(Some(why)),
        }
    }
    if out.is_empty() {
        return Err(refused
            .unwrap_or_else(|| "not a theme block (`:root`, `.dark`, or a theme class)".into()));
    }
    Ok(out)
}

/// The selectors a nested rule's chain names, each in full: `&` is its
/// parent, and a rule without `&` is a descendant of it (CSS nesting).
fn nested(rules: &[&String]) -> Vec<String> {
    let mut out = vec![String::new()];
    for prelude in rules {
        let alternatives = split_top(prelude);
        out = out
            .iter()
            .flat_map(|parent| {
                alternatives.iter().map(move |alt| {
                    let alt = alt.trim();
                    match (alt.contains('&'), parent.is_empty()) {
                        (true, _) => alt.replace('&', parent),
                        (false, true) => alt.to_string(),
                        (false, false) => format!("{parent} {alt}"),
                    }
                })
            })
            .collect();
        out.sort();
        out.dedup();
        // A list too long to be a theme's: none.
        if out.len() > 1024 {
            return Vec::new();
        }
    }
    out
}

/// A selector list's selectors: its commas outside parentheses and
/// brackets (`:is(.a, .b)` is one).
fn split_top(list: &str) -> Vec<&str> {
    let (mut out, mut start, mut depth) = (Vec::new(), 0, 0i32);
    for (i, c) in list.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&list[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&list[start..]);
    out
}

/// One selector: `:root`, `.light`, `html` …, with `.dark` anywhere for the
/// dark side and a theme class (`.theme-x`, `[data-theme=x]`) for a theme;
/// `None` for a selector that names neither a root nor a theme.
fn target(selector: &str, dark: bool) -> Result<Option<Target>, String> {
    let (mut theme, mut is_dark, mut rooted) = (None, dark, false);
    for word in selector.split_whitespace() {
        for part in split_compound(word) {
            match part {
                ".dark" => is_dark = true,
                ":root" | ".light" | "html" | "body" => rooted = true,
                p => {
                    let name = p.strip_prefix(".theme-").map(str::to_string).or_else(|| {
                        let inner = p.strip_prefix("[data-theme=")?.strip_suffix(']')?;
                        Some(inner.trim_matches(|c| c == '"' || c == '\'').to_string())
                    });
                    match name {
                        Some(name) => theme = Some(name),
                        // A component, a state, a combinator: some elements
                        // only, which no theme block is.
                        None => {
                            return Err(format!(
                                "`{selector}` sets the variables on some elements only, not a theme"
                            ))
                        }
                    }
                }
            }
        }
    }
    let theme = match theme {
        // The default theme by its own name.
        Some(t) if t == "default" => None,
        Some(t) if !crate::lex::is_ident(&t) => {
            return Err(format!("the theme `{t}` is no axis value (an identifier)"))
        }
        Some(t) => Some(t),
        None if rooted || is_dark => None,
        None => return Ok(None),
    };
    Ok(Some((theme, is_dark)))
}

/// A compound selector's simple selectors: `.dark.theme-x` → `.dark`,
/// `.theme-x`; `[data-theme=x].dark` → `[data-theme=x]`, `.dark`.
fn split_compound(w: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut depth) = (0, 0);
    for (i, c) in w.char_indices() {
        match c {
            '[' => {
                if depth == 0 && i > start {
                    out.push(&w[start..i]);
                    start = i;
                }
                depth += 1;
            }
            ']' => depth -= 1,
            '.' | ':' if depth == 0 && i > start => {
                out.push(&w[start..i]);
                start = i;
            }
            _ => {}
        }
    }
    out.push(&w[start..]);
    out
}

/// A literal colour: as written, or v3's bare channels (`222.2 84% 4.9%`,
/// any form `hsl()` reads) as `hsl()`.
/// One line, spaces collapsed; `currentColor`, which no token can be, is
/// none.
fn literal(value: &str) -> Option<String> {
    let v = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let ok = |c: &str| {
        exact_color::parse(c).is_some_and(|p| !matches!(p, exact_color::Parsed::Current))
            && super::unfolded(c).is_none()
    };
    if ok(&v) {
        return Some(v);
    }
    if v.contains('(') || v.is_empty() {
        return None;
    }
    let hsl = format!("hsl({v})");
    ok(&hsl).then_some(hsl)
}

/// The name a variable becomes: `--chart-1` → `chart.s1`, `--sidebar-x` →
/// `sidebar.x`, the rest whole.
fn segments(var: &str) -> Vec<String> {
    let bare = var.trim_start_matches('-');
    for group in ["chart", "sidebar"] {
        if let Some(rest) = bare.strip_prefix(group).and_then(|r| r.strip_prefix('-')) {
            return vec![group.to_string(), rest.to_string()];
        }
    }
    vec![bare.to_string()]
}

/// Import `css` under `prefix` (`shadcn` by default), planned against
/// `design`; `vocab` names the roles an override may already spell.
pub fn import(
    css: &str,
    prefix: &str,
    design: &Design,
    vocab: &Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    let mut source = 0;
    let mut unmapped = Vec::new();
    let report = |unmapped: &mut Vec<Unmapped>,
                  block: &str,
                  name: &str,
                  value: &str,
                  why: String,
                  token: Option<String>| {
        unmapped.push(Unmapped {
            block: block.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            why,
            token,
        });
    };
    // Each declaration once, with the targets it sets.
    let mut decls: Vec<Decl> = Vec::new();
    let squash = |p: &str| {
        p.split_whitespace()
            .collect::<String>()
            .to_ascii_lowercase()
    };
    for block in css_blocks(css) {
        if block.context.iter().any(|p| p.starts_with("@theme")) {
            continue;
        }
        // The selectors, every nested rule resolved against its parents;
        // every at-rule around them is a condition.
        let rules: Vec<&String> = block
            .context
            .iter()
            .filter(|p| !p.starts_with('@'))
            .collect();
        let selectors = nested(&rules);
        let enclosing: Vec<&String> = block
            .context
            .iter()
            .filter(|p| p.starts_with('@'))
            .collect();
        // `@layer` changes nothing here; a dark colour scheme is the dark
        // side; any other condition is no theme block's.
        let dark = |p: &&String| squash(p) == "@media(prefers-color-scheme:dark)";
        let dark_media = enclosing.iter().any(dark);
        let other = enclosing
            .iter()
            .find(|p| !p.starts_with("@layer") && !dark(p));
        let label = block.context.join(" ");
        for (name, value) in block.decls {
            source += 1;
            if let Some(cond) = other {
                let why = format!("inside `{cond}`, which no theme block is");
                report(&mut unmapped, &label, &name, &value, why, None);
                continue;
            }
            match targets(&selectors, dark_media) {
                Ok(t) => decls.push((label.clone(), name, value, t)),
                Err(why) => report(&mut unmapped, &label, &name, &value, why, None),
            }
        }
    }
    // The themes the source names, whether or not their values map; one
    // whose axis would take the design past its tuples is reported.
    let mut themes: Vec<String> = Vec::new();
    for (_, _, _, targets) in &decls {
        for t in targets {
            if let Some(theme) = &t.0 {
                if !themes.contains(theme) {
                    themes.push(theme.clone());
                }
            }
        }
    }
    // The theme axis as reconcile would leave it: what the design has,
    // `default`, and the source's themes; past the tuples, the import is
    // refused whole rather than any theme dropped.
    let others: usize = design
        .axes
        .iter()
        .filter(|a| a.name.value != "theme")
        .map(|a| a.values.len())
        .product();
    let mut axis: Vec<String> = design
        .axes
        .iter()
        .find(|a| a.name.value == "theme")
        .map_or(Vec::new(), |a| a.values.clone());
    for v in std::iter::once("default".to_string()).chain(themes.iter().cloned()) {
        if !axis.contains(&v) {
            axis.push(v);
        }
    }
    if !themes.is_empty() && others * axis.len() > crate::MAX_TUPLES {
        return Err(vec![Diagnostic::new(
            "tamber-import-tuples",
            format!(
                "the theme axis would be {} ({} values), taking the design to {} tuples; a design has at most {}",
                axis.join(" | "),
                axis.len(),
                others * axis.len(),
                crate::MAX_TUPLES
            ),
            1,
            1,
        )]);
    }
    let named = engine::name(&decls, |var| {
        let segs = segments(var);
        let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
        tamber_name(prefix, &refs)
    });
    let names = &named.names;
    let (tokens, fates) = engine::assemble(&decls, &named, &themes, &literal);
    // Each declaration mapped, or reported once with why.
    let mut mapped = 0;
    for ((label, var, value, _), fate) in decls.iter().zip(&fates) {
        let token = names.get(var).cloned();
        let (why, token) = match fate {
            Fate::Mapped => {
                mapped += 1;
                continue;
            }
            Fate::Collided(first) => (format!("its name is `{first}`'s ({})", names[first]), None),
            Fate::Superseded => (
                "declared again later for the same theme block, which wins".to_string(),
                None,
            ),
            Fate::NotColour if value.trim().is_empty() => ("an empty value".to_string(), None),
            Fate::NotColour => match engine::reference(value) {
                Some(r) => (
                    format!("`{r}` is no colour where this block reads it"),
                    token,
                ),
                None if exact_color::parse(value).is_some() && super::unfolded(value).is_some() => {
                    let why = super::unfolded(value).unwrap_or_default();
                    (format!("a colour Tamber does not fold yet: {why}"), token)
                }
                None if looks_like_length(value) => {
                    ("kind pending (stage 6): length".to_string(), token)
                }
                None => (
                    "not a colour, and no other kind is built yet (stage 6)".to_string(),
                    token,
                ),
            },
        };
        report(&mut unmapped, label, var, value, why, token);
    }
    // `--radius`'s derived sizes, as shadcn computes them: reported with it.
    if names.contains_key("--radius") {
        for (size, expr) in [
            ("sm", "- 4px"),
            ("md", "- 2px"),
            ("lg", ""),
            ("xl", "+ 4px"),
        ] {
            unmapped.push(Unmapped {
                block: "(derived)".into(),
                name: format!("--radius-{size}"),
                value: if expr.is_empty() {
                    "var(--radius)".into()
                } else {
                    format!("calc(var(--radius) {expr})")
                },
                why: "kind pending (stage 6): length, derived from --radius".into(),
                token: Some(tamber_name(prefix, &["radius", size])),
            });
        }
    }
    let axes = if themes.is_empty() {
        Vec::new()
    } else {
        let mut values = vec!["default".to_string()];
        values.extend(themes.iter().cloned());
        vec![("theme".to_string(), values)]
    };
    let ops = reconcile(design, &axes, OWNED, &tokens);
    let offers = offers(prefix, design, vocab, &axes, &tokens);
    let derived = unmapped.iter().filter(|u| u.block == "(derived)").count();
    let counted = counted(source, mapped, unmapped.len() - derived)?;
    Ok(Import {
        ops,
        offers,
        unmapped,
        counted,
    })
}

fn looks_like_length(v: &str) -> bool {
    let v = v.trim();
    ["px", "rem", "em", "%"].iter().any(|u| {
        v.strip_suffix(u)
            .is_some_and(|n| n.trim().parse::<f64>().is_ok())
    }) || v.starts_with("calc(")
}

/// The role overrides the mapped tokens mean, each offered alone; none
/// where the design has it so already, under any spelling of the role. A
/// role has one value (LLP 1121 D5): one whose token varies by an app axis
/// once the import is accepted is not offered, and any role the design has
/// that would vary then is named, each with a note and no operations
/// (removing a role can break what reads its colour; the person decides).
fn offers(
    prefix: &str,
    design: &Design,
    vocab: &Vocab,
    axes: &[(String, Vec<String>)],
    tokens: &[Mapped],
) -> Vec<Offer> {
    let has = |name: &str| tokens.iter().any(|t| t.name == name);
    let roles: Vec<(String, bool)> = vocab
        .roles
        .iter()
        .map(|r| (r.name.clone(), r.system))
        .collect();
    let mut out = Vec::new();
    for (var, role) in [
        ("background", "-exact-background"),
        ("foreground", "-exact-label"),
        ("card", "-exact-secondary-background"),
        ("border", "-exact-separator"),
        ("primary", "AccentColor"),
    ] {
        let token = tamber_name(prefix, &[var]);
        if !has(&token) {
            continue;
        }
        let existing = crate::edit::token_index(design, &roles, role)
            .ok()
            .map(|i| &design.tokens[i]);
        if varies(&token, tokens, design, &roles, &mut Vec::new()) {
            out.push(Offer {
                role: role.into(),
                note: Some(format!("not offered: `{token}` varies by an app axis, and a role has one value, which the hosts' own surfaces read (LLP 1121 D5)")),
                token,
                ops: Vec::new(),
            });
            continue;
        }
        let ops = match existing {
            Some(t) if t.value.value.trim() == token && t.whens.is_empty() => continue,
            // The role is the token in every mode: its `when` lines go.
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
    // The design's own role overrides that would vary once the import is
    // accepted, unless an offer above rebinds them: naming a token that
    // varies, or with `when` lines on an axis the import adds values to.
    for t in &design.tokens {
        let Some(key) = crate::edit::role_key(&roles, &t.name.value) else {
            continue;
        };
        let rebound = out.iter().any(|o| {
            !o.ops.is_empty() && crate::edit::role_key(&roles, &o.role).as_ref() == Some(&key)
        });
        let values = std::iter::once(&t.value.value).chain(t.whens.iter().map(|w| &w.value.value));
        let named = values
            .flat_map(|v| words(v))
            .find(|w| varies(w, tokens, design, &roles, &mut Vec::new()));
        let widened = t.whens.iter().any(|w| {
            w.conds
                .iter()
                .any(|(a, _)| axes.iter().any(|(n, _)| n == a))
        });
        if rebound || (named.is_none() && !widened) {
            continue;
        }
        let what = match named {
            Some(n) => {
                format!("names `{n}`, which varies by an app axis once this import is accepted")
            }
            None => "has `when` lines on an axis this import adds values to".into(),
        };
        out.push(Offer {
            role: t.name.value.clone(),
            token: named.unwrap_or(&t.value.value).to_string(),
            ops: Vec::new(),
            note: Some(format!("the design's `{}` {what}: a role has one value (LLP 1121 D5), so the build refuses it (`tamber-role-axis`) until it names one colour", t.name.value)),
        });
    }
    out
}

/// The axes the importer owns: their `when` lines are the source's.
const OWNED: &[&str] = &["theme"];

/// Whether `name` varies by an app axis once the import is accepted: a
/// `when` line the source gives it, one the design keeps (on an axis the
/// importer does not own, or on a token it does not map), or a token its
/// values name that varies.
fn varies<'a>(
    name: &'a str,
    tokens: &'a [Mapped],
    design: &'a Design,
    roles: &[(String, bool)],
    seen: &mut Vec<&'a str>,
) -> bool {
    if seen.contains(&name) {
        return false;
    }
    seen.push(name);
    let mapped = tokens.iter().find(|t| t.name == name);
    // A design's token under any spelling of its role, as lowering reads it.
    let have = crate::edit::token_index(design, roles, name)
        .ok()
        .map(|i| &design.tokens[i]);
    let kept = have.is_some_and(|t| {
        t.whens
            .iter()
            .any(|w| mapped.is_none() || w.conds.iter().any(|(a, _)| !OWNED.contains(&a.as_str())))
    });
    if kept || mapped.is_some_and(|t| !t.whens.is_empty()) {
        return true;
    }
    let value = match (mapped, have) {
        (Some(t), _) => t.value.as_str(),
        (None, Some(t)) => t.value.value.as_str(),
        (None, None) => return false,
    };
    words(value).any(|w| varies(w, tokens, design, roles, seen))
}

/// The names a value may hold.
fn words(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .filter(|w| !w.is_empty())
}

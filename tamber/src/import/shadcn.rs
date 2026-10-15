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
//!   `--primary` as the root's `accent-color` (D9, a Contract edit).
//! - Tailwind's own blocks (`@theme`, `@theme inline`), which map these
//!   variables to utilities, are not the source and are not read.

use super::{counted, css_blocks, reconcile, tamber_name, Import, Mapped, Offer, Unmapped};
use crate::edit::Op;
use crate::{Design, Diagnostic, Vocab};
use std::collections::{BTreeMap, BTreeSet};

/// A theme block: its theme (`None` the default) and whether it is dark.
type Target = (Option<String>, bool);

/// Where a block's declarations go: each target its selector list names,
/// or why none. `dark` is an enclosing `prefers-color-scheme: dark`.
fn targets(selector: &str, dark: bool) -> Result<Vec<Target>, String> {
    let mut out = Vec::new();
    for alternative in selector.split(',') {
        if let Some(t) = target(alternative.trim(), dark)? {
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    if out.is_empty() {
        return Err("not a theme block (`:root`, `.dark`, or a theme class)".into());
    }
    Ok(out)
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
                    if let Some(name) = name {
                        theme = Some(name);
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
fn literal(value: &str) -> Option<String> {
    let v = value.trim();
    if exact_color::parse(v).is_some() {
        return Some(v.to_string());
    }
    if v.contains('(') || v.is_empty() {
        return None;
    }
    let hsl = format!("hsl({v})");
    exact_color::parse(&hsl).is_some().then_some(hsl)
}

/// `var(--x)`'s name.
fn reference(value: &str) -> Option<&str> {
    value
        .trim()
        .strip_prefix("var(")?
        .strip_suffix(')')
        .map(str::trim)
        .filter(|n| n.starts_with("--") && !n.contains(','))
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

/// One declaration as read: its block, variable, value and targets.
type Decl = (String, String, String, Vec<Target>);

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
        // The selector is the innermost one; every at-rule around it (or
        // nested in it, as CSS nesting allows) is a condition.
        let selector = block
            .context
            .iter()
            .rev()
            .find(|p| !p.starts_with('@'))
            .cloned()
            .unwrap_or_default();
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
            match targets(&selector, dark_media) {
                Ok(t) => decls.push((label.clone(), name, value, t)),
                Err(why) => report(&mut unmapped, &label, &name, &value, why, None),
            }
        }
    }
    // Names, first seen first; of two variables that would be one token,
    // the later one's declarations are reported.
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut taken: BTreeMap<String, String> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut collided: BTreeMap<String, String> = BTreeMap::new();
    for (_, var, _, _) in &decls {
        if names.contains_key(var) || collided.contains_key(var) {
            continue;
        }
        let segs = segments(var);
        let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
        let token = tamber_name(prefix, &refs);
        match taken.get(&token) {
            Some(first) => {
                collided.insert(var.clone(), first.clone());
            }
            None => {
                taken.insert(token.clone(), var.clone());
                names.insert(var.clone(), token);
                order.push(var.clone());
            }
        }
    }
    // The variables that become colour tokens: a literal colour somewhere,
    // or a reference to one that does (a cycle never does).
    let mut colours: BTreeSet<String> = decls
        .iter()
        .filter(|(_, v, value, _)| names.contains_key(v) && literal(value).is_some())
        .map(|(_, v, _, _)| v.clone())
        .collect();
    loop {
        let more: Vec<String> = decls
            .iter()
            .filter(|(_, v, value, _)| {
                names.contains_key(v)
                    && !colours.contains(v)
                    && reference(value).is_some_and(|r| colours.contains(r))
            })
            .map(|(_, v, _, _)| v.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        colours.extend(more);
    }
    // Each declaration mapped (a colour, to each target) or reported.
    let mut mapped = 0;
    let mut sides: BTreeMap<(Target, String), (String, usize)> = BTreeMap::new();
    let mut themes: Vec<String> = Vec::new();
    let mut superseded = vec![false; decls.len()];
    for (k, (label, var, value, targets)) in decls.iter().enumerate() {
        if let Some(first) = collided.get(var) {
            let why = format!("its name is `{first}`'s ({})", names[first]);
            report(&mut unmapped, label, var, value, why, None);
            continue;
        }
        let colour = literal(value).or_else(|| {
            reference(value)
                .filter(|r| colours.contains(*r))
                .map(|r| names[r].clone())
        });
        let Some(colour) = colour else {
            let token = Some(names[var].clone());
            let (why, token) = if value.trim().is_empty() {
                ("an empty value".to_string(), None)
            } else if let Some(r) = reference(value) {
                (format!("`{r}` becomes no colour token"), token)
            } else if looks_like_length(value) {
                ("kind pending (stage 6): length".to_string(), token)
            } else {
                let why = "not a colour, and no other kind is built yet (stage 6)";
                (why.to_string(), token)
            };
            report(&mut unmapped, label, var, value, why, token);
            continue;
        };
        mapped += 1;
        for t in targets {
            if let Some(theme) = &t.0 {
                if !themes.contains(theme) {
                    themes.push(theme.clone());
                }
            }
            // A later declaration for the same block wins, as in CSS.
            if let Some((_, was)) = sides.insert((t.clone(), var.clone()), (colour.clone(), k)) {
                superseded[was] = true;
            }
        }
    }
    // A declaration every target of which a later one took is reported,
    // and no longer mapped.
    for (k, (label, var, value, targets)) in decls.iter().enumerate() {
        let live = targets.iter().any(|t| {
            sides
                .get(&(t.clone(), var.clone()))
                .is_some_and(|(_, at)| *at == k)
        });
        if superseded[k] && !live {
            mapped -= 1;
            let why = "declared again later for the same theme block, which wins".to_string();
            report(&mut unmapped, label, var, value, why, None);
        }
    }
    let pair = |l: Option<&String>, d: Option<&String>| match (l, d) {
        (Some(l), Some(d)) if l == d => Some(l.clone()),
        (Some(l), Some(d)) => Some(format!("light-dark({l}, {d})")),
        (Some(x), None) | (None, Some(x)) => Some(x.clone()),
        (None, None) => None,
    };
    let mut tokens = Vec::new();
    for var in &order {
        let get = |theme: &Option<String>, dark: bool| {
            sides
                .get(&((theme.clone(), dark), var.clone()))
                .map(|(c, _)| c)
        };
        let mut whens = Vec::new();
        for t in &themes {
            let theme = Some(t.clone());
            if get(&theme, false).is_none() && get(&theme, true).is_none() {
                continue;
            }
            // A theme's missing side keeps the default theme's.
            let value = pair(
                get(&theme, false).or_else(|| get(&None, false)),
                get(&theme, true).or_else(|| get(&None, true)),
            );
            if let Some(v) = value {
                whens.push((vec![("theme".to_string(), t.clone())], v));
            }
        }
        // A variable only a theme sets takes that theme's value as its
        // default; a `when` equal to the default is none.
        let default = pair(get(&None, false), get(&None, true));
        let Some(value) = default.or_else(|| whens.first().map(|(_, v)| v.clone())) else {
            continue;
        };
        whens.retain(|(_, v)| *v != value);
        tokens.push(Mapped {
            name: names[var].clone(),
            value,
            whens,
        });
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
    let ops = reconcile(design, &axes, &tokens);
    let offers = offers(prefix, design, vocab, &tokens);
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

/// The role overrides and the accent the mapped tokens mean, each offered
/// alone; none where the design has it so already, under any spelling of
/// the role.
fn offers(prefix: &str, design: &Design, vocab: &Vocab, tokens: &[Mapped]) -> Vec<Offer> {
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
    ] {
        let token = tamber_name(prefix, &[var]);
        if !has(&token) {
            continue;
        }
        let existing = crate::edit::token_index(design, &roles, role)
            .ok()
            .map(|i| &design.tokens[i]);
        let op = match existing {
            Some(t) if t.value.value.trim() == token && t.whens.is_empty() => continue,
            Some(t) => Op::SetToken {
                name: t.name.value.clone(),
                when: Vec::new(),
                value: token.clone(),
                at: None,
                comment: None,
            },
            None => Op::AddToken {
                name: role.into(),
                kind: None,
                value: token.clone(),
                meta: Vec::new(),
                whens: Vec::new(),
                comment: None,
                before: None,
            },
        };
        out.push(Offer {
            role: role.into(),
            token,
            ops: vec![op],
            note: None,
        });
    }
    let primary = tamber_name(prefix, &["primary"]);
    if has(&primary) {
        out.push(Offer {
            role: "accent-color".into(),
            token: primary.clone(),
            ops: Vec::new(),
            note: Some(format!(
                "the root's `accent-color` in app.contract names `{primary}` (LLP 1121 D9): a Contract edit, made by hand until the server writes Contract (LLP 1122's set-row)"
            )),
        });
    }
    out
}

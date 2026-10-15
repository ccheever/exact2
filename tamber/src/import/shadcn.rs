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

use super::{counted, css_blocks, reconcile, tamber_name, Import, Mapped, Offer, Unmapped};
use crate::edit::Op;
use crate::{Design, Diagnostic, Vocab};
use std::collections::{BTreeMap, BTreeSet};

/// A theme block: its theme (`None` the default) and whether it is dark.
type Target = (Option<String>, bool);

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
    let ok =
        |c: &str| exact_color::parse(c).is_some_and(|p| !matches!(p, exact_color::Parsed::Current));
    if ok(&v) {
        return Some(v);
    }
    if v.contains('(') || v.is_empty() {
        return None;
    }
    let hsl = format!("hsl({v})");
    ok(&hsl).then_some(hsl)
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

/// Where a block reads a variable it does not set, nearest first, as CSS
/// does: a dark theme block from the theme, then the default dark side.
fn chain(at: &Target) -> Vec<Target> {
    match at {
        (Some(t), true) => vec![
            (Some(t.clone()), true),
            (Some(t.clone()), false),
            (None, true),
            (None, false),
        ],
        (Some(t), false) => vec![(Some(t.clone()), false), (None, false)],
        (None, true) => vec![(None, true), (None, false)],
        (None, false) => vec![(None, false)],
    }
}

/// Whether `to` is reachable from `from` in `graph` by one step or more.
fn reaches(graph: &BTreeMap<String, BTreeSet<String>>, from: &str, to: &str) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<&str> = graph
        .get(from)
        .map_or(Vec::new(), |n| n.iter().map(String::as_str).collect());
    while let Some(n) = stack.pop() {
        if n == to {
            return true;
        }
        if seen.insert(n) {
            stack.extend(graph.get(n).into_iter().flatten().map(String::as_str));
        }
    }
    false
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
    // The declaration that wins each block for each variable: the last, as
    // in CSS, whatever its value.
    let mut winners: BTreeMap<(Target, String), usize> = BTreeMap::new();
    for (k, (_, var, _, targets)) in decls.iter().enumerate() {
        if collided.contains_key(var) {
            continue;
        }
        for t in targets {
            winners.insert((t.clone(), var.clone()), k);
        }
    }
    // A colour as a block sees it: a literal, or a reference to a variable
    // that is a colour where the block reads it (its own block, the theme's
    // light side, the default's: CSS's nearest), never through a cycle.
    // Each as written (a reference stays a token's name) and resolved.
    fn seen(
        at: &Target,
        var: &str,
        decls: &[Decl],
        winners: &BTreeMap<(Target, String), usize>,
        names: &BTreeMap<String, String>,
        visiting: &mut Vec<String>,
    ) -> Option<(String, String)> {
        let k = chain(at)
            .iter()
            .find_map(|c| winners.get(&(c.clone(), var.to_string())))?;
        colour_of(&decls[*k].2, at, decls, winners, names, visiting)
    }
    fn colour_of(
        value: &str,
        at: &Target,
        decls: &[Decl],
        winners: &BTreeMap<(Target, String), usize>,
        names: &BTreeMap<String, String>,
        visiting: &mut Vec<String>,
    ) -> Option<(String, String)> {
        if let Some(c) = literal(value) {
            return Some((c.clone(), c));
        }
        let r = reference(value)?;
        if visiting.iter().any(|v| v == r) || !names.contains_key(r) {
            return None;
        }
        visiting.push(r.to_string());
        let found = seen(at, r, decls, winners, names, visiting);
        visiting.pop();
        found.map(|(_, resolved)| (names[r].clone(), resolved))
    }
    // Each block's own colour for each variable, where its winner is one:
    // as written, resolved, and the variable a reference names.
    let mut sides: BTreeMap<(Target, String), (String, String, Option<String>)> = BTreeMap::new();
    for ((t, var), k) in &winners {
        let mut visiting = vec![var.clone()];
        let value = &decls[*k].2;
        if let Some((written, resolved)) =
            colour_of(value, t, &decls, &winners, &names, &mut visiting)
        {
            let names_var = reference(value)
                .filter(|_| literal(value).is_none())
                .map(str::to_string);
            sides.insert((t.clone(), var.clone()), (written, resolved, names_var));
        }
    }
    // A reference round a cycle of tokens (each side fine alone, the
    // tokens' `light-dark()`s each other's) is written as the colour it
    // resolves to. The tokens fold a missing side as CSS reads it (see the
    // `when` lines below), so a reference elsewhere folds to what it read.
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ((_, var), (_, _, r)) in &sides {
        if let Some(r) = r {
            graph.entry(var.clone()).or_default().insert(r.clone());
        }
    }
    let cyclic: BTreeSet<String> = graph
        .keys()
        .filter(|v| reaches(&graph, v, v))
        .cloned()
        .collect();
    let keys: Vec<(Target, String)> = sides.keys().cloned().collect();
    for key in keys {
        let (_, var) = &key;
        let entry = &sides[&key];
        if entry.2.is_some() && cyclic.contains(var) {
            let resolved = entry.1.clone();
            sides.insert(key, (resolved.clone(), resolved, None));
        }
    }
    // Each declaration mapped (it wins a block, and is a colour there) or
    // reported, once.
    let mut mapped = 0;
    for (k, (label, var, value, targets)) in decls.iter().enumerate() {
        if let Some(first) = collided.get(var) {
            let why = format!("its name is `{first}`'s ({})", names[first]);
            report(&mut unmapped, label, var, value, why, None);
            continue;
        }
        let won: Vec<&Target> = targets
            .iter()
            .filter(|t| winners.get(&((*t).clone(), var.clone())) == Some(&k))
            .collect();
        if won.is_empty() {
            let why = "declared again later for the same theme block, which wins".to_string();
            report(&mut unmapped, label, var, value, why, None);
            continue;
        }
        if won
            .iter()
            .any(|t| sides.contains_key(&((*t).clone(), var.clone())))
        {
            mapped += 1;
            continue;
        }
        let token = Some(names[var].clone());
        let (why, token) = if value.trim().is_empty() {
            ("an empty value".to_string(), None)
        } else if let Some(r) = reference(value) {
            (
                format!("`{r}` is no colour where this block reads it"),
                token,
            )
        } else if looks_like_length(value) {
            ("kind pending (stage 6): length".to_string(), token)
        } else {
            let why = "not a colour, and no other kind is built yet (stage 6)";
            (why.to_string(), token)
        };
        report(&mut unmapped, label, var, value, why, token);
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
                .map(|(written, _, _)| written)
        };
        let mut whens = Vec::new();
        for t in &themes {
            let theme = Some(t.clone());
            if get(&theme, false).is_none() && get(&theme, true).is_none() {
                continue;
            }
            // A theme's missing side reads as CSS does: its dark side the
            // theme's light, then the default's dark; its light the default's.
            let value = pair(
                get(&theme, false).or_else(|| get(&None, false)),
                get(&theme, true)
                    .or_else(|| get(&theme, false))
                    .or_else(|| get(&None, true)),
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
    let ops = reconcile(design, &axes, &["theme"], &tokens);
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

/// The role overrides the mapped tokens mean, each offered alone; none
/// where the design has it so already, under any spelling of the role. A
/// role has one value (LLP 1121 D5): one whose token varies by an app axis
/// is offered with no operations, and a note.
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
        ("primary", "AccentColor"),
    ] {
        let token = tamber_name(prefix, &[var]);
        if !has(&token) {
            continue;
        }
        if varies(&token, tokens, &mut Vec::new()) {
            out.push(Offer {
                role: role.into(),
                note: Some(format!("not offered: `{token}` varies by an app axis, and a role has one value, which the hosts' own surfaces read (LLP 1121 D5)")),
                token,
                ops: Vec::new(),
            });
            continue;
        }
        let existing = crate::edit::token_index(design, &roles, role)
            .ok()
            .map(|i| &design.tokens[i]);
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
    out
}

/// Whether `name` varies by an app axis: a `when` line of its own, or of a
/// token its values name.
fn varies<'a>(name: &'a str, tokens: &'a [Mapped], seen: &mut Vec<&'a str>) -> bool {
    let Some(t) = tokens.iter().find(|t| t.name == name) else {
        return false;
    };
    if !t.whens.is_empty() {
        return true;
    }
    if seen.contains(&name) {
        return false;
    }
    seen.push(name);
    t.value
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .any(|word| {
            tokens
                .iter()
                .any(|o| o.name == word && varies(&o.name, tokens, seen))
        })
}

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
//! - A length (`--radius: 0.625rem`), and a shadow or a family by its name
//!   (`--shadow-sm`, `--font-sans`), is its kind's token (stage 6 H), a
//!   theme's value its `when` line; one with two values on the scheme's
//!   sides, or that does not fold, is reported. `--radius`'s sizes as
//!   shadcn derives them (`radius.sm/md/lg/xl`) are proposed with it, each
//!   that folds (a `rem` radius in `calc()` does not yet), the rest
//!   reported outside the count.
//! - Role overrides are offered apart: `--background` as
//!   `-exact-background`, `--foreground` as `-exact-label`, `--card` as
//!   `-exact-secondary-background`, `--border` as `-exact-separator`, and
//!   `--primary` as `AccentColor`, the tint every host takes from the theme
//!   (an authored `accent-color` still wins its subtree).
//! - Tailwind's own blocks (`@theme`, `@theme inline`), which map these
//!   variables to utilities, are not the source and are not read.

use super::engine::{self, Decl, Fate, Target};
use super::{counted, css_read, reconcile, tamber_name, Import, Mapped, Offer, Unmapped};
use crate::edit::Op;
use crate::{Design, Diagnostic, Vocab};
use std::collections::{HashMap, HashSet};

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

/// One selector: `:root`, `.light`, `html` …, with `.dark` anywhere for the
/// dark side and a theme class (`.theme-x`, `[data-theme=x]`) for a theme;
/// `None` for a selector that names neither a root nor a theme.
fn target(selector: &str, dark: bool) -> Result<Option<Target>, String> {
    let (mut theme, mut is_dark, mut rooted) = (None, dark, false);
    for word in selector.split_whitespace() {
        for part in engine::split_compound(word) {
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

/// One line, spaces collapsed; `currentColor`, which no token can be, is
/// none.
fn literal(value: &str) -> Option<String> {
    super::colour_literal(value).or_else(|| {
        let v = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if v.contains('(') || v.is_empty() {
            return None;
        }
        super::colour_literal(&format!("hsl({v})"))
    })
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
    // Its work charged to one import's budget, as DTCG's and Pencil's is.
    let _work = crate::budget::Work::begin(crate::budget::IMPORT);
    super::bounded(import_unbounded(css, prefix, design, vocab))
}

/// [`import`] before the checker's bound at import (`super::bounded`).
fn import_unbounded(
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
    let (blocks, deep) = css_read(css);
    super::source_read(&blocks, &deep)?;
    // A rule the reader did not read: one value, reported with its line.
    for d in &deep {
        source += 1;
        let why = format!("not read: {} (line {})", d.why, d.line);
        report(&mut unmapped, "(not read)", &d.prelude, "", why, None);
    }
    for block in blocks {
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
        let selectors = engine::nested(&rules);
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
    let mut named_themes: HashSet<&str> = HashSet::new();
    for (_, _, _, targets) in &decls {
        for t in targets {
            if let Some(theme) = &t.0 {
                if named_themes.insert(theme) {
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
    let mut have: std::collections::HashSet<String> = axis.iter().cloned().collect();
    for v in std::iter::once("default".to_string()).chain(themes.iter().cloned()) {
        if have.insert(v.clone()) {
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
        Some(tamber_name(prefix, &refs))
    });
    let names = &named.names;
    let (mut tokens, fates) = engine::assemble(&decls, &named, &themes, &literal);
    // The declarations of the kinds besides colour, each made a token or
    // reported there (stage 6 H).
    let mut others: Vec<(usize, &'static str)> = Vec::new();
    // Each token's source declarations, for a token refused where it lands.
    let mut sources: Vec<(String, Vec<Unmapped>)> = Vec::new();
    // Each declaration mapped, or reported once with why.
    let mut mapped = 0;
    // A variable that makes no token is no token's to name.
    let dropped: HashSet<&String> = decls
        .iter()
        .zip(&fates)
        .filter(|(_, f)| matches!(f, Fate::Unresolved(_)))
        .map(|((_, var, _, _), _)| var)
        .collect();
    let kinds = Kinds::of(&decls);
    // Each variable's declarations read once: whether any is a colour where
    // it wins, and the one other kind its no-colour ones all are (`None`
    // when they differ; none of them, any kind).
    // (Coloured; then none read yet, `Some(None)` kinds that differ, or the one kind.)
    type Seen = (bool, Option<Option<Option<&'static str>>>);
    let mut seen_as: HashMap<&str, Seen> = HashMap::new();
    for ((_, v, x, _), f) in decls.iter().zip(&fates) {
        let e = seen_as.entry(v.as_str()).or_insert((false, None));
        e.0 |= !matches!(f, Fate::NotColour | Fate::Superseded);
        if matches!(f, Fate::NotColour) {
            let k = kinds.other_kind(v, x);
            e.1 = match e.1 {
                None => Some(Some(k)),
                Some(Some(was)) if was == k => Some(Some(k)),
                _ => Some(None),
            };
        }
    }
    for (i, ((label, var, value, _), fate)) in decls.iter().zip(&fates).enumerate() {
        let token = names.get(var).filter(|_| !dropped.contains(&var)).cloned();
        // One other kind in every block it wins (a colour in one, or a
        // value of no kind, is no token of either kind).
        let (coloured, uniform) = seen_as[var.as_str()];
        // (Only a declaration that is no colour has one: a colour's chain is
        // not walked again for each link of it.)
        let kind = match fate {
            Fate::NotColour => kinds.other_kind(var, value),
            _ => None,
        };
        let one_kind = !coloured && uniform.is_none_or(|u| u == Some(kind));
        if let (Fate::NotColour, Some(kind), true) = (fate, kind, one_kind) {
            others.push((i, kind));
            continue;
        }
        let (why, token) = match fate {
            Fate::Mapped => {
                mapped += 1;
                continue;
            }
            // Every shadcn variable names a token.
            Fate::Aux => continue,
            Fate::Unresolved(t) => (
                format!(
                    "`{var}` is no colour in `{}`, where it is set too, so it makes no token",
                    engine::block_name(t)
                ),
                None,
            ),
            Fate::Collided(first) => (format!("its name is `{first}`'s ({})", names[first]), None),
            Fate::Superseded => (
                "declared again later for the same theme block, which wins".to_string(),
                None,
            ),
            Fate::Deep => ("an alias chain longer than 256".to_string(), None),
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
                None if kind.is_some() => (
                    match coloured {
                        true => format!(
                            "a {} here and a colour in another block: a token is one kind",
                            kind.unwrap_or_default()
                        ),
                        false => format!(
                            "a {} here, and another block's value is no {}: a token is one kind",
                            kind.unwrap_or_default(),
                            kind.unwrap_or_default()
                        ),
                    },
                    token,
                ),
                None => (
                    "not a colour, nor a length, shadow or family by its name and value"
                        .to_string(),
                    token,
                ),
            },
        };
        report(&mut unmapped, label, var, value, why, token);
    }
    let axes = if themes.is_empty() {
        Vec::new()
    } else {
        let mut values = vec!["default".to_string()];
        values.extend(themes.iter().cloned());
        vec![("theme".to_string(), values)]
    };
    mapped += other_kinds(
        prefix,
        &decls,
        &others,
        names,
        &themes,
        &axes,
        (design, vocab),
        &mut tokens,
        &mut unmapped,
        &mut sources,
    );
    // Each colour's declarations mapped, by its token.
    // Each token's place in `sources`, by name.
    let mut source_at: HashMap<&str, usize> = HashMap::new();
    for ((label, var, value, _), fate) in decls.iter().zip(&fates) {
        let Some(name) = names.get(var).filter(|_| *fate == Fate::Mapped) else {
            continue;
        };
        let record = Unmapped {
            block: label.clone(),
            name: var.clone(),
            value: value.clone(),
            why: String::new(),
            token: Some(name.clone()),
        };
        match source_at.get(name.as_str()) {
            Some(&k) => sources[k].1.push(record),
            None => {
                source_at.insert(name.as_str(), sources.len());
                sources.push((name.clone(), vec![record]));
            }
        }
    }
    // What would not hold where it lands: its declarations reported.
    mapped -= super::landing::settle(
        &mut tokens,
        &sources,
        (&axes, OWNED),
        design,
        vocab,
        &mut unmapped,
    )?;
    let ops = reconcile(design, &axes, OWNED, &tokens);
    let offers = offers(prefix, design, vocab, &axes, &tokens);
    let derived = unmapped.iter().filter(|u| u.block == "(derived)").count();
    let counted = counted(source, mapped, unmapped.len() - derived)?;
    Ok(Import {
        ops,
        offers,
        unmapped,
        counted,
        dials: Vec::new(),
        pins: Vec::new(),
        notes: Vec::new(),
        accent: Some(tamber_name(prefix, &["primary"]))
            .filter(|name| tokens.iter().any(|t| t.name == *name && t.kind.is_none())),
    })
}

/// Each variable's winning declaration (its last), for [`Kinds::other_kind`].
struct Kinds<'a> {
    decls: &'a [Decl],
    last: HashMap<&'a str, usize>,
}

impl<'a> Kinds<'a> {
    fn of(decls: &'a [Decl]) -> Kinds<'a> {
        let last = decls
            .iter()
            .enumerate()
            .map(|(k, (_, v, _, _))| (v.as_str(), k))
            .collect();
        Kinds { decls, last }
    }

    /// The kind besides colour a declaration that is no colour is: a length
    /// by its value (`--radius: 0.625rem`), else a shadow or a family by its
    /// name (`--shadow-sm`, `--font-sans`); a reference the kind of what it
    /// names.
    fn other_kind(&self, var: &str, value: &str) -> Option<&'static str> {
        self.through(var, value, &mut HashSet::new())
            .unwrap_or(None)
    }

    /// [`Kinds::other_kind`] through a chain of references: a cycle is none
    /// (and each variable before it its name's kind), and a chain past 256
    /// references none at all (`Err`, which no name's kind stands in for).
    fn through(
        &self,
        var: &'a str,
        value: &str,
        seen: &mut HashSet<&'a str>,
    ) -> Result<Option<&'static str>, ()> {
        let named = || {
            if var == "--shadow" || var.starts_with("--shadow-") {
                Some("box-shadow")
            } else if var.starts_with("--font-") {
                Some("family")
            } else {
                None
            }
        };
        // A reference: the kind of what it names, by its last declaration
        // (the one that wins), else this variable's by its name.
        if let Some(r) = engine::reference(value) {
            if seen.contains(var) || r == var {
                return Ok(None);
            }
            seen.insert(var);
            if seen.len() > 256 {
                return Err(());
            }
            let found = match self.last.get(r).map(|&k| &self.decls[k]) {
                Some((_, v, value, _)) => self.through(v, value, seen)?,
                None => None,
            };
            return Ok(found.or_else(named));
        }
        Ok(if looks_like_length(value) {
            Some("length")
        } else {
            named()
        })
    }
}

/// The declarations of the kinds besides colour (`others`, each a
/// declaration's index and its kind) as tokens beside the colour `tokens`:
/// each block's winner as CSS reads it, a theme's value its `when` line,
/// `var()` the token it names; one whose value differs between the
/// scheme's sides, or that does not fold, is reported. `--radius`'s
/// derived sizes, as shadcn computes them, are proposed with it (each that
/// folds; the rest reported outside the count). The count mapped.
#[allow(clippy::too_many_arguments)]
fn other_kinds(
    prefix: &str,
    decls: &[Decl],
    others: &[(usize, &'static str)],
    names: &std::collections::BTreeMap<String, String>,
    themes: &[String],
    axes: &[(String, Vec<String>)],
    (design, vocab): (&Design, &Vocab),
    tokens: &mut Vec<Mapped>,
    unmapped: &mut Vec<Unmapped>,
    sources: &mut Vec<(String, Vec<Unmapped>)>,
) -> usize {
    use super::kinds;
    let report = |unmapped: &mut Vec<Unmapped>, i: usize, why: String| {
        let (label, var, value, _) = &decls[i];
        unmapped.push(Unmapped {
            block: label.clone(),
            name: var.clone(),
            value: value.clone(),
            why,
            token: names.get(var).cloned(),
        });
    };
    // Each variable, first seen first, with its declarations in order.
    let mut vars: Vec<&str> = Vec::new();
    let mut own_of: HashMap<&str, Vec<(usize, &'static str)>> = HashMap::new();
    for &(i, kind) in others {
        let var = decls[i].1.as_str();
        let own = own_of.entry(var).or_default();
        if own.is_empty() {
            vars.push(var);
        }
        own.push((i, kind));
    }
    let start = tokens.len();
    // A token made: a colour, or one of these.
    let colours: HashSet<String> = tokens.iter().map(|t| t.name.clone()).collect();
    let token_of = |var: &str| {
        names
            .get(var)
            .filter(|n| colours.contains(*n) || own_of.contains_key(var))
            .cloned()
    };
    // Each token made: its kind, and the declarations it carries.
    let mut made: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for &var in &vars {
        let own: &[(usize, &'static str)] = &own_of[var];
        let kind = own[0].1;
        // Each block's winner: the last that sets it.
        let mut winner: std::collections::BTreeMap<&Target, &str> = Default::default();
        for (i, _) in own {
            for t in &decls[*i].3 {
                winner.insert(t, &decls[*i].2);
            }
        }
        let get = |theme: &Option<String>, dark: bool| {
            winner.get(&(theme.clone(), dark)).map(|v| v.trim())
        };
        // A theme's sides as CSS reads them, as the colours' are.
        let side = |theme: &Option<String>| {
            let light = get(theme, false).or_else(|| get(&None, false));
            let dark = get(theme, true)
                .or_else(|| get(theme, false))
                .or_else(|| get(&None, true));
            (light.or(dark), dark.or(light))
        };
        let all: Vec<usize> = own.iter().map(|(i, _)| *i).collect();
        let mut values: Vec<(Option<String>, &str)> = Vec::new();
        let mut sided = false;
        for theme in std::iter::once(None).chain(themes.iter().cloned().map(Some)) {
            match side(&theme) {
                (Some(l), Some(d)) if l == d => values.push((theme, l)),
                (Some(_), Some(_)) => sided = true,
                _ => {}
            }
        }
        if sided {
            for i in &all {
                report(unmapped, *i, kinds::SIDED.to_string());
            }
            continue;
        }
        let made_value = (|| {
            let (_, default) = values.first().ok_or("no value")?;
            let default = kinds::bare(default, token_of)?;
            let mut whens = Vec::new();
            for (theme, v) in &values[1..] {
                let v = kinds::bare(v, token_of)?;
                if v != default {
                    let t = theme.clone().unwrap_or_default();
                    whens.push((vec![("theme".to_string(), t)], v));
                }
            }
            Ok::<_, String>((default, whens))
        })();
        match made_value {
            Ok((value, whens)) => {
                tokens.push(Mapped {
                    name: names[var].clone(),
                    value,
                    whens,
                    kind: Some(kinds::written(kind)),
                });
                made.push((kind, all));
            }
            Err(why) => {
                for i in &all {
                    report(unmapped, *i, why.clone());
                }
            }
        }
    }
    // `--radius`'s derived sizes.
    let radius = names
        .get("--radius")
        .filter(|r| tokens[start..].iter().any(|t| t.name == **r));
    let mut derived = Vec::new();
    if let Some(radius) = radius {
        for (size, expr) in [
            ("sm", "- 4px"),
            ("md", "- 2px"),
            ("lg", ""),
            ("xl", "+ 4px"),
        ] {
            let name = tamber_name(prefix, &["radius", size]);
            if tokens.iter().any(|t| t.name == name) {
                continue;
            }
            let (written, value) = match expr {
                "" => ("var(--radius)".to_string(), radius.clone()),
                e => (
                    format!("calc(var(--radius) {e})"),
                    format!("calc({radius} {e})"),
                ),
            };
            derived.push((format!("--radius-{size}"), written, tokens.len()));
            tokens.push(Mapped {
                name,
                value,
                whens: Vec::new(),
                kind: Some("length"),
            });
        }
    }
    let why = kinds::unfolded(tokens, axes, design, vocab);
    let mut mapped = 0;
    let record = |i: usize| {
        let (label, var, value, _) = &decls[i];
        Unmapped {
            block: label.clone(),
            name: var.clone(),
            value: value.clone(),
            why: String::new(),
            token: names.get(var).cloned(),
        }
    };
    for (k, (kind, carried)) in made.iter().enumerate() {
        match &why[start + k] {
            None => {
                mapped += carried.len();
                let from = carried.iter().map(|&i| record(i)).collect();
                sources.push((tokens[start + k].name.clone(), from));
            }
            Some(why) => {
                for i in carried {
                    report(unmapped, *i, format!("not a {kind} Tamber folds: {why}"));
                }
            }
        }
    }
    for (name, value, at) in &derived {
        if why[*at].is_none() {
            let derived = Unmapped {
                block: "(derived)".into(),
                name: name.clone(),
                value: value.clone(),
                why: String::new(),
                token: None,
            };
            sources.push((tokens[*at].name.clone(), vec![derived]));
        }
        if let Some(why) = &why[*at] {
            unmapped.push(Unmapped {
                block: "(derived)".into(),
                name: name.clone(),
                value: value.clone(),
                why: format!("derived from --radius, and not a length Tamber folds: {why}"),
                token: Some(tokens[*at].name.clone()),
            });
        }
    }
    let mut at = 0;
    tokens.retain(|_| {
        at += 1;
        why[at - 1].is_none()
    });
    mapped
}

fn looks_like_length(v: &str) -> bool {
    let v = v.trim();
    // A bare zero is a length (`0px`).
    if v.parse::<f64>().is_ok_and(|n| n == 0.0) {
        return true;
    }
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
    // A colour token: a role is a colour.
    let has = |name: &str| tokens.iter().any(|t| t.name == name && t.kind.is_none());
    let roles: Vec<(String, bool)> = vocab
        .roles
        .iter()
        .map(|r| (r.name.clone(), r.system))
        .collect();
    let varying = super::reach::names_reaching(design, &roles, tokens, &varies_here);
    let varies = |name: &str| varying.contains(name);
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
        if varies(&token) {
            out.push(Offer {
                role: role.into(),
                note: Some(format!("not offered: `{token}` varies by an app axis, and a role has one value, which the hosts' own surfaces read (LLP 1121 D5; the build refuses it, `tamber-role-axis`)")),
                token,
                ops: Vec::new(),
            });
            continue;
        }
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
        let named = values.flat_map(|v| words(v)).find(|w| varies(w));
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
    let split = super::split_roles(design, &roles, tokens, &out);
    out.extend(split);
    out
}

/// The axes the importer owns: their `when` lines are the source's.
const OWNED: &[&str] = &["theme"];

/// Whether a name varies by an app axis here, before what it names: a
/// `when` line the source gives it (`mapped`), or one the design keeps on
/// it (`have`) on an axis the importer does not own, or on a token it does
/// not map. A name varies once the import is accepted when it reaches one
/// that does (`reach::names_reaching`).
fn varies_here(mapped: Option<&Mapped>, have: Option<&crate::Token>) -> bool {
    let kept = have.is_some_and(|t| {
        t.whens
            .iter()
            .any(|w| mapped.is_none() || w.conds.iter().any(|(a, _)| !OWNED.contains(&a.as_str())))
    });
    kept || mapped.is_some_and(|t| !t.whens.is_empty())
}

/// The names a value may hold.
fn words(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .filter(|w| !w.is_empty())
}

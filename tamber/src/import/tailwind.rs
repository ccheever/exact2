//! Tailwind CSS v4 `@theme` (LLP 1121 D14, importer 2): the theme as
//! Tailwind builds it from each `@theme` block (in order: a reset clears
//! what it names, a `default` key yields to a value the key has, a later
//! value wins; an enclosing media query does not condition it), and the
//! stylesheet's own whole blocks: the root (`:root`, `html`) and its dark
//! side (`.dark`, `[data-theme=dark]`, `@media (prefers-color-scheme: dark)`,
//! `@variant dark`).
//!
//! - `--color-<name>` and `--color-<name>-<step>` become `<prefix>.<name>`
//!   and `<prefix>.<name>.s<step>`, read on both sides: a dark redefinition
//!   is the dark side of a `light-dark()`, and `var(--color-x)` a reference.
//!   A backing variable (`:root { --ink }`, its dark side) makes no token; a
//!   key that names it reads its colour on each side. An `inline` key's
//!   redefinition is reported: its utilities read its value.
//! - Spacing, radii, fonts, type, weights, leading, tracking, the three
//!   shadow kinds and easings are their kinds' tokens (stage 6 H):
//!   `space.<n>`, `radius.<n>`, `family.<n>`, `type.<n>` (`--text-<n>` and
//!   its `--line-height` and `--font-weight`, one type token), `weight.<n>`
//!   …, a `var()` the token it names. One that does not fold (a text
//!   shadow list), a sub-property a token holds no part of, and a key the
//!   stylesheet redefines outside `@theme` are reported.
//! - Breakpoints, containers, blur, drop shadows, perspective, aspect and
//!   animations are reported: no Tamber kind (breakpoints are layout, CSS's
//!   own).
//! - `--*: initial` and `--<namespace>-*: initial` are resets, honoured as
//!   Tailwind does (`--font-*` keeps `--font-weight-*`): what they clear is
//!   reported, and so is each reset.
//! - A rule narrower than a whole block (`:root:not(.dark)`, `.dark .card`,
//!   `.card { @variant dark { … } }`) is reported: it is no theme's.

use super::engine::{self, Decl, Fate};
use super::{counted, css_blocks, reconcile, tamber_name, Import, Unmapped};
use crate::{Design, Diagnostic};

/// What Tamber makes of a variable by its namespace.
enum Kind {
    /// A colour token, by these segments under the prefix.
    Colour(Vec<String>),
    /// A token of another kind: the kind and the token's segments.
    Other(&'static str, Vec<String>),
    /// No Tamber kind: why.
    Reported(&'static str),
}

/// `red-500` → `red`, `500`; `brand` → `brand`.
fn stepped(rest: &str) -> Vec<String> {
    match rest.rsplit_once('-') {
        Some((name, step))
            if !name.is_empty() && step.chars().all(|c| c.is_ascii_digit()) && !step.is_empty() =>
        {
            vec![name.to_string(), step.to_string()]
        }
        _ => vec![rest.to_string()],
    }
}

fn kind(var: &str) -> Kind {
    let seg = |group: &str, rest: &str| vec![group.to_string(), rest.to_string()];
    // A sub-property (`--text-xs--line-height`) belongs to its base's token.
    let base = |rest: &str| rest.split("--").next().unwrap_or(rest).to_string();
    let after = |p: &str| var.strip_prefix(p).filter(|r| !r.is_empty());
    if let Some(r) = after("--color-") {
        return Kind::Colour(stepped(r));
    }
    for (prefix, kind, group) in [
        ("--font-weight-", "number", "weight"),
        ("--text-shadow-", "text-shadow", "text-shadow"),
        ("--inset-shadow-", "box-shadow (inset)", "inset-shadow"),
        ("--shadow-", "box-shadow", "shadow"),
        ("--spacing-", "length", "space"),
        ("--radius-", "length", "radius"),
        ("--leading-", "number", "leading"),
        ("--tracking-", "length", "tracking"),
        ("--ease-", "easing", "ease"),
        ("--font-size-", "type", "type"),
        ("--font-", "family", "family"),
        ("--text-", "type", "type"),
    ] {
        if let Some(r) = after(prefix) {
            return Kind::Other(kind, seg(group, &base(r)));
        }
    }
    match var {
        "--spacing" => return Kind::Other("length", seg("space", "base")),
        "--radius" => return Kind::Other("length", vec!["radius".into()]),
        "--shadow" => return Kind::Other("box-shadow", vec!["shadow".into()]),
        _ => {}
    }
    for (prefix, why) in [
        (
            "--breakpoint-",
            "no Tamber kind: a breakpoint is layout, CSS's own",
        ),
        ("--container-", "no Tamber kind: a container size is layout"),
        ("--blur", "no Tamber kind: blur"),
        ("--drop-shadow", "no Tamber kind: drop-shadow"),
        ("--perspective-", "no Tamber kind: perspective"),
        ("--aspect-", "no Tamber kind: aspect ratio"),
        ("--animate-", "no Tamber kind: an animation"),
        ("--default-", "Tailwind's own default, no theme value"),
    ] {
        if var.starts_with(prefix) {
            return Kind::Reported(why);
        }
    }
    Kind::Reported(UNNAMED)
}

/// A variable in none of the namespaces Tamber's mapping names.
const UNNAMED: &str = "a namespace Tamber's Tailwind mapping does not name";

/// The project's `@custom-variant dark` when it is one Tamber does not read
/// as a whole dark block (`.dark`, `[data-theme=dark]`, the dark colour
/// scheme), as written; `None` when there is none or it is one of those.
fn custom_variant(css: &str) -> Option<String> {
    // The last `@custom-variant dark`, comments aside, any case and spacing.
    let text = super::strip_comments(css);
    let lower = text.to_ascii_lowercase();
    let mut def = None;
    let mut from = 0;
    while let Some(at) = lower[from..].find("@custom-variant") {
        let start = from + at + "@custom-variant".len();
        from = start;
        let rest = &text[start..];
        let name_at = rest.len() - rest.trim_start().len();
        let after = &rest[name_at..];
        if !after.to_ascii_lowercase().starts_with("dark") {
            continue;
        }
        let tail = &after[4..];
        if tail
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        let body = tail.trim_start();
        // `(selector)` to its close, or a block form, read whole.
        let (open, close) = match body.chars().next() {
            Some('(') => ('(', ')'),
            Some('{') => ('{', '}'),
            _ => continue,
        };
        let mut depth = 0;
        let mut end = body.len();
        for (i, c) in body.char_indices() {
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
        }
        def = Some(body[..end].trim().to_string());
    }
    let def = def?;
    let squashed: String = def
        .split_whitespace()
        .collect::<String>()
        .replace(['"', '\''], "");
    let known = [
        "(&:where(.dark,.dark*))",
        "(&:is(.dark*))",
        "(&:where(.dark*))",
        "(&:is(.dark,.dark*))",
        "(&:where([data-theme=dark],[data-theme=dark]*))",
        "(&:is([data-theme=dark]*))",
        "(&:where([data-theme=dark]*))",
        "(@media(prefers-color-scheme:dark))",
    ];
    (!known.contains(&squashed.to_ascii_lowercase().as_str())).then_some(def)
}

/// Whether a selector is a whole theme block: the root's (`None`), or its
/// dark side (`Some(true)`): `.dark`, `[data-theme=dark]`, alone or on the
/// root. Anything narrower (`:not(.dark)`, `.card`, a state) is an error.
fn block_of(selector: &str) -> Result<bool, String> {
    let mut dark = false;
    for word in selector.split_whitespace() {
        for part in engine::split_compound(word) {
            let unquoted = part.replace(['"', '\''], "");
            match unquoted.as_str() {
                ".dark" | "[data-theme=dark]" => dark = true,
                ":root" | "html" | "body" | ".light" => {}
                _ => {
                    return Err(format!(
                        "`{selector}` sets the variables on some elements only, not the theme"
                    ))
                }
            }
        }
    }
    Ok(dark)
}

/// Whether `reset` (`--*`, `--color-*`, or one name) clears `var`, as
/// Tailwind's theme does: a namespace keeps the keys another namespace owns
/// (`--font-*` keeps `--font-weight-*`).
fn resets(reset: &str, var: &str) -> bool {
    if reset == "--*" {
        return true;
    }
    let Some(ns) = reset.strip_suffix("-*") else {
        return var == reset;
    };
    let kept: &[&str] = match ns {
        "--font" => &["--font-weight", "--font-size"],
        "--inset" => &["--inset-shadow", "--inset-ring"],
        "--text" => &[
            "--text-color",
            "--text-decoration-color",
            "--text-decoration-thickness",
            "--text-indent",
            "--text-shadow",
            "--text-underline-offset",
        ],
        "--grid-column" => &["--grid-column-start", "--grid-column-end"],
        "--grid-row" => &["--grid-row-start", "--grid-row-end"],
        _ => &[],
    };
    var.starts_with(ns) && !kept.iter().any(|k| var.starts_with(k))
}

/// One theme declaration as read, before resets and defaults decide.
struct Entry {
    /// Its block's place in the stylesheet.
    seq: usize,
    block: String,
    name: String,
    value: String,
    dark: bool,
    default: bool,
    inline: bool,
}

/// Import `css` under `prefix` (`tw` by default), planned against
/// `design`.
pub fn import(css: &str, prefix: &str, design: &Design) -> Result<Import, Vec<Diagnostic>> {
    import_with(css, prefix, design, &crate::Vocab::default())
}

/// [`import`], the design checked against `vocab` where the import lands.
pub fn import_with(
    css: &str,
    prefix: &str,
    design: &Design,
    vocab: &crate::Vocab,
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
    let squash = |p: &str| {
        p.split_whitespace()
            .collect::<String>()
            .to_ascii_lowercase()
    };
    let dark_media = |p: &str| squash(p) == "@media(prefers-color-scheme:dark)";
    let variant_dark = |p: &str| squash(p) == "@variantdark";
    // The theme's declarations in source order (`@theme` keys, light), and
    // the stylesheet's own blocks (`:root`, `.dark` …), each as an entry.
    let mut theme: Vec<Entry> = Vec::new();
    let mut own: Vec<Entry> = Vec::new();
    // Where Tailwind writes the theme: at its first `@theme`.
    let mut first_theme = None;
    let custom_dark = custom_variant(css);
    for (seq, block) in css_blocks(css).into_iter().enumerate() {
        let label = block.context.join(" ");
        let at_theme = block.context.iter().position(|p| p.starts_with("@theme"));
        if let Some(i) = at_theme {
            first_theme.get_or_insert(seq);
            let options: Vec<&str> = block.context[i].split_whitespace().skip(1).collect();
            // `@theme` holds custom properties and keyframes; a rule inside
            // it is none of Tailwind's.
            let nested = block.context.len() > i + 1;
            for (name, value) in block.decls {
                source += 1;
                if nested {
                    let why = format!(
                        "inside `{}` within `@theme`, which Tailwind refuses",
                        block.context[i + 1..].join(" ")
                    );
                    report(&mut unmapped, &label, &name, &value, why, None);
                    continue;
                }
                // An enclosing media query does not condition a theme value.
                theme.push(Entry {
                    seq,
                    block: label.clone(),
                    name,
                    value,
                    dark: false,
                    default: options.contains(&"default"),
                    inline: options.contains(&"inline"),
                });
            }
            continue;
        }
        // The stylesheet's own block: the root's or its dark side, as a
        // whole; any narrower rule or other condition is no theme's.
        let rules: Vec<&String> = block
            .context
            .iter()
            .filter(|p| !p.starts_with('@'))
            .collect();
        let ats: Vec<&String> = block
            .context
            .iter()
            .filter(|p| p.starts_with('@'))
            .collect();
        let other = ats
            .iter()
            .find(|p| !p.starts_with("@layer") && !dark_media(p) && !variant_dark(p));
        let selectors = engine::nested(&rules);
        let side = if !rules.is_empty() && selectors.is_empty() {
            Err("a selector list too long to read".to_string())
        } else if selectors.iter().all(|s| s.trim().is_empty()) {
            Ok(false)
        } else {
            selectors
                .iter()
                .map(|s| block_of(s))
                .collect::<Result<Vec<bool>, String>>()
                .and_then(|sides| match sides.iter().all(|d| *d == sides[0]) {
                    true => Ok(sides[0]),
                    false => Err("a selector list of both the root and its dark side".to_string()),
                })
        };
        // `@variant dark` as the project defines it: one Tamber cannot follow
        // is no theme block.
        let variant = ats.iter().any(|p| variant_dark(p));
        for (name, value) in block.decls {
            source += 1;
            if let (true, Some(def)) = (variant, &custom_dark) {
                let why = format!(
                    "`@variant dark` is `{def}` here (`@custom-variant`), not a theme block"
                );
                report(&mut unmapped, &label, &name, &value, why, None);
                continue;
            }
            if let Some(cond) = other {
                report(
                    &mut unmapped,
                    &label,
                    &name,
                    &value,
                    format!("inside `{cond}`, which no theme block is"),
                    None,
                );
                continue;
            }
            match &side {
                Ok(dark) => own.push(Entry {
                    seq,
                    block: label.clone(),
                    name,
                    value,
                    dark: *dark || ats.iter().any(|p| dark_media(p) || variant_dark(p)),
                    default: false,
                    inline: false,
                }),
                Err(why) => report(&mut unmapped, &label, &name, &value, why.clone(), None),
            }
        }
    }
    // The theme as Tailwind builds it: in order, a reset clears what it
    // names, a default yields to a value the key has, a later value wins.
    let mut keys: Vec<(String, usize)> = Vec::new();
    let mut gone: Vec<(usize, String)> = Vec::new();
    for (k, e) in theme.iter().enumerate() {
        let held = |name: &str| keys.iter().find(|(n, _)| n == name).map(|(_, at)| *at);
        // A default yields to a value the key has, its own reset too; a
        // namespace reset clears whatever its declaration.
        if e.default && !e.name.ends_with("-*") {
            if let Some(at) = held(&e.name).filter(|at| !theme[*at].default) {
                let _ = at;
                gone.push((k, "a default, and the key has a value already".into()));
                continue;
            }
        }
        if e.value.trim().eq_ignore_ascii_case("initial") {
            keys.retain(|(name, at)| {
                let cleared = resets(&e.name, name);
                if cleared {
                    gone.push((*at, format!("reset by `{}: initial`", e.name)));
                }
                !cleared
            });
            gone.push((k, "a reset: what it names before it is not imported".into()));
            continue;
        }
        match keys.iter().position(|(name, _)| *name == e.name) {
            Some(i) => {
                gone.push((
                    keys[i].1,
                    "declared again later in the theme, which wins".into(),
                ));
                keys[i].1 = k;
            }
            None => keys.push((e.name.clone(), k)),
        }
    }
    for (k, why) in &gone {
        let e = &theme[*k];
        report(
            &mut unmapped,
            &e.block,
            &e.name,
            &e.value,
            why.clone(),
            None,
        );
    }
    let inline: Vec<String> = keys
        .iter()
        .filter(|(_, k)| theme[*k].inline)
        .map(|(n, _)| n.clone())
        .collect();
    // The stylesheet's own blocks: a namespace "reset" is no custom property
    // CSS keeps; a single `initial` makes the variable invalid on its side
    // (kept, so its side is no colour); a theme key's redefinition is that
    // side, unless the key is `inline` (its utilities read its value, not
    // it); any other variable is a backing one, read through references.
    let mut own_gone: Vec<Option<String>> = vec![None; own.len()];
    for (k, e) in own.iter().enumerate() {
        if e.name.ends_with("-*") {
            own_gone[k] = Some("a namespace reset outside `@theme`, which CSS drops".into());
        } else if inline.contains(&e.name) {
            own_gone[k] = Some(
                "an `inline` theme key: its utilities read its value, not this redefinition".into(),
            );
        }
    }
    // An inline key the stylesheet redefines: a key that reads it follows the
    // redefinition, which the key's own token does not; such a key is
    // reported, not mapped wrong.
    // The inline keys the stylesheet redefines, and what reads one, through
    // backing variables and redefinitions, to the end of the chain (the
    // inline key's own token stays).
    let first: Vec<String> = inline
        .iter()
        .filter(|n| own.iter().any(|e| e.name == **n))
        .cloned()
        .collect();
    let mut shadowed = first.clone();
    loop {
        let reads =
            |value: &str| engine::reference(value).is_some_and(|r| shadowed.iter().any(|n| n == r));
        let more: Vec<String> = keys
            .iter()
            .map(|(_, k)| &theme[*k])
            .chain(own.iter())
            .filter(|e| !shadowed.contains(&e.name) && reads(&e.value))
            .map(|e| e.name.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        shadowed.extend(more);
    }
    let tainted = |name: &String| shadowed.contains(name) && !first.contains(name);
    let dark_set: Vec<String> = own
        .iter()
        .enumerate()
        .filter(|(k, e)| e.dark && own_gone[*k].is_none())
        .map(|(_, e)| e.name.clone())
        .collect();
    let both = |name: &String, dark: bool| match (dark, dark_set.contains(name)) {
        (true, _) => vec![(None, true)],
        (false, true) => vec![(None, false)],
        (false, false) => vec![(None, false), (None, true)],
    };
    let mut decls: Vec<(usize, Decl)> = Vec::new();
    let mut others: Vec<Other> = Vec::new();
    let mut redefined: Vec<(String, String)> = Vec::new();
    let at_theme = first_theme.unwrap_or(0);
    for (_, k) in &keys {
        let e = &theme[*k];
        if let Some(r) = engine::reference(&e.value).filter(|_| tainted(&e.name)) {
            let why = format!("reads `{r}`, an `inline` key the stylesheet redefines, which its token cannot follow");
            report(&mut unmapped, &e.block, &e.name, &e.value, why, None);
            continue;
        }
        match kind(&e.name) {
            // A theme key is the root's, read on both sides: its references
            // resolve where each side reads them, and a dark redefinition
            // later takes the dark side.
            Kind::Colour(_) => decls.push((
                at_theme,
                (
                    e.block.clone(),
                    e.name.clone(),
                    e.value.clone(),
                    both(&e.name, false),
                ),
            )),
            // A variable in no namespace backs keys that name it.
            Kind::Reported(why) if why == UNNAMED => decls.push((
                at_theme,
                (
                    e.block.clone(),
                    e.name.clone(),
                    e.value.clone(),
                    both(&e.name, false),
                ),
            )),
            Kind::Other(kind, segs) => {
                let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
                others.push(Other {
                    block: e.block.clone(),
                    name: e.name.clone(),
                    value: e.value.clone(),
                    kind,
                    token: tamber_name(prefix, &refs),
                });
            }
            Kind::Reported(why) => report(
                &mut unmapped,
                &e.block,
                &e.name,
                &e.value,
                why.to_string(),
                None,
            ),
        }
    }
    let theme_keys: Vec<&String> = keys.iter().map(|(n, _)| n).collect();
    for (k, e) in own.iter().enumerate() {
        if tainted(&e.name) {
            let r = engine::reference(&e.value).unwrap_or_default();
            let why = format!("reads `{r}`, which follows an `inline` key the stylesheet redefines, which no token can follow");
            report(&mut unmapped, &e.block, &e.name, &e.value, why, None);
            continue;
        }
        if let Some(why) = &own_gone[k] {
            report(
                &mut unmapped,
                &e.block,
                &e.name,
                &e.value,
                why.clone(),
                None,
            );
            continue;
        }
        let backing = matches!(kind(&e.name), Kind::Colour(_) | Kind::Reported(UNNAMED));
        if theme_keys.contains(&&e.name) && !backing {
            report(
                &mut unmapped,
                &e.block,
                &e.name,
                &e.value,
                "a redefinition of a theme key that is no colour: its utilities follow it, which one token value cannot (and only a colour has a dark side)".into(),
                None,
            );
            redefined.push((e.name.clone(), e.block.clone()));
            continue;
        }
        // The root's variable is what the dark side inherits: a root
        // declaration sets both, unless the variable has a dark declaration,
        // which then holds the dark side (Tailwind's dark variant reaches the
        // descendants of `.dark`, whatever a later root rule says).
        let targets = both(&e.name, e.dark);
        decls.push((
            e.seq,
            (e.block.clone(), e.name.clone(), e.value.clone(), targets),
        ));
    }
    // In the stylesheet's order, the theme where Tailwind writes it.
    decls.sort_by_key(|(seq, _)| *seq);
    let decls: Vec<Decl> = decls.into_iter().map(|(_, d)| d).collect();
    let keyset: Vec<String> = keys.iter().map(|(n, _)| n.clone()).collect();
    let named = engine::name(&decls, |var| match kind(var) {
        // A colour the theme declares is a token; any other variable backs
        // one.
        Kind::Colour(segs) if keyset.contains(&var.to_string()) => {
            let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
            Some(tamber_name(prefix, &refs))
        }
        _ => None,
    });
    let (tokens, fates) = engine::assemble(&decls, &named, &[], &super::colour_literal);
    let mut mapped = 0;
    for ((label, var, value, _), fate) in decls.iter().zip(&fates) {
        let token = named.names.get(var).cloned();
        let (why, token) = match fate {
            Fate::Mapped => {
                mapped += 1;
                continue;
            }
            Fate::Aux => (
                "not a theme key: read only where a theme key names it".to_string(),
                None,
            ),
            Fate::Unresolved(t) => (
                format!(
                    "`{var}` is no colour in `{}`, where it is set too, so it makes no token",
                    engine::block_name(t)
                ),
                None,
            ),
            Fate::Collided(first) => (
                format!("its name is `{first}`'s ({})", named.names[first]),
                None,
            ),
            Fate::Superseded => (
                "declared again later for the same block, which wins".to_string(),
                None,
            ),
            Fate::NotColour if value.trim().is_empty() => ("an empty value".to_string(), None),
            Fate::NotColour if value.trim().eq_ignore_ascii_case("initial") => (
                "`initial` makes the variable invalid on its side".to_string(),
                None,
            ),
            Fate::NotColour => match engine::reference(value) {
                Some(r) => (
                    format!("`{r}` is no colour where this block reads it"),
                    token,
                ),
                None => match super::unfolded(value) {
                    Some(why) if exact_color::parse(value).is_some() => {
                        (format!("a colour Tamber does not fold yet: {why}"), token)
                    }
                    _ => ("not a colour".to_string(), token),
                },
            },
        };
        report(&mut unmapped, label, var, value, why, token);
    }
    let mut tokens = tokens;
    // Each token's source declarations, for a token refused where it lands.
    let mut sources: Vec<(String, Vec<Unmapped>)> = Vec::new();
    mapped += other_kinds(
        &others,
        &named.names,
        (&redefined, &shadowed),
        (design, vocab),
        (&mut tokens, &mut unmapped, &mut sources),
    );
    // Each colour's declarations mapped, by its token.
    for ((label, var, value, _), fate) in decls.iter().zip(&fates) {
        let Some(name) = named.names.get(var).filter(|_| *fate == Fate::Mapped) else {
            continue;
        };
        let record = Unmapped {
            block: label.clone(),
            name: var.clone(),
            value: value.clone(),
            why: String::new(),
            token: Some(name.clone()),
        };
        match sources.iter_mut().find(|(n, _)| n == name) {
            Some((_, from)) => from.push(record),
            None => sources.push((name.clone(), vec![record])),
        }
    }
    // What would not hold where it lands: its declarations reported.
    mapped -= super::landing::settle(
        &mut tokens,
        &sources,
        (&[], &[]),
        design,
        vocab,
        &mut unmapped,
    )?;
    let ops = reconcile(design, &[], &[], &tokens);
    let reported = unmapped.len();
    let counted = counted(source, mapped, reported)?;
    Ok(Import {
        ops,
        offers: Vec::new(),
        unmapped,
        counted,
    })
}

/// `value` with each `var()` replaced by what the key it names holds,
/// through a chain of them (16 deep at most).
fn written_out(
    value: &str,
    value_of: &dyn Fn(&str) -> Result<String, String>,
    depth: u8,
) -> Result<String, String> {
    if depth > 16 {
        return Err("a chain of `var()` past 16".into());
    }
    let failed = std::cell::RefCell::new(None);
    let out = super::kinds::bare(value, |var| {
        match value_of(var).and_then(|v| written_out(&v, value_of, depth + 1)) {
            Ok(v) => Some(v),
            Err(why) => {
                failed.borrow_mut().get_or_insert(why);
                None
            }
        }
    });
    match failed.into_inner() {
        Some(why) => Err(why),
        None => out,
    }
}

/// A theme key of a kind besides colour, as the theme leaves it.
struct Other {
    block: String,
    name: String,
    value: String,
    kind: &'static str,
    token: String,
}

/// The theme keys of the kinds besides colour as tokens, beside the colour
/// `tokens` (`colours` names each colour's variable's): a key and the
/// sub-properties beside it (`--text-xs--line-height`, a type's leading)
/// one token, each `var()` the token it names. Each key is mapped (the
/// count returned) or reported.
fn other_kinds(
    others: &[Other],
    colours: &std::collections::BTreeMap<String, String>,
    (redefined, shadowed): (&[(String, String)], &[String]),
    (design, vocab): (&Design, &crate::Vocab),
    (tokens, unmapped, sources): super::Written<'_>,
) -> usize {
    use super::kinds;
    let report = |unmapped: &mut Vec<Unmapped>, o: &Other, why: String| {
        unmapped.push(Unmapped {
            block: o.block.clone(),
            name: o.name.clone(),
            value: o.value.clone(),
            why,
            token: Some(o.token.clone()),
        });
    };
    // `--text-xs--line-height` → `--text-xs`, `line-height`.
    let split = |name: &str| match name[2..].split_once("--") {
        Some((base, sub)) => (format!("--{base}"), Some(sub.to_string())),
        None => (name.to_string(), None),
    };
    // Each token: its kind, its value, and the keys it carries.
    let mut made: Vec<(&'static str, Vec<&Other>)> = Vec::new();
    let start = tokens.len();
    // Each token's key, in the theme's order: a second key to one token is
    // reported.
    let mut owners: Vec<(&str, String)> = Vec::new();
    for o in others {
        let (base, _) = split(&o.name);
        if tokens[..start].iter().any(|t| t.name == o.token) {
            report(unmapped, o, format!("its name is a colour's ({})", o.token));
            continue;
        }
        match owners.iter().find(|(t, _)| *t == o.token) {
            Some((_, b)) if *b != base => {
                report(unmapped, o, format!("its token is `{b}`'s ({})", o.token));
            }
            Some(_) => {}
            None => owners.push((&o.token, base)),
        }
    }
    // A reference names the token of the key that owns it; a key another
    // owns, or one redefined outside `@theme`, makes none.
    let redefined_at = |var: &str| redefined.iter().find(|(n, _)| n == var).map(|(_, b)| b);
    let base_of: std::collections::BTreeMap<String, &str> = owners
        .iter()
        .filter(|(_, base)| redefined_at(base).is_none())
        .map(|(token, base)| (base.clone(), *token))
        .collect();
    // What a key holds: one that owns its token, or a sub-property of one.
    let value_of = |var: &str| match redefined_at(var) {
        Some(block) => Err(format!("`{var}` is redefined in `{block}`")),
        None => others
            .iter()
            .find(|o| o.name == var && base_of.contains_key(&split(var).0))
            .map(|o| o.value.trim().to_string())
            .ok_or(format!("`{var}` makes no token of another kind")),
    };
    let made_colours: Vec<String> = tokens.iter().map(|t| t.name.clone()).collect();
    let token_of = |var: &str| {
        colours
            .get(var)
            .filter(|n| made_colours.contains(n))
            .cloned()
            .or_else(|| base_of.get(var).map(|t| t.to_string()))
    };
    for (token, base) in owners {
        let keys: Vec<&Other> = others
            .iter()
            .filter(|o| o.token == token && split(&o.name).0 == base)
            .collect();
        let Some(head) = keys.iter().find(|o| o.name == base) else {
            for o in &keys {
                report(
                    unmapped,
                    o,
                    format!("beside `{base}`, which the theme does not declare"),
                );
            }
            continue;
        };
        // Any of its keys redefined outside `@theme`: its utilities follow
        // that, which one token value cannot.
        if let Some((name, block)) = keys
            .iter()
            .find_map(|o| redefined_at(&o.name).map(|b| (&o.name, b)))
        {
            for o in &keys {
                report(unmapped, o, format!("`{name}` is redefined in `{block}`, which its utilities follow and one token value cannot"));
            }
            continue;
        }
        // A key whose value reads an `inline` key the stylesheet redefines
        // (or what follows one), anywhere in it, follows that redefinition.
        if let Some((name, r)) = keys.iter().find_map(|o| {
            kinds::vars_in(&o.value)
                .into_iter()
                .find(|v| shadowed.contains(v))
                .map(|r| (&o.name, r))
        }) {
            for o in &keys {
                report(unmapped, o, format!("`{name}` reads `{r}`, which follows an `inline` key the stylesheet redefines, which its token cannot follow"));
            }
            continue;
        }
        let mut carried = vec![*head];
        // A type's size naming another type key is that key's size alone,
        // not the token its sub-properties make.
        // A type's size is written out, its `var()`s as what the keys hold:
        // another type key's is its size alone, not the token its
        // sub-properties make, and a length key's its length.
        let mut value = match head.kind {
            "type" => written_out(&head.value, &value_of, 0),
            _ => kinds::bare(&head.value, token_of),
        };
        if head.kind == "type" {
            let (mut weight, mut leading) = (None, None);
            for o in keys.iter().filter(|o| o.name != base) {
                match split(&o.name).1.as_deref() {
                    // A leading and a weight are written out, their `var()`s
                    // as what the keys hold: a type value's weight is a
                    // number, and its leading one word.
                    Some("line-height") => {
                        leading = Some(written_out(&o.value, &value_of, 0).and_then(|l| {
                            match l.contains(char::is_whitespace) || l.contains('(') {
                                true => kinds::folded("number", &l),
                                false => Ok(l),
                            }
                        }));
                        carried.push(o);
                    }
                    Some("font-weight") => {
                        weight = Some(written_out(&o.value, &value_of, 0));
                        carried.push(o);
                    }
                    _ => report(
                        unmapped,
                        o,
                        format!(
                            "a type token holds no `{}`",
                            split(&o.name).1.unwrap_or_default()
                        ),
                    ),
                }
            }
            value = (|| {
                let size = value?;
                let mut out = weight
                    .transpose()?
                    .map_or(String::new(), |w| format!("{w} "));
                out.push_str(&size);
                if let Some(l) = leading.transpose()? {
                    out.push_str(&format!("/{l}"));
                }
                Ok(out)
            })();
        } else {
            for o in keys.iter().filter(|o| o.name != base) {
                report(
                    unmapped,
                    o,
                    format!(
                        "a {} token holds no `{}`",
                        head.kind,
                        split(&o.name).1.unwrap_or_default()
                    ),
                );
            }
        }
        match value {
            Ok(value) => {
                tokens.push(crate::import::Mapped {
                    name: token.to_string(),
                    value,
                    whens: Vec::new(),
                    kind: Some(kinds::written(head.kind)),
                });
                made.push((head.kind, carried));
            }
            Err(why) => {
                for o in carried {
                    report(unmapped, o, why.clone());
                }
            }
        }
    }
    let why = kinds::unfolded(tokens, &[], design, vocab);
    let mut mapped = 0;
    let mut gone = Vec::new();
    for (i, why) in why.into_iter().enumerate().skip(start) {
        let (kind, carried) = &made[i - start];
        match why {
            None => {
                mapped += carried.len();
                let from = carried
                    .iter()
                    .map(|o| Unmapped {
                        block: o.block.clone(),
                        name: o.name.clone(),
                        value: o.value.clone(),
                        why: String::new(),
                        token: Some(o.token.clone()),
                    })
                    .collect();
                sources.push((tokens[i].name.clone(), from));
            }
            Some(why) => {
                for o in carried {
                    report(unmapped, o, format!("not a {kind} Tamber folds: {why}"));
                }
                gone.push(i);
            }
        }
    }
    let mut i = 0;
    tokens.retain(|_| {
        i += 1;
        !gone.contains(&(i - 1))
    });
    mapped
}

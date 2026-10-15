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
//!   shadow kinds and easings are `kind pending (stage 6)`, each with the
//!   token it would become (`space.<n>`, `radius.<n>`, `family.<n>`,
//!   `type.<n>`, `weight.<n>` …).
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
    /// A token of a kind stage 6 builds: the kind and the token's segments.
    Pending(&'static str, Vec<String>),
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
            return Kind::Pending(kind, seg(group, &base(r)));
        }
    }
    match var {
        "--spacing" => return Kind::Pending("length", seg("space", "base")),
        "--radius" => return Kind::Pending("length", vec!["radius".into()]),
        "--shadow" => return Kind::Pending("box-shadow", vec!["shadow".into()]),
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
    Kind::Reported("a namespace Tamber's Tailwind mapping does not name")
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
        _ => &[],
    };
    var.starts_with(ns) && !kept.iter().any(|k| var.starts_with(k))
}

/// One theme declaration as read, before resets and defaults decide.
struct Entry {
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
    for block in css_blocks(css) {
        let label = block.context.join(" ");
        let at_theme = block.context.iter().position(|p| p.starts_with("@theme"));
        if let Some(i) = at_theme {
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
        let side = if selectors.iter().all(|s| s.trim().is_empty()) {
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
        for (name, value) in block.decls {
            source += 1;
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
            Some(i) if e.default && !theme[keys[i].1].default => {
                gone.push((k, "a default, and the key has a value already".into()));
            }
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
    // The stylesheet's own blocks: a reset there clears what it names
    // before it on the same side; a theme key's redefinition is that side,
    // unless the key is `inline` (its utilities read its value, not it); any
    // other variable is a backing one, read only through references.
    let mut own_gone = vec![None; own.len()];
    for (k, e) in own.iter().enumerate() {
        if e.value.trim().eq_ignore_ascii_case("initial") {
            for (j, earlier) in own[..k].iter().enumerate() {
                if earlier.dark == e.dark && own_gone[j].is_none() && resets(&e.name, &earlier.name)
                {
                    own_gone[j] = Some(format!("reset by `{}: initial`", e.name));
                }
            }
            own_gone[k] = Some("a reset: what it names before it is not imported".into());
        } else if inline.contains(&e.name) {
            own_gone[k] = Some(
                "an `inline` theme key: its utilities read its value, not this redefinition".into(),
            );
        }
    }
    let mut decls: Vec<Decl> = Vec::new();
    for (_, k) in &keys {
        let e = &theme[*k];
        match kind(&e.name) {
            // A theme key is the root's, read on both sides: its references
            // resolve where each side reads them, and a dark redefinition
            // later takes the dark side.
            Kind::Colour(_) => decls.push((
                e.block.clone(),
                e.name.clone(),
                e.value.clone(),
                vec![(None, false), (None, true)],
            )),
            Kind::Pending(kind, segs) => {
                let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
                let token = tamber_name(prefix, &refs);
                report(
                    &mut unmapped,
                    &e.block,
                    &e.name,
                    &e.value,
                    format!("kind pending (stage 6): {kind}"),
                    Some(token),
                );
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
        let colour_key = matches!(kind(&e.name), Kind::Colour(_));
        if theme_keys.contains(&&e.name) && !colour_key {
            report(
                &mut unmapped,
                &e.block,
                &e.name,
                &e.value,
                "a redefinition of a theme key of a kind not built yet (stage 6)".into(),
                None,
            );
            continue;
        }
        decls.push((
            e.block.clone(),
            e.name.clone(),
            e.value.clone(),
            vec![(None, e.dark)],
        ));
    }
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
            Fate::Collided(first) => (
                format!("its name is `{first}`'s ({})", named.names[first]),
                None,
            ),
            Fate::Superseded => (
                "declared again later for the same block, which wins".to_string(),
                None,
            ),
            Fate::NotColour if value.trim().is_empty() => ("an empty value".to_string(), None),
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

//! Tailwind CSS v4 `@theme` (LLP 1121 D14, importer 2): the variables of
//! each `@theme` block (`@theme`, `@theme inline`, `@theme default …`), and
//! their dark redefinitions wherever the stylesheet makes them (`.dark { … }`,
//! `@media (prefers-color-scheme: dark)`, `@variant dark`).
//!
//! - `--color-<name>` and `--color-<name>-<step>` become `<prefix>.<name>`
//!   and `<prefix>.<name>.s<step>`; a dark redefinition is the dark side of
//!   a `light-dark()`; `var(--color-x)` a reference.
//! - Spacing, radii, fonts, type, weights, leading, tracking, the three
//!   shadow kinds and easings are `kind pending (stage 6)`, each with the
//!   token it would become (`space.<n>`, `radius.<n>`, `family.<n>`,
//!   `type.<n>`, `weight.<n>` …).
//! - Breakpoints, containers, blur, drop shadows, perspective, aspect and
//!   animations are reported: no Tamber kind (breakpoints are layout, CSS's
//!   own).
//! - `--*: initial` and `--<namespace>-*: initial` are resets, honoured: the
//!   names before them in their namespace are not imported, and each is
//!   reported.

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

/// A literal colour, on one line; `currentColor`, which no token can be, is
/// none.
fn literal(value: &str) -> Option<String> {
    let v = value.split_whitespace().collect::<Vec<_>>().join(" ");
    exact_color::parse(&v)
        .filter(|p| !matches!(p, exact_color::Parsed::Current))
        .filter(|_| super::unfolded(&v).is_none())
        .map(|_| v)
}

/// Whether a selector names the dark side: a `.dark` class, or
/// `[data-theme=dark]`.
fn dark_selector(selector: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    selector
        .match_indices(".dark")
        .any(|(at, m)| !selector[at + m.len()..].starts_with(ident))
        || selector
            .replace(['"', '\'', ' '], "")
            .contains("[data-theme=dark]")
}

/// Whether `reset` (`--*`, `--color-*`, or one name) resets `var`.
fn resets(reset: &str, var: &str) -> bool {
    match reset.strip_suffix('*') {
        Some(ns) => var.starts_with(ns) && var != reset,
        None => var == reset,
    }
}

/// Import `css` under `prefix` (`tw` by default), planned against
/// `design`.
pub fn import(css: &str, prefix: &str, design: &Design) -> Result<Import, Vec<Diagnostic>> {
    let mut source = 0;
    let mut unmapped = Vec::new();
    let mut report = |block: &str, name: &str, value: &str, why: String, token: Option<String>| {
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
    // Colour declarations for the engine; the rest reported as read.
    let mut decls: Vec<Decl> = Vec::new();
    // Every theme declaration in order, for resets: (index into decls when
    // a colour, its name, its block, its value).
    let mut theme_order: Vec<(Option<usize>, String, String, String)> = Vec::new();
    for block in css_blocks(css) {
        let label = block.context.join(" ");
        let in_theme = block.context.iter().any(|p| p.starts_with("@theme"));
        let dark = block.context.iter().any(|p| {
            let p = squash(p);
            p == "@media(prefers-color-scheme:dark)" || p == "@variantdark"
        }) || block
            .context
            .iter()
            .filter(|p| !p.starts_with('@'))
            .any(|p| dark_selector(p));
        let other = block.context.iter().find(|p| {
            let s = squash(p);
            p.starts_with('@')
                && !p.starts_with("@theme")
                && !p.starts_with("@layer")
                && s != "@media(prefers-color-scheme:dark)"
                && s != "@variantdark"
        });
        for (name, value) in block.decls {
            source += 1;
            if let Some(cond) = other {
                report(
                    &label,
                    &name,
                    &value,
                    format!("inside `{cond}`, which no theme block is"),
                    None,
                );
                continue;
            }
            if !in_theme && !dark {
                report(
                    &label,
                    &name,
                    &value,
                    "not an `@theme` block or a dark redefinition".into(),
                    None,
                );
                continue;
            }
            // A reset: honoured in the theme blocks before it.
            if value.trim() == "initial" {
                if in_theme && !dark {
                    theme_order.push((None, name.clone(), label.clone(), value.clone()));
                }
                report(
                    &label,
                    &name,
                    &value,
                    "a reset: what it names before it is not imported".into(),
                    None,
                );
                continue;
            }
            match kind(&name) {
                Kind::Colour(_) => {
                    if in_theme && !dark {
                        theme_order.push((
                            Some(decls.len()),
                            name.clone(),
                            label.clone(),
                            value.clone(),
                        ));
                    }
                    decls.push((label.clone(), name, value, vec![(None, dark)]));
                }
                Kind::Pending(k, segs) => {
                    let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
                    let token = tamber_name(prefix, &refs);
                    report(
                        &label,
                        &name,
                        &value,
                        format!("kind pending (stage 6): {k}"),
                        Some(token),
                    );
                }
                Kind::Reported(why) => report(&label, &name, &value, why.to_string(), None),
            }
        }
    }
    // Resets, in order: a colour declaration a later reset names is taken
    // out and reported.
    let mut reset_out = vec![false; decls.len()];
    for (i, (_, name, _, value)) in theme_order.iter().enumerate() {
        if value.trim() != "initial" {
            continue;
        }
        for (k, earlier, _, _) in &theme_order[..i] {
            if let Some(k) = k {
                if resets(name, earlier) {
                    reset_out[*k] = true;
                }
            }
        }
    }
    let mut kept: Vec<Decl> = Vec::new();
    for (k, d) in decls.into_iter().enumerate() {
        if reset_out[k] {
            let why = "reset by a later `initial` in its namespace".to_string();
            report(&d.0, &d.1, &d.2, why, None);
        } else {
            kept.push(d);
        }
    }
    let decls = kept;
    let named = engine::name(&decls, |var| match kind(var) {
        Kind::Colour(segs) => {
            let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
            tamber_name(prefix, &refs)
        }
        _ => String::new(),
    });
    let (tokens, fates) = engine::assemble(&decls, &named, &[], &literal);
    let mut mapped = 0;
    for ((label, var, value, _), fate) in decls.iter().zip(&fates) {
        let token = named.names.get(var).cloned();
        let (why, token) = match fate {
            Fate::Mapped => {
                mapped += 1;
                continue;
            }
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
        report(label, var, value, why, token);
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

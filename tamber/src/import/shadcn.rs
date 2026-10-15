//! shadcn/ui (LLP 1121 D14, importer 1): the `:root` and `.dark` blocks of a
//! `globals.css`, in v4's `oklch()` or v3's bare HSL channels
//! (`222.2 84% 4.9%`, read as `hsl()`).
//!
//! - Each colour variable becomes `<prefix>.<name>` (`--chart-1` →
//!   `.chart.s1`, `--sidebar-primary` → `.sidebar.primary`): `:root`'s value
//!   the light side, `.dark`'s the dark side, one `light-dark()`.
//! - A further theme class (`.theme-blue`, `[data-theme=blue]`, and its dark
//!   block `.dark .theme-blue`) is a proposed axis, `theme = default | blue`,
//!   with `when theme=blue` lines.
//! - `--radius` and other lengths are `kind pending (stage 6)`, with the
//!   tokens they would become (`radius`, and `radius.sm/md/lg/xl` as shadcn
//!   derives them).
//! - Role overrides are offered apart: `--background` as
//!   `-exact-background`, `--foreground` as `-exact-label`, `--card` as
//!   `-exact-secondary-background`, `--border` as `-exact-separator`, and
//!   `--primary` as the root's `accent-color` (D9, in Contract).
//! - Tailwind's own blocks (`@theme`, `@theme inline`), which map these
//!   variables to utilities, are not the source and are not read.

use super::{counted, css_blocks, reconcile, tamber_name, Import, Mapped, Offer, Unmapped};
use crate::edit::Op;
use crate::{Design, Diagnostic};
use std::collections::BTreeMap;

/// Which theme block a selector is: its theme (`None` the default) and
/// whether it is the dark one; `None` for a block that is no theme's.
fn theme_of(selector: &str) -> Option<(Option<String>, bool)> {
    let s = selector.split_whitespace().collect::<Vec<_>>().join(" ");
    let class = |w: &str| -> Option<String> {
        if let Some(t) = w.strip_prefix(".theme-") {
            return Some(t.to_string());
        }
        let inner = w.strip_prefix("[data-theme=")?.strip_suffix(']')?;
        Some(inner.trim_matches(|c| c == '"' || c == '\'').to_string())
    };
    match s.as_str() {
        ":root" | ".light" | ":root, .light" => return Some((None, false)),
        ".dark" | ":root.dark" | ".dark:root" => return Some((None, true)),
        _ => {}
    }
    // `.theme-x`, `.dark .theme-x`, `.theme-x.dark`, `.dark.theme-x`,
    // `.theme-x .dark`, and the `[data-theme=x]` spellings of each.
    let words: Vec<&str> = s.split(' ').collect();
    let mut theme = None;
    let mut dark = false;
    for w in &words {
        for part in split_compound(w) {
            match part {
                ".dark" => dark = true,
                ":root" | ".light" => {}
                p => theme = Some(class(p)?),
            }
        }
    }
    theme.map(|t| (Some(t), dark))
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

/// A source value as a Tamber colour value: v3's bare channels as `hsl()`,
/// `var(--x)` as the token `--x` becomes; `None` for no colour.
fn colour(value: &str, names: &BTreeMap<String, String>) -> Option<String> {
    let v = value.trim();
    if let Some(inner) = v.strip_prefix("var(").and_then(|r| r.strip_suffix(')')) {
        return names.get(inner.trim()).cloned();
    }
    if exact_color::parse(v).is_some() {
        return Some(v.to_string());
    }
    // `222.2 84% 4.9%` (and `/ 0.5`): hue, saturation, lightness.
    let (main, alpha) = v
        .split_once('/')
        .map_or((v, None), |(m, a)| (m, Some(a.trim())));
    let parts: Vec<&str> = main.split_whitespace().collect();
    let number = |p: &str| p.parse::<f64>().is_ok();
    let percent = |p: &str| p.strip_suffix('%').is_some_and(number);
    if parts.len() == 3 && number(parts[0]) && percent(parts[1]) && percent(parts[2]) {
        let hsl = match alpha {
            Some(a) => format!("hsl({} / {a})", parts.join(" ")),
            None => format!("hsl({})", parts.join(" ")),
        };
        return exact_color::parse(&hsl).is_some().then_some(hsl);
    }
    None
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
/// `design`.
pub fn import(css: &str, prefix: &str, design: &Design) -> Result<Import, Vec<Diagnostic>> {
    let mut source = 0;
    let mut unmapped = Vec::new();
    // theme (None the default) → light/dark → var → value.
    let mut themes: BTreeMap<Option<String>, [BTreeMap<String, String>; 2]> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut theme_order: Vec<String> = Vec::new();
    for (selector, decls) in css_blocks(css) {
        if selector.starts_with("@theme") {
            continue;
        }
        let Some((theme, dark)) = theme_of(&selector) else {
            for (name, value) in decls {
                source += 1;
                unmapped.push(Unmapped {
                    block: selector.clone(),
                    name,
                    value,
                    why: "not a theme block (`:root`, `.dark`, or a theme class)".into(),
                    token: None,
                });
            }
            continue;
        };
        if let Some(t) = &theme {
            if !theme_order.contains(t) {
                theme_order.push(t.clone());
            }
        }
        let side = &mut themes.entry(theme).or_default()[dark as usize];
        for (name, value) in decls {
            source += 1;
            if !order.contains(&name) {
                order.push(name.clone());
            }
            // A later declaration of the same name in the same block wins,
            // as in CSS; the earlier one is reported.
            if let Some(old) = side.insert(name.clone(), value) {
                unmapped.push(Unmapped {
                    block: selector.clone(),
                    name,
                    value: old,
                    why: "declared again later in the same block, which wins".into(),
                    token: None,
                });
            }
        }
    }
    let names: BTreeMap<String, String> = order
        .iter()
        .map(|v| {
            let segs = segments(v);
            let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
            (v.clone(), tamber_name(prefix, &refs))
        })
        .collect();
    let block_name = |theme: &Option<String>, dark: bool| match (theme, dark) {
        (None, false) => ":root".to_string(),
        (None, true) => ".dark".to_string(),
        (Some(t), false) => format!(".theme-{t}"),
        (Some(t), true) => format!(".dark .theme-{t}"),
    };
    // Each declaration read once: mapped when it is a colour, else reported.
    let mut mapped = 0;
    let mut colours: BTreeMap<(Option<String>, bool, String), String> = BTreeMap::new();
    for (theme, sides) in &themes {
        for (dark, side) in sides.iter().enumerate() {
            for (var, value) in side {
                match colour(value, &names) {
                    Some(c) => {
                        mapped += 1;
                        colours.insert((theme.clone(), dark == 1, var.clone()), c);
                    }
                    None => unmapped.push(Unmapped {
                        block: block_name(theme, dark == 1),
                        name: var.clone(),
                        value: value.clone(),
                        why: if value.trim().is_empty() {
                            "an empty value".into()
                        } else if looks_like_length(value) {
                            "kind pending (stage 6): length".into()
                        } else {
                            "not a colour, and no other kind is built yet (stage 6)".into()
                        },
                        token: (!value.trim().is_empty()).then(|| names[var].clone()),
                    }),
                }
            }
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
        let get =
            |theme: &Option<String>, dark: bool| colours.get(&(theme.clone(), dark, var.clone()));
        let default = pair(get(&None, false), get(&None, true));
        let mut whens = Vec::new();
        for t in &theme_order {
            let theme = Some(t.clone());
            if get(&theme, false).is_none() && get(&theme, true).is_none() {
                continue;
            }
            // A theme's missing side keeps the default theme's.
            let value = pair(
                get(&theme, false).or_else(|| get(&None, false)),
                get(&theme, true).or_else(|| get(&None, true)),
            );
            if let Some(v) = value.filter(|v| Some(v) != default.as_ref()) {
                whens.push((vec![("theme".to_string(), t.clone())], v));
            }
        }
        if let Some(value) = default.or_else(|| whens.first().map(|(_, v)| v.clone())) {
            tokens.push(Mapped {
                name: names[var].clone(),
                value,
                whens,
            });
        }
    }
    // `--radius`'s derived sizes, as shadcn computes them: reported with it.
    if themes
        .values()
        .any(|s| s[0].contains_key("--radius") || s[1].contains_key("--radius"))
    {
        for (size, expr) in [
            ("sm", "- 4px"),
            ("md", "- 2px"),
            ("lg", ""),
            ("xl", "+ 4px"),
        ] {
            let _ = expr;
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
    let axes = if theme_order.is_empty() {
        Vec::new()
    } else {
        let mut values = vec!["default".to_string()];
        values.extend(theme_order.iter().cloned());
        vec![("theme".to_string(), values)]
    };
    let ops = reconcile(design, &axes, &tokens);
    let offers = offers(prefix, design, &tokens);
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
/// alone; none where the design has it so already.
fn offers(prefix: &str, design: &Design, tokens: &[Mapped]) -> Vec<Offer> {
    let has = |name: &str| tokens.iter().any(|t| t.name == name);
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
        let op = match design.tokens.iter().find(|t| t.name.value == role) {
            Some(t) if t.value.value.trim() == token => continue,
            Some(_) => Op::SetToken {
                name: role.into(),
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
                "the root's `accent-color` in app.contract names `{primary}` (LLP 1121 D9): a Contract edit, made by hand"
            )),
        });
    }
    out
}

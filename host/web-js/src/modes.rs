//! @ref LLP 1121 D7 — the app's modes on the JS target, which keeps no kernel
//! at run time, so the browser picks each token's slice:
//!
//! - a token that varies by an app axis is an unregistered custom property,
//!   `--x-<name>` (`--x-s<n>` for a synthetic one), which a row names as
//!   `var()`; its value is substituted where a row uses it, so `light-dark()`
//!   and a role inside resolve at the using element;
//! - the page has one rule per full tuple, `[data-x-modes="axis=value,…"]`,
//!   declaring every such token; the root's rule is also `:root`'s, so the
//!   first paint has the default tuple's values;
//! - a boundary (a node with `-exact-mode`, `color-scheme`, or, when a varying
//!   token names the accent, `accent-color`, whose rule then declares it in
//!   the element's own accent) is `data-x-b`, holding the inherited colour
//!   rows it sets itself as bits (an accent alone, all); a literal pin is
//!   `data-x-pin`. The runtime (`modes.js`) gives each its full tuple and
//!   writes every inherited colour row it does not set again, from the text
//!   of the nearest ancestor's row when that row read a token: each class
//!   says, per such row, the text's index or that it read none.
//!
//! The rows are the template kernel's, projected twice: once as the plan
//! says, and once with each varying token bound to a sentinel colour no row
//! of the first projection writes, which is then each token's `var()`.

use exact_kernel::StyleId;
use exact_plan::{BindingKind, Plan};
use exact_runner::bridge;
use exact_web::host::template::Parts;
use std::fmt::Write as _;

/// The inherited colour rows a boundary writes again, by bit, and each
/// one's CSS property (`modes.js`'s `CH`). `SymbolPalette` is no CSS here.
pub const CHANNELS: [(StyleId, &str); 7] = [
    (StyleId::TextColor, "color"),
    (StyleId::CaretColor, "caret-color"),
    (StyleId::Fill, "fill"),
    (StyleId::Stroke, "stroke"),
    (StyleId::AccentColor, "accent-color"),
    (StyleId::TextShadow, "text-shadow"),
    (StyleId::TextStrokeColor, "-webkit-text-stroke-color"),
];

/// Whether `plan` has modes on the web: app axes, or a theme and a
/// `color-scheme` boundary, whose inherited token colours are written again.
pub fn active(plan: &Plan) -> bool {
    !plan.axes.is_empty()
        || !plan.theme.is_empty()
            && plan
                .bindings
                .iter()
                .any(|b| b.kind == BindingKind::Style && b.id == StyleId::ColorScheme as u16)
}

/// Each token that varies by an app axis, by its theme row's name, and its
/// custom property: its name with each character no identifier takes as
/// `-`, or `s<n>` for a synthetic token, `-<n>` added for a clash.
pub fn properties(plan: &Plan) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (i, row) in plan.theme.iter().enumerate() {
        if row.slices.len == 0 {
            continue;
        }
        let name = plan.str(row.name);
        let mut prop = match name.contains('(') {
            true => format!("--x-s{i}"),
            false => format!(
                "--x-{}",
                name.chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '-'
                    })
                    .collect::<String>()
            ),
        };
        if out.iter().any(|(_, p)| *p == prop) {
            prop = format!("{prop}-{i}");
        }
        out.push((name.to_string(), prop));
    }
    out
}

/// A plan's modes as the build writes them.
pub struct Modes {
    /// Each varying token: its name, its property and its sentinel's CSS.
    tokens: Vec<(String, String, [String; 2])>,
    /// Whether a varying token's value names the accent in some tuple: an
    /// element with its own `accent-color` declares them again.
    accent: bool,
    /// What the rows read from a token write, by index: a class's channel.
    pub texts: Vec<String>,
}

impl Modes {
    /// `None` when `plan` has no modes ([`active`]).
    pub fn new(plan: &Plan) -> Option<Modes> {
        if !active(plan) {
            return None;
        }
        let tokens = properties(plan)
            .into_iter()
            .map(|(name, prop)| (name, prop, Default::default()))
            .collect();
        let mut modes = Modes {
            tokens,
            accent: false,
            texts: Vec::new(),
        };
        modes.accent = modes.rule(plan).1;
        Some(modes)
    }

    /// The tree's parts (`style::project`) with every varying token a
    /// `var()`, each inherited colour row's channel in its CSS, and the
    /// boundaries' attributes.
    pub fn project(
        &mut self,
        plan: &Plan,
        sites: &crate::emit::Sites,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<Option<Parts>>, String> {
        let mut parts = crate::style::project(plan, sites, warnings)?;
        if !self.tokens.is_empty() {
            let plain = written(&parts);
            // A red byte whose sentinels the plain projection never writes.
            let red = (0..=0xffu32)
                .map(|k| (0xe1 + k) & 0xff)
                .find(|r| {
                    (0..self.tokens.len()).all(|i| {
                        sentinel(*r, i)
                            .iter()
                            .all(|form| !plain.contains(form.as_str()))
                    })
                })
                .ok_or("no sentinel colour is free of the app's own colours")?;
            for (i, (name, _, forms)) in self.tokens.iter_mut().enumerate() {
                *forms = sentinel(red, i);
                bridge::edit_token(plan, name, &forms[0], &[]);
            }
            let projected = crate::style::project(plan, sites, &mut Vec::new());
            // The plan's own tokens again (an edit is the runner's, D13).
            let _ = bridge::set_role_overrides(plan);
            parts = projected?;
            for p in parts.iter_mut().flatten() {
                p.css = self.substitute(&p.css);
                p.props.retain(|_, v| {
                    *v = self.substitute(v);
                    true
                });
            }
        }
        for (i, node) in plan.nodes.iter().enumerate() {
            if let Some(p) = parts[i].as_mut() {
                self.boundary(plan, node, p);
            }
        }
        Ok(parts)
    }

    /// `text` with each sentinel its token's `var()`.
    fn substitute(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (_, prop, forms) in &self.tokens {
            for form in forms {
                if out.contains(form.as_str()) {
                    out = out.replace(form.as_str(), &format!("var({prop})"));
                }
            }
        }
        out
    }

    /// A node's channels, and its boundary attributes when it is one.
    fn boundary(&mut self, plan: &Plan, node: &exact_plan::NodesRow, p: &mut Parts) {
        let rows: Vec<_> = node
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .filter(|b| b.kind == BindingKind::Style)
            .collect();
        let has = |id: StyleId| rows.iter().find(|b| b.id == id as u16);
        let mut own = 0u32;
        let mut channels = String::new();
        for (j, (id, prop)) in CHANNELS.iter().enumerate() {
            let declared = declaration(&p.css, prop).map(str::to_string);
            if has(*id).is_some() || declared.is_some() {
                own |= 1 << j;
            }
            let Some(value) = declared else { continue };
            // What the literal read: a token, or none (a host's own colour).
            let named = has(*id).is_some_and(|b| {
                crate::style::literal(plan, plan.code(b.expr)).is_some_and(|v| {
                    exact_kernel::style::theme::take_named();
                    let mut scratch = exact_kernel::StyleProps::default();
                    let ok = bridge::set_binding_style(&mut scratch, b, &v, plan).is_ok();
                    exact_kernel::style::theme::take_named() != 0 && ok
                })
            });
            let at = match named {
                true => self.text(&value) as i64,
                false => -1,
            };
            let _ = write!(channels, "--x-i{j}:{at};");
        }
        p.css.push_str(&channels);
        let pin = has(StyleId::Mode);
        let accent = self.accent && has(StyleId::AccentColor).is_some();
        // An accent alone declares the tokens again and writes no row: an
        // inherited colour is the one its parent computed, as the kernel's.
        if pin.is_some() || has(StyleId::ColorScheme).is_some() {
            p.props.insert("data-x-b".into(), own.to_string());
        } else if accent {
            p.props
                .insert("data-x-b".into(), ((1 << CHANNELS.len()) - 1).to_string());
        }
        if let Some(text) = pin.and_then(|b| crate::style::literal(plan, plan.code(b.expr))) {
            if let Some(text) = text.as_str() {
                p.props.insert("data-x-pin".into(), text.to_string());
            }
        }
    }

    /// The index of `text` in [`Modes::texts`].
    fn text(&mut self, text: &str) -> usize {
        match self.texts.iter().position(|t| t == text) {
            Some(i) => i,
            None => {
                self.texts.push(text.to_string());
                self.texts.len() - 1
            }
        }
    }

    /// The page's tuple rules, and whether a token names the accent.
    fn rule(&self, plan: &Plan) -> (String, bool) {
        let axes: Vec<(&str, Vec<&str>)> = plan
            .axes
            .iter()
            .map(|a| (plan.str(a.name), plan.str(a.values).split('|').collect()))
            .collect();
        let _profiles = exact_kernel::style::profiled::declarations(
            plan.profiles
                .iter()
                .map(|row| (plan.str(row.name), plan.str(row.src), plan.str(row.intent))),
        );
        let (mut css, mut accent) = (String::new(), false);
        if axes.is_empty() {
            return (css, accent);
        }
        for tuple in 0..bridge::tuples(plan) {
            let table = bridge::theme_table_at(plan, tuple);
            let selector = format!("[data-x-modes=\"{}\"]", key(&axes, tuple));
            css.push_str(&if tuple == 0 {
                format!(":root,{selector}{{")
            } else {
                format!("{selector}{{")
            });
            for (name, prop, _) in &self.tokens {
                let Some((_, value, _)) = table.iter().find(|(n, _, role)| **n == **name && !role)
                else {
                    continue;
                };
                let colour = exact_kernel::ColorValue::parse_light_dark(value).or_else(|| {
                    exact_kernel::Color::parse(value).map(exact_kernel::ColorValue::Fixed)
                });
                let mut text = String::new();
                match colour {
                    Some(c) => exact_kernel::gradient::color_css(&mut text, c),
                    None => text.push_str("transparent"),
                }
                accent |= text.contains("--exact-accent");
                let _ = write!(css, "{prop}:{text};");
            }
            css.push('}');
        }
        (css, accent)
    }

    /// The page's CSS for its modes.
    pub fn css(&self, plan: &Plan) -> String {
        self.rule(plan).0
    }

    /// The module's call that installs the runtime (`modes.js`), each class
    /// named by `classes` index plus one (`c<n>`): the axes, each class's
    /// channels, the texts, and each class's properties that name a token.
    pub fn js(&self, plan: &Plan, classes: &[String]) -> String {
        let axes: Vec<String> = plan
            .axes
            .iter()
            .map(|a| {
                let values: Vec<&str> = plan.str(a.values).split('|').collect();
                format!(
                    "[{},{}]",
                    json(plan.str(a.name)),
                    serde_json::to_string(&values).unwrap()
                )
            })
            .collect();
        // Each class's declarations that name a varying token, which a
        // page rendered ahead wrote inline as their default tuple's values.
        let (mut channels, mut vars) = (Vec::new(), Vec::new());
        for (i, c) in classes.iter().enumerate() {
            let mut each = Vec::new();
            for j in 0..CHANNELS.len() {
                let tag = format!("--x-i{j}:");
                if let Some(at) = c.find(&tag) {
                    let rest = &c[at + tag.len()..];
                    each.push(format!(
                        "{j}:{}",
                        &rest[..rest.find(';').unwrap_or(rest.len())]
                    ));
                }
            }
            if !each.is_empty() {
                channels.push(format!("c{}:{{{}}}", i + 1, each.join(",")));
            }
            let named: Vec<&str> = declarations(c)
                .into_iter()
                .filter(|(_, value)| value.contains("var(--x-"))
                .map(|(name, _)| name)
                .collect();
            if !named.is_empty() {
                vars.push(format!(
                    "c{}:{}",
                    i + 1,
                    serde_json::to_string(&named).unwrap()
                ));
            }
        }
        format!(
            "$modes([{}],{{{}}},{},{{{}}});",
            axes.join(","),
            channels.join(","),
            serde_json::to_string(&self.texts).unwrap(),
            vars.join(",")
        )
    }
}

/// A full tuple's key: every axis as `axis=value`, as declared, by commas.
fn key(axes: &[(&str, Vec<&str>)], tuple: u32) -> String {
    let mut rest = tuple;
    let mut pins = vec![String::new(); axes.len()];
    for (i, (name, values)) in axes.iter().enumerate().rev() {
        pins[i] = format!("{name}={}", values[rest as usize % values.len()]);
        rest /= values.len() as u32;
    }
    pins.join(",")
}

/// Token `i`'s sentinel: an opaque colour with red `red`, its index in green
/// and blue, as the kernel writes a colour and as the web host's CSS does
/// (`exact_web::css::rgba`).
fn sentinel(red: u32, i: usize) -> [String; 2] {
    [
        format!("#{red:02x}{i:04x}ff"),
        format!("rgba({red},{},{},1)", i >> 8, i & 0xff),
    ]
}

/// Everything the parts write, to look for a sentinel in.
fn written(parts: &[Option<Parts>]) -> String {
    let mut out = String::new();
    for p in parts.iter().flatten() {
        out.push_str(&p.css);
        for v in p.props.values() {
            out.push_str(v);
        }
    }
    out
}

/// The value of `css`'s top-level declaration of `prop`, the last one.
fn declaration<'a>(css: &'a str, prop: &str) -> Option<&'a str> {
    declarations(css)
        .into_iter()
        .rev()
        .find(|(name, _)| *name == prop)
        .map(|(_, value)| value)
}

/// `css`'s top-level declarations, `(property, value)`, in order.
fn declarations(css: &str) -> Vec<(&str, &str)> {
    let mut depth = 0i32;
    let mut start = 0;
    let mut out = Vec::new();
    for (i, c) in css.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ';' if depth == 0 => {
                if let Some((name, value)) = css[start..i].split_once(':') {
                    out.push((name.trim(), value.trim()));
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    out
}

fn json(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

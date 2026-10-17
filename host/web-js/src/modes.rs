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
//! says, and once with each varying token bound to a sentinel no row of the
//! first projection writes (a colour, a length or a number, by its kind:
//! `measures.rs`), which is then each token's `var()`.
//!
//! A development build (`--dev-reload`, LLP 1121 stage 4) makes every token
//! that overrides no role a property, varying or not, so the live table can
//! set any of them in place ([`every`], [`design`]); a release build folds
//! the others inline, as before, byte for byte.

use exact_kernel::style::theme::Kind;
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

thread_local! {
    /// A development build's: every token a property ([`every`]).
    static EVERY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Make every token that overrides no role a property while the returned
/// guard lives (a development build, LLP 1121 stage 4); restored after.
pub fn every(on: bool) -> impl Drop {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            EVERY.with(|e| e.set(self.0));
        }
    }
    Restore(EVERY.with(|e| e.replace(on)))
}

fn every_token() -> bool {
    EVERY.with(std::cell::Cell::get)
}

/// Whether `plan` has modes on the web: app axes, or a theme and a
/// `color-scheme` boundary, whose inherited token colours are written again;
/// in a development build, any theme.
pub fn active(plan: &Plan) -> bool {
    every_token() && !plan.theme.is_empty()
        || !plan.axes.is_empty()
        || !plan.theme.is_empty()
            && plan
                .bindings
                .iter()
                .any(|b| b.kind == BindingKind::Style && b.id == StyleId::ColorScheme as u16)
}

/// Each token that varies by an app axis, by its theme row's name, and its
/// custom property: its name with each character no identifier takes as
/// `-`, or `s<n>` for a synthetic token, `-<n>` added until it is unique.
/// In a development build every token, and a role override (not the tint's,
/// which rows keep as the role) as its role's sheet property (LLP 1121
/// stage 5), so a row naming the role reads the property a live edit sets.
pub fn properties(plan: &Plan) -> Vec<(String, String)> {
    use exact_kernel::style::roles;
    let mut out: Vec<(String, String)> = Vec::new();
    // A number token a font weight names stays inline (`measures`).
    let integer = if every_token() {
        crate::measures::inline_tokens(plan)
    } else {
        Vec::new()
    };
    for (i, row) in plan.theme.iter().enumerate() {
        let role = roles::role(plan.str(row.role));
        if crate::measures::kind(row).is_none()
            || row.slices.len == 0 && !every_token()
            || role.is_some_and(roles::tint)
            || integer.iter().any(|n| n == plan.str(row.name))
        {
            continue;
        }
        let name = plan.str(row.name);
        if let Some(role) = role.and_then(roles::role_of) {
            out.push((name.to_string(), roles::theme_property(role)));
            continue;
        }
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
        let base = prop.clone();
        let mut n = i;
        while out.iter().any(|(_, p)| *p == prop) {
            prop = format!("{base}-{n}");
            n += 1;
        }
        out.push((name.to_string(), prop));
    }
    out
}

/// A plan's modes as the build writes them.
pub struct Modes {
    /// Each varying token: its name, its property and its sentinel's texts
    /// (the value first, then the CSS it writes).
    tokens: Vec<(String, String, Vec<String>)>,
    /// The varying tokens whose value names the accent in some tuple, by
    /// property: an element with its own `accent-color` declares them again.
    accent: Vec<String>,
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
            accent: Vec::new(),
            texts: Vec::new(),
        };
        modes.accent = rules(plan, &properties(plan)).1;
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
            let kinds: Vec<Kind> = self
                .tokens
                .iter()
                .map(|(name, ..)| {
                    plan.theme
                        .iter()
                        .find(|r| plan.str(r.name) == name)
                        .and_then(crate::measures::kind)
                        .unwrap_or(Kind::Color)
                })
                .collect();
            let forms = |k: u32, i: usize| match kinds[i] {
                Kind::Color => sentinel((0xe1 + k) & 0xff, i).to_vec(),
                kind => crate::measures::sentinel(kind, k, i),
            };
            // An offset whose sentinels the plain projection never writes,
            // and none of which writes another's.
            let k = (0..=0xffu32)
                .find(|k| {
                    let all: Vec<Vec<String>> =
                        (0..self.tokens.len()).map(|i| forms(*k, i)).collect();
                    all.iter().enumerate().all(|(i, f)| {
                        f.iter().all(|form| {
                            !plain.contains(form.as_str())
                                && all.iter().enumerate().all(|(j, g)| {
                                    i == j || g.iter().all(|other| !other.contains(form.as_str()))
                                })
                        })
                    })
                })
                .ok_or("no sentinel is free of the app's own values")?;
            for (i, (name, _, f)) in self.tokens.iter_mut().enumerate() {
                *f = forms(k, i);
                bridge::edit_token(plan, name, &f[0], &[]);
            }
            let projected = crate::style::project(plan, sites, &mut Vec::new());
            // The plan's own tokens again (an edit is the runner's, D13).
            let _ = bridge::role_overrides(plan);
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
        // The rows its CSS sets: a bound one is the runtime's to say (`Sx`).
        let mut own = 0u32;
        let mut channels = String::new();
        for (j, (id, prop)) in CHANNELS.iter().enumerate() {
            // A clear sets no row: the one around reaches through it.
            let Some(value) = declaration(&p.css, prop)
                .filter(|v| {
                    !(v.eq_ignore_ascii_case("unset")
                        || v.eq_ignore_ascii_case("inherit")
                        || *id == StyleId::TextColor && v.eq_ignore_ascii_case("currentcolor"))
                })
                .map(str::to_string)
            else {
                continue;
            };
            own |= 1 << j;
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
                true => self.text(&format!("{prop}:{value}")) as i64,
                false => -1,
            };
            let _ = write!(channels, "--xi{j}:{at};");
        }
        p.css.push_str(&channels);
        let pin = has(StyleId::Mode);
        let boundary = pin.is_some() || has(StyleId::ColorScheme).is_some();
        // An accent that is a token naming the accent: on a boundary, which
        // declares the token itself, `--exact-accent` would name itself (as
        // css.rs leaves `var(--exact-accent, AccentColor)` out), so the one
        // around stands; elsewhere it reads the token from above.
        let named = declaration(&p.css, "accent-color").and_then(|v| {
            self.accent
                .iter()
                .find(|a| v.contains(&format!("var({a})")))
                .cloned()
        });
        let names_accent = named.is_some();
        if let Some(prop) = named.filter(|_| boundary) {
            if let Some(v) = declaration(&p.css, "--exact-accent").map(str::to_string) {
                p.css = p.css.replace(&format!("--exact-accent:{v};"), "");
            }
            p.props.insert("data-x-accent".into(), prop);
        }
        let accent =
            !self.accent.is_empty() && has(StyleId::AccentColor).is_some() && !names_accent;
        // An accent alone declares the tokens again and writes no row: an
        // inherited colour is the one its parent computed, as the kernel's.
        if boundary {
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

    /// The page's CSS for its modes.
    pub fn css(&self, plan: &Plan) -> String {
        rules(plan, &properties(plan)).0
    }

    /// The module's call that installs the runtime (`modes.js`), each class
    /// named by `classes` index plus one (`c<n>`): the axes, each class's
    /// channels, the texts, each class's properties that name a token, the
    /// properties of the tokens that name the accent, and each varying
    /// token's name by its property (`layout <id>`).
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
                let tag = format!("--xi{j}:");
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
            // A development build's role overrides read their sheet
            // property too (LLP 1121 stage 5): a page rendered ahead drops
            // its baked value for it, as for a token's.
            let named: Vec<&str> = declarations(c)
                .into_iter()
                .filter(|(_, value)| {
                    value.contains("var(--x-") || every_token() && value.contains("var(--exact-")
                })
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
        // A development build's design and structure digests, for the live
        // table's base and the batches its page takes.
        let design = match every_token() {
            true => format!(
                ",{},{}",
                json(&exact_runner::design::digest(
                    &exact_runner::design::tokens(plan)
                )),
                json(&exact_runner::design::shape(plan))
            ),
            false => String::new(),
        };
        format!(
            "$modes([{}],{{{}}},{},{{{}}},{},{{{}}}{design});",
            axes.join(","),
            channels.join(","),
            serde_json::to_string(&self.texts).unwrap(),
            vars.join(","),
            serde_json::to_string(&self.accent).unwrap(),
            self.tokens
                .iter()
                .map(|(name, prop, _)| format!("{}:{}", json(prop), json(name)))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

/// The page's tuple rules for the varying tokens `props` (name, property),
/// and the properties of those whose value names the accent in some tuple.
/// A boundary whose own accent is one (`data-x-accent`) declares
/// `--exact-accent` in each tuple where that token is a colour of its own.
fn rules(plan: &Plan, props: &[(String, String)]) -> (String, Vec<String>) {
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
    let (mut css, mut accent) = (String::new(), Vec::new());
    if axes.is_empty() && !every_token() {
        return (css, accent);
    }
    let mut texts = Vec::new();
    for tuple in 0..bridge::tuples(plan) {
        let table = bridge::theme_table_at(plan, tuple);
        let selector = format!("[data-x-modes=\"{}\"]", key(&axes, tuple));
        css.push_str(&if tuple == 0 {
            format!(":root,{selector}{{")
        } else {
            format!("{selector}{{")
        });
        for (name, prop) in props {
            let Some(entry) = table.iter().find(|e| *e.name == **name && !e.role) else {
                continue;
            };
            let text = css_of(entry);
            if text.contains("--exact-accent") && !accent.contains(prop) {
                accent.push(prop.clone());
            }
            let _ = write!(css, "{prop}:{text};");
            texts.push((selector.clone(), prop.clone(), text));
        }
        css.push('}');
    }
    for (selector, prop, text) in texts {
        if accent.contains(&prop) && !text.contains("--exact-accent") {
            let _ = write!(
                css,
                "{selector}[data-x-accent=\"{prop}\"]{{--exact-accent:{text}}}"
            );
        }
    }
    (css, accent)
}

/// A token's text as the page's rules declare it: a colour's CSS, a length's
/// or number's (`measures::css`).
fn css_of(entry: &exact_kernel::style::theme::Entry) -> String {
    if entry.kind != Kind::Color {
        return crate::measures::css(entry.kind, &entry.value);
    }
    let value = &*entry.value;
    let colour = exact_kernel::ColorValue::parse_light_dark(value)
        .or_else(|| exact_kernel::Color::parse(value).map(exact_kernel::ColorValue::Fixed));
    let mut text = String::new();
    match colour {
        Some(c) => exact_kernel::gradient::color_css(&mut text, c),
        None => text.push_str("transparent"),
    }
    text
}

/// A development build's design for the live table (LLP 1121 stage 4): the
/// plan's structure and design digests and each token's wire text, and for
/// each token the page declares, its property and its CSS per full tuple, as
/// [`rules`] writes them. The dev loop sends the changed ones as a batch.
pub fn design(plan: &Plan) -> serde_json::Value {
    use exact_runner::design;
    // The app's profiles, as `rules` reads a profile colour.
    let _profiles = exact_kernel::style::profiled::declarations(
        plan.profiles
            .iter()
            .map(|row| (plan.str(row.name), plan.str(row.src), plan.str(row.intent))),
    );
    let tokens = design::tokens(plan);
    let props = properties(plan);
    let inline = crate::measures::inline_tokens(plan);
    let tables: Vec<_> = (0..bridge::tuples(plan))
        .map(|tuple| bridge::theme_table_at(plan, tuple))
        .collect();
    let rows: Vec<serde_json::Value> = tokens
        .iter()
        .map(|t| {
            let mut row = serde_json::json!({"name": t.name, "value": t.value, "slices": t.slices});
            if let Some((_, prop)) = props.iter().find(|(n, _)| *n == t.name) {
                let css: Vec<String> = tables
                    .iter()
                    .map(|table| {
                        table
                            .iter()
                            .find(|e| *e.name == *t.name && !e.role)
                            .map_or_else(|| "transparent".to_string(), css_of)
                    })
                    .collect();
                // Which tuples name the accent: the build writes a companion
                // rule where one does not, and lists the token as one that
                // names it where any does (`rules`); a change is a rebuild.
                let accent: Vec<bool> = css.iter().map(|c| c.contains("--exact-accent")).collect();
                row["prop"] = prop.clone().into();
                row["css"] = css.into();
                row["accent"] = accent.into();
            } else if inline.contains(&t.name) {
                // Written inline: a change builds (`measures`).
                row["inline"] = true.into();
            } else if let Some(prop) = tint_property(plan, &t.name) {
                // The tint's override: its property in the theme's own
                // `:root` rule (`root`), which rows read as the role and an
                // authored `accent-color` redeclares below (LLP 1121 stage
                // 5); a tuple's rule would mask an authored one above it.
                row["prop"] = prop.into();
                row["css"] = vec![css_of(&exact_kernel::style::theme::Entry {
                    name: t.name.as_str().into(),
                    value: t.value.as_str().into(),
                    role: false,
                    kind: Kind::Color,
                })]
                .into();
                row["root"] = true.into();
            }
            row
        })
        .collect();
    serde_json::json!({
        "structure": design::structure(plan),
        "shape": design::shape(plan),
        "digest": design::digest(&tokens),
        "tokens": rows,
    })
}

/// The sheet property of the tint's override `name` (`AccentColor`), if it
/// is one: [`properties`] leaves it out, as rows keep the tint as the role.
fn tint_property(plan: &Plan, name: &str) -> Option<String> {
    use exact_kernel::style::roles;
    let row = plan.theme.iter().find(|r| plan.str(r.name) == name)?;
    let id = roles::role(plan.str(row.role)).filter(|id| roles::tint(*id))?;
    roles::role_of(id).map(roles::theme_property)
}

/// The varying tokens whose value names the accent in some tuple, by
/// property (rows.rs: a boundary's bound accent naming one).
pub fn accent_properties(plan: &Plan) -> Vec<String> {
    rules(plan, &properties(plan)).1
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

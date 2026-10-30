//! A param's levels (LLP 1121 stage 12 B2-levels, *1.15*): one walker over
//! a param's value, per tuple, from the app's own value of it (`params`)
//! or else its default, through each bare reference it makes (a param, an
//! app token, a package token), at most [`HOPS`] hops, to the line whose
//! value is no reference (a literal or an expression), a base name or a
//! role. `explain` and `resolve` of a param say its `levels`, `resolve`'s
//! tuples each their winning `level` and `name` per side (`{light, dark}`),
//! and `describe`'s param rows what their default `reaches`.

use crate::tamber_params::AppParams;
use contract_lower::params::{self, ParamToken};
use serde_json::{json, Value};

/// The most references the walker follows: past it, the line it is on wins.
pub const HOPS: usize = 8;

/// Where a tuple's value comes from: the level (`app-param`, `app-token`,
/// `package`, `base`, `role`, or `default`: the param's own default, a
/// literal or an expression) and the name at that level (a param's full
/// name, a token's, a base name, a role's spelling).
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// `app-param`, `app-token`, `package`, `base`, `role` or `default`.
    pub level: &'static str,
    /// The name at that level.
    pub name: String,
}

/// The designs a walk reads: the app's (its file and text) and each package
/// design, parsed once.
pub struct Designs<'a> {
    /// The app's design file, as answers name it.
    pub file: &'a str,
    /// The app's design, parsed (none when it has none or it does not parse).
    pub app: Option<exact_tamber::Design>,
    /// Each package design, with its source.
    pub packages: Vec<(&'a contract_syntax::DesignSource, exact_tamber::Design)>,
    /// The app's params and components.
    pub app_params: &'a AppParams,
    /// Each role's row spelling (`AccentColor`, `-exact-secondary-label`),
    /// as the checker keys a role.
    pub roles: Vec<String>,
    /// Each param's first `params` line in the app's design, by token, its
    /// index there: found once ([`Designs::set`]).
    pub lines: std::cell::OnceCell<std::collections::HashMap<String, usize>>,
    /// The params, the app's lines and the packages' lines by name, each
    /// the first, found once ([`Names`]).
    pub names: std::cell::OnceCell<Names>,
}

/// Where each name's line is, for the walks: made once a `Designs`.
#[derive(Default)]
pub struct Names {
    params: std::collections::HashMap<String, usize>,
    /// (in the tokens, then the icons: `false`, `true`), its index
    app: std::collections::HashMap<String, (bool, usize)>,
    roles: std::collections::HashMap<String, (bool, usize)>,
    /// the package, (tokens or icons), its index
    packages: std::collections::HashMap<String, (usize, bool, usize)>,
}

impl<'a> Designs<'a> {
    /// `file`'s `text` (the app's design, or none) and `app`'s packages.
    pub fn read(
        file: &'a str,
        text: Option<&str>,
        app: &'a AppParams,
        vocab: &exact_tamber::Vocab,
    ) -> Designs<'a> {
        Designs {
            lines: std::cell::OnceCell::new(),
            names: std::cell::OnceCell::new(),
            file,
            app: text.and_then(|t| exact_tamber::parse(t).ok()),
            packages: app
                .packages
                .iter()
                .filter_map(|p| Some((p, exact_tamber::parse(&p.text).ok()?)))
                .collect(),
            app_params: app,
            roles: vocab
                .roles
                .iter()
                .map(|r| match r.system {
                    true => r.name.clone(),
                    false => format!("-exact-{}", r.name),
                })
                .collect(),
        }
    }

    /// [`Designs::read`] of an app design already parsed.
    pub fn parsed(
        file: &'a str,
        design: Option<exact_tamber::Design>,
        app: &'a AppParams,
        vocab: &exact_tamber::Vocab,
    ) -> Designs<'a> {
        Designs {
            lines: std::cell::OnceCell::new(),
            names: std::cell::OnceCell::new(),
            app: design,
            ..Designs::read(file, None, app, vocab)
        }
    }

    fn axes(&self) -> &[exact_tamber::Axis] {
        self.app.as_ref().map_or(&[], |d| d.axes.as_slice())
    }

    /// The app's `params` line that sets `p`, by its full name or its short
    /// one where that names it alone.
    pub fn set(&self, p: &ParamToken) -> Option<&exact_tamber::Token> {
        let app = self.app.as_ref()?;
        let lines = self.lines.get_or_init(|| {
            let a = self.app_params;
            let resolver = params::Resolver::new(&a.tokens, &a.identities);
            let mut out = std::collections::HashMap::new();
            for (i, e) in app.params.iter().enumerate() {
                if let Ok(q) = resolver.resolve(&e.name.value) {
                    out.entry(q.token.clone()).or_insert(i);
                }
            }
            out
        });
        lines.get(&p.token).map(|&i| &app.params[i])
    }

    /// The role `key` names, by its row spelling, as the checker's
    /// `role_key` reads it: the row spelling in any case
    /// (`AccentColor`, `-exact-secondary-label`), or an Exact role's bare
    /// name (`secondary-label`).
    pub fn role(&self, key: &str) -> Option<&str> {
        self.row(key).or_else(|| {
            if key.starts_with("-exact-") {
                return None;
            }
            self.row(&format!("-exact-{key}"))
        })
    }

    /// The role `name` is, when it is a role's row spelling in any case:
    /// what a value names a role by, as the checker's fold reads it.
    fn row(&self, name: &str) -> Option<&str> {
        self.roles
            .iter()
            .find(|r| r.eq_ignore_ascii_case(name))
            .map(String::as_str)
    }

    /// The app's line `name` reads (a token or an icon): by its name, else
    /// a role's override in either spelling. With the name the fold
    /// publishes it by (a role's row spelling).
    /// [`Names`], made on first use.
    fn names(&self) -> &Names {
        self.names.get_or_init(|| {
            let mut n = Names::default();
            for (i, q) in self.app_params.tokens.iter().enumerate() {
                n.params.entry(q.token.clone()).or_insert(i);
            }
            if let Some(d) = &self.app {
                let lines = d
                    .tokens
                    .iter()
                    .map(|t| (false, t))
                    .chain(d.icons.iter().map(|t| (true, t)));
                let (mut tokens, mut icons) = (0, 0);
                for (icon, t) in lines {
                    let at = if icon { &mut icons } else { &mut tokens };
                    n.app.entry(t.name.value.clone()).or_insert((icon, *at));
                    if let Some(role) = self.role(&t.name.value) {
                        n.roles.entry(role.to_string()).or_insert((icon, *at));
                    }
                    *at += 1;
                }
            }
            for (k, (_, d)) in self.packages.iter().enumerate() {
                for (i, t) in d.tokens.iter().enumerate() {
                    n.packages
                        .entry(t.name.value.clone())
                        .or_insert((k, false, i));
                }
                for (i, t) in d.icons.iter().enumerate() {
                    n.packages
                        .entry(t.name.value.clone())
                        .or_insert((k, true, i));
                }
            }
            n
        })
    }

    fn app_token(&self, name: &str) -> Option<(String, &exact_tamber::Token)> {
        let d = self.app.as_ref()?;
        let line = |(icon, i): (bool, usize)| if icon { &d.icons[i] } else { &d.tokens[i] };
        if let Some(&at) = self.names().app.get(name) {
            let shown = self.role(name).map_or(name, |r| r);
            return Some((shown.to_string(), line(at)));
        }
        let role = self.role(name)?;
        let &at = self.names().roles.get(role)?;
        Some((role.to_string(), line(at)))
    }

    /// A package's line `name` reads: a token, or an icon (stage 13).
    fn package_token(
        &self,
        name: &str,
    ) -> Option<(&contract_syntax::DesignSource, &exact_tamber::Token)> {
        let &(k, icon, i) = self.names().packages.get(name)?;
        let (s, d) = &self.packages[k];
        Some((*s, if icon { &d.icons[i] } else { &d.tokens[i] }))
    }

    /// The param whose IR token `name` is.
    fn param(&self, name: &str) -> Option<&ParamToken> {
        let &i = self.names().params.get(name)?;
        self.app_params.tokens.get(i)
    }

    /// Whether `text` is a reference a walk follows: a param, an app or a
    /// package line, a base name or a role; not a literal (`transparent`,
    /// `-1`, `#333333`) or an expression.
    pub fn reference(&self, text: &str) -> bool {
        let r = text.trim();
        bare(r)
            && (self.param(r).is_some()
                || self.app_token(r).is_some()
                || self.package_token(r).is_some()
                || exact_kernel::style::scale::is_base(r)
                || self.row(r).is_some())
    }

    /// A line's value in `tuple`'s light or `dark` side: its `when` line's
    /// that holds most (the first of those, as the checker's `select`),
    /// else its own. A `when` naming an axis `under scheme=dark` is the
    /// dark side's alone (the checker's `under_value`): the light side
    /// takes the most specific holding `when` naming none.
    fn value_in<'t>(&self, t: &'t exact_tamber::Token, tuple: usize, dark: bool) -> &'t str {
        let axes = self.axes();
        let mut rest = tuple;
        let mut chosen = vec![""; axes.len()];
        for (i, a) in axes.iter().enumerate().rev() {
            chosen[i] = &a.values[rest % a.values.len()];
            rest /= a.values.len();
        }
        let holds = |(axis, v): &(String, String)| {
            axes.iter()
                .position(|a| &a.name.value == axis)
                .is_some_and(|i| chosen[i] == v)
        };
        let under = |(axis, _): &(String, String)| {
            axes.iter().any(|a| &a.name.value == axis && a.under_dark)
        };
        let holding: Vec<usize> = (0..t.whens.len())
            .filter(|&i| t.whens[i].conds.iter().all(holds))
            .filter(|&i| dark || !t.whens[i].conds.iter().any(under))
            .collect();
        let most = holding.iter().map(|&i| t.whens[i].conds.len()).max();
        match holding
            .into_iter()
            .find(|&i| Some(t.whens[i].conds.len()) == most)
        {
            Some(i) => &t.whens[i].value.value,
            None => &t.value.value,
        }
    }

    /// The walk of param `p` in `tuple`'s light or `dark` side.
    pub fn walk(&self, p: &ParamToken, tuple: usize, dark: bool) -> Step {
        let start = |q: &ParamToken| match self.set(q) {
            Some(e) => (
                "app-param",
                q.full.clone(),
                self.value_in(e, tuple, dark).to_string(),
            ),
            None => ("default", q.full.clone(), q.param.default.clone()),
        };
        let (mut level, mut name, mut value) = start(p);
        for _ in 0..HOPS {
            let r = value.trim().to_string();
            if !bare(&r) {
                break;
            }
            if let Some(q) = self.param(&r) {
                (level, name, value) = start(q);
            } else if let Some((shown, t)) = self.app_token(&r) {
                (level, name, value) = (
                    "app-token",
                    shown,
                    self.value_in(t, tuple, dark).to_string(),
                );
            } else if let Some((_, t)) = self.package_token(&r) {
                (level, name, value) = ("package", r, self.value_in(t, tuple, dark).to_string());
            } else if exact_kernel::style::scale::is_base(&r) {
                return Step {
                    level: "base",
                    name: r,
                };
            } else if let Some(role) = self.row(&r) {
                return Step {
                    level: "role",
                    name: role.to_string(),
                };
            } else {
                // A keyword (`transparent`, `auto`): the line it is on wins.
                break;
            }
        }
        Step { level, name }
    }

    /// `levels` of param `p` (`explain`, `resolve`): only when its default
    /// is a reference, else none (a literal or an expression).
    /// - `token`: the first app or package line the default reaches through
    ///   params' defaults (at most [`HOPS`]), `{name, package?, file, line,
    ///   value}` (a package's own line, with stage 11 A2's `package` and
    ///   `file`; else the app's);
    /// - `app`: the app's override of a package's token, `{file, line,
    ///   value}`, or null;
    /// - `base`: the base name every tuple's walk reaches, or null when
    ///   they differ or one reaches none;
    /// - `param`: the app's own value of the param, `{file, line, value}`,
    ///   or null.
    pub fn levels(&self, p: &ParamToken) -> Option<Value> {
        if !self.reference(&p.param.default) {
            return None;
        }
        // Through params' defaults (never the app's `params` lines) to the
        // first line that is no param's, within the walk's [`HOPS`]: the
        // default's reference is the first hop, so at most `HOPS - 1`
        // params are passed and the token is reached at hop `HOPS` at most.
        let mut r = p.param.default.trim();
        for _ in 1..HOPS {
            match self.param(r) {
                Some(q) => r = q.param.default.trim(),
                None => break,
            }
        }
        let line = |file: &str, t: &exact_tamber::Token| json!({"file": file, "line": t.name.line, "value": t.value.value});
        let (token, app) = match (self.package_token(r), self.app_token(r)) {
            (Some((s, t)), over) => {
                let mut row = json!({"name": r, "package": s.package, "file": s.home, "line": t.name.line, "value": t.value.value});
                if s.package.is_none() {
                    row.as_object_mut().map(|o| o.remove("package"));
                }
                (row, over.map_or(Value::Null, |(_, o)| line(self.file, o)))
            }
            (None, Some((shown, t))) => {
                let mut row = line(self.file, t);
                row["name"] = json!(shown);
                (row, Value::Null)
            }
            (None, None) => (Value::Null, Value::Null),
        };
        let base = self.base(p);
        let param = self.set(p).map_or(Value::Null, |e| line(self.file, e));
        Some(json!({"token": token, "app": app, "base": base, "param": param}))
    }

    /// Param `p`'s additions to a `resolve` or `explain` answer: each of
    /// its tuples' `level` and `name`, each `{light, dark}` as its value
    /// is (a `when` under `scheme=dark` decides the dark side alone), and
    /// its `levels` (null for a literal or an expression default).
    pub fn annotate(&self, p: &ParamToken, out: &mut Value) {
        if let Some(tuples) = out["tuples"].as_array_mut() {
            for (tuple, row) in tuples.iter_mut().enumerate() {
                let (light, dark) = (self.walk(p, tuple, false), self.walk(p, tuple, true));
                row["level"] = json!({"light": light.level, "dark": dark.level});
                row["name"] = json!({"light": light.name, "dark": dark.name});
            }
        }
        out["levels"] = self.levels(p).unwrap_or(Value::Null);
    }

    /// The tuples the app's axes make, one when it has none.
    fn tuples(&self) -> usize {
        self.axes().iter().map(|a| a.values.len().max(1)).product()
    }

    /// The base name every tuple's walk of `p`, on either side, reaches,
    /// when they all reach the same one; none when any reaches no base or
    /// another (each tuple's own is on `resolve`'s tuples).
    fn base(&self, p: &ParamToken) -> Option<String> {
        let mut base = None;
        for (tuple, dark) in (0..self.tuples()).flat_map(|t| [(t, false), (t, true)]) {
            let step = self.walk(p, tuple, dark);
            if step.level != "base" || base.as_ref().is_some_and(|b| *b != step.name) {
                return None;
            }
            base = Some(step.name);
        }
        base
    }

    /// `describe`'s compact `reaches` of param `p`, when its default is a
    /// reference: the first token it reaches, the base name every
    /// tuple reaches (as `levels`), and whether the app sets that token.
    pub fn reaches(&self, p: &ParamToken) -> Option<Value> {
        let levels = self.levels(p)?;
        Some(json!({
            "token": levels["token"]["name"],
            "base": levels["base"],
            "app_set": !levels["app"].is_null(),
        }))
    }
}

/// Whether `text` is one bare name (a token's, a param's IR name, a base
/// name, a role's), no literal's punctuation or function: what a walk follows.
pub fn bare(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        && text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '-' || c == '_')
}

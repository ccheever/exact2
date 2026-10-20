//! A param's levels (LLP 1121 stage 12 B2-levels, *1.15*): one walker over
//! a param's value, per tuple, from the app's own value of it (`params`)
//! or else its default, through each bare reference it makes (a param, an
//! app token, a package token), at most [`HOPS`] hops, to the line whose
//! value is no reference (a literal or an expression), a base name or a
//! role. `explain` and `resolve` of a param say its `levels`, `resolve`'s
//! tuples each their winning `level` and `name`, and `describe`'s param
//! rows what their default `reaches`.

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
    /// The roles' names, lowercased, as the checker matches a role's spelling.
    pub roles: Vec<String>,
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
                .map(|r| r.name.to_ascii_lowercase())
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
        let a = self.app_params;
        self.app.as_ref()?.params.iter().find(|e| {
            params::resolve(&a.tokens, &a.identities, &e.name.value)
                .is_ok_and(|q| q.token == p.token)
        })
    }

    fn app_token(&self, name: &str) -> Option<&exact_tamber::Token> {
        let role = self.is_role(name);
        self.app
            .as_ref()?
            .tokens
            .iter()
            .find(|t| t.name.value == name || role && t.name.value.eq_ignore_ascii_case(name))
    }

    fn package_token(
        &self,
        name: &str,
    ) -> Option<(&contract_syntax::DesignSource, &exact_tamber::Token)> {
        self.packages
            .iter()
            .find_map(|(s, d)| Some((*s, d.tokens.iter().find(|t| t.name.value == name)?)))
    }

    fn is_role(&self, name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        let bare = lower.strip_prefix("-exact-").unwrap_or(&lower);
        self.roles.iter().any(|r| r == bare)
    }

    /// A line's value in `tuple`: its `when` line's that holds most, else its own.
    fn value_in<'t>(&self, t: &'t exact_tamber::Token, tuple: usize) -> &'t str {
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
        let holding: Vec<usize> = (0..t.whens.len())
            .filter(|&i| t.whens[i].conds.iter().all(holds))
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

    /// The walk of param `p` in `tuple`.
    pub fn walk(&self, p: &ParamToken, tuple: usize) -> Step {
        let start = |q: &ParamToken| match self.set(q) {
            Some(e) => (
                "app-param",
                q.full.clone(),
                self.value_in(e, tuple).to_string(),
            ),
            None => ("default", q.full.clone(), q.param.default.clone()),
        };
        let (mut level, mut name, mut value) = start(p);
        for _ in 0..HOPS {
            let r = value.trim().to_string();
            if !bare(&r) {
                break;
            }
            if let Some(q) = self.app_params.tokens.iter().find(|q| q.token == r) {
                (level, name, value) = start(q);
            } else if let Some(t) = self.app_token(&r) {
                (level, name, value) = ("app-token", r, self.value_in(t, tuple).to_string());
            } else if let Some((_, t)) = self.package_token(&r) {
                (level, name, value) = ("package", r, self.value_in(t, tuple).to_string());
            } else if exact_kernel::style::scale::is_base(&r) {
                return Step {
                    level: "base",
                    name: r,
                };
            } else if self.is_role(&r) {
                return Step {
                    level: "role",
                    name: r,
                };
            } else {
                // A keyword (`transparent`, `auto`): the line it is on wins.
                break;
            }
        }
        Step { level, name }
    }

    /// `levels` of param `p` (`explain`, `resolve`): only when its default
    /// is a bare reference, else none.
    /// - `token`: the first token the default names, `{name, package?,
    ///   file, line, value}` (a package's own line, with stage 11 A2's
    ///   `package` and `file`; else the app's);
    /// - `app`: the app's override of a package's token, `{file, line,
    ///   value}`, or null;
    /// - `base`: the base name its first tuple's walk reaches, or null;
    /// - `param`: the app's own value of the param, `{file, line, value}`,
    ///   or null.
    pub fn levels(&self, p: &ParamToken) -> Option<Value> {
        let r = p.param.default.trim();
        if !bare(r) {
            return None;
        }
        let line = |file: &str, t: &exact_tamber::Token| json!({"file": file, "line": t.name.line, "value": t.value.value});
        let (token, app) = match (self.package_token(r), self.app_token(r)) {
            (Some((s, t)), over) => {
                let mut row = json!({"name": r, "package": s.package, "file": s.home, "line": t.name.line, "value": t.value.value});
                if s.package.is_none() {
                    row.as_object_mut().map(|o| o.remove("package"));
                }
                (row, over.map_or(Value::Null, |o| line(self.file, o)))
            }
            (None, Some(t)) => {
                let mut row = line(self.file, t);
                row["name"] = json!(r);
                (row, Value::Null)
            }
            (None, None) => (Value::Null, Value::Null),
        };
        let reached = self.walk(p, 0);
        let base = (reached.level == "base").then_some(reached.name);
        let param = self.set(p).map_or(Value::Null, |e| line(self.file, e));
        Some(json!({"token": token, "app": app, "base": base, "param": param}))
    }

    /// `describe`'s compact `reaches` of param `p`, when its default is a
    /// bare reference: the first token it names, the base name its first
    /// tuple reaches, and whether the app sets that token.
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

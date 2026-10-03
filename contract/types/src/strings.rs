//! `t("key", name=value)` against the app's strings tables (LLP 1060 D2).

use super::{err, infer, Ref, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};
use std::collections::BTreeMap;

/// The app's strings tables (LLP 1060 D1), as the driver read them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Strings {
    /// The locale every `t` is checked against and every table falls back to.
    pub base: String,
    /// Locale → key → text, the base's table included.
    pub tables: BTreeMap<String, BTreeMap<String, String>>,
}

impl Strings {
    /// The base locale's table.
    pub fn base_table(&self) -> &BTreeMap<String, String> {
        &self.tables[&self.base]
    }
}

/// Whether the call `name(…)` is the strings call: `t`, unless a scoped
/// action or action prop of that name is being called.
pub fn is_text_call(name: &str, scope: &Scope) -> bool {
    name == "t" && !matches!(scope.lookup(name), Some((Ref::Action(_) | Ref::Prop(_), _)))
}

/// A literal key the base table has, and a named argument for exactly the
/// placeholders its base text has, each a number, string, or bool.
pub(crate) fn check_call(
    args: &[Expr],
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    let Some(Expr::Str(key, key_span)) = args.first() else {
        return err(
            "type-strings-key",
            "`t` takes its key first, as a string literal the compiler can check: `t(\"greeting\")`",
            args.first().map_or(span, Expr::span),
        );
    };
    let mut given: BTreeMap<&str, Span> = BTreeMap::new();
    for arg in &args[1..] {
        let Expr::NamedArg(name, value, at) = arg else {
            return err(
                "type-strings-argument",
                "a placeholder is filled by name: `t(\"key\", name=value)`",
                arg.span(),
            );
        };
        if given.insert(name, *at).is_some() {
            return err(
                "type-strings-argument",
                format!("`{name}` is given twice"),
                *at,
            );
        }
        let t = infer(value, scope, shapes)?;
        if !t.is_text() && !matches!(t, Ty::Number | Ty::Bool) {
            return err(
                "type-strings-argument",
                format!("`{name}` fills in text, so it is a number, string, or bool, not `{t}`"),
                value.span(),
            );
        }
    }
    let Some(strings) = &shapes.strings else {
        return err(
            "type-strings-missing",
            "`t` reads the app's strings tables, `strings/<locale>.json` beside `app.contract`; there are none",
            span,
        );
    };
    let base = &strings.base;
    let Some(text) = strings.base_table().get(key) else {
        let hint =
            contract_syntax::suggestion(key, strings.base_table().keys().map(String::as_str))
                .map(|guess| format!("; did you mean \"{guess}\"?"))
                .unwrap_or_default();
        return err(
            "type-strings-unknown-key",
            format!("strings/{base}.json has no key \"{key}\"{hint}"),
            *key_span,
        );
    };
    let wanted: Vec<&str> = exact_plan::strings::placeholders(text).collect();
    if let Some(missing) = wanted.iter().find(|name| !given.contains_key(*name)) {
        return err(
            "type-strings-placeholder",
            format!("\"{key}\" has `{{{missing}}}` in strings/{base}.json: give it, `{missing}=…`"),
            span,
        );
    }
    if let Some((unused, at)) = given.iter().find(|(name, _)| !wanted.contains(name)) {
        return err(
            "type-strings-argument",
            format!("\"{key}\" has no `{{{unused}}}` in strings/{base}.json"),
            *at,
        );
    }
    Ok(Ty::String)
}

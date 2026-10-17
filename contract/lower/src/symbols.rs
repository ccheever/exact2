//! A `symbol:` image source names a role (LLP 1035.004.000); it also answers to the other
//! spellings of one (LLP 1116.007 D3.1): Apple's name for the role, dotted or dashed, and the
//! schema's aliases. The lowering rewrites a literal source to its role, so the plan and every
//! host see the role, which is what each draws by.

use crate::{err, LowerError};
use contract_syntax::{Expr, Span, TemplatePart};
use exact_kernel::generated::{symbol_role, SYMBOL_ROLES};

/// The role whose Apple name an SF Symbol is, exactly (LLP 1121 D10: `symbol:sf/chevron.backward`
/// draws `back` where no SF Symbol is).
pub(crate) fn role_by_apple_name(sf: &str) -> Option<&'static str> {
    SYMBOL_ROLES
        .iter()
        .copied()
        .find(|role| exact_kernel::generated::symbol(role).is_some_and(|s| s.0 == sf))
}

/// The role a literal source names by another spelling (`symbol:gear` is `settings`), or by its
/// Apple name as an SF Symbol (`symbol:sf/gearshape`).
fn role_of(source: &str) -> Option<(&str, &'static str)> {
    let name = source.strip_prefix("symbol:")?;
    if let Some(sf) = name.strip_prefix("sf/") {
        return role_by_apple_name(sf).map(|role| (name, role));
    }
    let role = symbol_role(name).filter(|role| *role != name)?;
    Some((name, role))
}

/// A literal source written as the plan holds it: a role by another spelling as the role, an icon
/// token (by name, or `sf/<name>` by its `sf`) as its source form (LLP 1121 D10).
/// An icon whose `sf` a source's SF Symbol is comes before the role whose Apple name it is, as a
/// host resolves a computed one.
fn literal(source: &str) -> Option<String> {
    let name = source.strip_prefix("symbol:")?;
    if let Some(icon) = crate::icons::named(name) {
        crate::icons::used(&icon.name);
        return Some(format!("symbol:{}", icon.form()));
    }
    role_of(source).map(|(_, role)| format!("symbol:{role}"))
}

/// `e` with each literal source (at the top and in every arm of a choice) written as the plan
/// holds it, or `None` when none changes. A source computed otherwise may name any icon, so
/// the plan keeps every one.
pub(crate) fn roles(e: &Expr) -> Option<Expr> {
    match e {
        Expr::Str(s, span) => literal(s).map(|s| Expr::Str(s, *span)),
        Expr::Ternary(cond, a, b, span) => {
            let (na, nb) = (roles(a), roles(b));
            if na.is_none() && nb.is_none() {
                return None;
            }
            let arm = |new: Option<Expr>, old: &Expr| Box::new(new.unwrap_or_else(|| old.clone()));
            Some(Expr::Ternary(cond.clone(), arm(na, a), arm(nb, b), *span))
        }
        // A template of literals alone (a component's argument, inlined, as
        // `symbol:sf/${symbol}` with `symbol="paintpalette"`) is the literal
        // it folds to.
        Expr::Template(parts, span) if constant(parts).is_some() => {
            let text = constant(parts)?;
            literal(&text).map(|s| Expr::Str(s, *span))
        }
        // A template that starts with other text (`https://…`) names no icon;
        // one that starts with a part of `symbol:` may.
        Expr::Template(parts, _)
            if matches!(parts.first(), Some(TemplatePart::Text(t))
                if !t.starts_with("symbol:") && !"symbol:".starts_with(t.as_str())) =>
        {
            None
        }
        _ => {
            crate::icons::computed();
            None
        }
    }
}

/// A template's text when every part of it is a literal.
fn constant(parts: &[TemplatePart]) -> Option<String> {
    parts.iter().try_fold(String::new(), |mut out, part| {
        match part {
            TemplatePart::Text(t) => out.push_str(t),
            TemplatePart::Expr(Expr::Str(s, _)) => out.push_str(s),
            TemplatePart::Expr(Expr::Template(inner, _)) => out.push_str(&constant(inner)?),
            TemplatePart::Expr(_) => return None,
        }
        Some(out)
    })
}

/// `lower-symbol-alias`: a literal source that names a role by another spelling builds as the
/// role, and says which. The role is what the recipe, `contract vocab` and the refusal's list
/// name, so the next edit finds it by search.
pub(crate) fn aliases(e: &Expr) -> Vec<LowerError> {
    match e {
        // A template of literals is linted as the literal it folds to.
        Expr::Template(parts, span) if constant(parts).is_some() => {
            aliases(&Expr::Str(constant(parts).unwrap_or_default(), *span))
        }
        Expr::Str(s, span) => {
            let alias = |message| LowerError {
                id: "lower-symbol-alias",
                message,
                span: *span,
            };
            // @ref LLP 1121 D10 — an SF Symbol an icon token names, or one no host but Apple's draws.
            if let Some(sf) = s.strip_prefix("symbol:sf/") {
                if let Some(icon) = crate::icons::named(&format!("sf/{sf}")) {
                    return vec![alias(format!(
                        "symbol `sf/{sf}` is the icon `{}`: write `symbol:{}`",
                        icon.name, icon.name
                    ))];
                }
            }
            if let Some((name, role)) = role_of(s) {
                return vec![alias(format!(
                    "symbol `{name}` is the role `{role}`: write `symbol:{role}`"
                ))];
            }
            let Some(sf) = s.strip_prefix("symbol:sf/") else {
                return Vec::new();
            };
            let targets = crate::icons::non_apple_targets();
            if sf.is_empty() || targets.is_empty() {
                return Vec::new();
            }
            vec![LowerError {
                id: "lower-symbol-platform",
                message: format!(
                    "symbol `sf/{sf}` draws only on Apple, and the app ships {}: declare an icon token with an `svg` for it (`icons` in app.tamber: `icon.{} = icon(sf \"{sf}\", svg \"icons/{sf}.svg\")`) and write `symbol:icon.{}`",
                    targets.join(", "),
                    sf.replace('.', "-"),
                    sf.replace('.', "-")
                ),
                span: *span,
            }]
        }
        Expr::Ternary(_, a, b, _) => [aliases(a), aliases(b)].concat(),
        _ => Vec::new(),
    }
}

/// Every literal source of `e`, at the top and in each arm of a choice.
pub(crate) fn literals(e: &Expr) -> Vec<&str> {
    match e {
        Expr::Str(s, _) => vec![s],
        Expr::Ternary(_, a, b, _) => [literals(a), literals(b)].concat(),
        _ => Vec::new(),
    }
}

/// A source that is not a role (or `sf/` and a name) is refused, naming the role one spelling
/// edit away and where the roles are listed (LLP 1116.021 D3: the hundred roles repeated in every
/// refusal cost a builder 1.3 KB each time); `e` is already written with roles (see [`roles`]).
pub(crate) fn check(e: &Expr, span: Span) -> Result<(), LowerError> {
    // A template of literals is checked as the literal it folds to, in
    // either arm of a choice too.
    match e {
        Expr::Template(parts, at) if constant(parts).is_some() => {
            return check(&Expr::Str(constant(parts).unwrap_or_default(), *at), span);
        }
        Expr::Ternary(_, a, b, _) => return check(a, span).and_then(|()| check(b, span)),
        _ => {}
    }
    for source in literals(e) {
        let Some(role) = source.strip_prefix("symbol:") else {
            continue;
        };
        // An icon the design declares and the build refused said why already.
        if !role.starts_with("sf/")
            && !role.starts_with("icon|")
            && symbol_role(role).is_none()
            && !crate::icons::is_refused(role)
        {
            let near = contract_syntax::suggestion(role, SYMBOL_ROLES.iter().copied())
                .map(|r| format!("did you mean `symbol:{r}`? "))
                .unwrap_or_default();
            return err(
                "lower-attr-value",
                format!(
                    "symbol `{role}` is not a role; {near}`contract vocab symbol` lists the roles; or name any SF Symbol as `symbol:sf/<name>` (`symbol:sf/{}`), drawn on Apple and blank elsewhere, so a tab or button that must show everywhere takes a role or an icon token (`icons` in app.tamber)",
                    if role.is_empty() { "star.fill" } else { role }
                ),
                span,
            );
        }
    }
    Ok(())
}

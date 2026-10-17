//! A `symbol:` image source names a role (LLP 1035.004.000); it also answers to the other
//! spellings of one (LLP 1116.007 D3.1): Apple's name for the role, dotted or dashed, and the
//! schema's aliases. The lowering rewrites a literal source to its role, so the plan and every
//! host see the role, which is what each draws by.

use crate::{err, LowerError};
use contract_syntax::{Expr, Span};
use exact_kernel::generated::{symbol_role, SYMBOL_ROLES};

/// The role a literal source names by another spelling (`symbol:gear` is `settings`).
fn role_of(source: &str) -> Option<(&str, &'static str)> {
    let name = source.strip_prefix("symbol:")?;
    let role = symbol_role(name).filter(|role| *role != name)?;
    Some((name, role))
}

/// `e` with each literal source (at the top and in every arm of a choice) written as its role,
/// or `None` when none changes.
pub(crate) fn roles(e: &Expr) -> Option<Expr> {
    match e {
        Expr::Str(s, span) => {
            role_of(s).map(|(_, role)| Expr::Str(format!("symbol:{role}"), *span))
        }
        Expr::Ternary(cond, a, b, span) => {
            let (na, nb) = (roles(a), roles(b));
            if na.is_none() && nb.is_none() {
                return None;
            }
            let arm = |new: Option<Expr>, old: &Expr| Box::new(new.unwrap_or_else(|| old.clone()));
            Some(Expr::Ternary(cond.clone(), arm(na, a), arm(nb, b), *span))
        }
        _ => None,
    }
}

/// `lower-symbol-alias`: a literal source that names a role by another spelling builds as the
/// role, and says which. The role is what the recipe, `contract vocab` and the refusal's list
/// name, so the next edit finds it by search.
pub(crate) fn aliases(e: &Expr) -> Vec<LowerError> {
    match e {
        Expr::Str(s, span) => role_of(s)
            .map(|(name, role)| LowerError {
                id: "lower-symbol-alias",
                message: format!("symbol `{name}` is the role `{role}`: write `symbol:{role}`"),
                span: *span,
            })
            .into_iter()
            .collect(),
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
    for source in literals(e) {
        let Some(role) = source.strip_prefix("symbol:") else {
            continue;
        };
        if !role.starts_with("sf/") && symbol_role(role).is_none() {
            let near = contract_syntax::suggestion(role, SYMBOL_ROLES.iter().copied())
                .map(|r| format!("did you mean `symbol:{r}`? "))
                .unwrap_or_default();
            return err(
                "lower-attr-value",
                format!(
                    "symbol `{role}` is not a role; {near}`contract vocab symbol` lists the roles; or name any SF Symbol as `symbol:sf/<name>` (`symbol:sf/{}`), drawn on Apple and blank elsewhere, so a tab or button that must show everywhere takes a role",
                    if role.is_empty() { "star.fill" } else { role }
                ),
                span,
            );
        }
    }
    Ok(())
}

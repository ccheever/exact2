//! `class=` (LLP 1017 P6): a node names a `style`, or chooses between two
//! with `class=(cond ? A : B)`. Either way its rows come in ahead of the
//! node's own attributes, which win for the same name.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, Expr};

impl Lowerer<'_> {
    /// A node's class rows as attributes, with a label for the source map:
    /// a named style's rows as declared; a choice's as one `cond ? A's : B's`
    /// per row either style sets, where a row only one side sets is `none`
    /// on the other — an explicit unset the runner clears to the kernel's
    /// default. Rows the node itself sets are left out.
    pub(crate) fn class_rows(
        &self,
        attrs: &[Attr],
    ) -> Result<Option<(String, Vec<Attr>)>, LowerError> {
        let Some(c) = attrs.iter().find(|a| a.name == "class") else {
            return Ok(None);
        };
        let own = |name: &str| attrs.iter().any(|a| a.name == name);
        let style = |name: &str, span| {
            self.styles.get(name).ok_or_else(|| LowerError {
                id: "lower-unknown-class",
                message: format!("`class={name}`: no `style {name}` in this file"),
                span,
            })
        };
        match &c.value {
            Expr::Ident(name, _) => {
                let rows = style(name, c.span)?
                    .iter()
                    .filter(|s| !own(&s.name))
                    .cloned()
                    .collect();
                Ok(Some((name.clone(), rows)))
            }
            Expr::Ternary(cond, yes, no, span) => {
                let (Expr::Ident(a, _), Expr::Ident(b, _)) = (&**yes, &**no) else {
                    return err(
                        "lower-class-name",
                        "`class=(cond ? A : B)` chooses between two styles by name",
                        c.span,
                    );
                };
                let (sa, sb) = (style(a, yes.span())?, style(b, no.span())?);
                let mut rows: Vec<Attr> = Vec::new();
                for s in sa.iter().chain(sb) {
                    if own(&s.name) || rows.iter().any(|r| r.name == s.name) {
                        continue;
                    }
                    let side = |style: &[Attr]| {
                        style
                            .iter()
                            .find(|r| r.name == s.name)
                            .map_or(Expr::None(*span), |r| r.value.clone())
                    };
                    let (va, vb) = (side(sa), side(sb));
                    // The same literal on both sides is that literal: what lets
                    // a shared `font-family`, literal-only in v1, be chosen.
                    let value = match (&va, &vb) {
                        (Expr::Str(x, _), Expr::Str(y, _)) if x == y => va.clone(),
                        (Expr::Number(x, _), Expr::Number(y, _)) if x == y => va.clone(),
                        _ => Expr::Ternary(cond.clone(), Box::new(va), Box::new(vb), *span),
                    };
                    rows.push(Attr {
                        name: s.name.clone(),
                        value,
                        span: s.span,
                    });
                }
                Ok(Some((format!("{a}/{b}"), rows)))
            }
            _ => err(
                "lower-class-name",
                "`class=` names a style declared with `style Name`, or chooses between two: `class=(cond ? A : B)`",
                c.span,
            ),
        }
    }
}

//! `class=` (LLP 1017 P6): a node names a `style`, or chooses between two
//! with `class=(cond ? A : B)`. Either way its rows come in ahead of the
//! node's own attributes, which win for the same name.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Node};

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
                    // @ref LLP 1069.011 D12 — a prop has no `none` to fall to.
                    let styleable = matches!(crate::tags::attr(&s.name), Some(crate::tags::AttrTarget::Prop(p)) if p.styleable());
                    if styleable && (matches!(va, Expr::None(_)) || matches!(vb, Expr::None(_))) {
                        return err(
                            "lower-style-prop",
                            format!("`class=(cond ? {a} : {b})`: `{}` is set by one style and not the other; set it in both or neither", s.name),
                            s.span,
                        );
                    }
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

impl Lowerer<'_> {
    /// Whether an absolutely positioned box can be among `nodes` or under
    /// them (LLP 1074 T1): a `position` that is not a literal `static` or
    /// `relative`, its own or its class's; a component, a slot or a native
    /// view, whose insides aren't seen here; a canvas; a row that exits or
    /// moves in its layout (the web host takes a leaving row out of flow). A
    /// box that clips or transforms with none of these under it is the
    /// containing block of nothing, so it needs no `position: relative`.
    pub(crate) fn may_hold_absolute(&self, nodes: &[Node]) -> bool {
        nodes.iter().any(|n| match n {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                let class = self.class_rows(attrs).ok().flatten().map(|(_, rows)| rows);
                let rows = attrs.iter().chain(class.iter().flatten());
                let positioned = |a: &Attr| {
                    a.name == "position"
                        && !matches!(&a.value, Expr::Str(v, _) if v == "static" || v == "relative" || v == "sticky")
                };
                let moves =
                    |a: &Attr| matches!(a.name.as_str(), "exit-animation" | "layout-transition");
                rows.clone().any(|a| positioned(a) || moves(a))
                    || crate::tags::tag(tag).is_none()
                    || tag == "canvas"
                    || self.may_hold_absolute(children)
            }
            Node::Use { .. } | Node::Children { .. } => true,
            Node::Each { body, .. } => self.may_hold_absolute(body),
            Node::When {
                then, otherwise, ..
            } => self.may_hold_absolute(then) || self.may_hold_absolute(otherwise),
            Node::Match { some, none, .. } => {
                self.may_hold_absolute(&some.1) || self.may_hold_absolute(none)
            }
        })
    }
}

//! Text fields default to the platform's control (LLP 1104 r8 D1, D2).
//! Appearance is decided once from the merged, expanded bindings: any
//! background, border or radius row makes a default field the bare box.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Span};
use exact_kernel::StyleId;
use exact_plan::{BindingKind, BindingsRow};

/// The surface a plan is lowered for. Field appearance is the same on
/// every surface; terminal admission is checked by the compiler driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    /// The web and native GUI hosts.
    #[default]
    Web,
    /// The terminal host (LLP 1101).
    Terminal,
}

const TYPES: &[&str] = &[
    "text", "email", "password", "search", "tel", "url", "number",
];

fn typed(value: &Expr) -> bool {
    match value {
        Expr::Str(t, _) => TYPES.iter().any(|k| t.eq_ignore_ascii_case(k)),
        Expr::Ternary(_, a, b, _) => typed(a) && typed(b),
        _ => false,
    }
}

fn field(tag: &str, attrs: &[Attr]) -> bool {
    let last = |name: &str| attrs.iter().rev().find(|a| a.name == name);
    match tag {
        "textarea" => {
            last("markup").is_none_or(|a| matches!(&a.value, Expr::Str(m, _) if m == "none"))
        }
        "input" => last("type").is_none_or(|a| typed(&a.value)),
        _ => false,
    }
}

/// CSS UI 4 §7.2.1's appearance-disabling longhands that Contract has.
pub(crate) fn disables(id: StyleId) -> bool {
    use StyleId::*;
    matches!(
        id,
        BackgroundColor
            | BackgroundImage
            | BorderWidthTop
            | BorderWidthRight
            | BorderWidthBottom
            | BorderWidthLeft
            | BorderStyleTop
            | BorderStyleRight
            | BorderStyleBottom
            | BorderStyleLeft
            | BorderColorTop
            | BorderColorRight
            | BorderColorBottom
            | BorderColorLeft
            | BorderRadiusTopLeft
            | BorderRadiusTopRight
            | BorderRadiusBottomRight
            | BorderRadiusBottomLeft
    )
}

impl Lowerer<'_> {
    /// Called only for a final TextInput, after class merge and shorthand
    /// expansion. Excluded types and the Markdown editor always get `none`;
    /// an admitted default field gets no appearance binding unless devolved.
    pub(crate) fn field_appearance(
        &mut self,
        tag: &str,
        attrs: &[Attr],
        span: Span,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), LowerError> {
        let appearance = attrs.iter().rev().find(|a| a.name == "appearance");
        let native = field(tag, attrs);
        let explicit = if native {
            match appearance {
                None => None,
                Some(a) => match &a.value {
                    Expr::Str(v, _) if v == "auto" || v == "none" => Some(v.as_str()),
                    _ => return err(
                        "lower-appearance",
                        "a text field's `appearance` is a literal: `\"auto\"` (the platform's own field) or `\"none\"` (the bare box). To switch between them, write `when` with two fields",
                        a.span,
                    ),
                },
            }
        } else {
            None
        };
        let rows: std::collections::BTreeSet<_> = bindings
            .iter()
            .filter(|b| b.kind == BindingKind::Style)
            .filter_map(|b| StyleId::from_bit(u32::from(b.id)))
            .filter(|&id| disables(id))
            .collect();
        if explicit == Some("auto") {
            let names = crate::vocab::attrs();
            for row in &rows {
                let name = names
                    .iter()
                    .find_map(|(name, target)| {
                        matches!(target, crate::tags::AttrTarget::Styles(ids) if *ids == [*row])
                            .then_some(*name)
                    })
                    .expect("a disabling row has a CSS longhand");
                self.errors.push(LowerError {
                    id: "lower-appearance",
                    message: format!("`{}`: a native field draws its own background, border and corners; to draw your own, write `appearance=\"none\"`", name),
                    span: attrs.iter().find(|a| {
                        match crate::tags::attr(&a.name) {
                            Some(crate::tags::AttrTarget::Styles(ids)) => ids.contains(row),
                            Some(crate::tags::AttrTarget::Shorthand) => crate::shorthands::rows(&a.name).contains(row),
                            _ => false,
                        }
                    }).map_or(span, |a| a.span),
                });
            }
        }
        if !native || (explicit.is_none() && !rows.is_empty()) {
            bindings.push(BindingsRow {
                kind: BindingKind::Style,
                id: StyleId::Appearance as u16,
                expr: self.fixed(true, "none"),
            });
        }
        Ok(())
    }
}

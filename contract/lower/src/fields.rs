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

/// `outline="none"` takes a text field's focus ring off (LLP 1104 D6), and
/// only a text field's: no host removes another element's ring yet (a
/// button's, a checkbox's, a `tabindex` box's), so the name is refused there
/// rather than honoured on the web alone. A Markdown editor is a `textarea`.
pub(crate) fn outline_refusal(name: &str, tag: &str, control: Option<&str>) -> Option<String> {
    let field = control == Some("field") || tag == "textarea";
    (matches!(name, "outline" | "outline-style") && !field).then(|| {
        let what = control.filter(|_| tag == "input").map_or(format!("`{tag}`"), |c| format!("an `input` of type `{c}`"));
        format!("`{name}` takes a text field's focus ring off (`input` of a text type, `textarea`), and nothing else's yet: no host removes the ring of {what}")
    })
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

/// `appearance: textfield` (LLP 1116.007 D4) is CSS UI 4's compat keyword on
/// a text field: the platform's field without a number field's stepper. On
/// any other element it would be a quiet `none` to the hosts that read
/// `auto` as the platform's control (a checkbox, a select, a date), so it is
/// refused there with the word that does what its author meant.
pub(crate) fn textfield_elsewhere(attrs: &[Attr]) -> Option<&Attr> {
    attrs
        .iter()
        .rev()
        .find(|a| a.name == "appearance")
        .filter(|a| {
            matches!(&a.value, Expr::Str(v, _) if v == "textfield")
                || matches!(&a.value, Expr::Ternary(_, x, y, _)
                if matches!(&**x, Expr::Str(v, _) if v == "textfield")
                    || matches!(&**y, Expr::Str(v, _) if v == "textfield"))
        })
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
                    Expr::Str(v, _) if matches!(v.as_str(), "auto" | "none" | "textfield") => {
                        Some(v.as_str())
                    }
                    _ => return err(
                        "lower-appearance",
                        "a text field's `appearance` is a literal: `\"auto\"` (the platform's own field), `\"textfield\"` (the same, without a number field's stepper) or `\"none\"` (the bare box). To switch between them, write `when` with two fields",
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
        if matches!(explicit, Some("auto" | "textfield")) {
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
        // A field that is never native whose author wrote `appearance="none"`
        // already has that row, and it is the author's (LLP 1115.000 D2): an
        // implied one after it would win and take the bit off.
        let said = !native
            && appearance
                .is_some_and(|a| a.written && matches!(&a.value, Expr::Str(v, _) if v == "none"));
        if (!native && !said) || (native && explicit.is_none() && !rows.is_empty()) {
            let none = self.fixed(true, "none");
            bindings.push(crate::style_row(StyleId::Appearance, none, crate::IMPLIED));
        }
        Ok(())
    }
}

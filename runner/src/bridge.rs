//! Values to kernel rows.
//!
//! @ref LLP 1004 D2 (kernel ordinals get meaning from `exact-kernel`)
//!
//! A binding names a kernel prop or style row by ordinal and yields a
//! [`Value`]. This is the one place a value becomes a kernel write: props
//! through `PropValue` by the prop's declared kind, style rows through the
//! kernel's own generated `StyleProps::set_dynamic`. Nothing is redeclared.

use exact_kernel::{PropId, PropValue, StyleId, StyleProps, StyleValue, StyleValueError};
use exact_plan::Value;

/// Why a binding's value could not become a kernel row.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    UnknownProp(u16),
    UnknownStyle(u16),
    PropKind { prop: PropId, value: Value },
    Style(StyleValueError),
    StyleKind { style: StyleId, value: Value },
    FontStack { index: u32, stacks: usize },
}

/// A prop value by the prop's declared kind.
pub fn prop_value(id: u16, value: &Value) -> Result<(PropId, PropValue), BridgeError> {
    let prop = PropId::from_wire(id).ok_or(BridgeError::UnknownProp(id))?;
    let mismatch = || BridgeError::PropKind {
        prop,
        value: value.clone(),
    };
    let out = match (prop.kind(), value) {
        (exact_kernel::PropKind::Str, v @ exact_plan::str_value!()) => {
            PropValue::Str(String::from(v.text()))
        }
        (exact_kernel::PropKind::Str, Value::Number(n)) => {
            PropValue::Str(crate::stdlib::format_number(*n))
        }
        (exact_kernel::PropKind::Bool, Value::Bool(b)) => PropValue::Bool(*b),
        (exact_kernel::PropKind::Int, Value::Number(n))
            if n.is_finite()
                && n.fract() == 0.0
                && *n >= i64::MIN as f64
                && *n < -(i64::MIN as f64) =>
        {
            PropValue::Int(*n as i64)
        }
        (exact_kernel::PropKind::Float, Value::Number(n)) => PropValue::Float(*n),
        _ => return Err(mismatch()),
    };
    Ok((prop, out))
}

/// Whether `value` is a CSS-wide keyword that leaves row `id` unset
/// ([`StyleValue::unsets`]): the row is cleared, as a class choice's
/// `none` is, and nothing is refused.
pub fn unsets(id: u16, value: &Value) -> bool {
    matches!(value, exact_plan::str_value!())
        && StyleId::from_bit(id as u32)
            .is_some_and(|style| StyleValue::Text(value.text().into()).unsets(style))
}

/// [`set_style`] for a binding of `plan`: a `platform-color()` only as one
/// of the plan's own string literals (LLP 1095 D3), so data, a template or
/// a concatenation never chooses which platform colour a host looks up. A
/// literal chosen by state (a ternary's arm, a class) is still the plan's.
pub fn set_plan_style(
    patch: &mut StyleProps,
    id: u16,
    value: &Value,
    plan: &exact_plan::Plan,
) -> Result<StyleId, BridgeError> {
    if let v @ exact_plan::str_value!() = value {
        let text = v.text();
        if text.contains("platform-color(") && !plan.strings.iter().any(|s| s == text) {
            let style = StyleId::from_bit(id as u32).ok_or(BridgeError::UnknownStyle(id))?;
            return Err(BridgeError::StyleKind {
                style,
                value: value.clone(),
            });
        }
    }
    set_style(patch, id, value, plan.stacks.len())
}

/// Set style row `id` on `patch` from `value`.
pub fn set_style(
    patch: &mut StyleProps,
    id: u16,
    value: &Value,
    stacks: usize,
) -> Result<StyleId, BridgeError> {
    let style = StyleId::from_bit(id as u32).ok_or(BridgeError::UnknownStyle(id))?;
    if style == StyleId::FontFamily {
        let index = match value {
            Value::Number(n) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 => *n as u32,
            _ => {
                return Err(BridgeError::StyleKind {
                    style,
                    value: value.clone(),
                })
            }
        };
        if index as usize >= stacks {
            return Err(BridgeError::FontStack { index, stacks });
        }
    }
    let style_value = match value {
        Value::Number(n) => StyleValue::Number(*n),
        v @ exact_plan::str_value!() => match v.text() {
            "auto" if style.codec() == exact_kernel::StyleCodec::Dimension => StyleValue::Auto,
            // A lone percentage is one; other text ending in `%` is a CSS
            // value (`transform-origin: 0 100%`), as the compiler reads it.
            t => match t
                .strip_suffix('%')
                .and_then(|p| exact_num::parse_f64(p).ok())
            {
                Some(p) => StyleValue::Percent(p),
                _ => StyleValue::Text(t.to_string()),
            },
        },
        _ => {
            return Err(BridgeError::StyleKind {
                style,
                value: value.clone(),
            })
        }
    };
    patch
        .set_dynamic(style, &style_value)
        .map_err(BridgeError::Style)?;
    Ok(style)
}

/// The plan's `@keyframes` by name, each parsed once per plan (LLP 1055 D5),
/// the first time an `animation` names it: a plan's rules are many and a
/// first frame starts few of them.
#[derive(Debug, Default)]
pub struct KeyframesTable(
    Vec<(
        String,
        String,
        std::cell::OnceCell<Option<exact_motion::Keyframes>>,
    )>,
);

/// The plan's `keyframes` table. The compiler validated every row; one that
/// no longer parses (a plan from another evaluator) names nothing.
pub fn keyframes(plan: &exact_plan::Plan) -> KeyframesTable {
    KeyframesTable(
        plan.keyframes
            .iter()
            .map(|row| {
                (
                    plan.str(row.name).to_string(),
                    plan.str(row.css).to_string(),
                    std::cell::OnceCell::new(),
                )
            })
            .collect(),
    )
}

impl KeyframesTable {
    /// Give an `animation` row its keyframes; a name no rule has starts no
    /// animation, as in CSS, and is returned as a journal line.
    pub fn resolve(&self, row: &mut exact_motion::Animations) -> Vec<String> {
        row.resolve(|name| {
            self.0
                .iter()
                .filter(|(n, ..)| n == name)
                .find_map(|(_, css, rule)| {
                    rule.get_or_init(|| exact_motion::Keyframes::parse(css).ok())
                        .as_ref()
                })
        })
        .into_iter()
        .map(|name| format!("animation-name `{name}` matches no keyframes: no animation"))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_platform_color_is_admitted_only_as_a_plan_literal() {
        let literal = "platform-color(ios bridgeTestColor, #010203)";
        let mut plan = exact_plan::Plan::default();
        plan.strings.push(literal.into());
        let color = StyleId::TextColor as u16;
        let mut style = StyleProps::default();
        assert!(set_plan_style(&mut style, color, &Value::str(literal), &plan).is_ok());
        for built in [
            "platform-color(ios bridgeTestOtherColor, #010203)",
            "linear-gradient(platform-color(ios bridgeTestColor, #010203), #fff)",
        ] {
            assert!(
                matches!(
                    set_plan_style(&mut style, color, &Value::str(built), &plan),
                    Err(BridgeError::StyleKind { .. })
                ),
                "{built}"
            );
        }
        assert!(set_plan_style(&mut style, color, &Value::str("#fff"), &plan).is_ok());
    }

    #[test]
    fn css_percentages_accept_browser_exponents() {
        let mut style = StyleProps::default();
        set_style(
            &mut style,
            StyleId::GridTemplateColumns as u16,
            &Value::str("1e2%"),
            0,
        )
        .unwrap();
        assert_eq!(style.grid_template_columns.css(), "100%");
    }
}

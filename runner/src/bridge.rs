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
        (exact_kernel::PropKind::Str, Value::Str(s)) => PropValue::Str(s.to_string()),
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
        Value::Str(s) => match s.as_ref() {
            "auto" if style.codec() == exact_kernel::StyleCodec::Dimension => StyleValue::Auto,
            t if t.ends_with('%') => t[..t.len() - 1]
                .parse::<f64>()
                .map(StyleValue::Percent)
                .map_err(|_| BridgeError::StyleKind {
                    style,
                    value: value.clone(),
                })?,
            t => StyleValue::Text(t.to_string()),
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

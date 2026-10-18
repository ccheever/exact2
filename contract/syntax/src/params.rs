//! A component's `params` section (LLP 1121 D15): the tokens it owns, each
//! with a kind, a default (any token value, a string) and the editor's
//! metadata. `param.<name>` in the component's rows and defaults names one;
//! the driver gives the component its identity, its declaring file relative
//! to the app or package root and its name (`post.contract#PostButton`).

use crate::Span;

/// A component's params and its identity.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Params {
    /// `post.contract#PostButton`, as the driver sets it; `None` until then.
    pub identity: Option<String>,
    /// The section's params, in order; empty without one.
    pub list: Vec<ComponentParam>,
}

/// `radius: length = "radius.md"  range="0..40"  step=1  control="slider"`.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentParam {
    /// Its name, which `param.<name>` names.
    pub name: String,
    /// Its kind as written (`length`, `box-shadow`).
    pub kind: String,
    /// Its default: a token value.
    pub default: String,
    /// Where the default is written.
    pub default_span: Span,
    /// Its attributes, in order.
    pub attrs: Vec<ParamAttr>,
    /// Where its name is written.
    pub span: Span,
}

/// One of a param's attributes: `range`, `step`, `control`, `options`,
/// `label`, `help`, `group` or `priority`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamAttr {
    /// Its name.
    pub name: String,
    /// Its value: a string, or a number.
    pub value: ParamValue,
    /// Where it is written.
    pub span: Span,
}

/// A param attribute's value.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    /// `"slider"`.
    Str(String),
    /// `1`.
    Number(f64),
}

/// The attributes a param admits (D15).
pub const ATTRIBUTES: [&str; 8] = [
    "range", "step", "control", "options", "label", "help", "group", "priority",
];

impl ComponentParam {
    /// The attribute `name`'s value, when written.
    pub fn attr(&self, name: &str) -> Option<&ParamValue> {
        self.attrs.iter().find(|a| a.name == name).map(|a| &a.value)
    }
}

impl ParamValue {
    /// The value as text: a number as Contract writes it.
    pub fn text(&self) -> String {
        match self {
            ParamValue::Str(s) => s.clone(),
            ParamValue::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
            ParamValue::Number(n) => n.to_string(),
        }
    }
}

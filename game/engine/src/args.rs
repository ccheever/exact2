use crate::Value;

/// Whether changing a bound field constructs a new world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentKind {
    /// Construct a new world when this value changes.
    Setup,
    /// Pass the new value to subsequent ticks.
    Live,
}
/// Typed canvas arguments. Derive this on a named struct; field order is wire order.
pub trait Args: Sized + 'static {
    /// Ordered field names and their binding behavior.
    const FIELDS: &'static [(&'static str, ArgumentKind)];
    /// Decode all values before any world or clock mutation.
    fn decode(values: &[Value]) -> Result<Self, String>;
    /// Whether any setup field differs.
    fn setup_changed(&self, next: &Self) -> bool;
}
impl Args for () {
    const FIELDS: &'static [(&'static str, ArgumentKind)] = &[];
    fn decode(values: &[Value]) -> Result<Self, String> {
        arity(values, &[])?;
        Ok(())
    }
    fn setup_changed(&self, _: &Self) -> bool {
        false
    }
}
/// Support for the Args derive; not a string lookup API.
#[doc(hidden)]
pub fn arity(values: &[Value], fields: &[(&str, ArgumentKind)]) -> Result<(), String> {
    let count = fields.len();
    if values.len() > count {
        Err(format!(
            "expected {count} arguments ({}), got {}",
            fields
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(", "),
            values.len()
        ))
    } else {
        Ok(())
    }
}
/// Supported scalar argument types, used by the derive.
#[doc(hidden)]
pub trait Argument: Sized {
    const EXPECTED: &'static str;
    fn value(value: &Value) -> Option<Self>;
}
/// Decode one positional field with an author-facing refusal.
#[doc(hidden)]
pub fn field<T: Argument>(values: &[Value], index: usize, name: &str) -> Result<T, String> {
    values.get(index).and_then(T::value).ok_or_else(|| {
        format!(
            "{name}: expected {}, got {}",
            T::EXPECTED,
            values.get(index).map_or_else(
                || "no value".into(),
                |v| crate::values::value_json(v, false)
            )
        )
    })
}
impl Argument for bool {
    const EXPECTED: &'static str = "a boolean";
    fn value(v: &Value) -> Option<Self> {
        v.as_bool()
    }
}
impl Argument for String {
    const EXPECTED: &'static str = "text";
    fn value(v: &Value) -> Option<Self> {
        v.as_str().map(str::to_owned)
    }
}
macro_rules! integer {
    ($t:ty, $lo:expr, $hi:expr, $expected:literal) => {
        impl Argument for $t {
            const EXPECTED: &'static str = $expected;
            fn value(v: &Value) -> Option<Self> {
                v.as_number()
                    .filter(|n| n.is_finite() && n.fract() == 0.0 && *n >= $lo && *n <= $hi)
                    .map(|n| n as Self)
            }
        }
    };
}
// Bound values are f64: 64-bit integer fields accept only exactly portable safe integers.
integer!(
    u32,
    0.0,
    u32::MAX as f64,
    "a whole number ≥ 0 (at most 4294967295)"
);
integer!(
    u64,
    0.0,
    9_007_199_254_740_991.0,
    "a whole number ≥ 0 (at most 9007199254740991)"
);
integer!(
    i32,
    i32::MIN as f64,
    i32::MAX as f64,
    "a whole number in -2147483648..=2147483647"
);
integer!(
    i64,
    -9_007_199_254_740_991.0,
    9_007_199_254_740_991.0,
    "a whole number in -9007199254740991..=9007199254740991"
);
impl Argument for f64 {
    const EXPECTED: &'static str = "a finite number";
    fn value(v: &Value) -> Option<Self> {
        v.as_number().filter(|n| n.is_finite())
    }
}
impl Argument for f32 {
    const EXPECTED: &'static str = "a finite f32 number";
    fn value(v: &Value) -> Option<Self> {
        v.as_number().map(|n| n as f32).filter(|n| n.is_finite())
    }
}
pub(crate) fn json<A: Args>(values: &[Value]) -> String {
    format!(
        "{{{}}}",
        A::FIELDS
            .iter()
            .zip(values)
            .map(|((name, _), value)| {
                format!(
                    "{}:{}",
                    crate::values::quote(name),
                    crate::values::value_json(value, true)
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    )
}

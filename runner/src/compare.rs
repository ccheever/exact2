//! Value comparison, once: identity, substitution, and the language's `==`.
//!
//! - [`same`]: the same object, O(1). What a memo asks when it must never
//!   walk a value.
//! - [`equivalent`]: indistinguishable to every expression. Numbers compare
//!   by bits (`-0` is not `0` — `1 / n > 0` tells them apart — and a NaN is
//!   itself); a shared object answers at once. What a cache may substitute
//!   for a fresh result.
//! - [`equal`]: the language's `==`. IEEE numbers (`-0 == 0`; NaN equals
//!   nothing, so a shared list may still be unequal to itself); `None` when
//!   the kinds differ, which the compiler rejects and a hostile plan may
//!   still attempt.
//!
//! All three stop at the first difference.

use exact_plan::Value;
use std::rc::Rc;

/// Whether `a` and `b` are the same object: equal scalars (numbers by bits),
/// or one shared allocation.
pub fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.to_bits() == b.to_bits(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Unit, Value::Unit) | (Value::Option(None), Value::Option(None)) => true,
        (Value::Str(a), Value::Str(b)) => Rc::ptr_eq(a, b),
        (Value::List(a), Value::List(b)) | (Value::Record(a), Value::Record(b)) => Rc::ptr_eq(a, b),
        (Value::Option(Some(a)), Value::Option(Some(b))) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

/// Whether no expression can tell `a` from `b`.
pub fn equivalent(a: &Value, b: &Value) -> bool {
    if same(a, b) {
        return true;
    }
    match (a, b) {
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::List(a), Value::List(b)) | (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| equivalent(a, b))
        }
        (Value::Option(Some(a)), Value::Option(Some(b))) => equivalent(a, b),
        _ => false,
    }
}

/// [`equivalent`] over two optional values (a frame's item or binding).
pub fn equivalent_opt(a: &Option<Value>, b: &Option<Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => equivalent(a, b),
        _ => false,
    }
}

/// [`equivalent`], element by element.
pub fn equivalent_all(a: &[Value], b: &[Value]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b))
}

/// The language's structural equality; `None` when the kinds differ before
/// the first difference.
pub fn equal(a: &Value, b: &Value) -> Option<bool> {
    Some(match (a, b) {
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Str(a), Value::Str(b)) => Rc::ptr_eq(a, b) || a == b,
        (Value::Unit, Value::Unit) => true,
        (Value::Option(None), Value::Option(None)) => true,
        (Value::Option(Some(_)), Value::Option(None))
        | (Value::Option(None), Value::Option(Some(_))) => false,
        (Value::Option(Some(a)), Value::Option(Some(b))) => equal(a, b)?,
        (Value::List(a), Value::List(b)) | (Value::Record(a), Value::Record(b)) => {
            if a.len() != b.len() {
                return Some(false);
            }
            for (x, y) in a.iter().zip(b.iter()) {
                if !equal(x, y)? {
                    return Some(false);
                }
            }
            true
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_zero_and_nan_keep_their_meanings() {
        let (zero, negative) = (Value::Number(0.0), Value::Number(-0.0));
        assert_eq!(equal(&zero, &negative), Some(true));
        assert!(!equivalent(&zero, &negative));
        let nan = Value::list(vec![Value::Number(f64::NAN)]);
        assert!(same(&nan, &nan.clone()));
        assert!(equivalent(&nan, &nan.clone()));
        assert_eq!(equal(&nan, &nan.clone()), Some(false));
    }

    #[test]
    fn the_first_difference_decides() {
        let a = Value::list(vec![Value::Number(1.0), Value::Bool(true)]);
        let b = Value::list(vec![Value::Number(2.0), Value::str("kind")]);
        assert_eq!(equal(&a, &b), Some(false));
        let c = Value::list(vec![Value::Number(1.0), Value::str("kind")]);
        assert_eq!(equal(&a, &c), None);
        assert!(!equivalent(&a, &b));
        let shared = Value::str("shared");
        assert!(same(&shared, &shared.clone()));
        assert!(!same(&shared, &Value::str("shared")));
        assert!(equivalent(&shared, &Value::str("shared")));
    }
}

//! The roster's list builders (LLP 1088 §9.1): `concat`, and `slice` and
//! `includes` over a list, as JavaScript's `Array.prototype` has them. What
//! each costs — its list steps and the extent of a list it builds — is the
//! VM's to charge (`vm.rs`, the `Call` arm), as it charges `join`'s.

use exact_plan::Value;

/// `xs.concat(ys)`: the items of both, in order.
pub fn concat(xs: &[Value], ys: &[Value]) -> Vec<Value> {
    xs.iter().chain(ys).cloned().collect()
}

/// `xs.slice(start, end)`: the items from `start` up to `end`, each index
/// clamped as `String.prototype.slice` clamps (`strings::clamp`); an omitted
/// `end` is `Number.MAX_VALUE`, the list's end.
pub fn slice(xs: &[Value], start: f64, end: f64) -> &[Value] {
    let (from, to) = (
        crate::strings::clamp(start, xs.len()),
        crate::strings::clamp(end, xs.len()),
    );
    &xs[from..to.max(from)]
}

/// Where `xs.includes(x)` finds `x`: the first item SameValueZero equal to
/// it — strings and bools by value, numbers by `==` with NaN equal to NaN
/// (`-0` is `0`). The checker gives `includes` only strings, numbers and
/// bools.
pub fn position(xs: &[Value], x: &Value) -> Option<usize> {
    xs.iter().position(|item| match (item, x) {
        (Value::Number(a), Value::Number(b)) => a == b || (a.is_nan() && b.is_nan()),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        _ => item.as_str().is_some_and(|a| x.as_str() == Some(a)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(ns: &[f64]) -> Vec<Value> {
        ns.iter().map(|n| Value::Number(*n)).collect()
    }

    /// Expected values from `bun -e`: `[1,2,3].slice(…)`, `.includes(…)`.
    #[test]
    fn slice_clamps_as_the_web_does() {
        let xs = numbers(&[1.0, 2.0, 3.0]);
        for (a, b, want) in [
            (0.0, f64::MAX, vec![1.0, 2.0, 3.0]),
            (1.0, f64::MAX, vec![2.0, 3.0]),
            (0.0, -1.0, vec![1.0, 2.0]),
            (-2.0, f64::MAX, vec![2.0, 3.0]),
            (1.9, -1.2, vec![2.0]),
            (f64::NAN, 2.0, vec![1.0, 2.0]),
            (2.0, 1.0, vec![]),
            (10.0, f64::MAX, vec![]),
            (f64::NEG_INFINITY, f64::INFINITY, vec![1.0, 2.0, 3.0]),
        ] {
            assert_eq!(
                slice(&xs, a, b),
                numbers(&want).as_slice(),
                "slice({a}, {b})"
            );
        }
    }

    #[test]
    fn includes_is_same_value_zero() {
        let xs = numbers(&[1.0, f64::NAN, -0.0]);
        assert_eq!(position(&xs, &Value::Number(f64::NAN)), Some(1));
        assert_eq!(position(&xs, &Value::Number(0.0)), Some(2));
        assert_eq!(position(&xs, &Value::Number(2.0)), None);
        let words = vec![Value::str("a"), Value::str("b")];
        assert_eq!(position(&words, &Value::str("b")), Some(1));
        assert_eq!(position(&words, &Value::str("B")), None);
        assert_eq!(
            position(&[Value::Bool(false)], &Value::Bool(false)),
            Some(0)
        );
        assert_eq!(concat(&words, &xs).len(), 5);
    }
}

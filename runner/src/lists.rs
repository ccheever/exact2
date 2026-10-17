//! The roster's list builders (LLP 1088 §9.1): `concat`, and `slice`,
//! `includes` and `indexOf` over a list, as JavaScript's `Array.prototype`
//! has them; `sum`, and `sortBy`'s order once its keys are known. What
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

/// Where `xs.indexOf(x)` finds `x`: the first item IsStrictlyEqual to it —
/// as [`position`], but NaN is never found (`-0` is still `0`).
pub fn index_of(xs: &[Value], x: &Value) -> Option<usize> {
    xs.iter().position(|item| match (item, x) {
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        _ => item.as_str().is_some_and(|a| x.as_str() == Some(a)),
    })
}

/// `sum(xs)`: `xs.reduce((a, x) => a + x, 0)`, IEEE addition left to right
/// from +0; `None` when an item is not a number.
pub fn sum(xs: &[Value]) -> Option<f64> {
    xs.iter()
        .try_fold(0.0, |total, x| Some(total + x.as_number()?))
}

/// A `sortBy` key: a number with NaN read as +0 (so NaN, -0 and +0 tie), or
/// text, compared by UTF-16 code units as `<` compares it.
enum Key<'a> {
    Number(f64),
    Text(&'a str),
}

/// Whether key `a` orders strictly before key `b` ascending.
fn key_before(a: &Key<'_>, b: &Key<'_>) -> bool {
    match (a, b) {
        (Key::Number(x), Key::Number(y)) => x < y,
        (Key::Text(x), Key::Text(y)) => crate::strings::order(x, y).is_lt(),
        _ => false,
    }
}

/// @ref LLP 1116.016 §5 — `sortBy(xs, x => key, direction)` once its keys
/// are computed (`keys[i]` is item `i`'s): the items ordered by key,
/// descending when `descending`, equal keys in input order either way.
/// `None` when the lengths differ, a key is neither a number nor text, or
/// the keys mix the two.
///
/// One bottom-up merge sort, the same in `host/web-js/budget.js`
/// (`x_sortBy`): runs of `width` positions merged pairwise into the other
/// buffer, `width` doubling from 1. A merge takes the right run's head only
/// when its key orders strictly before the left's, so a tie keeps input
/// order. The order is total on (key, position), so any stable sort gives
/// this result; the algorithm is fixed so that the two executors also take
/// the same steps.
pub fn sort_by(xs: &[Value], keys: &[Value], descending: bool) -> Option<Vec<Value>> {
    if xs.len() != keys.len() {
        return None;
    }
    let keyed: Vec<Key<'_>> = keys
        .iter()
        .map(|k| match k {
            Value::Number(n) => Some(Key::Number(if n.is_nan() { 0.0 } else { *n })),
            _ => k.as_str().map(Key::Text),
        })
        .collect::<Option<_>>()?;
    if keyed.windows(2).any(|w| {
        matches!(
            w,
            [Key::Number(_), Key::Text(_)] | [Key::Text(_), Key::Number(_)]
        )
    }) {
        return None;
    }
    // Whether the item at position `b` goes before the one at `a`.
    let before = |b: usize, a: usize| {
        if descending {
            key_before(&keyed[a], &keyed[b])
        } else {
            key_before(&keyed[b], &keyed[a])
        }
    };
    let n = xs.len();
    let mut src: Vec<usize> = (0..n).collect();
    let mut dst: Vec<usize> = vec![0; n];
    let mut width = 1;
    while width < n {
        let mut lo = 0;
        while lo < n {
            let mid = (lo + width).min(n);
            let hi = (lo + 2 * width).min(n);
            let (mut i, mut j, mut k) = (lo, mid, lo);
            while i < mid && j < hi {
                if before(src[j], src[i]) {
                    dst[k] = src[j];
                    j += 1;
                } else {
                    dst[k] = src[i];
                    i += 1;
                }
                k += 1;
            }
            while i < mid {
                dst[k] = src[i];
                i += 1;
                k += 1;
            }
            while j < hi {
                dst[k] = src[j];
                j += 1;
                k += 1;
            }
            lo = hi;
        }
        std::mem::swap(&mut src, &mut dst);
        width *= 2;
    }
    Some(src.into_iter().map(|i| xs[i].clone()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected orders from Bun: a stable `toSorted` on the decorated key,
    /// `(a, b) => (ka < kb ? -1 : kb < ka ? 1 : 0)` with NaN read as 0.
    #[test]
    fn sort_by_is_a_stable_key_sort() {
        let n = |v: f64| Value::Number(v);
        let s = |v: &str| Value::str(v);
        let items: Vec<Value> = (0..7).map(|i| n(i as f64)).collect();
        let keys = [
            n(3.0),
            n(f64::NAN),
            n(-0.0),
            n(1.0),
            n(0.0),
            n(-1.0),
            n(3.0),
        ];
        let order = |desc| {
            sort_by(&items, &keys, desc)
                .unwrap()
                .iter()
                .map(|v| v.as_number().unwrap() as usize)
                .collect::<Vec<_>>()
        };
        // NaN, -0 and +0 tie, in input order, both ways.
        assert_eq!(order(false), [5, 1, 2, 4, 3, 0, 6]);
        assert_eq!(order(true), [0, 6, 3, 1, 2, 4, 5]);
        // Code units: U+FF5E (one unit, 0xFF5E) after "😀" (0xD83D 0xDE00),
        // where code-point order puts it first; a prefix first.
        let words = [s("～"), s("😀"), s("a"), s("ab"), s(""), s("a")];
        let sorted = sort_by(&words, &words, false).unwrap();
        let texts: Vec<&str> = sorted.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(texts, ["", "a", "a", "ab", "😀", "～"]);
        assert_eq!(sort_by(&[], &[], true), Some(vec![]));
        assert_eq!(sort_by(&items[..2], &[n(1.0), s("a")], false), None);
        assert_eq!(sort_by(&items[..1], &[Value::Bool(true)], false), None);
        assert_eq!(sort_by(&items[..2], &[n(1.0)], false), None);
        // Every length to 40 against a reference stable sort.
        for len in 0..40usize {
            let xs: Vec<Value> = (0..len).map(|i| n(i as f64)).collect();
            let ks: Vec<Value> = (0..len).map(|i| n(((i * 7) % 5) as f64)).collect();
            for desc in [false, true] {
                let mut want: Vec<usize> = (0..len).collect();
                want.sort_by_key(|&i| {
                    let k = ((i * 7) % 5) as i64;
                    if desc {
                        -k
                    } else {
                        k
                    }
                });
                let got: Vec<usize> = sort_by(&xs, &ks, desc)
                    .unwrap()
                    .iter()
                    .map(|v| v.as_number().unwrap() as usize)
                    .collect();
                assert_eq!(got, want, "len {len} desc {desc}");
            }
        }
    }

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

    /// `[1, NaN, -0].indexOf(…)` in `bun -e`: NaN is not found.
    #[test]
    fn index_of_is_strict_equality() {
        let xs = numbers(&[1.0, f64::NAN, -0.0]);
        assert_eq!(index_of(&xs, &Value::Number(f64::NAN)), None);
        assert_eq!(index_of(&xs, &Value::Number(0.0)), Some(2));
        assert_eq!(index_of(&xs, &Value::Number(1.0)), Some(0));
        let words = vec![Value::str("a"), Value::str("b"), Value::str("b")];
        assert_eq!(index_of(&words, &Value::str("b")), Some(1));
        assert_eq!(index_of(&words, &Value::Number(1.0)), None);
    }
}

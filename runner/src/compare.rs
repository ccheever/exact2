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
//!
//! [`shared`] matches two lists by [`same`]: what a new answer kept of the
//! previous one, so the work of a live insert follows what changed.

use exact_plan::{Items, Value};
use std::rc::Rc;

/// Whether `a` and `b` are the same object: equal scalars (numbers by bits),
/// or one shared allocation.
pub fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.to_bits() == b.to_bits(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Unit, Value::Unit) | (Value::Option(None), Value::Option(None)) => true,
        (exact_plan::str_value!(), _) => Value::same_str(a, b),
        (Value::List(a), Value::List(b)) | (Value::Record(a), Value::Record(b)) => {
            Items::ptr_eq(a, b)
        }
        (Value::Option(Some(a)), Value::Option(Some(b))) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

/// Where a new list's items are the same objects as an old list's: a common
/// prefix and suffix, and between them, for each new item, the old position
/// of a same object if one was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shared {
    /// Leading items that are the same objects in both lists.
    pub prefix: usize,
    /// Trailing items that are the same objects in both lists (disjoint from
    /// the prefix in each).
    pub suffix: usize,
    /// Per new item in `prefix..new.len() - suffix`: an old position in
    /// `prefix..old.len() - suffix` holding the same object, or `None`.
    pub middle: Vec<Option<usize>>,
}

/// Match `new` against `old` by [`same`]: the common prefix and suffix, then
/// a lockstep walk of the middles that steps over single insertions,
/// removals and replacements, then (with `lookup`) an identity lookup for
/// what is left, which finds moved items at the cost of hashing it. Every
/// match is a same object; `None` only means none was found. O(N) pointer
/// comparisons, plus hashing the unmatched remainder: an insert at the top of
/// a 10,000-row answer is the whole old list as a suffix.
pub fn shared(old: &[Value], new: &[Value], lookup: bool) -> Shared {
    let prefix = old.iter().zip(new).take_while(|(a, b)| same(a, b)).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(room)
        .take_while(|(a, b)| same(a, b))
        .count();
    let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
    let mut middle = vec![None; new_end - prefix];
    let mut used = vec![false; old_end - prefix];
    let (mut i, mut j) = (prefix, prefix);
    while i < new_end && j < old_end {
        if same(&new[i], &old[j]) {
            middle[i - prefix] = Some(j);
            used[j - prefix] = true;
            i += 1;
            j += 1;
        } else if j + 1 < old_end && same(&new[i], &old[j + 1]) {
            // old[j] was removed.
            j += 1;
        } else {
            // new[i] was inserted if the next new item is old[j]; otherwise
            // it replaced old[j].
            let inserted = i + 1 < new_end && same(&new[i + 1], &old[j]);
            i += 1;
            if !inserted {
                j += 1;
            }
        }
    }
    if lookup && middle.iter().any(Option::is_none) && used.iter().any(|u| !u) {
        let mut by_identity = std::collections::HashMap::new();
        for (k, item) in old[prefix..old_end].iter().enumerate() {
            if !used[k] {
                by_identity.entry(identity(item)).or_insert(prefix + k);
            }
        }
        for (k, found) in middle.iter_mut().enumerate() {
            if found.is_none() {
                *found = by_identity.get(&identity(&new[prefix + k])).copied();
            }
        }
    }
    Shared {
        prefix,
        suffix,
        middle,
    }
}

/// A value's identity for [`same`]: equal exactly when `same` is true (a
/// scalar by its bits, anything else by its allocation's address).
fn identity(v: &Value) -> (u8, u64) {
    fn at<T: ?Sized>(rc: &Rc<T>) -> u64 {
        Rc::as_ptr(rc) as *const u8 as usize as u64
    }
    match v {
        Value::Number(n) => (0, n.to_bits()),
        Value::Bool(b) => (1, *b as u64),
        Value::Unit => (2, 0),
        Value::Option(None) => (3, 0),
        exact_plan::str_value!() => (4, v.str_identity().unwrap_or(0)),
        Value::Option(Some(v)) => (5, at(v)),
        Value::List(v) => (6, v.addr() as u64),
        Value::Record(v) => (7, v.addr() as u64),
    }
}

/// Whether no expression can tell `a` from `b`.
pub fn equivalent(a: &Value, b: &Value) -> bool {
    if same(a, b) {
        return true;
    }
    match (a, b) {
        (a, b) if a.is_str() && b.is_str() => a.as_str() == b.as_str(),
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

/// Which fields of a frame's value differ between `a` and `b`, as the
/// dependency table's field mask (LLP 1017.003 D6): bit `k` for field `k`,
/// bit 63 for 63 and past; `0` when [`equivalent_opt`]; every bit unless both
/// are records of one length.
pub fn changed_fields(a: &Option<Value>, b: &Option<Value>) -> u64 {
    match (a, b) {
        (Some(Value::Record(a)), Some(Value::Record(b))) if a.len() == b.len() => a
            .iter()
            .zip(b.iter())
            .enumerate()
            .filter(|(_, (a, b))| !equivalent(a, b))
            .fold(0, |mask, (k, _)| mask | 1 << k.min(63)),
        _ if equivalent_opt(a, b) => 0,
        _ => !0,
    }
}

/// [`equivalent`], element by element.
pub fn equivalent_all(a: &[Value], b: &[Value]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b))
}

/// The language's structural equality; `None` when the kinds differ before
/// the first difference.
///
/// The machine's ([`crate::machine::equal`]): the one the VM's `Eq` and `Ne`
/// run, proved against the semantics' `Value.equal`.
pub fn equal(a: &Value, b: &Value) -> Option<bool> {
    crate::machine::equal(a, b)
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
        let long = "a shared text longer than fourteen bytes";
        let shared = Value::str(long);
        assert!(same(&shared, &shared.clone()));
        assert!(!same(&shared, &Value::str(long)), "two allocations");
        assert!(equivalent(&shared, &Value::str(long)));
        // Inline text has no allocation: equal bytes are the same object.
        assert!(same(&Value::str("shared"), &Value::str("shared")));
        assert!(!same(&Value::str("shared"), &Value::str("shares")));
    }

    #[test]
    fn shared_finds_the_same_objects_around_an_edit() {
        // Shared text (longer than inline): identity is the allocation.
        let item = |i: usize| Value::str(&format!("item {i} of a longer list"));
        let old: Vec<Value> = (0..6).map(item).collect();
        let fresh = Value::str("new");
        // An insert at the top: the whole old list is the suffix.
        let mut new = old.clone();
        new.insert(0, fresh.clone());
        let s = shared(&old, &new, true);
        assert_eq!((s.prefix, s.suffix, s.middle), (0, 6, vec![None]));
        // One insert and one replacement: the lockstep steps over both.
        let mut new = old.clone();
        new.insert(1, fresh.clone());
        new[4] = item(3);
        let s = shared(&old, &new, true);
        assert_eq!((s.prefix, s.suffix), (1, 2));
        assert_eq!(s.middle, vec![None, Some(1), Some(2), None]);
        // A reversal: identity lookup, and an equal but new string is not
        // the same object.
        let mut new = old.clone();
        new.reverse();
        new.pop();
        new.push(item(0));
        let s = shared(&old, &new, true);
        let found: Vec<_> = (0..new.len())
            .map(|i| {
                if i < s.prefix {
                    Some(i)
                } else if i >= new.len() - s.suffix {
                    Some(old.len() - (new.len() - i))
                } else {
                    s.middle[i - s.prefix]
                }
            })
            .collect();
        assert_eq!(
            found,
            vec![Some(5), Some(4), Some(3), Some(2), Some(1), None]
        );
        // Inline and shared text with the same bytes (Charlie, 2026-09-28):
        // equal and equivalent; the same object only in one form.
        for t in ["", "bold", "exactly14bytes", "longer than fourteen bytes"] {
            let (inline, shared_form) = (Value::str(t), Value::str_shared_for_tests(t));
            assert_eq!(equal(&inline, &shared_form), Some(true));
            assert!(equivalent(&inline, &shared_form) && equivalent(&shared_form, &inline));
            let (a, b) = (
                Value::list(vec![inline.clone()]),
                Value::list(vec![shared_form.clone()]),
            );
            assert_eq!(equal(&a, &b), Some(true));
            assert!(equivalent(&a, &b));
            assert_eq!(equal(&inline, &Value::str("other")), Some(false));
        }
        // Inline text is the same object as equal inline text.
        let short: Vec<Value> = (0..3).map(|i| Value::str(&format!("{i}"))).collect();
        let s = shared(
            &short,
            &[Value::str("0"), Value::str("1"), Value::str("2")],
            true,
        );
        assert_eq!((s.prefix, s.suffix), (3, 0));
        // Numbers are the same by their bits.
        let numbers = [Value::Number(1.0), Value::Number(-0.0)];
        let s = shared(&numbers, &[Value::Number(0.0), Value::Number(1.0)], true);
        assert_eq!(s.middle, vec![None, Some(0)]);
    }
}

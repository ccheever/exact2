//! A settled resource's value, as the runner holds it and expressions read it.
//!
//! A resource that took its compiled value (LLP 1038 D5) holds what the
//! runner decoded from the plan's bytes. Once nothing outside the runner
//! holds that decoded object — a live tick replaced it on screen, or the
//! source adopted it (LLP 1027 D11) and answers its edits another way —
//! the copy is released: the plan's bytes stay in the executable, and a
//! later read decodes them again. Every holder inside the runner (the
//! settled state, the values expressions read, the tree's last inputs)
//! shares one cell, so "nothing else holds it" is one reference count.
//!
//! A value an expression had to decode again is kept from then on: a reader
//! that reads it and keeps nothing would otherwise decode it on every
//! evaluation. A read for inspection (the agent's `state`, a reload's carry,
//! a checkpoint) decodes into the cell and is released again after the next
//! update.

use exact_plan::{Bytes, Items, Plan, Str, Value};
use std::cell::{Cell, OnceCell};
use std::rc::Rc;

/// A settled resource's value: decoded, or a compiled one released to the
/// plan's bytes.
#[derive(Clone, Debug)]
pub struct Held(Rc<Inner>);

#[derive(Debug)]
struct Inner {
    /// Empty only when released: `compiled` then says where it decodes from.
    value: OnceCell<Value>,
    /// The plan's bytes this value decodes from, when it is the resource's
    /// compiled value.
    compiled: Option<Bytes>,
    /// An expression decoded it again after a release: kept from then on.
    kept: Cell<bool>,
}

impl Held {
    /// A value that is not the plan's (an answer, a carried or kept one).
    pub fn new(value: Value) -> Held {
        Held::with(value, None)
    }

    /// The plan's compiled value, decoded from `bytes`.
    pub fn compiled(value: Value, bytes: Bytes) -> Held {
        Held::with(value, Some(bytes))
    }

    fn with(value: Value, compiled: Option<Bytes>) -> Held {
        Held(Rc::new(Inner {
            value: OnceCell::from(value),
            compiled,
            kept: Cell::new(false),
        }))
    }

    /// The value, decoded again from `plan` when it was released.
    pub fn get(&self, plan: &Plan) -> &Value {
        self.0.value.get_or_init(|| {
            let bytes = self.0.compiled.expect("only a compiled value is released");
            Value::from_bytes(plan.bytes(bytes))
                .expect("a compiled value decodes as it did when it settled")
        })
    }

    /// What an expression reads: [`Held::get`], and a value decoded again
    /// for it is kept from then on.
    pub fn read(&self, plan: &Plan) -> Value {
        if self.0.value.get().is_none() {
            self.0.kept.set(true);
        }
        self.get(plan).clone()
    }

    /// The value, if it is decoded now.
    pub fn decoded(&self) -> Option<&Value> {
        self.0.value.get()
    }

    /// Whether this is the plan's compiled value.
    pub fn is_compiled(&self) -> bool {
        self.0.compiled.is_some()
    }

    /// Whether `a` and `b` are the same value for every reader: one cell,
    /// the same compiled bytes, or the same object ([`crate::compare::same`]).
    pub fn same(a: &Held, b: &Held) -> bool {
        Rc::ptr_eq(&a.0, &b.0)
            || (a.0.compiled.is_some() && a.0.compiled == b.0.compiled)
            || matches!((a.decoded(), b.decoded()), (Some(x), Some(y)) if crate::compare::same(x, y))
    }

    /// Whether `a` and `b` are indistinguishable to every expression:
    /// [`Held::same`], or decoded and [`crate::compare::equivalent`].
    pub fn equivalent(a: &Held, b: &Held) -> bool {
        Held::same(a, b)
            || matches!((a.decoded(), b.decoded()), (Some(x), Some(y)) if crate::compare::equivalent(x, y))
    }

    /// The released form of a compiled value no one outside this cell
    /// holds, to put in place of every copy of this one; `None` when it
    /// is not compiled, already released, kept, or still held elsewhere.
    pub fn released(&self) -> Option<Held> {
        let compiled = self.0.compiled?;
        if self.0.kept.get() || !self.0.value.get().is_some_and(sole) {
            return None;
        }
        Some(Held(Rc::new(Inner {
            value: OnceCell::new(),
            compiled: Some(compiled),
            kept: Cell::new(false),
        })))
    }
}

/// Whether this cell's is the only reference to `value`'s allocation, so
/// dropping it frees the value. A scalar has nothing to free.
fn sole(value: &Value) -> bool {
    match value {
        Value::Str(s) => Str::strong_count(s) == 1,
        Value::List(items) | Value::Record(items) => Items::strong_count(items) == 1,
        Value::Option(Some(inner)) => Rc::strong_count(inner) == 1,
        Value::Number(_) | Value::Bool(_) | Value::Unit | Value::Option(None) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_plan::builder::PlanBuilder;

    fn plan_with(value: &Value) -> (Plan, Bytes) {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let bytes = b.data(value);
        (b.finish().expect("plan"), bytes)
    }

    #[test]
    fn a_sole_compiled_value_is_released_and_decodes_again() {
        let value = Value::list(vec![
            Value::str("a long enough string to be its own"),
            Value::Number(1.0),
        ]);
        let (plan, bytes) = plan_with(&value);
        let held = Held::compiled(Value::from_bytes(plan.bytes(bytes)).unwrap(), bytes);
        let released = held.released().expect("no one else holds it");
        assert!(released.decoded().is_none());
        assert!(
            Held::same(&held, &released),
            "the same value for every reader"
        );
        assert_eq!(released.get(&plan), &value);
        // Decoded for inspection: released again once no one holds it.
        assert!(released.released().is_some());
    }

    #[test]
    fn a_value_held_elsewhere_or_not_compiled_stays() {
        let value = Value::list(vec![Value::Number(1.0)]);
        let (plan, bytes) = plan_with(&value);
        let held = Held::compiled(Value::from_bytes(plan.bytes(bytes)).unwrap(), bytes);
        let reader = held.get(&plan).clone();
        assert!(held.released().is_none(), "a reader holds it");
        drop(reader);
        assert!(held.released().is_some());
        assert!(
            Held::new(value).released().is_none(),
            "an answer is not the plan's"
        );
        let scalar = Value::Number(2.0);
        let (plan, bytes) = plan_with(&scalar);
        let held = Held::compiled(Value::from_bytes(plan.bytes(bytes)).unwrap(), bytes);
        assert!(held.released().is_none(), "nothing to free");
    }

    #[test]
    fn an_expression_read_after_a_release_keeps_it() {
        let value = Value::list(vec![Value::Number(1.0)]);
        let (plan, bytes) = plan_with(&value);
        let held = Held::compiled(Value::from_bytes(plan.bytes(bytes)).unwrap(), bytes);
        let released = held.released().unwrap();
        drop(released.read(&plan));
        assert!(
            released.released().is_none(),
            "decoded again by an expression: kept"
        );
    }
}

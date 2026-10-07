//! Type checks that follow what changed (LLP 1053 §0 G8).
//!
//! A value that crosses the data seam, or a derive's result, is checked
//! against its declared type ([`Value::conforms`]) every time. A live answer
//! is mostly the previous answer's objects: a 10,000-row feed with one row
//! inserted and one replaced. [`Conformed`] remembers, per list type, the
//! last list it found conforming, and checks a new list of that type only
//! where its items are not the same objects ([`crate::compare::shared`]).
//!
//! Sound because a value is immutable and the remembered list holds its
//! items: while it is held, an object at the same address is that object,
//! and no one can change it in place (`Rc::get_mut` and `Rc::make_mut` see
//! the second reference). Every item that is not the same object is checked
//! in full, so an invalid value is refused as before. Bounded by one list per
//! list type the plan declares; a plan is fixed for a runner's life (a
//! reload is a new runner), so the types never change under it.
use crate::compare;
use exact_plan::{Items, Plan, TypeKind, TypesId, Value};
use std::collections::BTreeMap;

/// The lists found conforming, by their type.
#[derive(Debug, Default)]
pub(crate) struct Conformed {
    lists: BTreeMap<u32, Items>,
    /// List items checked in full, for tests and measurement.
    pub(crate) items_checked: usize,
}

impl Conformed {
    /// [`Value::conforms`], skipping list items already found conforming.
    /// Records and options are followed to reach the lists inside them;
    /// a list's own items are checked in full (their nested lists are many
    /// different lists of one type, which one entry per type cannot hold).
    pub(crate) fn conforms(&mut self, plan: &Plan, value: &Value, ty: TypesId) -> bool {
        let row = plan.type_(ty);
        match (row.kind, value) {
            (TypeKind::Option, Value::Option(Some(inner))) => {
                row.elem.is_some_and(|e| self.conforms(plan, inner, e))
            }
            (TypeKind::Record, Value::Record(values)) => {
                let fields = row.fields;
                values.len() == fields.len as usize
                    && fields
                        .iter()
                        .zip(values.iter())
                        .all(|(f, v)| self.conforms(plan, v, plan.field(f).ty))
            }
            (TypeKind::List, Value::List(items)) => {
                row.elem.is_some_and(|e| self.list(plan, items, ty, e))
            }
            _ => value.conforms(plan, ty),
        }
    }

    fn list(&mut self, plan: &Plan, items: &Items, ty: TypesId, elem: TypesId) -> bool {
        let mut checked = 0;
        let mut check = |item: &Value| {
            checked += 1;
            item.conforms(plan, elem)
        };
        let ok = match self.lists.get(&ty.0) {
            Some(previous) if Items::ptr_eq(previous, items) => return true,
            Some(previous) => {
                // Moved items are checked again: finding them costs about
                // what checking them does.
                let shared = compare::shared(previous, items, false);
                let end = items.len() - shared.suffix;
                shared
                    .middle
                    .iter()
                    .zip(&items[shared.prefix..end])
                    .all(|(found, item)| found.is_some() || check(item))
            }
            None => items.iter().all(check),
        };
        self.items_checked += checked;
        if ok {
            self.lists.insert(ty.0, items.clone());
        }
        ok
    }
}

/// Where a value that does not conform to `ty` first differs from it: a
/// path from the value's root and what was there, the refusal's `why`
/// (LLP 1101.002 §0 P9). Only asked once a value is refused.
pub(crate) fn mismatch(plan: &Plan, value: &Value, ty: TypesId) -> String {
    let mut path = String::from(plan.str(plan.type_(ty).name));
    let what = differs(plan, value, ty, &mut path).unwrap_or_else(|| "it conforms".into());
    format!("{path}: {what}")
}

fn differs(plan: &Plan, value: &Value, ty: TypesId, path: &mut String) -> Option<String> {
    let row = plan.type_(ty);
    match (row.kind, value) {
        (TypeKind::Option, Value::Option(Some(inner))) => differs(plan, inner, row.elem?, path),
        (TypeKind::List, Value::List(items)) => {
            let elem = row.elem?;
            let (i, item) = items
                .iter()
                .enumerate()
                .find(|(_, v)| !v.conforms(plan, elem))?;
            path.push_str(&format!("[{i}]"));
            differs(plan, item, elem, path)
        }
        (TypeKind::Record, Value::Record(values)) => {
            let declared = row.fields.len as usize;
            for (i, f) in row.fields.iter().enumerate() {
                let field = plan.field(f);
                let Some(v) = values.get(i) else { break };
                if !v.conforms(plan, field.ty) {
                    path.push('.');
                    path.push_str(plan.str(field.name));
                    return differs(plan, v, field.ty, path);
                }
            }
            (values.len() != declared).then(|| {
                let first = row.fields.iter().nth(values.len().min(declared));
                let missing = match first {
                    Some(f) if values.len() < declared => {
                        format!(", from `{}` on", plan.str(plan.field(f).name))
                    }
                    _ => String::new(),
                };
                format!(
                    "{} fields where the shape declares {declared}{missing}",
                    values.len()
                )
            })
        }
        _ if value.conforms(plan, ty) => None,
        (kind, _) => Some(format!(
            "a {kind:?} was declared, and {} answered",
            found(value)
        )),
    }
}

/// A value as a refusal names it: a scalar itself (a string cut to 40
/// characters), anything larger by its kind and size.
fn found(value: &Value) -> String {
    match value {
        Value::Number(n) => format!("{n}"),
        Value::Bool(b) => format!("{b}"),
        Value::Unit => "unit".into(),
        Value::Option(None) => "none".into(),
        Value::Option(Some(v)) => format!("some({})", found(v)),
        Value::List(items) => format!("a list of {}", items.len()),
        Value::Record(items) => format!("a record of {} fields", items.len()),
        _ => {
            let text = value.as_str().unwrap_or_default();
            let mut cut: String = text.chars().take(40).collect();
            if cut.len() < text.len() {
                cut.push('…');
            }
            format!("{cut:?}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_plan::builder::PlanBuilder;

    fn plan() -> (Plan, TypesId) {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let number = b.primitive(TypeKind::Number);
        let string = b.primitive(TypeKind::String);
        let item = b.record("Item", &[("id", string), ("count", number)]);
        let rows = b.list(item);
        let feed = b.record("Feed", &[("rows", rows)]);
        (b.finish().unwrap(), feed)
    }

    fn item(i: usize) -> Value {
        Value::record(vec![Value::str(&format!("m{i}")), Value::Number(i as f64)])
    }

    fn feed(rows: &Items) -> Value {
        Value::record(vec![Value::List(rows.clone())])
    }

    #[test]
    fn a_shared_valid_list_is_checked_once() {
        let (plan, ty) = plan();
        let mut c = Conformed::default();
        let rows = Items::from((0..10_000).map(item).collect::<Vec<_>>());
        assert!(c.conforms(&plan, &feed(&rows), ty));
        assert_eq!(c.items_checked, 10_000);
        // The same answer again (the settle pass after the action's check).
        assert!(c.conforms(&plan, &feed(&rows), ty));
        assert_eq!(c.items_checked, 10_000);
        // A live tick: one inserted at the top, one replaced.
        let mut next = rows.to_vec();
        next.insert(0, item(10_000));
        next[5_000] = item(4_999);
        let next = Items::from(next);
        assert!(c.conforms(&plan, &feed(&next), ty));
        assert_eq!(c.items_checked, 10_002);
        // An equal list of new objects is checked in full.
        let copy = Items::from((0..10_000).map(item).collect::<Vec<_>>());
        assert!(c.conforms(&plan, &feed(&copy), ty));
        assert_eq!(c.items_checked, 20_002);
    }

    #[test]
    fn an_invalid_record_inside_a_shared_list_is_refused() {
        let (plan, ty) = plan();
        let mut c = Conformed::default();
        let rows = Items::from((0..1_000).map(item).collect::<Vec<_>>());
        assert!(c.conforms(&plan, &feed(&rows), ty));
        let bad = [
            Value::record(vec![Value::str("m1"), Value::str("one")]),
            Value::record(vec![Value::str("m1"), Value::Number(f64::NAN)]),
            Value::record(vec![Value::str("m1")]),
            Value::Number(1.0),
        ];
        for bad in bad {
            let mut next = rows.to_vec();
            next.insert(0, item(1_000));
            next[500] = bad;
            let next = Items::from(next);
            assert!(!c.conforms(&plan, &feed(&next), ty));
            // What was refused is not remembered; a full check agrees.
            assert!(!feed(&next).conforms(&plan, ty));
            assert!(!c.conforms(&plan, &feed(&next), ty));
        }
        // The remembered list still answers for the good one.
        let before = c.items_checked;
        assert!(c.conforms(&plan, &feed(&rows), ty));
        assert_eq!(c.items_checked, before);
    }

    #[test]
    fn a_refusal_names_where_the_answer_differs() {
        let (plan, ty) = plan();
        let rows = |bad: Value| {
            let mut all: Vec<Value> = (0..3).map(item).collect();
            all[2] = bad;
            feed(&Items::from(all))
        };
        let why = |v: &Value| mismatch(&plan, v, ty);
        assert_eq!(
            why(&rows(Value::record(vec![
                Value::str("m2"),
                Value::str("two")
            ]))),
            "Feed.rows[2].count: a Number was declared, and \"two\" answered"
        );
        assert_eq!(
            why(&rows(Value::record(vec![Value::str("m2")]))),
            "Feed.rows[2]: 1 fields where the shape declares 2, from `count` on"
        );
    }
}

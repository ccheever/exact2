//! The roster's list calls that step on an evaluation's budget, split from
//! `vm.rs`, whose private `step` and `Extents` they use.

use super::{step, Extents, Trap};
use exact_plan::{Opcode, Stdlib, Value};

/// `concat`, `split`, `sum`, and `slice`, `includes` and `indexOf` over a list
/// (LLP 1088 §9.1), on this evaluation's budget as `join` is: the list steps
/// first — one per item `concat`, `slice` and `split` keep or `sum` adds, one
/// per item `includes` and `indexOf` scan up to the match — then a list they
/// build measured as `Opcode::List` measures one (LLP 1090 D3). `None` for
/// `slice`, `includes` and `indexOf` over text.
pub(super) fn list_call(
    f: Stdlib,
    args: &[Value],
    steps: &mut u32,
    extents: &mut Extents,
    pc: usize,
) -> Result<Option<Value>, Trap> {
    let mismatch = Trap::TypeMismatch {
        pc,
        op: Opcode::Call,
    };
    if f == Stdlib::Split {
        let (Some(s), Some(sep)) = (args[0].as_str(), args[1].as_str()) else {
            return Err(mismatch);
        };
        step(steps, crate::strings::pieces(s, sep), pc)?;
        let built: Vec<Value> = (crate::strings::split(s, sep).iter())
            .map(|p| Value::str(p))
            .collect();
        let e = extents.built(&built, pc)?;
        let v = Value::list(built);
        extents.remember(&v, e);
        return Ok(Some(v));
    }
    let items = match (f, args.first()) {
        (
            Stdlib::Concat | Stdlib::Slice | Stdlib::Includes | Stdlib::IndexOf | Stdlib::Sum,
            Some(Value::List(items)),
        ) => items,
        (Stdlib::Concat | Stdlib::Sum, _) => return Err(mismatch),
        _ => return Ok(None),
    };
    let built = match (f, args.get(1), args.get(2)) {
        (Stdlib::Concat, Some(Value::List(more)), _) => {
            step(steps, items.len() + more.len(), pc)?;
            crate::lists::concat(items, more)
        }
        (Stdlib::Slice, Some(Value::Number(a)), Some(Value::Number(b))) => {
            let kept = crate::lists::slice(items, *a, *b);
            step(steps, kept.len(), pc)?;
            kept.to_vec()
        }
        (Stdlib::Includes, Some(x), _) => {
            let at = crate::lists::position(items, x);
            step(steps, at.map_or(items.len(), |i| i + 1), pc)?;
            return Ok(Some(Value::Bool(at.is_some())));
        }
        (Stdlib::Sum, _, _) => {
            step(steps, items.len(), pc)?;
            return crate::lists::sum(items)
                .map(|n| Some(Value::Number(n)))
                .ok_or(mismatch);
        }
        (Stdlib::IndexOf, Some(x), _) => {
            let at = crate::lists::index_of(items, x);
            step(steps, at.map_or(items.len(), |i| i + 1), pc)?;
            return Ok(Some(Value::Number(at.map_or(-1.0, |i| i as f64))));
        }
        _ => return Err(mismatch),
    };
    let e = extents.built(&built, pc)?;
    let v = Value::list(built);
    extents.remember(&v, e);
    Ok(Some(v))
}

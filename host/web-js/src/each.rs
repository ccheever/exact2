//! A resource over a list (LLP 1017.000 P4c, amended 2026-11-04): a
//! resource for each element, keyed (`each.js`), whose arguments read the
//! element as `p0`; each shows its type's zero until it answers.

use super::{type_code, zero};
use crate::code::{self, Scope, Uses};
use exact_plan::Plan;

/// Row `i`'s declaration when it is over a list, its arguments `args`.
pub(super) fn resource(
    plan: &Plan,
    i: usize,
    args: &[String],
    top: &Scope,
    uses: &mut Uses,
) -> Result<Option<String>, String> {
    let r = &plan.resources[i];
    let Some((list, key)) = plan.resource_each(r) else {
        return Ok(None);
    };
    let mut part = |c| {
        code::expression(plan, plan.code(c), top, uses)
            .map_err(|e| format!("resource {}: {e}", plan.str(r.name)))
    };
    let (list, key) = (part(list)?, part(key)?);
    let elem = plan.types[r.ty.0 as usize]
        .elem
        .ok_or("a list resource's type")?;
    Ok(Some(format!(
        "const r_{i}={}({},{},()=>{list},p0=>{key},p0=>[{}],{},{},{});",
        uses.rt("resEach"),
        serde_json::to_string(plan.str(r.name)).unwrap(),
        serde_json::to_string(plan.str(r.source)).unwrap(),
        args.join(","),
        serde_json::to_string(&type_code(plan, r.ty)).unwrap(),
        serde_json::to_string(&type_code(plan, elem)).unwrap(),
        zero(plan, elem),
    )))
}

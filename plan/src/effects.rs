//! Ownership and boot-seed invariants beyond generated index/range checks.

use crate::{Plan, PlanError, RegionKind, RegionsId, Value};

fn bad(table: &'static str, row: usize, field: &'static str) -> PlanError {
    PlanError::BadReference {
        table,
        row: row as u32,
        field,
    }
}

impl Plan {
    pub(crate) fn validate_effects(&self) -> Result<(), PlanError> {
        let owner_ok = |owner: RegionsId, allow_each: bool| {
            matches!(self.region(owner).kind, RegionKind::Scope)
                || allow_each && self.region(owner).kind == RegionKind::Each
        };
        for (i, slot) in self.slots.iter().enumerate() {
            // Hand-built row-slot plans are still useful VM fixtures; the compiler
            // exclusively emits Scope ownership for child declarations.
            if slot.owner.is_some_and(|owner| !owner_ok(owner, true)) {
                return Err(bad("slots", i, "owner"));
            }
        }
        for (i, resource) in self.resources.iter().enumerate() {
            if resource.owner.is_some_and(|owner| !owner_ok(owner, false)) {
                return Err(bad("resources", i, "owner"));
            }
            if resource.owner.is_some() && resource.initial.len != 0 {
                return Err(bad("resources", i, "initial"));
            }
        }
        for (i, timer) in self.timers.iter().enumerate() {
            if timer.owner.is_some_and(|owner| !owner_ok(owner, false)) {
                return Err(bad("timers", i, "owner"));
            }
            if timer.args.len != self.action(timer.action).params.len {
                return Err(bad("timers", i, "args"));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for (i, seed) in self.resource_boot.iter().enumerate() {
            let resource = self.resource(seed.resource);
            if resource.owner.is_none() {
                return Err(bad("resource_boot", i, "resource"));
            }
            let path = Value::from_bytes(self.bytes(seed.path))?;
            let Value::List(keys) = &path else {
                return Err(bad("resource_boot", i, "path"));
            };
            if keys
                .iter()
                .any(|key| !matches!(key, Value::Str(_) | Value::Bool(_) | Value::Number(_)))
            {
                return Err(bad("resource_boot", i, "path"));
            }
            // Canonicalize numeric zero as the runtime key map does, and use
            // ordered bytes so validation remains O(n log n) for repeated rows.
            let key = Value::list(
                keys.iter()
                    .map(|key| match key {
                        Value::Number(n) if *n == 0.0 => Value::Number(0.0),
                        other => other.clone(),
                    })
                    .collect(),
            )
            .to_bytes();
            if !seen.insert((seed.resource, key)) {
                return Err(bad("resource_boot", i, "path"));
            }
            let args = Value::from_bytes(self.bytes(seed.args))?;
            if !matches!(args, Value::List(ref args) if args.len() == resource.args.len as usize) {
                return Err(bad("resource_boot", i, "args"));
            }
            let value = Value::from_bytes(self.bytes(seed.value))?;
            if !value.conforms(self, resource.ty) {
                return Err(bad("resource_boot", i, "value"));
            }
        }
        Ok(())
    }
}

//! Typed props replace only the snapshot entry's root slot (LLP 1126 D2).
use exact_plan::{Plan, TypeKind, TypesId, Value};
use exact_runner::Carried;

pub(crate) fn props(plan: &Plan, bytes: &[u8]) -> Result<Carried, String> {
    let row = plan
        .slots
        .iter()
        .find(|s| plan.str(s.name) == "snapshot_props" && s.owner.is_none())
        .ok_or("entry has no props slot")?;
    let json = serde_json::from_slice(bytes).map_err(|e| format!("entry props: {e}"))?;
    let value =
        decode(plan, row.ty, &json).ok_or("entry props do not match their declared shape")?;
    Ok(Carried {
        slots: vec![(plan.str(row.name).into(), value)],
        ..Carried::default()
    })
}
fn decode(plan: &Plan, ty: TypesId, json: &serde_json::Value) -> Option<Value> {
    let row = plan.type_(ty);
    let value = match row.kind {
        TypeKind::Number => Value::Number(json.as_f64().filter(|n| n.is_finite())?),
        TypeKind::Bool => Value::Bool(json.as_bool()?),
        TypeKind::String => Value::str(json.as_str()?),
        TypeKind::Unit if json.is_null() => Value::Unit,
        TypeKind::Option => Value::Option(if json.is_null() {
            None
        } else {
            Some(std::rc::Rc::new(decode(plan, row.elem?, json)?))
        }),
        TypeKind::List => Value::list(
            json.as_array()?
                .iter()
                .map(|v| decode(plan, row.elem?, v))
                .collect::<Option<Vec<_>>>()?,
        ),
        TypeKind::Record => {
            let fields = json.as_object()?;
            if fields.len() != row.fields.len as usize {
                return None;
            }
            Value::record(
                row.fields
                    .iter()
                    .map(|id| {
                        let field = plan.field(id);
                        decode(plan, field.ty, fields.get(plan.str(field.name))?)
                    })
                    .collect::<Option<Vec<_>>>()?,
            )
        }
        _ => return None,
    };
    value.conforms(plan, ty).then_some(value)
}

impl<D: exact_runner::DataSource> crate::Host<D> {
    pub(crate) fn entry_timeline(&mut self, now: f64) -> Result<String, String> {
        if self.runner.plan().snapshot_entry.is_empty() {
            return Err("not a snapshot entry".into());
        }
        let data = self.runner.data_ref();
        let (bindings, error) = crate::store::endow_bound(data.grants(), data.app_id(), false);
        if let Some(error) = error {
            return Err(error);
        }
        let snapshot =
            crate::store::snapshot_in(bindings.as_ref(), &crate::store::Prefs::of(data.app_id()));
        self.runner.snapshot_timeline(now, snapshot)
    }
}

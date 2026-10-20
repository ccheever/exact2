//! `exactMIDI()`'s shape (LLP 1125 D6): the runner answers `state`,
//! `inputs`, `outputs` and `sent`, each port's `id`, `name`, `manufacturer`
//! and `state`, and each sent message's `output`, `data` and `timeStamp`, so
//! a declared field it does not know would refuse every boot; it is refused
//! here, naming it. The top level is `fact_shape`'s rule; a list's item
//! shape is this module's, which that rule does not look into.

use super::BakeError;
use exact_plan::{Plan, TypeKind};
use exact_runner::midi::{PORT_FIELDS, SENT_FIELDS, SOURCE};

/// Each `exactMIDI` reader's lists: a record of [`PORT_FIELDS`] only, or
/// of [`SENT_FIELDS`] for `sent`.
pub(super) fn shape(plan: &Plan) -> Result<(), BakeError> {
    for row in plan.resources.iter() {
        if plan.str(row.source) != SOURCE {
            continue;
        }
        let ty = plan.type_(row.ty);
        for f in ty.fields.iter() {
            let field = plan.field(f);
            let name = plan.str(field.name);
            let (known, what) = match name {
                "inputs" | "outputs" => (PORT_FIELDS, ("port", "a MIDI port")),
                "sent" => (SENT_FIELDS, ("message", "a sent message")),
                _ => continue,
            };
            let list = plan.type_(field.ty);
            let item = (list.kind == TypeKind::List)
                .then_some(list.elem)
                .flatten()
                .map(|e| plan.type_(e))
                .filter(|t| t.kind == TypeKind::Record);
            let Some(item) = item else {
                return Err(BakeError::Lint {
                    site: None,
                    id: "bake-midi-field",
                    message: format!(
                        "`{name}` of `resource {} = {SOURCE}()` is a list of a {} shape of {}",
                        plan.str(row.name),
                        what.0,
                        known.join(", ")
                    ),
                });
            };
            for p in item.fields.iter() {
                let port = plan.str(plan.field(p).name);
                if !known.contains(&port) {
                    return Err(BakeError::Lint {
                        site: None,
                        id: "bake-midi-field",
                        message: format!(
                            "`{}` declares `{port}`, which {} does not answer; it answers {}",
                            plan.str(item.name),
                            what.1,
                            known.join(", ")
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

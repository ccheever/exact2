//! @ref LLP 1126.001 WP9 — validate nested facts even when the baked list is empty.
use super::BakeError;
use exact_plan::{Plan, TypeKind, TypesId};

pub(super) fn shape(plan: &Plan) -> Result<(), BakeError> {
    for resource in plan
        .resources
        .iter()
        .filter(|r| plan.str(r.source) == "exactActivities")
    {
        for id in plan.type_(resource.ty).fields.iter() {
            let field = plan.field(id);
            let name = plan.str(field.name);
            let ty = plan.type_(field.ty);
            let valid = if name == "activities" {
                ty.kind == TypeKind::List && ty.elem.is_some_and(|item| item_shape(plan, item))
            } else {
                ty.kind == TypeKind::Bool
            };
            if !valid {
                return Err(BakeError::Lint {
                    site: None,
                    id: "bake-activities-field",
                    message: format!("`{}`: exactActivities.{name} needs activities: Activity[] or a bool fact; Activity fields are id, kind, state, activityState (string), token (option<string>)", plan.str(resource.name)),
                });
            }
        }
    }
    Ok(())
}

fn item_shape(plan: &Plan, id: TypesId) -> bool {
    let ty = plan.type_(id);
    ty.kind == TypeKind::Record
        && ty.fields.iter().all(|id| {
            let field = plan.field(id);
            let ty = plan.type_(field.ty);
            match plan.str(field.name) {
                "id" | "kind" | "state" | "activityState" => ty.kind == TypeKind::String,
                "token" => {
                    ty.kind == TypeKind::Option
                        && ty
                            .elem
                            .is_some_and(|id| plan.type_(id).kind == TypeKind::String)
                }
                _ => false,
            }
        })
}

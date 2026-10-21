//! CSS names for the plan's font families and stacks.
use super::{css_string, generic_stack, is_generic_family};

/// A runtime list as safe CSS family members in authored fallback order.
pub fn runtime_font_family(list: &str) -> String {
    exact_runner::bridge::runtime_family_members(list)
        .iter()
        .map(|member| {
            let lower = member.name.to_ascii_lowercase();
            if !member.quoted && is_generic_family(&lower) {
                generic_stack(&lower).to_string()
            } else {
                css_string(&member.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// CSS family aliases for declared fonts; local families keep their CSS names.
pub fn font_alias(plan: &exact_plan::Plan, family: exact_plan::FamiliesId) -> String {
    let row = plan.familie(family);
    if row.faces.len == 0 {
        return plan.str(row.name).into();
    }
    let i = plan
        .stacks
        .iter()
        .position(|s| {
            s.members.len == 1
                && plan.stack_member(s.members.iter().next().unwrap()).family == Some(family)
        })
        .unwrap_or(8 + family.0 as usize);
    format!("ExactPlanStack{i}")
}

/// Every stack's complete CSS fallback list in authored order.
pub fn font_family_names(plan: &exact_plan::Plan) -> Vec<String> {
    plan.stacks
        .iter()
        .map(|stack| {
            stack
                .members
                .iter()
                .map(|id| {
                    let member = plan.stack_member(id);
                    match member.kind {
                        exact_plan::StackMemberKind::Family => {
                            css_string(&font_alias(plan, member.family.unwrap()))
                        }
                        generic => generic_stack(generic.name()).into(),
                    }
                })
                .collect::<Vec<String>>()
                .join(", ")
        })
        .collect()
}

#[cfg(test)]
mod runtime_font_tests {
    #[test]
    fn a_runtime_list_escapes_names_and_expands_ui_generic_fallback() {
        assert_eq!(
            super::runtime_font_family("'A, B', ui-monospace"),
            "\"A, B\", ui-monospace,monospace"
        );
    }

    #[test]
    fn a_quoted_generic_remains_a_family_name() {
        assert_eq!(
            super::runtime_font_family("'serif', Arial"),
            "\"serif\", \"Arial\""
        );
    }
}

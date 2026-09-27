use super::*;

pub(super) struct FontFace {
    pub(super) family: String,
    /// The family's declared name, which canvas text names (LLP 1056 D8).
    pub(super) declared: String,
    pub(super) source: String,
    pub(super) weight: u16,
    pub(super) italic: bool,
}

pub(super) fn font_names(plan: &Plan) -> Vec<String> {
    plan.stacks
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let stack = plan.stack(StacksId(i as u32));
            let member = plan.stack_member(stack.members.iter().next().expect("validated stack"));
            match member.kind {
                StackMemberKind::Family => format!("ExactPlanStack{i}"),
                generic => generic.name().to_string(),
            }
        })
        .collect()
}

thread_local! {
    // The plan a font query last decoded, beside its bytes: the glue asks for
    // a plan's faces right before it boots that plan, and decoding validates
    // the whole plan, so the boot takes this one instead of doing it again.
    static DECODED: std::cell::RefCell<Option<(Vec<u8>, Plan)>> = const { std::cell::RefCell::new(None) };
}

/// Decode a candidate's font catalog without replacing or starting a host.
pub(crate) fn plan_font_catalog(bytes: &[u8]) -> Result<String, exact_plan::PlanError> {
    let plan = Plan::decode(bytes)?;
    let catalog = font_catalog(&font_faces(&plan));
    DECODED.with(|decoded| *decoded.borrow_mut() = Some((bytes.to_vec(), plan)));
    Ok(catalog)
}

/// `bytes` as a decoded, validated plan: the one a font query just decoded
/// when the bytes are the same, else decoded now.
pub(crate) fn decode_plan(bytes: &[u8]) -> Result<Plan, exact_plan::PlanError> {
    match DECODED.with(|decoded| decoded.borrow_mut().take()) {
        Some((same, plan)) if same == bytes => Ok(plan),
        _ => Plan::decode(bytes),
    }
}

pub(super) fn font_faces(plan: &Plan) -> Vec<FontFace> {
    let mut out = Vec::new();
    for (stack_index, stack) in plan.stacks.iter().enumerate() {
        let member = plan.stack_member(
            stack
                .members
                .iter()
                .next()
                .expect("validated non-empty stack"),
        );
        if member.kind != StackMemberKind::Family {
            continue;
        }
        let family = plan.familie(member.family.expect("validated family member"));
        let name = format!("ExactPlanStack{stack_index}");
        for face_id in family.faces.iter() {
            let face = plan.face(face_id);
            out.push(FontFace {
                family: name.clone(),
                declared: plan.str(family.name).to_string(),
                source: plan.str(face.source).to_string(),
                weight: face.weight,
                italic: face.italic,
            });
        }
    }
    out
}

pub(super) fn font_catalog(faces: &[FontFace]) -> String {
    let mut out = String::from("[");
    for (i, face) in faces.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"family\":");
        crate::batch::quote(&face.family, &mut out);
        out.push_str(",\"declared\":");
        crate::batch::quote(&face.declared, &mut out);
        out.push_str(",\"source\":");
        crate::batch::quote(&face.source, &mut out);
        let _ = write!(
            out,
            ",\"weight\":{},\"style\":\"{}\"}}",
            face.weight,
            if face.italic { "italic" } else { "normal" }
        );
    }
    out.push(']');
    out
}

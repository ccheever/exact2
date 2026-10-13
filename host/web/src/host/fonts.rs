//! The plan's declared fonts (LLP 1019): the faces, their CSS family names
//! and the catalog the glue loads before first paint.

use super::*;

/// One declared face, under the family name the host's CSS uses
/// (`ExactPlanStack<i>`); the JS target declares the same (LLP 1071 §7).
pub struct FontFace {
    /// The CSS family: `ExactPlanStack<stack index>`.
    pub family: String,
    /// The family's declared name, which canvas text names (LLP 1056 D8).
    pub declared: String,
    /// The face's file, relative to the app.
    pub source: String,
    /// Its weight.
    pub weight: u16,
    /// Whether it is italic.
    pub italic: bool,
}

pub(super) fn font_names(plan: &Plan) -> Vec<String> {
    crate::css::font_family_names(plan)
}

pub(super) fn font_names_for_runner<D: DataSource>(runner: &Runner<D>) -> Vec<String> {
    let mut names = font_names(runner.plan());
    names.extend(
        runner
            .runtime_font_stacks()
            .iter()
            .map(|list| crate::css::runtime_font_family(list)),
    );
    names
}

impl<D: DataSource> Host<D> {
    pub(super) fn sync_runtime_font_names(&mut self) {
        let compiled = self.runner.plan().stacks.len();
        if self.runner.runtime_font_count() == self.font_names.len().saturating_sub(compiled) {
            return;
        }
        let runtime = self.runner.runtime_font_stacks();
        for name in runtime
            .iter()
            .skip(self.font_names.len().saturating_sub(compiled))
        {
            self.font_names.push(crate::css::runtime_font_family(name));
        }
    }
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

/// The plan's declared faces, in stack order.
pub fn font_faces(plan: &Plan) -> Vec<FontFace> {
    let mut out = Vec::new();
    for (i, family) in plan.families.iter().enumerate() {
        let name = crate::css::font_alias(plan, exact_plan::FamiliesId(i as u32));
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

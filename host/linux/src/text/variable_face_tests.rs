use super::*;

const VARIABLE: &[u8] = include_bytes!("../../../apple/tests/fonts/InterVariable.ttf");
const STATIC: &[u8] = include_bytes!("../../tests/fonts/Inter-Regular.ttf");
const TEXT: &str = "Hamburgefonts";

fn declared(bytes: &'static [u8], weights: &[u16]) -> (TextEngine, u16) {
    declared_assets(
        weights,
        Assets::selected(
            Path::new("").into(),
            Arc::new(move |name| Ok((name == "fonts/InterVariable.ttf").then(|| Arc::from(bytes)))),
        ),
    )
}

fn declared_assets(weights: &[u16], assets: Assets) -> (TextEngine, u16) {
    let plan = contract::compile("component App\n  view\n    text \"fixture\"\n").unwrap();
    let mut builder = exact_plan::builder::PlanBuilder::from_plan(plan);
    let faces: Vec<_> = weights
        .iter()
        .map(|weight| ("fonts/InterVariable.ttf", *weight, false))
        .collect();
    let family = builder.font_family("Pinned", &faces);
    builder.font_stack(&[(StackMemberKind::Family, Some(family))]);
    let plan = builder.finish().unwrap();
    let catalog = catalog::Catalog::for_assets(&plan, &assets);
    let family = catalog
        .families
        .iter()
        .position(
            |f| matches!(f, FamilyChoice::Declared(name) if name.starts_with("ExactPlanStack")),
        )
        .expect("declared font registered") as u16;
    (TextEngine::with_catalog(catalog), family)
}

fn shape(engine: &mut TextEngine, family: u16, weight: u16) -> Rc<Paragraph> {
    let style = exact_kernel::StyleProps {
        font_family: family,
        font_weight: weight,
        font_size: 32.,
        ..Default::default()
    };
    engine.paragraph(&crate::paint::text_spec(&style, TEXT), None)
}

fn system(bytes: &[u8], family: &str) -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::from_bytes(&[bytes], family))
}

fn advances(p: &Paragraph) -> Vec<f32> {
    p.layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|g| g.w))
        .collect()
}

fn assert_pinned(p: &Paragraph, reference: &Paragraph, weight: u16) {
    assert_eq!(p.lines().faces.len(), 1);
    let face = &p.lines().faces[0];
    assert_eq!(face.font.data.data(), VARIABLE);
    let settings = face.synthesis.variation_settings();
    assert_eq!(
        settings
            .iter()
            .find(|(tag, _)| tag.to_be_bytes() == *b"wght")
            .map(|(_, v)| *v),
        Some(f32::from(weight)),
        "declared wght must reach both shaping and presenters"
    );
    let font = swash::FontRef::from_index(VARIABLE, 0).unwrap();
    let coords: Vec<_> = font
        .variations()
        .normalized_coords([(swash::tag_from_bytes(b"wght"), f32::from(weight))])
        .collect();
    // Parley omits normalized coordinates when every axis is at its default.
    assert!(face.coords.len() <= coords.len());
    let mut actual = face.coords.to_vec();
    actual.resize(coords.len(), 0);
    assert_eq!(actual, coords);
    assert!(!face.synthesis.embolden());
    assert_eq!(
        advances(p),
        advances(reference),
        "weight must be pinned before shaping"
    );
    assert_eq!(p.width, reference.width);
}

#[test]
fn declared_variable_400_stays_400_for_requests_400_and_600() {
    let mut installed = system(VARIABLE, "Inter Variable");
    let reference = shape(&mut installed, 0, 400);
    let (mut engine, family) = declared(VARIABLE, &[400]);
    // Request the non-default weight first to expose substitution by the request.
    for requested in [600, 400] {
        assert_pinned(&shape(&mut engine, family, requested), &reference, 400);
    }
}

#[test]
fn declared_variable_600_stays_600_for_requests_400_and_600() {
    let mut installed = system(VARIABLE, "Inter Variable");
    let reference = shape(&mut installed, 0, 600);
    let (mut engine, family) = declared(VARIABLE, &[600]);
    // An equal request still needs a coordinate when the file defaults to 400.
    for requested in [600, 400] {
        assert_pinned(&shape(&mut engine, family, requested), &reference, 600);
    }
}

#[test]
fn declared_variable_file_can_supply_two_weights_and_clamps_to_its_axis() {
    let mut installed = system(VARIABLE, "Inter Variable");
    let (mut engine, family) = declared_assets(
        &[400, 600],
        Assets::embedded(Path::new(env!("CARGO_MANIFEST_DIR")).join("../apple/tests")),
    );
    let mut previous_face = None;
    for requested in [400, 600] {
        let reference = shape(&mut installed, 0, requested);
        let p = shape(&mut engine, family, requested);
        let face = p.lines().faces[0].id();
        if let Some(previous) = previous_face.replace(face) {
            assert_eq!(
                face, previous,
                "both declarations share one file-backed blob"
            );
        }
        assert_pinned(&p, &reference, requested);
    }
    for (declared_weight, clamped) in [(1, 100), (1000, 900)] {
        let (mut engine, family) = declared(VARIABLE, &[declared_weight]);
        let reference = shape(&mut installed, 0, clamped);
        for requested in [declared_weight, 400, 600] {
            assert_pinned(&shape(&mut engine, family, requested), &reference, clamped);
        }
    }
}

#[test]
fn system_variable_and_declared_static_weights_keep_their_behavior() {
    let mut installed = system(VARIABLE, "Inter Variable");
    let regular = shape(&mut installed, 0, 400);
    let semibold = shape(&mut installed, 0, 600);
    assert!(
        (advances(&regular).iter().sum::<f32>() - 234.14).abs() < 0.01,
        "{}",
        regular.width
    );
    assert!(
        (advances(&semibold).iter().sum::<f32>() - 240.89).abs() < 0.01,
        "{}",
        semibold.width
    );
    assert_ne!(advances(&regular), advances(&semibold));
    assert!(regular.lines().faces[0]
        .synthesis
        .variation_settings()
        .is_empty());
    assert_pinned(&semibold, &semibold, 600);

    let mut installed = system(STATIC, "Inter");
    let (mut engine, family) = declared(STATIC, &[400]);
    for requested in [400, 600] {
        let reference = shape(&mut installed, 0, requested);
        let p = shape(&mut engine, family, requested);
        assert_eq!(p.lines().faces.len(), 1);
        let face = &p.lines().faces[0];
        assert_eq!(face.font.data.data(), STATIC);
        assert!(face.coords.is_empty());
        assert!(face.synthesis.variation_settings().is_empty());
        assert_eq!(face.synthesis, reference.lines().faces[0].synthesis);
        assert_eq!(advances(&p), advances(&reference));
    }
}

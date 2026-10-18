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

#[test]
fn reversed_weight_bounds_register_and_shape_without_a_pin() {
    let mut bytes = VARIABLE.to_vec();
    let table = (0..u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize)
        .map(|i| 12 + 16 * i)
        .find(|&i| &bytes[i..i + 4] == b"fvar")
        .unwrap();
    let fvar = u32::from_be_bytes(bytes[table + 8..table + 12].try_into().unwrap()) as usize;
    let start = fvar + u16::from_be_bytes(bytes[fvar + 4..fvar + 6].try_into().unwrap()) as usize;
    let count = u16::from_be_bytes(bytes[fvar + 8..fvar + 10].try_into().unwrap()) as usize;
    let size = u16::from_be_bytes(bytes[fvar + 10..fvar + 12].try_into().unwrap()) as usize;
    let axis = (0..count)
        .map(|i| start + i * size)
        .find(|&i| &bytes[i..i + 4] == b"wght")
        .unwrap();
    bytes[axis + 4..axis + 8].copy_from_slice(&(900i32 << 16).to_be_bytes());
    bytes[axis + 12..axis + 16].copy_from_slice(&(100i32 << 16).to_be_bytes());
    let bytes: Arc<[u8]> = bytes.into();
    let mut installed = system(&bytes, "Inter Variable");
    let asset = bytes.clone();
    let (mut engine, family) = declared_assets(
        &[400],
        Assets::selected(
            Path::new("").into(),
            Arc::new(move |name| Ok((name == "fonts/InterVariable.ttf").then(|| asset.clone()))),
        ),
    );
    for weight in [400, 600] {
        let reference = shape(&mut installed, 0, weight);
        let p = shape(&mut engine, family, weight);
        assert_eq!(p.lines().faces.len(), 1);
        let face = &p.lines().faces[0];
        assert_eq!(face.font.data.data(), &*bytes);
        assert_eq!(face.synthesis, reference.lines().faces[0].synthesis);
        assert_eq!(face.coords, reference.lines().faces[0].coords);
        assert_eq!(advances(&p), advances(&reference));
        assert!(p.width.is_finite() && p.width > 0.);
    }
}

#[test]
fn unordered_weight_bounds_leave_synthesis_unchanged() {
    let source = fontique::SourceInfo::new(
        fontique::SourceId::new(),
        fontique::SourceKind::Memory(fontique::Blob::new(Arc::new(VARIABLE))),
    );
    let parsed = fontique::FontInfo::from_source(source.clone(), 0).unwrap();
    // fvar stores fixed-point values; NaN can enter through from_parts metadata.
    let mut panicked = Vec::new();
    for (min, max) in [(900., 100.), (f32::NAN, 900.), (100., f32::NAN)] {
        let mut axes = parsed.axes().to_vec();
        let axis = axes
            .iter_mut()
            .find(|axis| axis.tag.to_be_bytes() == *b"wght")
            .unwrap();
        axis.min = min;
        axis.max = max;
        let original = fontique::FontInfo::from_parts(
            source.clone(),
            0,
            parsed.width(),
            parsed.style(),
            parsed.weight(),
            &axes,
            parsed.charmap_index(),
        );
        let pinned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            original.clone().with_pinned_weight()
        }));
        let Ok(pinned) = pinned else {
            panicked.push((min, max));
            continue;
        };
        for weight in [400., 600.] {
            let weight = fontique::FontWeight::new(weight);
            assert_eq!(
                pinned.synthesis(parsed.width(), parsed.style(), weight),
                original.synthesis(parsed.width(), parsed.style(), weight),
                "bounds {min}..{max}"
            );
        }
    }
    assert!(panicked.is_empty(), "pinning panicked for {panicked:?}");
}

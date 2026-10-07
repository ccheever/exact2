//! Actual thread/recipe/adoption tests, independent of the region controller.
use super::transfer::*;
use super::*;
use exact_kernel::*;
use std::thread;

// Test convenience only: production controller schedules preparation after its
// pending shell. The UI never joins in production.
fn freeze_catalog(engine: &TextEngine) -> Result<(FontRecipe, RasterCatalog), TransferError> {
    fn send<T: Send>() {}
    send::<PreparedCatalog>();
    let snapshot = snapshot_catalog(engine)?;
    let prepared = thread::spawn(move || prepare_catalog_generation(snapshot))
        .join()
        .unwrap()?;
    Ok(adopt_catalog_generation(prepared))
}
fn fixture_catalog() -> catalog::Catalog {
    catalog::Catalog::from_bytes(
        &[include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").as_slice()],
        "DejaVu Sans",
    )
}
fn request(catalog: u64, width: f32) -> (Kernel, RegionTextRequest) {
    let mut k = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let mut ops = Vec::new();
    for (id, node_type) in [
        (1, NodeType::View),
        (2, NodeType::View),
        (3, NodeType::View),
        (4, NodeType::Text),
        (5, NodeType::Text),
    ] {
        ops.push(Op::CreateView { id, node_type });
    }
    let mut style = StyleProps::default();
    for id in [
        StyleId::Width,
        StyleId::Height,
        StyleId::OverflowX,
        StyleId::OverflowY,
        StyleId::PositionType,
    ] {
        style.mask.set(id);
    }
    // A region's owner is positioned: it contains its placeholder (LLP 1074 T1).
    style.position_type = PositionType::Relative;
    style.width = Dimension::Percent(100.);
    style.height = Dimension::Points(180.);
    style.overflow_x = Overflow::Hidden;
    style.overflow_y = Overflow::Hidden;
    ops.extend([
        Op::SetStyle {
            id: 2,
            patch: Box::new(style),
        },
        Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "office ffi e\u{301} العربية שלום words words words words".into(),
        },
        Op::SetProp {
            id: 5,
            prop: PropId::Text,
            value: "Loading".into(),
        },
        Op::SetChildren {
            id: 3,
            children: vec![4],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 5],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ]);
    k.apply(0, 0, &ops).unwrap();
    k.set_content_region(Some(ContentRegion {
        owner: k.node(2).unwrap().key,
        content: k.node(3).unwrap().key,
        pending: k.node(5).unwrap().key,
    }))
    .unwrap();
    k.compute_region_layout(
        1,
        Offer::definite(width, 300.),
        RegionInputs {
            catalog,
            consumer_revision: 1,
        },
    )
    .unwrap();
    let q = k.region_text_request().unwrap().clone();
    (k, q)
}
/// Every line and glyph, faces by identity: a worker shapes with the very
/// faces the UI does (shared blobs), so nothing is renumbered.
fn glyphs(p: &Paragraph) -> Vec<String> {
    p.layout_runs()
        .map(|r| {
            let mut s = format!(
                "{}:{}:{}:{:?}",
                r.line_i,
                r.text,
                r.rtl,
                [r.line_y, r.line_top, r.line_height, r.line_w].map(f32::to_bits),
            );
            for g in r.glyphs {
                let face = &p.lines().faces[g.face as usize];
                s.push_str(&format!(
                    "{:?}:{:?}",
                    (
                        g.start,
                        g.end,
                        face.id(),
                        face.weight,
                        &face.coords,
                        face.skew,
                        g.glyph_id,
                        g.level,
                        g.metadata,
                    ),
                    [g.font_size, g.x, g.y, g.w].map(f32::to_bits),
                ));
            }
            s
        })
        .collect()
}
fn pixels(engine: &mut TextEngine, p: &Paragraph) -> Vec<u8> {
    let mut image = Pixmap::new(400, 400).unwrap();
    engine.paint(
        &mut image,
        p,
        &[RunPaint {
            color: [25, 60, 170, 255],
            source: 7,
        }],
        (0., 0.),
        1.,
        Transform::identity(),
        None,
    );
    assert!(image.data().chunks_exact(4).any(|p| p[3] != 0));
    image.data().to_vec()
}
#[test]
fn real_thread_constructs_fonts_and_returns_exact_baselines_glyphs_and_rgba() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<FontRecipe>();
    send_sync::<PreparedText>();
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 250.);
    let spec = q.with_request(Spec::from_request);
    let expected = engine.paragraph(
        &spec,
        match q.offer().width {
            AxisOffer::Definite(w) => Some(w),
            _ => None,
        },
    );
    let before = shaping::shape_line_calls();
    let freeze_work = work::read();
    assert_eq!(freeze_work.catalog_builds, 0);
    let input = prepare(&recipe, q.clone(), PaintContext::new(1.).unwrap(), None).unwrap();
    let converted = work::read();
    assert!(converted.source_bytes > 0);
    let again = prepare(
        &recipe,
        q,
        PaintContext::new(1.).unwrap(),
        Some(input.source()),
    )
    .unwrap();
    assert_eq!(work::read(), converted);
    assert!(Arc::ptr_eq(&input.source().0, &again.source().0));
    assert_eq!(before, shaping::shape_line_calls());
    let ui = thread::current().id();
    let job = input.clone();
    let (output, worker_id, calls) = thread::spawn(move || {
        let mut worker = FontWorker::new(recipe).unwrap();
        let output = worker.execute(job).unwrap();
        (output, thread::current().id(), shaping::shape_line_calls())
    })
    .join()
    .unwrap();
    assert_ne!(ui, worker_id);
    assert!(calls > 0);
    let adopted = adopt(output, &input, &raster).unwrap();
    assert_eq!(shaping::shape_line_calls(), before);
    assert_eq!(glyphs(adopted.paragraph().unwrap()), glyphs(&expected));
    assert_eq!(
        adopted
            .paragraph()
            .unwrap()
            .baselines
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        expected
            .baselines
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(adopted.metrics(), paragraph_metrics(&expected));
    assert_eq!(
        work::read(),
        converted,
        "adopt performs no font construction, source conversion, shape or layout"
    );
    assert_eq!(
        pixels(&mut engine, adopted.paragraph().unwrap()),
        pixels(&mut engine, &expected)
    );
    eprintln!(
        "Background generation prepared fonts; UI work after prepare/adopt={:?}",
        work::read()
    );
}

#[test]
fn intrinsic_probe_arrays_die_and_final_paint_keeps_definite_layout() {
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (mut k, q) = request(recipe.catalog_label(), 160.);
    let mut flex = StyleProps::default();
    flex.mask.set(StyleId::Display);
    flex.display = Display::Flex;
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 3,
            patch: Box::new(flex),
        }],
    )
    .unwrap();
    k.compute_region_layout(
        1,
        Offer::definite(160., 300.),
        RegionInputs {
            catalog: recipe.catalog_label(),
            consumer_revision: 1,
        },
    )
    .unwrap();
    let mut source = None;
    let mut ready: Vec<Rc<AdoptedText>> = Vec::new();
    let mut intrinsic = 0;
    let mut definite = 0;
    // Actual kernel offer discovery, not handcrafted request ordinals.
    for _ in 0..64 {
        let Some(q) = k.region_text_request().cloned() else {
            break;
        };
        let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), source.as_ref()).unwrap();
        source = Some(input.source().clone());
        let r = recipe.clone();
        let job = input.clone();
        let (output, ink_work) = thread::spawn(move || {
            let output = FontWorker::new(r).unwrap().execute(job).unwrap();
            (output, ink::build_work())
        })
        .join()
        .unwrap();
        let is_definite = matches!(output.request().offer().width, AxisOffer::Definite(_));
        if is_definite {
            definite += 1;
            assert!(output.layout_capacity_bytes() > 0);
            assert!(output.ink_capacity_bytes() > 0);
            let reused = ready.iter().any(|a| {
                a.paragraph().is_some_and(|p| {
                    std::sync::Weak::ptr_eq(&output.probe, &Arc::downgrade(&p.layout_lifetime))
                })
            });
            assert_eq!(ink_work.attempts, usize::from(!reused));
            if reused {
                assert_eq!(ink_work, ink::BuildWork::default());
            }
            assert!(output.ink_probe.upgrade().is_some());
            assert!(output.probe.upgrade().is_some());
        } else {
            intrinsic += 1;
            assert_eq!(output.layout_capacity_bytes(), 0);
            assert_eq!(output.ink_capacity_bytes(), 0);
            assert_eq!(ink_work, ink::BuildWork::default());
            assert!(output.ink_probe.upgrade().is_none());
            assert!(output.probe.upgrade().is_none());
        }
        let adopted = Rc::new(adopt(output, &input, &raster).unwrap());
        assert_eq!(adopted.paragraph().is_some(), is_definite);
        assert!(adopted.source.0.shape.get().is_some());
        assert!(k
            .resolve_region_text(adopted.request(), adopted.metrics(), adopted.clone())
            .unwrap());
        ready.push(adopted);
        let receipt = k
            .compute_region_layout(
                1,
                Offer::definite(160., 300.),
                RegionInputs {
                    catalog: recipe.catalog_label(),
                    consumer_revision: 1,
                },
            )
            .unwrap();
        if receipt.current {
            break;
        }
    }
    assert!(intrinsic > 0, "kernel must exercise a real intrinsic probe");
    assert!(definite > 0, "paint requires final definite artifact");
    assert!(ready
        .windows(2)
        .all(|v| Arc::ptr_eq(&v[0].source.0, &v[1].source.0)));
    assert_eq!(q.catalog(), recipe.catalog_label());
}

#[test]
fn stale_completion_or_foreign_raster_cannot_adopt() {
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 200.);
    let first = prepare(&recipe, q.clone(), PaintContext::new(1.).unwrap(), None).unwrap();
    let second = prepare(
        &recipe,
        q,
        PaintContext::new(1.).unwrap(),
        Some(first.source()),
    )
    .unwrap();
    let mut worker = FontWorker::new(recipe).unwrap();
    let output = worker.execute(first).unwrap();
    assert!(matches!(
        adopt(output, &second, &raster),
        Err(TransferError::StaleResult)
    ));
    let (_other_recipe, other_raster) = freeze_catalog(&engine).unwrap();
    let output = worker.execute(second.clone()).unwrap();
    assert!(matches!(
        adopt(output, &second, &other_raster),
        Err(TransferError::CatalogMismatch)
    ));
}

#[test]
fn staged_binary_recipe_preserves_a_declared_face() {
    let mut catalog = fixture_catalog();
    // A plan's declared face from bytes, under its stack's alias.
    let bytes = include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans-Bold.ttf");
    let blob = fontique::Blob::new(Arc::new(bytes.to_vec()));
    let source = fontique::SourceInfo::new(
        fontique::SourceId::new(),
        fontique::SourceKind::Memory(blob.clone()),
    );
    let parsed = fontique::FontInfo::from_source(source.clone(), 0).unwrap();
    let info = fontique::FontInfo::from_parts(
        source,
        0,
        fontique::FontWidth::NORMAL,
        fontique::FontStyle::Normal,
        fontique::FontWeight::new(400.0),
        parsed.axes(),
        parsed.charmap_index(),
    );
    catalog
        .fonts
        .collection
        .register_described("ExactPlanStack8", vec![info]);
    let id = FaceId {
        blob: blob.id(),
        index: 0,
    };
    catalog
        .families
        .push(FamilyChoice::Declared("ExactPlanStack8".into()));
    catalog.declared_faces.insert((8, 400, false), id);
    let recipe = super::catalog_recipe::CatalogSnapshot::capture(&catalog)
        .unwrap()
        .prepare()
        .unwrap();
    let mut spec = crate::paint::text_spec(&StyleProps::default(), "Declared ffi 123");
    spec.strut.family = 8;
    for run in &mut spec.runs {
        run.family = 8;
    }
    let mut original = TextEngine::with_catalog(catalog);
    assert_eq!(original.resolved_face_id(8, 400, false), Some(id));
    let original_paragraph = original.paragraph(&spec, Some(210.));
    let original_glyphs = glyphs(&original_paragraph);
    let original_metrics = paragraph_metrics(&original_paragraph);
    let original_pixels = pixels(&mut original, &original_paragraph);
    let (declared, native_glyphs, native_metrics, native_pixels) = thread::spawn(move || {
        let local = recipe.catalog();
        let declared = local.declared_face_id(8, 400, false);
        let mut engine = TextEngine::with_catalog(local);
        let p = engine.paragraph(&spec, Some(210.));
        (
            declared,
            glyphs(&p),
            paragraph_metrics(&p),
            pixels(&mut engine, &p),
        )
    })
    .join()
    .unwrap();
    assert_eq!(declared, Some(id));
    assert_eq!(native_glyphs, original_glyphs);
    assert_eq!(native_metrics, original_metrics);
    assert_eq!(native_pixels, original_pixels);
}

#[test]
fn already_used_file_face_can_freeze_before_atomic_path_replacement() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory =
        Directory(std::env::temp_dir().join(format!("exact-transfer-font-{}", std::process::id())));
    std::fs::create_dir(&directory.0).expect("unique owned fixture directory");
    let path = directory.0.join("face.ttf");
    std::fs::write(
        &path,
        include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf"),
    )
    .unwrap();
    let mut engine = file_engine(&path);
    let spec = crate::paint::text_spec(&StyleProps::default(), "Already cached ffi Arabic العربية");
    let old = engine.paragraph(&spec, Some(200.));
    let expected = pixels(&mut engine, &old);
    // Current production read path has already loaded and rasterized this face.
    // Capture MUST succeed before this path is replaced, with no future reread.
    let (recipe, _raster) = freeze_catalog(&engine)
        .expect("file-backed catalog capture required, not a permanent refusal");
    let replacement = directory.0.join("replacement.ttf");
    std::fs::write(
        &replacement,
        include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans-Bold.ttf"),
    )
    .unwrap();
    std::fs::rename(replacement, &path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let captured = recipe.0.clone();
    let actual = thread::spawn(move || {
        let mut worker = TextEngine::with_catalog(captured.catalog());
        let paragraph = worker.paragraph(&spec, Some(200.));
        pixels(&mut worker, &paragraph)
    })
    .join()
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(pixels(&mut engine, &old), expected);
}

fn next_request(k: &mut Kernel, catalog: u64, width: f32) -> RegionTextRequest {
    k.compute_region_layout(
        1,
        Offer::definite(width, 300.),
        RegionInputs {
            catalog,
            consumer_revision: 1,
        },
    )
    .unwrap();
    k.region_text_request()
        .expect("new exact offer must be requested")
        .clone()
}

#[test]
fn two_widths_share_shape_and_release_source_after_last_owner() {
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (mut k, q) = request(recipe.catalog_label(), 120.);
    let a_input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let source_weak = Arc::downgrade(&a_input.source().0);
    let r = recipe.clone();
    let job = a_input.clone();
    let (a_output, a_counts) = thread::spawn(move || {
        let mut worker = FontWorker::new(r).unwrap();
        let output = worker.execute(job).unwrap();
        (output, work::read())
    })
    .join()
    .unwrap();
    assert_eq!(a_counts.shapes, 1);
    let a_layout = a_output.probe.clone();
    let a = adopt(a_output, &a_input, &raster).unwrap();
    let a_signature = glyphs(a.paragraph().unwrap());
    let shape_weak = Arc::downgrade(&a.paragraph().unwrap().source.data);
    let q = next_request(&mut k, recipe.catalog_label(), 600.);
    let before = work::read();
    let b_input = prepare(
        &recipe,
        q,
        PaintContext::new(1.).unwrap(),
        Some(a_input.source()),
    )
    .unwrap();
    assert_eq!(
        work::read(),
        before,
        "second offer must not reconvert source"
    );
    let r = recipe.clone();
    let job = b_input.clone();
    let (b_output, b_counts) = thread::spawn(move || {
        let mut worker = FontWorker::new(r).unwrap();
        let output = worker.execute(job).unwrap();
        (output, work::read())
    })
    .join()
    .unwrap();
    assert_eq!(
        b_counts.shapes, 0,
        "existing immutable shape must be borrowed"
    );
    assert!(
        b_counts.layouts > 0,
        "a different width still performs real layout"
    );
    let b_layout = b_output.probe.clone();
    let b = adopt(b_output, &b_input, &raster).unwrap();
    assert!(Arc::ptr_eq(
        &a.paragraph().unwrap().source.data,
        &b.paragraph().unwrap().source.data
    ));
    assert!(!Rc::ptr_eq(a.paragraph().unwrap(), b.paragraph().unwrap()));
    assert!(
        a.metrics().height > b.metrics().height,
        "fixture must really rewrap"
    );
    assert_eq!(
        glyphs(a.paragraph().unwrap()),
        a_signature,
        "B never mutates accepted A"
    );
    drop(a_input);
    drop(b_input);
    drop(k);
    drop(a);
    assert!(a_layout.upgrade().is_none());
    assert!(b_layout.upgrade().is_some());
    assert!(shape_weak.upgrade().is_some());
    drop(b);
    assert!(b_layout.upgrade().is_none());
    assert!(shape_weak.upgrade().is_none());
    assert!(
        source_weak.upgrade().is_none(),
        "worker retains no source history after completion"
    );
    // Recipe/raster remain live: neither may retain a source or a width.
    assert!(recipe.catalog_label() > 0);
    drop(raster);
}

#[test]
fn rejected_result_releases_only_its_owner_without_changing_accepted_sibling() {
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 200.);
    let input = prepare(&recipe, q.clone(), PaintContext::new(1.).unwrap(), None).unwrap();
    let mut worker = FontWorker::new(recipe.clone()).unwrap();
    let accepted = adopt(worker.execute(input.clone()).unwrap(), &input, &raster).unwrap();
    let signature = glyphs(accepted.paragraph().unwrap());
    let accepted_life = Arc::downgrade(&accepted.paragraph().unwrap().layout_lifetime);
    let owners = accepted_life.strong_count();
    let stale = worker.execute(input.clone()).unwrap();
    let stale_layout = stale.probe.clone();
    let new_expected = prepare(
        &recipe,
        q,
        PaintContext::new(1.).unwrap(),
        Some(input.source()),
    )
    .unwrap();
    let before = work::read();
    assert!(matches!(
        adopt(stale, &new_expected, &raster),
        Err(TransferError::StaleResult)
    ));
    assert_eq!(before, work::read(), "refusal happens before UI work");
    assert!(std::sync::Weak::ptr_eq(&stale_layout, &accepted_life));
    assert_eq!(
        stale_layout.strong_count(),
        owners,
        "rejected sibling retained no owner"
    );
    assert_eq!(glyphs(accepted.paragraph().unwrap()), signature);
    drop(accepted);
    assert!(
        stale_layout.upgrade().is_none(),
        "weak slot kept rejected storage alive"
    );
}

#[test]
fn old_raster_catalog_remains_correct_after_catalog_replacement() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 200.);
    let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let r = recipe.clone();
    let job = input.clone();
    let output = thread::spawn(move || FontWorker::new(r).unwrap().execute(job).unwrap())
        .join()
        .unwrap();
    let accepted = adopt(output, &input, &raster).unwrap();
    let original = pixels(&mut engine, accepted.paragraph().unwrap());
    let old_catalog = Rc::downgrade(&accepted.paragraph().unwrap().source.catalog);
    // A different font under the same family name: a new catalog whose
    // first raster slot names another face.
    engine = TextEngine::with_catalog(catalog::Catalog::from_bytes(
        &[
            include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans-Bold.ttf")
                .as_slice(),
        ],
        "DejaVu Sans",
    ));
    let (new_recipe, new_raster) = freeze_catalog(&engine).unwrap();
    assert_ne!(new_recipe.catalog_label(), recipe.catalog_label());
    assert_eq!(pixels(&mut engine, accepted.paragraph().unwrap()), original);
    drop(raster);
    drop(input);
    drop(recipe);
    assert!(
        old_catalog.upgrade().is_some(),
        "accepted artifact retains old raster lease"
    );
    drop(accepted);
    assert!(
        old_catalog.upgrade().is_none(),
        "new catalog must not retain old catalog"
    );
    drop(new_raster);
}

#[test]
fn height_only_offer_change_rejects_old_completion_before_adoption() {
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (mut k, _) = request(recipe.catalog_label(), 200.);
    let mut flex = StyleProps::default();
    flex.mask.set(StyleId::Display);
    flex.display = Display::Flex;
    let mut text = StyleProps::default();
    text.mask.set(StyleId::Height);
    text.height = Dimension::Points(55.);
    k.apply(
        0,
        0,
        &[
            Op::SetStyle {
                id: 3,
                patch: Box::new(flex),
            },
            Op::SetStyle {
                id: 4,
                patch: Box::new(text),
            },
        ],
    )
    .unwrap();
    let first = next_request(&mut k, recipe.catalog_label(), 200.);
    assert_eq!(first.offer().height, AxisOffer::Definite(55.));
    let input = prepare(&recipe, first.clone(), PaintContext::new(1.).unwrap(), None).unwrap();
    let r = recipe.clone();
    let job = input.clone();
    let output = thread::spawn(move || FontWorker::new(r).unwrap().execute(job).unwrap())
        .join()
        .unwrap();
    let mut text = StyleProps::default();
    text.mask.set(StyleId::Height);
    text.height = Dimension::Points(77.);
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id: 4,
            patch: Box::new(text),
        }],
    )
    .unwrap();
    let next = next_request(&mut k, recipe.catalog_label(), 200.);
    assert_eq!(next.offer().width, first.offer().width);
    assert_eq!(next.offer().height, AxisOffer::Definite(77.));
    assert!(next.stamp().same_metrics(first.stamp()));
    let expected = prepare(
        &recipe,
        next,
        PaintContext::new(1.).unwrap(),
        Some(input.source()),
    )
    .unwrap();
    let before = work::read();
    assert!(matches!(
        adopt(output, &expected, &raster),
        Err(TransferError::StaleResult)
    ));
    assert_eq!(before, work::read());
}

struct FontDirectory(std::path::PathBuf);
impl FontDirectory {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let p = std::env::temp_dir().join(format!(
            "exact-transfer-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn regular(&self) -> std::path::PathBuf {
        let p = self.0.join("regular.ttf");
        std::fs::write(
            &p,
            include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf"),
        )
        .unwrap();
        p
    }
}
impl Drop for FontDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn file_engine(path: &Path) -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::from_files(&[path], "DejaVu Sans"))
}

/// Preparing a generation reads, copies and renumbers no font: the worker's
/// catalog shares the UI's faces (LLP 1085.000 §4; the storage spike's Q2).
#[test]
fn capture_reads_and_copies_no_font_bytes() {
    let dir = FontDirectory::new();
    let path = dir.regular();
    let linked = dir.0.join("linked.ttf");
    std::fs::hard_link(&path, &linked).unwrap();
    let engine = TextEngine::with_catalog(catalog::Catalog::from_files(
        &[path.as_path(), linked.as_path()],
        "DejaVu Sans",
    ));
    let (recipe, _raster) = freeze_catalog(&engine).unwrap();
    let cost = recipe.capture_cost();
    assert_eq!(cost.faces, 2);
    assert_eq!(cost.sources, 2);
    assert_eq!(
        (cost.file_reads, cost.file_bytes, cost.binary_bytes_copied),
        (0, 0, 0)
    );
}

/// A worker's glyphs name the same faces as the UI's: the same blobs.
#[test]
fn worker_and_ui_share_face_blobs_geometry_glyphs_and_pixels() {
    let dir = FontDirectory::new();
    let path = dir.regular();
    let mut engine = file_engine(&path);
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 200.);
    let spec = q.with_request(Spec::from_request);
    let old = engine.paragraph(&spec, Some(200.));
    let old_pixels = pixels(&mut engine, &old);
    let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let r = recipe.clone();
    let job = input.clone();
    let result = thread::spawn(move || FontWorker::new(r).unwrap().execute(job).unwrap())
        .join()
        .unwrap();
    let adopted = adopt(result, &input, &raster).unwrap();
    let p = adopted.paragraph().unwrap();
    assert_eq!(glyphs(p), glyphs(&old));
    let faces = |p: &Paragraph| -> Vec<_> { p.lines().faces.iter().map(|f| f.id()).collect() };
    assert_eq!(faces(p), faces(&old));
    assert_eq!(adopted.metrics(), paragraph_metrics(&old));
    assert_eq!(pixels(&mut engine, p), old_pixels);
}

/// A file replaced after its face was first used changes no catalog that
/// shares the face: its mapping is of the old file, for UI and worker alike.
#[test]
fn replacement_after_first_use_keeps_every_sharing_catalog_consistent() {
    let dir = FontDirectory::new();
    let path = dir.regular();
    let mut engine = file_engine(&path);
    let spec = crate::paint::text_spec(&StyleProps::default(), "Already cached office ffi words");
    let old = engine.paragraph(&spec, Some(220.));
    let old_pixels = pixels(&mut engine, &old);
    let replacement = dir.0.join("replacement.ttf");
    std::fs::write(
        &replacement,
        include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans-Bold.ttf"),
    )
    .unwrap();
    std::fs::rename(replacement, &path).unwrap();
    let (recipe, _raster) = freeze_catalog(&engine).unwrap();
    let r = recipe.0.clone();
    let worker_pixels = thread::spawn(move || {
        let mut worker = TextEngine::with_catalog(r.catalog());
        let p = worker.paragraph(&spec, Some(220.));
        pixels(&mut worker, &p)
    })
    .join()
    .unwrap();
    assert_eq!(worker_pixels, old_pixels);
    assert_eq!(pixels(&mut engine, &old), old_pixels);
    // A catalog made afresh reads the file as it is now.
    let mut fresh = file_engine(&path);
    let p = fresh.paragraph(
        &crate::paint::text_spec(&StyleProps::default(), "Already cached office ffi words"),
        Some(220.),
    );
    assert_ne!(pixels(&mut fresh, &p), old_pixels);
}

/// Capture never opens a file, so a face's file removed or replaced before
/// capture cannot fail it; a face first used after its file became a FIFO
/// maps as nothing, without waiting for a writer, and text falls back.
#[test]
#[cfg(unix)]
fn missing_or_fifo_font_files_neither_fail_capture_nor_block() {
    let dir = FontDirectory::new();
    let path = dir.regular();
    let engine = file_engine(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .unwrap()
        .success());
    let snapshot = snapshot_catalog(&engine).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let prepared = prepare_catalog_generation(snapshot).map(|p| p.recipe().clone());
        let shaped = prepared.as_ref().ok().map(|recipe| {
            let mut engine = TextEngine::with_catalog(recipe.0.catalog());
            let spec = crate::paint::text_spec(&StyleProps::default(), "never mapped");
            engine.measure(&spec, AxisOffer::MaxContent)
        });
        tx.send((prepared.is_ok(), shaped)).unwrap();
    });
    // A blocked-I/O discriminator, not a latency benchmark.
    let answer = rx.recv_timeout(std::time::Duration::from_secs(5));
    if answer.is_err() {
        use std::os::unix::fs::OpenOptionsExt;
        let writer = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path);
        drop(writer);
    }
    worker.join().unwrap();
    let (prepared, shaped) = answer.expect("capture or first use waited on a FIFO");
    assert!(prepared);
    assert!(shaped.is_some());
}

#[test]
fn one_worker_constructs_both_font_owners_and_moves_raster_without_ui_rebuild() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let ui_before = work::read();
    let snapshot = snapshot_catalog(&engine).unwrap();
    assert_eq!(work::read(), ui_before);
    let (catalog_tx, catalog_rx) = std::sync::mpsc::channel();
    let (input_tx, input_rx) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let prepared = prepare_catalog_generation(snapshot).unwrap();
        let mut worker = FontWorker::new(prepared.recipe().clone()).unwrap();
        let init = work::read();
        assert_eq!(init.catalog_builds, 2);
        assert_eq!((init.source_bytes, init.shapes, init.layouts), (0, 0, 0));
        catalog_tx.send((prepared, init)).unwrap();
        let output = worker.execute(input_rx.recv().unwrap()).unwrap();
        (output, work::read())
    });
    let (prepared, init) = catalog_rx.recv().unwrap();
    let (recipe, raster) = adopt_catalog_generation(prepared);
    assert_eq!(work::read(), ui_before);
    let (_k, q) = request(recipe.catalog_label(), 180.);
    let spec = q.with_request(Spec::from_request);
    let expected = engine.paragraph(&spec, Some(180.));
    let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let pre_adopt = work::read();
    input_tx.send(input.clone()).unwrap();
    let (output, finished) = worker.join().unwrap();
    assert_eq!(finished.catalog_builds, init.catalog_builds);
    assert_eq!(finished.source_bytes, 0);
    assert_eq!(finished.shapes, 1);
    assert!(finished.layouts > 0);
    let adopted = adopt(output, &input, &raster).unwrap();
    assert_eq!(work::read(), pre_adopt);
    assert_eq!(glyphs(adopted.paragraph().unwrap()), glyphs(&expected));
    assert_eq!(adopted.metrics(), paragraph_metrics(&expected));
    assert_eq!(
        pixels(&mut engine, adopted.paragraph().unwrap()),
        pixels(&mut engine, &expected)
    );
    eprintln!(
        "single-worker initialization={init:?}; execution={finished:?}; capture={:?}",
        recipe.capture_cost()
    );
}

#[test]
fn current_system_catalog_generation_preserves_all_glyphs_and_rgba() {
    let mut engine = TextEngine::new();
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 250.);
    let spec = q.with_request(Spec::from_request);
    let expected = engine.paragraph(&spec, Some(250.));
    let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let job = input.clone();
    let worker_recipe = recipe.clone();
    let output = thread::spawn(move || {
        FontWorker::new(worker_recipe)
            .unwrap()
            .execute(job)
            .unwrap()
    })
    .join()
    .unwrap();
    let actual = adopt(output, &input, &raster).unwrap();
    assert_eq!(glyphs(actual.paragraph().unwrap()), glyphs(&expected));
    assert_eq!(actual.metrics(), paragraph_metrics(&expected));
    assert_eq!(actual.paragraph().unwrap().baselines, expected.baselines);
    assert_eq!(
        pixels(&mut engine, actual.paragraph().unwrap()),
        pixels(&mut engine, &expected)
    );
    eprintln!(
        "current system capture (not a timing): {:?}",
        recipe.capture_cost()
    );
}

mod prepared_ink;

mod font_admission;

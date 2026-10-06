//! Worker preparation must remove the first-paint full index scan.
use super::*;

fn large_request(label: u64) -> (Kernel, RegionTextRequest) {
    let (mut k, _) = request(label, 220.);
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "office ffi العربية words 12345 ".repeat(2048).into(),
        }],
    )
    .unwrap();
    let q = next_request(&mut k, label, 220.);
    (k, q)
}
fn palette() -> [RunPaint; 1] {
    [RunPaint {
        color: [25, 60, 170, 255],
        source: 7,
    }]
}
fn full(p: &Paragraph, y: f32, scale: f32) -> Vec<u8> {
    let mut target = Pixmap::new(320, 128).unwrap();
    let mut catalog = p.source.catalog.borrow_mut();
    let slots = ink::slots(&mut catalog, p.lines());
    for (g, baseline, ink) in p.paint_glyphs(&palette()) {
        let (key, x, y) = ink::physical(
            g,
            slots[g.face as usize],
            (0., (y + baseline) * scale),
            scale,
        );
        let Some(glyph) = catalog.glyph(key, ink.color) else {
            continue;
        };
        target.draw_pixmap(
            x + glyph.left,
            y - glyph.top,
            glyph.pixmap.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    target.data().to_vec()
}
fn clipped(engine: &mut TextEngine, p: &Paragraph, y: f32, scale: f32) -> Vec<u8> {
    let mut target = Pixmap::new(320, 128).unwrap();
    engine.paint_clipped(
        &mut target,
        p,
        &palette(),
        (0., y),
        scale,
        Transform::from_scale(scale, scale),
        None,
        (0., 0., 320., 128.),
    );
    target.data().to_vec()
}
#[test]
fn first_paint_of_transferred_paragraph_does_not_build_ink_on_ui() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = large_request(recipe.catalog_label());
    let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let job = input.clone();
    let (output, worker_work) = thread::spawn(move || {
        let output = FontWorker::new(recipe).unwrap().execute(job).unwrap();
        (output, ink::build_work())
    })
    .join()
    .unwrap();
    assert_eq!(worker_work.attempts, 1);
    assert!(worker_work.glyphs > 20_000);
    assert!(worker_work.raster_phases > 0);
    let ui_work = ink::build_work();
    let bytes = output.ink_capacity_bytes();
    let owned = output.source().shape_capacity_bytes() + output.layout_capacity_bytes() + bytes;
    assert!(bytes > 0 && bytes <= ink::MAX_BYTES);
    let adopted = adopt(output, &input, &raster).unwrap();
    let p = adopted.paragraph().unwrap();
    assert_eq!(p.ink_capacity_bytes(), bytes);
    assert_eq!(p.owned_capacity_bytes(), owned);
    assert_eq!(adopted.paint_context(), PaintContext::new(1.).unwrap());
    let all = p.paint_glyphs(&palette()).count();
    assert_eq!(worker_work.glyphs, all);
    let builds = engine.ink_builds;
    let mut visible_calls = Vec::new();
    for y in [0., -p.height / 2., -p.height + 100.] {
        let before_visits = engine.ink_visits;
        let actual = clipped(&mut engine, p, y, 1.);
        assert_eq!(actual, full(p, y, 1.));
        assert!(actual.chunks_exact(4).any(|px| px[3] != 0));
        let calls = engine.ink_visits - before_visits;
        assert!(calls < all / 10);
        visible_calls.push(calls);
    }
    assert_eq!(ink::build_work(), ui_work);
    eprintln!("worker index work={worker_work:?}; UI visible glyph-cache calls={visible_calls:?}, UI builds={}; index bytes={bytes}", engine.ink_builds - builds);
    assert_eq!(
        engine.ink_builds, builds,
        "first UI paint scanned the full paragraph to build ink"
    );
}

#[test]
fn paint_context_rejects_invalid_scale_and_keeps_exact_bits() {
    let before = work::read();
    for scale in [0., -0., -1., f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            PaintContext::new(scale),
            Err(TransferError::InvalidPaintContext)
        );
    }
    for scale in [f32::MIN_POSITIVE, 1., 1.25, 2., f32::MAX] {
        assert_eq!(
            PaintContext::new(scale).unwrap().scale().to_bits(),
            scale.to_bits()
        );
    }
    assert_ne!(
        PaintContext::new(1.).unwrap(),
        PaintContext::new(f32::from_bits(1f32.to_bits() + 1)).unwrap()
    );
    assert_eq!(work::read(), before);
}

#[test]
fn scale_change_refuses_old_job_and_each_exact_scale_keeps_pixels_without_ui_build() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 220.);
    let a = prepare(&recipe, q.clone(), PaintContext::new(1.25).unwrap(), None).unwrap();
    let b = prepare(&recipe, q, PaintContext::new(2.).unwrap(), Some(a.source())).unwrap();
    let (ja, jb) = (a.clone(), b.clone());
    let (oa, late, ob) = thread::spawn(move || {
        let mut worker = FontWorker::new(recipe).unwrap();
        (
            worker.execute(ja.clone()).unwrap(),
            worker.execute(ja).unwrap(),
            worker.execute(jb).unwrap(),
        )
    })
    .join()
    .unwrap();
    let late_index = late.ink_probe.clone();
    let a_index = oa.ink_probe.clone();
    let b_index = ob.ink_probe.clone();
    let stale = ink::build_work();
    assert!(std::sync::Weak::ptr_eq(&late_index, &a_index));
    let owners = a_index.strong_count();
    assert!(matches!(
        adopt(late, &b, &raster),
        Err(TransferError::StaleResult)
    ));
    assert!(
        late_index.upgrade().is_some(),
        "valid sibling still owns shared index"
    );
    assert_eq!(a_index.strong_count(), owners);
    assert_eq!(ink::build_work(), stale);
    let pa = adopt(oa, &a, &raster).unwrap();
    let pb = adopt(ob, &b, &raster).unwrap();
    assert_eq!(pa.paint_context(), a.paint_context());
    assert_eq!(pb.paint_context(), b.paint_context());
    for (p, scale) in [
        (pa.paragraph().unwrap(), 1.25),
        (pb.paragraph().unwrap(), 2.),
    ] {
        let actual = clipped(&mut engine, p, 0., scale);
        assert_eq!(actual, full(p, 0., scale));
        assert!(p
            .ink
            .borrow()
            .matches(&p.source.catalog.borrow().ink_catalog, scale));
    }
    assert_eq!(engine.ink_builds, 0);
    assert_eq!(ink::build_work(), stale);
    drop(pa);
    assert!(a_index.upgrade().is_none());
    assert!(b_index.upgrade().is_some());
    drop(pb);
    assert!(b_index.upgrade().is_none());
}

#[test]
fn refused_worker_index_returns_no_completion_and_drops_layout() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 220.);
    let input = prepare(&recipe, q.clone(), PaintContext::new(1.).unwrap(), None).unwrap();
    let next = prepare(
        &recipe,
        q,
        PaintContext::new(2.).unwrap(),
        Some(input.source()),
    )
    .unwrap();
    let (j, n) = (input.clone(), next.clone());
    let (accepted, error, dropped) = thread::spawn(move || {
        let mut worker = FontWorker::new(recipe).unwrap();
        let accepted = worker.execute(j).unwrap();
        worker.index_limit = 0; // Exercises the same bounded Index refusal, cheaply.
        let before = ink::build_work();
        let error = worker.execute(n).err();
        let after = ink::build_work();
        assert_eq!(after.glyphs, before.glyphs);
        assert_eq!(after.raster_phases, before.raster_phases);
        (accepted, error, worker.last_layout.upgrade().is_none())
    })
    .join()
    .unwrap();
    assert_eq!(error, Some(TransferError::InkIndexRefused));
    assert!(dropped);
    let p = adopt(accepted, &input, &raster).unwrap();
    assert!(p.paragraph().unwrap().ink_capacity_bytes() > 0);
    assert_eq!(
        clipped(&mut engine, p.paragraph().unwrap(), 0., 1.),
        full(p.paragraph().unwrap(), 0., 1.)
    );
    assert_eq!(engine.ink_builds, 0);
    assert!(input.source().0.shape.get().is_some());
}

#[test]
fn index_arrays_are_send_and_foreign_catalog_refusal_releases_them() {
    fn send<T: Send>() {}
    send::<ink::Index>();
    send::<CompletedText>();
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, _raster) = freeze_catalog(&engine).unwrap();
    let (_other, foreign) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 220.);
    let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let job = input.clone();
    let output = thread::spawn(move || FontWorker::new(recipe).unwrap().execute(job).unwrap())
        .join()
        .unwrap();
    let weak = output.ink_probe.clone();
    assert!(weak.upgrade().is_some());
    assert!(matches!(
        adopt(output, &input, &foreign),
        Err(TransferError::CatalogMismatch)
    ));
    assert!(weak.upgrade().is_none());
}

#[test]
fn prepared_query_refuses_missing_identity_and_uncertain_geometry_without_work() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 220.);
    let cold = engine.paragraph(&q.with_request(Spec::from_request), Some(220.));
    let origin = (0., 0.);
    let ts = Transform::identity();
    let clip = (0., 0., 320., 128.);
    let before = (ink::build_work(), engine.ink_builds, engine.ink_visits);
    assert!(!cold.prepared_ink_supports(origin, 1., ts, clip));
    cold.ink
        .borrow_mut()
        .reset(&cold.source.catalog.borrow().ink_catalog, 1.);
    assert!(
        !cold.prepared_ink_supports(origin, 1., ts, clip),
        "matching identity is not an index"
    );
    assert_eq!(cold.ink_capacity_bytes(), 0);
    assert_eq!(
        (ink::build_work(), engine.ink_builds, engine.ink_visits),
        before
    );

    let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let job = input.clone();
    let output = thread::spawn(move || FontWorker::new(recipe).unwrap().execute(job).unwrap())
        .join()
        .unwrap();
    let adopted = adopt(output, &input, &raster).unwrap();
    let p = adopted.paragraph().unwrap();
    let bytes = p.ink_capacity_bytes();
    assert!(p.prepared_ink_supports(origin, 1., ts, clip));
    for scale in [0., f32::NAN, 2., f32::from_bits(1f32.to_bits() + 1)] {
        assert!(!p.prepared_ink_supports(origin, scale, ts, clip));
    }
    for origin in [(f32::NAN, 0.), (0., f32::INFINITY), (0., 30_000_000.)] {
        assert!(!p.prepared_ink_supports(origin, 1., ts, clip));
    }
    for ts in [
        Transform::from_scale(0., 1.),
        Transform::from_scale(f32::NAN, 1.),
    ] {
        assert!(!p.prepared_ink_supports(origin, 1., ts, clip));
    }
    for clip in [(f32::NAN, 0., 320., 128.), (0., 0., f32::INFINITY, 128.)] {
        assert!(!p.prepared_ink_supports(origin, 1., ts, clip));
    }
    let token = p.source.catalog.borrow().ink_catalog.clone();
    p.source.catalog.borrow_mut().ink_catalog = Rc::new(());
    assert!(!p.prepared_ink_supports(origin, 1., ts, clip));
    p.source.catalog.borrow_mut().ink_catalog = token;
    {
        let _exclusive = p.ink.borrow_mut();
        assert!(!p.prepared_ink_supports(origin, 1., ts, clip));
    }
    {
        let _exclusive = p.source.catalog.borrow_mut();
        assert!(!p.prepared_ink_supports(origin, 1., ts, clip));
    }
    assert!(p.prepared_ink_supports(origin, 1., ts, clip));
    assert_eq!(p.ink_capacity_bytes(), bytes);
    assert_eq!(
        (ink::build_work(), engine.ink_builds, engine.ink_visits),
        before
    );
}

#[test]
fn prepared_query_supported_fractional_scale_matches_actual_clipped_paint() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = large_request(recipe.catalog_label());
    let scale = 1.25;
    let input = prepare(&recipe, q, PaintContext::new(scale).unwrap(), None).unwrap();
    let job = input.clone();
    let output = thread::spawn(move || FontWorker::new(recipe).unwrap().execute(job).unwrap())
        .join()
        .unwrap();
    let adopted = adopt(output, &input, &raster).unwrap();
    let p = adopted.paragraph().unwrap();
    let ts = Transform::from_scale(scale, scale).pre_scale(1. / scale, 1. / scale);
    let clip = (0., 0., 320., 128.);
    let bytes = p.ink_capacity_bytes();
    let work = ink::build_work();
    let token = p.source.catalog.borrow().ink_catalog.clone();
    let weak_count = Rc::weak_count(&token);
    for _ in 0..1000 {
        assert!(p.prepared_ink_supports((0., 0.), scale, ts, clip));
    }
    assert_eq!(Rc::weak_count(&token), weak_count);
    assert_eq!(engine.ink_visits, 0);
    assert_eq!(ink::build_work(), work);
    for y in [0., -p.height / 2., -p.height + 70.] {
        assert!(p.prepared_ink_supports((0., y), scale, ts, clip));
        let actual = clipped(&mut engine, p, y, scale);
        assert_eq!(actual, full(p, y, scale));
        assert!(actual.chunks_exact(4).any(|px| px[3] != 0));
    }
    assert_eq!(engine.ink_builds, 0);
    assert_eq!(ink::build_work(), work);
    assert_eq!(p.ink_capacity_bytes(), bytes);
}

#[test]
fn prepared_query_does_not_change_ordinary_lazy_build_or_full_fallback() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let spec = crate::paint::text_spec(&StyleProps::default(), "Ordinary eager paragraph ffi");
    let p = engine.paragraph(&spec, Some(220.));
    let clip = (0., 0., 320., 128.);
    assert!(!p.prepared_ink_supports((0., 0.), 1., Transform::identity(), clip));
    assert_eq!(engine.ink_builds, 0);
    let actual = clipped(&mut engine, &p, 0., 1.);
    assert_eq!(actual, full(&p, 0., 1.));
    assert_eq!(engine.ink_builds, 1, "ordinary paint still builds lazily");
    let ts = Transform::from_scale(0., 1.);
    assert!(!p.prepared_ink_supports((0., 0.), 1., ts, clip));
    let all = p.paint_glyphs(&palette()).count();
    let before = engine.ink_visits;
    let mut target = Pixmap::new(320, 128).unwrap();
    engine.paint_clipped(&mut target, &p, &palette(), (0., 0.), 1., ts, None, clip);
    assert_eq!(
        engine.ink_visits - before,
        all,
        "ordinary fail-open renderer stays unchanged"
    );
    assert_eq!(engine.ink_builds, 1);
}

#[test]
fn real_two_axis_offers_share_one_definite_layout_and_index() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (mut kernel, _) = large_request(recipe.catalog_label());
    // A block now needs only one offer in Taffy 0.14. A definite-height flex
    // column still exercises intrinsic and constrained height at one width.
    let mut column = StyleProps::default();
    for id in [StyleId::Display, StyleId::FlexDirection, StyleId::Height] {
        column.mask.set(id);
    }
    column.display = exact_kernel::Display::Flex;
    column.flex_direction = exact_kernel::FlexDirection::Column;
    column.height = Dimension::Points(300.);
    kernel
        .apply(
            0,
            0,
            &[Op::SetStyle {
                id: 3,
                patch: Box::new(column),
            }],
        )
        .unwrap();
    let catalog = recipe.catalog_label();
    let q = next_request(&mut kernel, catalog, 600.);
    assert!(q.source().bytes() >= 65536);
    let (jobs, incoming) = std::sync::mpsc::channel::<PreparedText>();
    let (outgoing, results) = std::sync::mpsc::channel();
    let worker_recipe = recipe.clone();
    let worker = thread::spawn(move || {
        let mut worker = FontWorker::new(worker_recipe).unwrap();
        for job in incoming {
            let before = (work::read(), ink::build_work());
            let output = worker.execute(job).unwrap();
            let after = (work::read(), ink::build_work());
            outgoing.send((output, before, after)).unwrap();
        }
    });
    let mut source = None;
    let mut ready = Vec::new();
    let mut offers = Vec::new();
    let mut builds = (0, 0);
    let mut current = false;
    for _ in 0..64 {
        let q = kernel.region_text_request().unwrap().clone();
        offers.push(q.offer());
        eprintln!("actual kernel offer {}: {:?}", offers.len(), q.offer());
        let input = prepare(&recipe, q, PaintContext::new(1.).unwrap(), source.as_ref()).unwrap();
        source = Some(input.source().clone());
        jobs.send(input.clone()).unwrap();
        let (output, before, after) = results.recv().unwrap();
        if matches!(input.request().offer().width, AxisOffer::Definite(_)) {
            builds.0 += after.0.layouts - before.0.layouts;
            builds.1 += after.1.attempts - before.1.attempts;
        }
        let adopted = Rc::new(adopt(output, &input, &raster).unwrap());
        assert!(kernel
            .resolve_region_text(adopted.request(), adopted.metrics(), adopted.clone())
            .unwrap());
        ready.push(adopted);
        let receipt = kernel
            .compute_region_layout(
                1,
                Offer::definite(600., 300.),
                RegionInputs {
                    catalog,
                    consumer_revision: 1,
                },
            )
            .unwrap();
        if receipt.current {
            current = true;
            break;
        }
    }
    drop(jobs);
    worker.join().unwrap();
    assert!(current);
    let definite: Vec<_> = ready.iter().filter(|a| a.paragraph().is_some()).collect();
    assert!(
        definite.len() >= 2,
        "actual two-axis requests required: {offers:?}"
    );
    let first = definite[0].paragraph().unwrap();
    for a in &definite {
        assert_eq!(a.request().offer().width, AxisOffer::Definite(600.));
        let p = a.paragraph().unwrap();
        assert_eq!(a.metrics(), definite[0].metrics());
        for y in [0., -p.height / 2., -p.height + 100.] {
            assert_eq!(clipped(&mut engine, p, y, 1.), full(first, y, 1.));
        }
    }
    assert!(definite
        .windows(2)
        .any(|a| a[0].request().offer().height != a[1].request().offer().height));
    eprintln!(
        "definite layouts={} indexes={}; offers={offers:?}",
        builds.0, builds.1
    );
    assert_eq!(
        builds,
        (1, 1),
        "same effective inputs built duplicate numeric payloads"
    );
    for a in definite.iter().skip(1) {
        let p = a.paragraph().unwrap();
        assert_eq!(Arc::as_ptr(&first.layouts), Arc::as_ptr(&p.layouts));
        assert_eq!(first.baselines.as_ptr(), p.baselines.as_ptr());
    }
}

#[test]
fn definite_reuse_rejects_other_job_without_retaining_or_dropping_valid_sibling() {
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 220.);
    let a = prepare(&recipe, q.clone(), PaintContext::new(1.).unwrap(), None).unwrap();
    let b = prepare(&recipe, q, PaintContext::new(1.).unwrap(), Some(a.source())).unwrap();
    let mut worker = FontWorker::new(recipe).unwrap();
    let output = worker.execute(a.clone()).unwrap();
    let layout_life = output.probe.clone();
    let ink_life = output.ink_probe.clone();
    let pa = adopt(output, &a, &raster).unwrap();
    let before = (work::read(), ink::build_work());
    let owners = (layout_life.strong_count(), ink_life.strong_count());
    let other = worker.execute(b.clone()).unwrap();
    assert_eq!(
        (work::read(), ink::build_work()),
        before,
        "reuse ran giant work"
    );
    assert!(std::sync::Weak::ptr_eq(&layout_life, &other.probe));
    assert!(std::sync::Weak::ptr_eq(&ink_life, &other.ink_probe));
    assert!(matches!(
        adopt(other, &a, &raster),
        Err(TransferError::StaleResult)
    ));
    assert_eq!(
        (layout_life.strong_count(), ink_life.strong_count()),
        owners
    );
    drop(pa);
    assert!(layout_life.upgrade().is_none());
    assert!(ink_life.upgrade().is_none());
    // Worker, PreparedSource and both private requests still exist. None owns
    // historical definite arrays after the last valid snapshot has gone.
    let before = work::read();
    let fresh = worker.execute(b).unwrap();
    assert_eq!(work::read().layouts, before.layouts + 1);
    assert!(!std::sync::Weak::ptr_eq(&layout_life, &fresh.probe));
}

#[test]
fn definite_reuse_single_slot_misses_width_scale_and_returns_to_old_width() {
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (mut k, q) = request(recipe.catalog_label(), 220.);
    let a = prepare(&recipe, q, PaintContext::new(1.).unwrap(), None).unwrap();
    let mut worker = FontWorker::new(recipe.clone()).unwrap();
    let first = worker.execute(a.clone()).unwrap();
    let pa = adopt(first, &a, &raster).unwrap();
    let mut held = vec![pa];
    for (width, scale) in [(240., 1.), (220., 1.), (220., 2.), (220., 1.)] {
        let q = next_request(&mut k, recipe.catalog_label(), width);
        let input = prepare(
            &recipe,
            q,
            PaintContext::new(scale).unwrap(),
            Some(a.source()),
        )
        .unwrap();
        let before = (work::read(), ink::build_work());
        let result = worker.execute(input.clone()).unwrap();
        assert_eq!(work::read().layouts, before.0.layouts + 1);
        assert_eq!(ink::build_work().attempts, before.1.attempts + 1);
        assert_eq!(work::read().shapes, before.0.shapes);
        held.push(adopt(result, &input, &raster).unwrap());
    }
    assert_ne!(
        Arc::as_ptr(&held[0].paragraph().unwrap().layouts),
        Arc::as_ptr(&held[2].paragraph().unwrap().layouts),
        "one-slot policy must not keep a history of widths"
    );
    for pair in held.windows(2) {
        assert_ne!(
            Arc::as_ptr(&pair[0].paragraph().unwrap().layouts),
            Arc::as_ptr(&pair[1].paragraph().unwrap().layouts)
        );
    }
}

#[test]
fn definite_shared_capacity_deduplicates_arrays_and_local_ink_independently() {
    let engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (_k, q) = request(recipe.catalog_label(), 220.);
    let a = prepare(&recipe, q.clone(), PaintContext::new(1.).unwrap(), None).unwrap();
    let b = prepare(&recipe, q, PaintContext::new(1.).unwrap(), Some(a.source())).unwrap();
    let mut worker = FontWorker::new(recipe).unwrap();
    let oa = worker.execute(a.clone()).unwrap();
    let life = (oa.probe.clone(), oa.ink_probe.clone());
    let pa = adopt(oa, &a, &raster).unwrap();
    let pb = adopt(worker.execute(b.clone()).unwrap(), &b, &raster).unwrap();
    let (p, q) = (pa.paragraph().unwrap(), pb.paragraph().unwrap());
    let owned = p.owned_capacity_bytes();
    let index_bytes = p.ink_capacity_bytes();
    let mut cache = cache::Cache::default();
    let r = cache.retiring([p, q, p].into_iter());
    assert_eq!((r.owners, r.paragraphs), (3, 2));
    assert_eq!(
        r.owned_capacity_bytes, owned,
        "L+I double-counted across wrappers"
    );
    cache.hold_measured(1, Some(220.).into(), p);
    cache.hold_measured(2, Some(220.).into(), q);
    assert_eq!(cache.handoff_residency().owned_capacity_bytes, owned);
    cache.finish_handoff();
    let key = cache.identity(&p.source.spec);
    cache.set_source(key, p.source.clone());
    cache.insert(key, Some(220.).into(), p);
    let current = cache.residency();
    assert_eq!(
        current.owned_capacity_bytes - current.key_capacity_bytes,
        owned
    );
    assert_eq!(
        cache.retiring([q].into_iter()).owned_capacity_bytes,
        0,
        "current/retiring shared data overlap"
    );
    // Reset is local. A retains its prepared index; B's replacement is a new
    // immutable numeric index over the same shared layout arrays.
    let catalog = q.source.catalog.clone();
    q.ink.borrow_mut().reset(&catalog.borrow().ink_catalog, 2.);
    assert_eq!(p.ink_capacity_bytes(), index_bytes);
    assert_eq!(q.ink_capacity_bytes(), 0);
    q.ink.borrow_mut().index = ink::Index::build(&mut catalog.borrow_mut(), q, 2.).map(Into::into);
    let second_index_bytes = q.ink_capacity_bytes();
    assert!(second_index_bytes > 0);
    assert_eq!(
        cache.retiring([q].into_iter()).owned_capacity_bytes,
        second_index_bytes
    );
    cache.hold_measured(1, Some(220.).into(), p);
    cache.hold_measured(2, Some(220.).into(), q);
    assert_eq!(
        cache.handoff_residency().owned_capacity_bytes,
        owned + second_index_bytes
    );
    cache.clear();
    drop(cache);
    drop(pa);
    assert!(life.1.upgrade().is_none());
    assert!(
        life.0.upgrade().is_some(),
        "B still owns shared layout arrays"
    );
    drop(pb);
    assert!(life.0.upgrade().is_none());
    // Source/worker/request handles are still live but only have weak slots.
    assert!(life.1.upgrade().is_none());
}

#[test]
fn compacted_adopted_a_keeps_fresh_job_reuse_pixels_and_last_owner_retirement() {
    let mut engine = TextEngine::with_catalog(fixture_catalog());
    let (recipe, raster) = freeze_catalog(&engine).unwrap();
    let (mut k, _) = large_request(recipe.catalog_label());
    let q = next_request(&mut k, recipe.catalog_label(), 600.);
    assert!(q.source().bytes() >= 65536);
    let a = prepare(&recipe, q.clone(), PaintContext::new(1.).unwrap(), None).unwrap();
    let b = prepare(&recipe, q, PaintContext::new(1.).unwrap(), Some(a.source())).unwrap();
    let mut worker = FontWorker::new(recipe.clone()).unwrap();
    let result = worker.execute(a.clone()).unwrap();
    let life = (result.probe.clone(), result.ink_probe.clone());
    let pa = adopt(result, &a, &raster).unwrap();
    // CompletedText has been consumed. Only adopted A holds the backing when
    // the fresh private job executes; the source slot itself is still Weak.
    let old = pa.paragraph().unwrap();
    super::super::sharing_tests::assert_tight_glyph_storage(old);
    let pictures: Vec<_> = [0., -old.height / 2., -old.height + 100.]
        .into_iter()
        .map(|y| full(old, y, 1.))
        .collect();
    let old_capacity = old.owned_capacity_bytes();
    let before = (work::read(), ink::build_work());
    let result = worker.execute(b.clone()).unwrap();
    assert_eq!((work::read(), ink::build_work()), before);
    assert!(std::sync::Weak::ptr_eq(&life.0, &result.probe));
    assert!(std::sync::Weak::ptr_eq(&life.1, &result.ink_probe));
    let pb = adopt(result, &b, &raster).unwrap();
    let reused = pb.paragraph().unwrap();
    assert!(Arc::ptr_eq(&old.layouts, &reused.layouts));
    assert!(Arc::ptr_eq(&old.baselines, &reused.baselines));
    assert_eq!(reused.owned_capacity_bytes(), old_capacity);
    let retiring = cache::Cache::default().retiring([old, reused].into_iter());
    assert_eq!(retiring.owned_capacity_bytes, old_capacity);
    for (y, expected) in [0., -old.height / 2., -old.height + 100.]
        .into_iter()
        .zip(&pictures)
    {
        assert_eq!(&clipped(&mut engine, reused, y, 1.), expected);
    }
    let wrong = worker.execute(b.clone()).unwrap();
    assert!(matches!(
        adopt(wrong, &a, &raster),
        Err(TransferError::StaleResult)
    ));
    assert!(life.0.upgrade().is_some());
    // A novel width builds one new independent backing without mutating A.
    let q = next_request(&mut k, recipe.catalog_label(), 984.);
    let c = prepare(&recipe, q, PaintContext::new(1.).unwrap(), Some(a.source())).unwrap();
    let before = (work::read(), ink::build_work());
    let result = worker.execute(c.clone()).unwrap();
    assert_eq!(work::read().layouts, before.0.layouts + 1);
    assert_eq!(work::read().shapes, before.0.shapes);
    assert_eq!(ink::build_work().attempts, before.1.attempts + 1);
    let novel_life = (result.probe.clone(), result.ink_probe.clone());
    let pc = adopt(result, &c, &raster).unwrap();
    super::super::sharing_tests::assert_tight_glyph_storage(pc.paragraph().unwrap());
    assert!(!Arc::ptr_eq(&old.layouts, &pc.paragraph().unwrap().layouts));
    for (y, expected) in [0., -old.height / 2., -old.height + 100.]
        .into_iter()
        .zip(&pictures)
    {
        assert_eq!(&clipped(&mut engine, old, y, 1.), expected);
    }
    assert_eq!(old.owned_capacity_bytes(), old_capacity);
    drop(pa);
    assert!(life.0.upgrade().is_some());
    drop(pb);
    assert!(life.0.upgrade().is_none());
    assert!(life.1.upgrade().is_none());
    assert!(novel_life.0.upgrade().is_some());
    drop(pc);
    assert!(novel_life.0.upgrade().is_none());
    assert!(novel_life.1.upgrade().is_none());
    // Worker and PreparedSources remain alive, without retaining width history.
    assert!(a.source().0.shape.get().is_some());
}

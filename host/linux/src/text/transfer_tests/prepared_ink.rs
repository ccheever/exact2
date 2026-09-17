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
    for (g, baseline, ink) in p.paint_glyphs(&palette()) {
        let phys = g.physical((0., (y + baseline) * scale), scale);
        let Some(glyph) = catalog.glyph(phys.cache_key, ink.color) else {
            continue;
        };
        target.draw_pixmap(
            phys.x + glyph.left,
            phys.y - glyph.top,
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
    assert!(matches!(
        adopt(late, &b, &raster),
        Err(TransferError::StaleResult)
    ));
    assert!(late_index.upgrade().is_none());
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

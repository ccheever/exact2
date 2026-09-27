use super::*;

struct Fixture {
    engine: TextEngine,
    keys: Vec<(u64, u64)>,
    weak: Vec<std::rc::Weak<Paragraph>>,
}
fn fixture(count: usize, widths: bool) -> Fixture {
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(Arc::new(
        include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").to_vec(),
    )));
    db.set_sans_serif_family("DejaVu Sans");
    let mut engine = TextEngine::with_catalog(catalog::Catalog::with_fonts(
        FontSystem::new_with_locale_and_db("en-US".into(), db),
    ));
    engine.paragraphs.trim_test_target(usize::MAX);
    let mut keys = Vec::new();
    let mut weak = Vec::new();
    for i in 0..count {
        let s = spec(&format!(
            "sort fixture {i:03} words café words\nsecond line"
        ));
        let key = engine.paragraphs.identity(&s);
        keys.push(key);
        if widths {
            engine.measure(&s, AxisOffer::MaxContent);
            let p = engine.paragraph(&s, Some(143.25));
            weak.push(Rc::downgrade(&p));
        }
    }
    Fixture { engine, keys, weak }
}
fn compare(
    a: &mut Fixture,
    b: &mut Fixture,
    target: usize,
    keep: Option<u64>,
    calls: (usize, usize),
) {
    a.engine.paragraphs.trim_test_target(target);
    b.engine.paragraphs.trim_test_target(target);
    assert_eq!(
        a.engine.paragraphs.trim_test_state(),
        b.engine.paragraphs.trim_test_state()
    );
    let before = cache::trim_sort_calls();
    a.engine.paragraphs.trim(keep);
    let after = cache::trim_sort_calls();
    b.engine.paragraphs.trim_reference(keep);
    assert_eq!(
        a.engine.paragraphs.trim_test_state(),
        b.engine.paragraphs.trim_test_state()
    );
    assert_eq!(
        a.weak.iter().map(|w| w.strong_count()).collect::<Vec<_>>(),
        b.weak.iter().map(|w| w.strong_count()).collect::<Vec<_>>()
    );
    let actual = (after.0 - before.0, after.1 - before.1);
    eprintln!("target={target} keep={keep:?} sort_calls={actual:?} expected={calls:?}");
    assert_eq!(actual, calls, "unnecessary actual sort-site call");
}

#[test]
fn below_and_exact_byte_target_skip_both_sorts_without_changing_state() {
    for exact in [false, true] {
        let mut a = fixture(3, true);
        let mut b = fixture(3, true);
        let bytes = a.engine.residency().cold_policy_bytes;
        assert!(bytes > 0 && a.engine.residency().cold_paragraphs == 3);
        compare(&mut a, &mut b, bytes + usize::from(!exact), None, (0, 0));
        assert!(a.weak.iter().all(|w| w.strong_count() == 1));
    }
}

#[test]
fn one_byte_over_width_eviction_restores_budget_before_key_sort() {
    let mut a = fixture(3, true);
    let mut b = fixture(3, true);
    let target = a.engine.residency().cold_policy_bytes - 1;
    compare(&mut a, &mut b, target, None, (1, 0));
    assert!(
        a.weak[0].upgrade().is_none(),
        "oldest width was not evicted"
    );
    assert!(a.weak[1..].iter().all(|w| w.strong_count() == 1));
    assert_eq!(a.engine.residency().identities, 3);
    assert!(a.engine.residency().cold_policy_bytes <= target);
}

#[test]
fn insufficient_width_reclamation_still_sorts_and_evicts_keys() {
    let mut a = fixture(3, true);
    let mut b = fixture(3, true);
    compare(&mut a, &mut b, 0, None, (1, 1));
    assert!(a.weak.iter().all(|w| w.upgrade().is_none()));
    assert_eq!(a.engine.residency().identities, 0);
    assert_eq!(a.engine.residency().cold_policy_bytes, 0);
}

#[test]
fn identity_limit_counts_keep_even_if_absent_and_preserves_existing_keep() {
    for (count, which, retained, key_sort) in [
        (256, 0, 256, 0),
        (257, 0, 256, 1),
        (256, 1, 256, 0),
        (257, 1, 256, 1),
        (256, 2, 255, 1),
    ] {
        let mut a = fixture(count, false);
        let mut b = fixture(count, false);
        assert_eq!(a.engine.residency().identities, count);
        let keep = match which {
            0 => None,
            1 => Some(a.keys[0].1),
            _ => Some(u64::MAX),
        };
        compare(&mut a, &mut b, usize::MAX, keep, (0, key_sort));
        assert_eq!(a.engine.residency().identities, retained);
        if which == 1 {
            assert!(a.engine.paragraphs.spec(a.keys[0]).is_some());
        }
    }
}

#[test]
fn tied_identity_ages_keep_original_full_tuple_eviction_order() {
    let mut a = fixture(257, false);
    let mut b = fixture(257, false);
    a.engine.paragraphs.trim_test_tie_keys();
    b.engine.paragraphs.trim_test_tie_keys();
    let oldest = *a.keys.iter().min().unwrap();
    compare(&mut a, &mut b, usize::MAX, None, (0, 1));
    assert!(a.engine.paragraphs.spec(oldest).is_none());
    assert_eq!(a.engine.residency().identities, 256);
}

#[test]
fn no_eviction_still_prunes_dead_widths_and_stale_bindings() {
    let mut a = fixture(3, true);
    let mut b = fixture(3, true);
    let mut kernel = text_tree("bound", 140.);
    kernel
        .apply(
            0,
            3,
            &[
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 3,
                    prop: exact_kernel::PropId::Text,
                    value: exact_kernel::PropValue::Str("stale".into()),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3],
                },
            ],
        )
        .unwrap();
    let bound = kernel.node(2).unwrap().paragraph_stamp().unwrap();
    let stale = kernel.node(3).unwrap().paragraph_stamp().unwrap();
    assert_ne!(bound.owner(), stale.owner());
    for f in [&mut a, &mut b] {
        f.engine.paragraphs.bind(&bound, f.keys[0]);
        f.engine.paragraphs.bind(&stale, (u64::MAX, u64::MAX));
        f.engine.paragraphs.trim_test_dead_width(f.keys[0]);
        assert_eq!(f.engine.paragraphs.binding_count(), 2);
        assert_eq!(f.engine.paragraphs.indexed_widths(), 4);
    }
    compare(&mut a, &mut b, usize::MAX, None, (0, 0));
    assert_eq!(a.engine.paragraphs.binding_count(), 1);
    assert_eq!(a.engine.paragraphs.indexed_widths(), 3);
    assert_eq!(
        a.engine.paragraphs.identified(&bound).map(|(key, _)| key),
        Some(a.keys[0])
    );
    assert_eq!(
        a.engine.paragraphs.identified(&stale).map(|(key, _)| key),
        None
    );
}

#[test]
fn pinned_keep_handoff_and_lazy_ink_keep_exact_lifetimes() {
    let mut a = fixture(3, true);
    let mut b = fixture(3, true);
    let pa = a.weak[0].upgrade().unwrap();
    let pb = b.weak[0].upgrade().unwrap();
    let mut targets = Vec::new();
    for f in [&mut a, &mut b] {
        let measured = f.weak[1].upgrade().unwrap();
        f.engine
            .paragraphs
            .hold_measured(f.keys[1].1, Some(143.25).into(), &measured);
        drop(measured);
        let target = f.engine.residency().cold_policy_bytes;
        let cold = f.weak[2].upgrade().unwrap();
        let ink = paint_lazy_ink(&mut f.engine, &cold, 1.);
        assert!(ink > 0);
        drop(cold);
        assert_eq!(f.engine.residency().cold_policy_bytes, target + ink);
        targets.push(target);
    }
    assert_eq!(targets[0], targets[1]);
    let keep = Some(a.keys[0].1);
    compare(&mut a, &mut b, targets[0], keep, (1, 0));
    assert!(a.weak[2].upgrade().is_none());
    assert_eq!(a.engine.handoff_residency().paragraphs, 1);
    assert_eq!(Rc::strong_count(&pa), 2);
    assert_eq!(Rc::strong_count(&pb), 2);
    // The kept id is pinned, not in cold_keys; its phantom one in count
    // remains original policy. Zero budget may remove all other cold keys.
    compare(&mut a, &mut b, 0, keep, (1, 1));
    assert!(a.engine.paragraphs.spec(a.keys[0]).is_some());
    assert!(a.engine.paragraphs.spec(a.keys[1]).is_some());
    drop((pa, pb));
    // Release only the handoff here so the next explicit comparison owns
    // maintenance in both arms; finish_text_frame itself also trims.
    a.engine.paragraphs.finish_handoff();
    b.engine.paragraphs.finish_handoff();
    compare(&mut a, &mut b, 0, None, (1, 1));
    assert!(a.weak.iter().all(|w| w.upgrade().is_none()));
}

fn scratch_budget(exact: bool) {
    let mut a = fixture(3, true);
    let mut b = fixture(3, true);
    let bytes = a.engine.residency().cold_policy_bytes;
    assert!(bytes > 0 && a.engine.residency().cold_paragraphs == 3);
    let before = cache::trim_vector_entries();
    compare(&mut a, &mut b, bytes + usize::from(!exact), None, (0, 0));
    let after = cache::trim_vector_entries();
    let entries = (after.0 - before.0, after.1 - before.1);
    eprintln!("exact={exact} eviction_vector_entries={entries:?}");
    assert_eq!(entries, (0, 0), "under-budget eviction scratch");
}

#[test]
fn trim_scratch_below_budget_materializes_no_candidates() {
    scratch_budget(false);
}

#[test]
fn trim_scratch_exact_budget_materializes_no_candidates() {
    scratch_budget(true);
}

#[test]
fn trim_scratch_identity_pressure_does_not_collect_widths() {
    let mut a = fixture(256, true);
    let mut b = fixture(256, true);
    for f in [&mut a, &mut b] {
        f.engine.paragraphs.identity(&spec("257th cold identity"));
        assert_eq!(f.engine.residency().identities, 257);
        assert_eq!(f.engine.residency().cold_paragraphs, 256);
    }
    let before = cache::trim_vector_entries();
    compare(&mut a, &mut b, usize::MAX, None, (0, 1));
    let after = cache::trim_vector_entries();
    assert_eq!(after.1 - before.1, 257, "identity eviction still required");
    assert_eq!(
        after.0 - before.0,
        0,
        "byte budget needs no width candidates"
    );
}

#[test]
fn trim_scratch_messages_10000_32_setup_and_saturated_revisions() {
    use messages_trim_model::{history, Controls};
    use sha2::Digest;
    const COUNT: usize = 10_000;
    const BATCH: usize = 32;
    let initial = history(Controls::new(COUNT, 0, BATCH).unwrap(), "").unwrap();
    let mut f = fixture(0, false);
    f.engine.paragraphs.trim_test_target(cache::COLD_BYTES);
    let mut previous = Vec::<Rc<Paragraph>>::new();
    let mut work = [(0, 0); 3]; // Revisions 1..2, 3..44 warmup, 45..48 saturated.
    let mut mask = tiny_skia::Mask::new(320, 128).unwrap();
    let path = tiny_skia::PathBuilder::from_rect(
        tiny_skia::Rect::from_xywh(10.125, 7.25, 275.5, 110.5).unwrap(),
    );
    mask.fill_path(
        &path,
        tiny_skia::FillRule::Winding,
        true,
        Transform::identity(),
    );
    let rgba = |engine: &mut TextEngine, p: &Paragraph| {
        let mut image = Pixmap::new(320, 128).unwrap();
        image.fill(tiny_skia::Color::WHITE);
        engine.paint(
            &mut image,
            p,
            &[RunPaint {
                color: [31, 72, 211, 230],
                source: 2,
            }],
            (7.375, -0.625),
            1.,
            Transform::identity(),
            Some(&mask),
        );
        image.data().to_vec()
    };
    for revision in 1..=48 {
        let rows = history(Controls::new(COUNT, revision, BATCH).unwrap(), "").unwrap();
        assert_eq!(rows.len(), COUNT);
        assert_eq!(rows[..COUNT - BATCH], initial[..COUNT - BATCH]);
        let before = cache::trim_vector_entries();
        let current = rows[COUNT - BATCH..]
            .iter()
            .map(|row| {
                let mut s = spec(&row.body);
                s.strut.size = 14.;
                s.strut.line_height = Some(14. * 1.45);
                s.runs[0].size = 14.;
                s.runs[0].line_height = s.strut.line_height;
                s.white_space = exact_kernel::WhiteSpace::PreWrap;
                let measured = f.engine.measure(&s, AxisOffer::Definite(280.));
                let p = f.engine.paragraph(&s, Some(280.));
                assert_eq!(measured, paragraph_metrics(&p));
                p
            })
            .collect::<Vec<_>>();
        assert_eq!(current.len(), BATCH);
        assert_eq!(
            current
                .iter()
                .map(Rc::as_ptr)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            6,
            "ordinary equal-body sharing must remain"
        );
        if !previous.is_empty() {
            for (old, new) in previous.iter().zip(&current) {
                assert!(!Rc::ptr_eq(&old.source, &new.source));
                assert!(Rc::ptr_eq(&old.source.catalog, &new.source.catalog));
            }
        }
        drop(previous); // Retire A only after B exists; no synthetic cold clones.
        f.engine.finish_text_frame();
        let after = cache::trim_vector_entries();
        let phase = if revision <= 2 {
            0
        } else {
            usize::from(revision > 44) + 1
        };
        work[phase].0 += after.0 - before.0;
        work[phase].1 += after.1 - before.1;
        let residency = f.engine.residency();
        assert!(
            residency.cold_policy_bytes < cache::COLD_BYTES,
            "STOP: byte pressure"
        );
        if revision > 44 {
            assert!(
                residency.identities >= cache::COLD_IDENTITIES,
                "STOP: not saturated"
            );
        }
        if revision == 2 || revision == 48 {
            let pixels = current
                .iter()
                .map(|p| rgba(&mut f.engine, p))
                .collect::<Vec<_>>();
            eprintln!(
                "revision={revision} metrics={:?} rgba_sha256={:x}",
                current
                    .iter()
                    .map(|p| (paragraph_metrics(p), &p.baselines))
                    .collect::<Vec<_>>(),
                sha2::Sha256::digest(pixels.concat())
            );
            let state = f.engine.paragraphs.trim_test_state();
            f.engine.trim_paragraphs();
            assert_eq!(state, f.engine.paragraphs.trim_test_state());
            for (p, pixels) in current.iter().zip(pixels) {
                assert_eq!(pixels, rgba(&mut f.engine, p), "full RGBA changed");
            }
        }
        previous = current;
    }
    eprintln!("Messages body projection 10000/32 setup/warmup/saturated entries={work:?}");
    assert!(
        work[2].1 > 0,
        "STOP: no actual saturated identity-eviction work"
    );
    assert_eq!(work[0], (0, 0), "setup needs no eviction vectors");
    assert_eq!(
        work[2].0, 0,
        "saturated identity budget needs no width vector"
    );
}

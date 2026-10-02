use super::*;
use exact_kernel::Op;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

/// A hang detector, not a speed claim: a wait returns once nothing is
/// pending, and a 2048² decode took over 3 s on this Mac at load 170.
const SETTLED: Duration = Duration::from_secs(60);

fn encoded(w: u32, h: u32, color: [u8; 4]) -> Arc<[u8]> {
    let mut data = Vec::new();
    let mut encoder = png::Encoder::new(&mut data, w, h);
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header().unwrap();
    {
        use std::io::Write;
        let row: Vec<_> = (0..w).flat_map(|_| color).collect();
        let mut stream = writer.stream_writer().unwrap();
        for _ in 0..h {
            stream.write_all(&row).unwrap();
        }
        stream.finish().unwrap();
    }
    writer.finish().unwrap();
    data.into()
}
fn assets() -> Assets {
    Assets::selected(
        PathBuf::new(),
        Arc::new(|name| {
            let n = name.parse::<u32>().unwrap_or(0);
            Ok(Some(encoded(
                16 + n % 3,
                10 + n % 5,
                [(n % 251) as u8, 100, 70, 255],
            )))
        }),
    )
}
fn kernel(count: u32) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut ops = Vec::new();
    for id in 1..=count {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::Image,
        });
        ops.push(Op::SetProp {
            id,
            prop: PropId::ImageSource,
            value: id.to_string().into(),
        });
        ops.push(Op::AttachRoot { id });
    }
    k.apply(0, 1, &ops).unwrap();
    k
}
fn source(k: &mut Kernel, id: u32, name: &str) {
    let epoch = k.epoch();
    k.apply(
        0,
        epoch + 1,
        &[Op::SetProp {
            id,
            prop: PropId::ImageSource,
            value: name.into(),
        }],
    )
    .unwrap();
}
/// Wait for a condition. The deadline only detects a hang: it is not a
/// budget, and a loaded machine can take seconds to schedule a worker.
fn until(mut test: impl FnMut() -> bool) {
    let started = Instant::now();
    while !test() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "bounded worker progress"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn symbols_clear_rasters_without_loading_and_follow_font_size() {
    let mut k = kernel(1);
    let mut images = Images::with_assets(assets());
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    assert!(images.bitmaps.contains_key(&1));
    for name in ["symbol:sf/airpodsmax", "symbol:sf/", "symbol:unknown"] {
        source(&mut k, 1, name);
        assert_eq!(images.sync(&k, &k.roots()), vec![(1, Some((16., 16.)))]);
        assert!(!images.bitmaps.contains_key(&1));
        assert!(images.poll().is_empty());
        assert!(images.views[&1].source_id.is_none());
        assert!(images.views[&1].refusal.is_none());
        assert!(images.sync(&k, &k.roots()).is_empty());
    }
    let epoch = k.epoch();
    k.apply(
        0,
        epoch + 1,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new({
                let mut style = exact_kernel::StyleProps {
                    font_size: 28.,
                    ..Default::default()
                };
                style.mask.set(exact_kernel::StyleId::FontSize);
                style
            }),
        }],
    )
    .unwrap();
    assert_eq!(images.sync(&k, &k.roots()), vec![(1, Some((28., 28.)))]);
    source(&mut k, 1, "1");
    assert_eq!(images.sync(&k, &k.roots()), vec![(1, None)]);
    images.wait(SETTLED);
    assert_eq!(images.bitmaps[&1].natural(), (17, 11));
}

#[test]
fn replacement_keeps_old_pixels_and_geometry_and_drops_cancelled_backing_once() {
    let mut k = kernel(1);
    let mut images = Images::with_assets(assets());
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    let a = images.bitmaps[&1].clone();
    let released = Arc::new(AtomicBool::new(false));
    let weak_b = Arc::new(Mutex::new(None));
    let release = released.clone();
    let observe = weak_b.clone();
    *images.backend.hook.lock().unwrap() = Some(Arc::new(move |name, backing| {
        if name == "2" {
            *observe.lock().unwrap() = Some(Arc::downgrade(backing));
            until(|| release.load(Ordering::Acquire));
        }
    }));
    source(&mut k, 1, "2");
    assert!(images.sync(&k, &k.roots()).is_empty());
    until(|| {
        images.poll();
        weak_b.lock().unwrap().is_some()
    });
    assert!(Arc::ptr_eq(&images.bitmaps[&1], &a));
    assert_eq!(images.bitmaps[&1].natural(), (17, 11));
    let charged = images.stats().reserved_bytes;
    assert!(charged > 0);
    source(&mut k, 1, "3");
    images.sync(&k, &k.roots());
    assert!(
        images.stats().reserved_bytes >= charged,
        "cancel cannot refund allocated B"
    );
    released.store(true, Ordering::Release);
    images.wait(SETTLED);
    assert_eq!(images.bitmaps[&1].natural(), (16, 13));
    assert_eq!(images.bitmaps[&1].as_ref().as_ref()[0], 3);
    until(|| weak_b.lock().unwrap().as_ref().unwrap().upgrade().is_none());
    assert!(images.poll().is_empty(), "no duplicate acceptance");
    assert_eq!(images.loaded, vec![("3".into(), (16, 13))]);
    images.reset();
    drop(a);
    until(|| images.stats().resident_bytes == 0 && images.stats().reserved_bytes == 0);
}

#[test]
fn twenty_replacement_waves_decode_distinct_sources_without_retaining_history() {
    let mut k = kernel(24);
    let mut images = Images::with_assets(assets());
    for wave in 0..20 {
        for id in 1..=24 {
            source(&mut k, id, &(wave * 24 + id).to_string());
        }
        images.sync(&k, &k.roots());
        images.wait(Duration::from_secs(60));
        assert_eq!(images.bitmaps.len(), 24);
        for id in 1..=24 {
            assert_eq!(
                images.bitmaps[&id].as_ref().as_ref()[0],
                ((wave * 24 + id) % 251) as u8
            );
        }
        let s = images.stats();
        assert!(s.peak_bytes <= SESSION_BYTES && s.delivery_cells <= 2 && s.pending_jobs <= 64);
        assert!(s.subscribers <= 1024 && s.cold_entries <= exact_raster::COLD_ENTRIES);
        assert!(Gate::process().stats().running <= 2);
        // A worker can own a replaced source briefly after delivering it (the
        // workers' cap test waits the same way): retention is bounded eventually.
        until(|| images.backend.sources() <= 24 + exact_raster::COLD_ENTRIES);
        assert_eq!(images.loaded.len(), 24);
    }
    images.sync(&k, &[]);
    images.backend.session.trim();
    assert_eq!(images.backend.sources(), 0);
    assert!(images.loaded.is_empty());
    until(|| images.stats().resident_bytes == 0 && images.stats().reserved_bytes == 0);
}

#[test]
fn full_undrained_session_does_not_block_another_sessions_native_workers() {
    let k = kernel(2);
    let mut a = Images::with_assets(assets());
    // Prepare metadata without occupying a decode worker. Holding one worker
    // while waiting for more metadata could deadlock another gated fixture.
    assert!(a.prepare_metadata(&k, &k.roots(), SETTLED));
    let release = Arc::new(AtomicBool::new(false));
    struct ReleaseOnDrop(Arc<AtomicBool>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let _release_on_unwind = ReleaseOnDrop(release.clone());
    let released = release.clone();
    *a.backend.hook.lock().unwrap() = Some(Arc::new(move |_, _| {
        until(|| released.load(Ordering::Acquire));
    }));
    a.enable_decode();
    a.poll();
    assert_eq!(a.views.len(), 2);
    assert!(a.views.values().all(|v| v.request.is_some()));
    assert!(a.bitmaps.is_empty());
    // Worker admission is asynchronous; requests can still be queued here.
    until(|| a.stats().running == 2);
    let blocked = a.stats();
    assert_eq!(blocked.ready, 0);
    assert_eq!(blocked.delivery_cells, 2, "cells are reserved at admission");
    // No poll may consume A between releasing decode and checking B's progress.
    release.store(true, Ordering::Release);
    until(|| {
        let stats = a.stats();
        stats.ready == 2 && stats.running == 0
    });
    assert_eq!(a.stats().delivery_cells, 2);
    // A receives no more UI polls during B's metadata, admission and decoding.
    let mut b = Images::with_assets(assets());
    b.sync(&k, &k.roots());
    b.wait(SETTLED);
    assert_eq!(b.bitmaps.len(), 2);
    let undrained = a.stats();
    assert_eq!(undrained.ready, 2);
    assert_eq!(undrained.running, 0);
    assert_eq!(undrained.delivery_cells, 2);
}

#[test]
fn resize_buckets_and_invalid_source_cancel_without_discarding_accepted_pixels() {
    let data = encoded(4000, 2000, [42, 80, 90, 255]);
    let header = png_decode::inspect(&mut std::io::Cursor::new(&data), data.len() as u64).unwrap();
    assert!(!resize_changes_decode(
        header,
        (300., 150.),
        (310.125, 155.0625)
    ));
    assert!(resize_changes_decode(header, (300., 150.), (700., 350.)));
    let mut k = kernel(1);
    let mut images = Images::with_assets(assets());
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    let old = images.bitmaps[&1].clone();
    source(&mut k, 1, &"x".repeat(4097));
    images.sync(&k, &k.roots());
    assert!(images.views[&1].request.is_none());
    assert_eq!(images.views[&1].refusal, Some(Refusal::DecodeFailed));
    assert!(Arc::ptr_eq(&old, &images.bitmaps[&1]));
}

#[test]
fn pinned_old_allocation_causes_downsize_and_real_visible_progress() {
    let bytes = encoded(2048, 2048, [71, 80, 90, 255]);
    let mut images = Images::with_assets(Assets::selected(
        PathBuf::new(),
        Arc::new(move |_| Ok(Some(bytes.clone()))),
    ));
    let reservation = images
        .backend
        .session
        .reserve_allocation(20 * 1024 * 1024)
        .unwrap();
    let charge = reservation.charge();
    let pinned = Bitmap::new(
        tiny_skia::Pixmap::new(2560, 2048).unwrap(),
        (2560, 2048),
        charge,
    );
    drop(reservation.commit(20 * 1024 * 1024).unwrap());
    let k = kernel(1);
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    let image = &images.bitmaps[&1];
    assert_eq!(image.natural(), (2048, 2048));
    assert!(image.width() < 2048);
    assert_eq!(image.as_ref().as_ref()[0], 71);
    assert!(images.stats().peak_bytes <= SESSION_BYTES);
    drop(pinned);
}

#[test]
fn source_identity_survives_displayed_replacement_and_cold_unmount() {
    let mut k = kernel(1);
    let mut images = Images::with_assets(assets());
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    let first = Arc::downgrade(&images.bitmaps[&1]);
    source(&mut k, 1, "2");
    images.sync(&k, &k.roots());
    let epoch = k.epoch();
    k.apply(
        0,
        epoch + 1,
        &[
            Op::CreateView {
                id: 2,
                node_type: NodeType::Image,
            },
            Op::SetProp {
                id: 2,
                prop: PropId::ImageSource,
                value: "1".into(),
            },
            Op::AttachRoot { id: 2 },
        ],
    )
    .unwrap();
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    assert!(
        Arc::ptr_eq(&first.upgrade().unwrap(), &images.bitmaps[&2]),
        "displayed A is reused during B replacement"
    );
    let cold = Arc::downgrade(&images.bitmaps[&2]);
    images.sync(&k, &[]);
    images.sync(&k, &[2]);
    images.wait(SETTLED);
    assert!(
        Arc::ptr_eq(&cold.upgrade().unwrap(), &images.bitmaps[&2]),
        "cold cached A keeps its source identity"
    );
}

#[test]
fn metadata_progresses_under_known_decode_pressure_in_shared_pool() {
    use std::sync::atomic::AtomicUsize;
    let decoded = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    struct ReleaseOnDrop(Arc<AtomicBool>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let _release_on_unwind = ReleaseOnDrop(release.clone());
    let k = kernel(2);
    let mut sessions = Vec::new();
    for _ in 0..8 {
        let mut images = Images::with_assets(assets());
        assert!(images.prepare_metadata(&k, &k.roots(), SETTLED));
        let count = decoded.clone();
        let released = release.clone();
        *images.backend.hook.lock().unwrap() = Some(Arc::new(move |_, _| {
            count.fetch_add(1, Ordering::AcqRel);
            until(|| released.load(Ordering::Acquire));
        }));
        sessions.push(images);
    }
    for a in &mut sessions {
        a.enable_decode();
        a.poll();
    }
    until(|| decoded.load(Ordering::Acquire) == 2);
    let mut b = Images::with_assets(assets());
    b.sync(&k, &[1]);
    let source = b.views[&1].source_id.as_ref().unwrap().id;
    release.store(true, Ordering::Release);
    // This pool also serves other parallel tests. A global completion count at
    // resolver entry is not a bound on metadata admission: competing metadata
    // turns and a descheduled Reading worker can both increase it legitimately.
    // Exact dispatch ordering is covered by workers::fairness_tests on the same
    // production turn method with an isolated gate and controlled arrivals.
    until(|| {
        matches!(
            b.backend.prepared(source),
            Some(Prepared::Ready(_) | Prepared::Failed(_))
        )
    });
    assert!(matches!(
        b.backend.prepared(source),
        Some(Prepared::Ready(_))
    ));
}

#[test]
fn provider_identity_outlives_metadata_churn_and_generation_retirement() {
    use std::sync::atomic::AtomicUsize;
    let decoded = Arc::new(AtomicUsize::new(0));
    let count = decoded.clone();
    let mut images = Images::with_assets(assets());
    *images.backend.hook.lock().unwrap() = Some(Arc::new(move |_, _| {
        count.fetch_add(1, Ordering::AcqRel);
    }));
    let mut k = kernel(1);
    images.sync(&k, &[1]);
    images.wait(SETTLED);
    let a = images.bitmaps[&1].clone();
    let owner = images.views[&1].source_id.as_ref().unwrap().clone();
    let weak_owner = Arc::downgrade(&owner);
    let original_id = owner.id;
    drop(owner);
    source(&mut k, 1, "2");
    images.sync(&k, &[1]);
    images.wait(SETTLED);
    for n in 100..180 {
        let owner = images
            .backend
            .source(images.generation, &n.to_string(), &images.assets)
            .unwrap();
        until(|| matches!(images.backend.prepared(owner.id), Some(Prepared::Ready(_))));
    }
    assert!(images.backend.sources() <= 3, "no visited metadata history");
    let before = decoded.load(Ordering::Acquire);
    source(&mut k, 1, "1");
    images.sync(&k, &[1]);
    images.wait(SETTLED);
    assert_eq!(images.views[&1].source_id.as_ref().unwrap().id, original_id);
    assert!(Arc::ptr_eq(&a, &images.bitmaps[&1]));
    assert_eq!(decoded.load(Ordering::Acquire), before);
    images.reset();
    assert!(
        weak_owner.upgrade().is_some(),
        "the actual provider owns source identity"
    );
    images.sync(&k, &[1]);
    images.wait(SETTLED);
    assert_ne!(images.views[&1].source_id.as_ref().unwrap().id, original_id);
    drop(a);
    until(|| weak_owner.upgrade().is_none());
    images.reset();
    assert_eq!(images.backend.sources(), 0);
}

#[test]
fn sharing_a_large_live_raster_does_not_require_a_second_decode_budget() {
    let bytes = encoded(2048, 2048, [17, 80, 90, 255]);
    let mut images = Images::with_assets(Assets::selected(
        PathBuf::new(),
        Arc::new(move |_| Ok(Some(bytes.clone()))),
    ));
    let mut k = kernel(2);
    source(&mut k, 2, "1");
    images.sync(&k, &[1]);
    images.wait(SETTLED);
    let first = images.bitmaps[&1].clone();
    assert_eq!(first.width(), 2048);
    images.sync(&k, &[1, 2]);
    images.wait(SETTLED);
    assert!(
        Arc::ptr_eq(&first, &images.bitmaps[&2]),
        "reuse needs no new pixel/scratch reservation"
    );
    assert_eq!(images.stats().resident_bytes, 2048 * 2048 * 4);
}

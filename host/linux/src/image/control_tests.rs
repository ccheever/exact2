use super::*;
use exact_kernel::Op;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

/// A hang detector, not a speed claim: a wait returns once nothing is
/// pending, and a 2048² decode took over 3 s on this Mac at load 170.
const SETTLED: Duration = Duration::from_secs(60);
// These fixtures must occupy both process workers before releasing either.
// Running them together can leave each holding one while waiting for the other.
static BOTH_WORKERS: Mutex<()> = Mutex::new(());

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

#[test]
fn boot_metadata_waits_for_layout_size_before_decoding() {
    let mut k = kernel(2);
    source(&mut k, 2, "symbol:checkmark");
    let mut images = Images::with_assets(Assets::selected(
        PathBuf::new(),
        Arc::new(|_| Ok(Some(encoded(512, 512, [40, 100, 200, 255])))),
    ));
    images.prepare_boot(&k, &k.roots(), 2.);
    images.poll();
    assert!(
        images.views[&1].source_id.is_none(),
        "binding starts no I/O"
    );
    assert!(images.views[&1].request.is_none());
    assert!(images.bitmaps.is_empty());
    assert!(
        !images.views.contains_key(&2),
        "symbols still owe their intrinsic size"
    );
    boxed(&mut k, 1, 32., 32., exact_kernel::ObjectFit::Cover);
    images.enable_decode();
    let reports = images.sync_visible(&k, &k.roots(), 2., |_| true);
    assert!(reports.iter().any(|(id, size)| *id == 2 && size.is_some()));
    until(|| {
        images.poll();
        images.bitmaps.contains_key(&1)
    });
    let picture = &images.bitmaps[&1];
    assert!(
        picture.width() < 512 && picture.height() < 512,
        "layout must size the decode"
    );
    assert_eq!(picture.natural(), (512, 512));
}

#[cfg(target_os = "macos")]
#[test]
fn painted_boot_requests_no_images_below_the_first_screen() {
    use crate::{
        host::PlanBytes,
        presenter::{PainterBoot, PainterChoice, Presenter},
    };
    let mut source =
        String::from("component App\n  view\n    column width=100 height=2600\n      box height=200 flex-shrink=0\n");
    for i in 0..60 {
        source.push_str(&format!(
            "      image \"offscreen-{i}.png\" width=40 height=40 flex-shrink=0\n"
        ));
    }
    let bytes = contract::compile(&source).unwrap().encode();
    let decoded = std::cell::Cell::new(0);
    let ready = |_: &exact_plan::Plan| decoded.set(decoded.get() + 1);
    let (p, error) = Presenter::boot_selected(
        PlanBytes::Painted(&bytes, &ready),
        (),
        (100., 100.),
        1.,
        ".".into(),
        None,
        ("{}", exact_runner::Delivery::default()),
        "/",
        None,
        PainterBoot::selected(PainterChoice::Cpu),
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(decoded.get(), 1);
    assert_eq!(p.images().views.len(), 60);
    for id in p.images().views.keys() {
        let frame = p.host().kernel().node(*id).unwrap().frame;
        assert!(
            frame.y >= 200.,
            "off-screen fixture was laid out at {frame:?}"
        );
    }
    assert!(p
        .images()
        .views
        .values()
        .all(|v| !v.visible && v.source_id.is_none() && v.request.is_none()));
    assert!(!p.images().pending());
    let (ordinary, error) =
        Presenter::boot_with(&bytes, (), (100., 100.), 1., ".".into(), PainterChoice::Cpu).unwrap();
    assert!(error.is_none());
    assert!(
        ordinary.images().views.values().all(|v| v.visible),
        "ordinary hosts keep their pre-paint mounted-image policy"
    );
}

#[test]
fn a_boot_symbol_has_no_picture_request_to_wait_for() {
    let mut k = kernel(1);
    source(&mut k, 1, "symbol:checkmark");
    let mut images = Images::new(PathBuf::new());
    images.prepare_boot(&k, &k.roots(), 1.);
    images.enable_decode();
    images.sync_visible(&k, &k.roots(), 1., |_| true);
    assert!(images.views[&1].symbol_size.is_some());
    assert!(!images.pending());
}

#[test]
fn a_refused_boot_picture_is_settled_content() {
    let k = kernel(1);
    let mut images = Images::with_assets(Assets::selected(PathBuf::new(), Arc::new(|_| Ok(None))));
    images.prepare_boot(&k, &k.roots(), 1.);
    images.enable_decode();
    images.sync_visible(&k, &k.roots(), 1., |_| true);
    until(|| {
        images.poll();
        images.has_refusals()
    });
    assert!(
        !images.pending(),
        "a terminal refusal must not keep boot alive"
    );
}

#[test]
fn http_images_share_a_download_without_blocking_local_images() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/redirect", listener.local_addr().unwrap());
    let png = encoded(24, 16, [40, 120, 220, 255]);
    let (arrived, requested) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        for request in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(SETTLED)).unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
                assert!(header.len() < 8192);
            }
            let header = String::from_utf8(header).unwrap().to_ascii_lowercase();
            assert!(
                !header.contains("authorization:")
                    && !header.contains("cookie:")
                    && !header.contains("referer:")
            );
            if request == 0 {
                assert!(header.starts_with("get /redirect "));
                stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /photo\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            } else {
                assert!(header.starts_with("get /photo "));
                arrived.send(()).unwrap();
                released.recv_timeout(SETTLED).unwrap();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", png.len()).unwrap();
                stream.write_all(&png).unwrap();
            }
        }
    });
    let mut k = kernel(3);
    source(&mut k, 1, &url);
    source(&mut k, 2, &url);
    let mut images = Images::with_assets(assets());
    images.sync(&k, &k.roots());
    requested.recv_timeout(SETTLED).unwrap();
    until(|| {
        images.poll();
        images.bitmaps.contains_key(&3)
    });
    assert!(!images.bitmaps.contains_key(&1));
    release.send(()).unwrap();
    images.wait(SETTLED);
    server.join().unwrap();
    assert_eq!(
        (images.bitmaps[&1].width(), images.bitmaps[&1].height()),
        (24, 16)
    );
    assert!(Arc::ptr_eq(&images.bitmaps[&1], &images.bitmaps[&2]));
    let path = images.views[&1].source_id.as_ref().unwrap().file().unwrap();
    assert!(path.is_file());
    drop(images);
    // The shared download cache survives a view's lifetime. A later view
    // must reuse it even after the fixture server has closed.
    let mut again = Images::with_assets(assets());
    again.sync(&k, &k.roots());
    again.wait(SETTLED);
    assert!(again.bitmaps.contains_key(&1));
    assert_eq!(
        again.views[&1].source_id.as_ref().unwrap().file(),
        Some(path)
    );
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
    for name in [
        "symbol:sf/airpodsmax",
        "symbol:sf/",
        "symbol:",
        "symbol:unknown",
    ] {
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
    // No longer a symbol's square; then the picture, which the session
    // still holds decoded, in the same sync.
    assert_eq!(
        images.sync(&k, &k.roots()),
        vec![(1, None), (1, Some((17., 11.)))]
    );
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
    // A metadata scan can still own a source after trim drops its last bitmap.
    until(|| images.backend.sources() == 0);
    assert_eq!(images.backend.sources(), 0);
    assert!(images.loaded.is_empty());
    until(|| images.stats().resident_bytes == 0 && images.stats().reserved_bytes == 0);
}

#[test]
fn full_undrained_session_does_not_block_another_sessions_native_workers() {
    let _both = BOTH_WORKERS.lock().unwrap_or_else(|e| e.into_inner());
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
fn raster_demand_follows_the_content_box_fit_and_device_scale() {
    use exact_kernel::{AxisOffer, BoxSizing, Dimension, ObjectFit, Offer, StyleId, StyleProps};
    let mut k = kernel(1);
    let mut style = StyleProps {
        width: Dimension::Points(420.),
        height: Dimension::Points(140.),
        padding_left: Dimension::Points(16.),
        padding_right: Dimension::Points(16.),
        padding_top: Dimension::Points(16.),
        padding_bottom: Dimension::Points(16.),
        ..Default::default()
    };
    style.box_sizing = BoxSizing::BorderBox;
    style.border_width_top = 4.;
    style.border_width_right = 4.;
    style.border_width_bottom = 4.;
    style.border_width_left = 4.;
    style.border_style_top = exact_kernel::BorderStyle::Solid;
    style.border_style_right = exact_kernel::BorderStyle::Solid;
    style.border_style_bottom = exact_kernel::BorderStyle::Solid;
    style.border_style_left = exact_kernel::BorderStyle::Solid;
    style.object_fit = ObjectFit::Contain;
    for id in [
        StyleId::Width,
        StyleId::Height,
        StyleId::PaddingLeft,
        StyleId::PaddingRight,
        StyleId::PaddingTop,
        StyleId::PaddingBottom,
        StyleId::BoxSizing,
        StyleId::BorderWidthTop,
        StyleId::BorderWidthRight,
        StyleId::BorderWidthBottom,
        StyleId::BorderWidthLeft,
        StyleId::BorderStyleTop,
        StyleId::BorderStyleRight,
        StyleId::BorderStyleBottom,
        StyleId::BorderStyleLeft,
        StyleId::ObjectFit,
    ] {
        style.mask.set(id);
    }
    k.apply(
        0,
        2,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }],
    )
    .unwrap();
    k.compute_layout(
        1,
        Offer {
            width: AxisOffer::Definite(420.),
            height: AxisOffer::Definite(140.),
        },
    )
    .unwrap();
    let mut images = Images::with_assets(assets());
    images.sync_visible(&k, &k.roots(), 2., |_| true);
    let view = images.views.get_mut(&1).unwrap();
    assert_eq!(view.desired, (760., 200.));
    let header = png_decode::Header::jpeg(exact_raster::Metadata {
        natural: PixelSize {
            width: 1600,
            height: 900,
        },
        encoded_bytes: 1000,
        header_bytes: 100,
    });
    for (fit, longest) in [
        (ObjectFit::Contain, 384),
        (ObjectFit::Cover, 768),
        (ObjectFit::Fill, 768),
        (ObjectFit::None, 1600),
        (ObjectFit::ScaleDown, 384),
    ] {
        view.object_fit = fit;
        let plan = plan_view(header, view, 2., SESSION_BYTES).unwrap();
        assert_eq!(plan.pixels.width, longest, "{fit:?}");
    }
    view.object_fit = ObjectFit::Cover;
    view.requested_for = view.desired;
    view.requested_fit = view.object_fit;
    view.requested_scale = 2.;
    view.desired = (764., 200.);
    assert!(
        !resize_changes_view(header, view, 2.),
        "a small resize keeps its size class"
    );
    view.desired = view.requested_for;
    view.requested_fit = ObjectFit::Contain;
    assert!(
        resize_changes_view(header, view, 2.),
        "cover replaces the smaller contain raster"
    );
    view.requested_fit = ObjectFit::Cover;
    view.desired = (1100., 200.);
    assert!(resize_changes_view(header, view, 2.));
    assert_eq!(
        drawn_size(header, (4000., 4000.), ObjectFit::ScaleDown, 2.),
        (3200., 1800.)
    );
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
    let _both = BOTH_WORKERS.lock().unwrap_or_else(|e| e.into_inner());
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
            .source(images.generation, &n.to_string(), &images.assets, false)
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
    until(|| images.backend.sources() == 0);
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

/// A second view of a picture the session holds decoded has it in the sync
/// that first sees the view: nothing is waited for, and no later poll is
/// needed (on a list, the paint a row comes into view in).
#[test]
fn a_cached_picture_is_taken_in_the_poll_that_asks_for_it() {
    let mut k = kernel(1);
    let mut images = Images::with_assets(assets());
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    let first = images.bitmaps[&1].clone();
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
    let reports = images.sync(&k, &k.roots());
    assert!(
        images
            .bitmaps
            .get(&2)
            .is_some_and(|b| Arc::ptr_eq(b, &first)),
        "the cached picture, in the same sync"
    );
    assert!(reports
        .iter()
        .any(|(id, natural)| *id == 2 && natural.is_some()));
}

fn photo_assets() -> Assets {
    Assets::selected(
        PathBuf::new(),
        Arc::new(|_| Ok(Some(encoded(400, 200, [9, 100, 70, 255])))),
    )
}

/// Image `id` in a `width`×`height` box, laid out.
fn boxed(k: &mut Kernel, id: u32, width: f32, height: f32, fit: exact_kernel::ObjectFit) {
    use exact_kernel::{AxisOffer, Dimension, Offer, StyleId, StyleProps};
    let mut style = StyleProps {
        width: Dimension::Points(width),
        height: Dimension::Points(height),
        ..Default::default()
    };
    // This row is behind DerefMut; it cannot be a StyleProps initializer.
    style.object_fit = fit;
    for row in [StyleId::Width, StyleId::Height, StyleId::ObjectFit] {
        style.mask.set(row);
    }
    let epoch = k.epoch();
    k.apply(
        0,
        epoch + 1,
        &[Op::SetStyle {
            id,
            patch: Box::new(style),
        }],
    )
    .unwrap();
    let offer = Offer {
        width: AxisOffer::Definite(1000.0),
        height: AxisOffer::Definite(1000.0),
    };
    k.compute_layout(id, offer).unwrap();
}

/// A picture a box clips to itself is decoded as the part the box shows.
/// Two boxes of different shapes over one source hold two parts, never each
/// other's, and a box of the first's shape shares the first's.
#[test]
fn a_covering_box_holds_the_part_it_shows() {
    use exact_kernel::ObjectFit;
    let mut k = kernel(3);
    // 400×200 in a 100×200 box: the middle quarter of its width.
    boxed(&mut k, 1, 100.0, 200.0, ObjectFit::Cover);
    boxed(&mut k, 2, 200.0, 200.0, ObjectFit::Cover);
    boxed(&mut k, 3, 100.0, 200.0, ObjectFit::Cover);
    for id in 2..=3 {
        source(&mut k, id, "1");
    }
    let mut images = Images::with_assets(photo_assets());
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    let (tall, square, twin) = (
        &images.bitmaps[&1],
        &images.bitmaps[&2],
        &images.bitmaps[&3],
    );
    let part = tall.crop();
    assert_eq!((part.full.width, part.full.height), (400, 200));
    assert_eq!(
        (part.x, part.y, tall.width(), tall.height()),
        (148, 0, 104, 200)
    );
    assert_eq!(
        tall.natural(),
        (400, 200),
        "layout sees the picture's own size"
    );
    assert_eq!(
        (square.crop().x, square.width(), square.height()),
        (98, 204, 200)
    );
    assert!(
        Arc::ptr_eq(tall, twin),
        "one part for two boxes of one shape"
    );
    assert!(!Arc::ptr_eq(tall, square));
    // Where a painter draws the part: the whole goes to (-150, 0, 400, 200)
    // in the 100×200 box, and the part's pixels to their place in that.
    assert_eq!(
        tall.placed((-150.0, 0.0, 400.0, 200.0)),
        (-2.0, 0.0, 104.0, 200.0)
    );
    let stats = images.stats();
    assert_eq!(stats.resident_bytes, (104 * 200 + 204 * 200) * 4);
}

/// A picture that is contained is decoded whole, and a box that would clip
/// it shares the whole one the session already holds: no second decode.
#[test]
fn a_contained_picture_is_whole_and_shared() {
    use exact_kernel::ObjectFit;
    let mut k = kernel(1);
    boxed(&mut k, 1, 400.0, 200.0, ObjectFit::Contain);
    let mut images = Images::with_assets(photo_assets());
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    let whole = images.bitmaps[&1].clone();
    assert!(whole.crop().whole());
    assert_eq!(
        (whole.width(), whole.height(), whole.natural()),
        (400, 200, (400, 200))
    );
    assert_eq!(whole.placed((1.0, 2.0, 3.0, 4.0)), (1.0, 2.0, 3.0, 4.0));
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
    boxed(&mut k, 2, 100.0, 200.0, ObjectFit::Cover);
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    assert!(Arc::ptr_eq(&whole, &images.bitmaps[&2]));
    assert_eq!(images.stats().resident_bytes, 400 * 200 * 4);
}

/// A box that changes size under a part gets the whole picture, and keeps
/// it: a box that animates does not ask for a part a frame.
#[test]
fn a_box_that_changes_size_gets_the_whole_picture() {
    use exact_kernel::ObjectFit;
    let mut k = kernel(1);
    boxed(&mut k, 1, 100.0, 200.0, ObjectFit::Cover);
    let mut images = Images::with_assets(photo_assets());
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    assert!(!images.bitmaps[&1].crop().whole());
    boxed(&mut k, 1, 110.0, 200.0, ObjectFit::Cover);
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    assert!(images.bitmaps[&1].crop().whole());
    assert_eq!(images.bitmaps[&1].width(), 400);
    boxed(&mut k, 1, 100.0, 200.0, ObjectFit::Cover);
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    assert!(
        images.bitmaps[&1].crop().whole(),
        "back at the first size, still the whole"
    );
}

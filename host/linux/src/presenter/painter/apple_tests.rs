use super::*;
use exact_ink::device::apple::Startup;
use std::sync::{mpsc, Arc};
use std::time::Duration;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "exact-metal-startup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn archive_miss_starts_with_cpu_pixels_then_replaces_the_whole_frame_with_ink() {
    let directory = Directory::new();
    let archive = directory.0.join("wrong-device/archive.bin");
    let (finished, ready) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::channel();
    let preparing = match Startup::with_archive_gated(
        archive.clone(),
        Arc::new(move || {
            let _ = finished.send(());
        }),
        gate,
    ) {
        Ok(preparing) => preparing,
        Err(error) => {
            assert!(std::env::var_os("EXACT_REQUIRED").is_none(), "{error}");
            eprintln!("Metal unavailable: {error}");
            return;
        }
    };
    let plan = contract::compile(
        r##"component App
  view
    column width=160 height=120 background-color="#23394f" padding=8
      text "Archive startup" color="#f3e7ca" font-size=18
      box width=75 height=35 background-color="#e83a5b" border-radius=7
"##,
    )
    .unwrap()
    .encode();
    let make = || {
        let (p, warning) =
            Presenter::boot_with(&plan, (), (160., 120.), 1., ".".into(), PainterChoice::Cpu)
                .unwrap();
        assert!(warning.is_none(), "{warning:?}");
        p
    };
    let expected = make().frame();
    let mut presenter = make();
    presenter.install_apple_painter(preparing).unwrap();
    assert_eq!(presenter.painter.name, "cpu");
    assert_eq!(
        presenter.painter_state()["device"],
        "CPU startup; Metal pipelines pending"
    );
    assert!(presenter.painter_state()["name"].is_null());
    assert!(!presenter
        .surfaces
        .apple_boot
        .as_ref()
        .unwrap()
        .preparing
        .as_ref()
        .unwrap()
        .ready());
    let prepared = presenter.prepare_apple_window_frame().unwrap().unwrap();
    assert_eq!(prepared.0.unwrap().data(), expected.data());
    assert!(
        presenter.painter_state()["name"].is_null(),
        "preparing is not presenting"
    );
    let first = presenter
        .apple_frame_for(crate::paint::FrameTarget::Pixels)
        .unwrap()
        .unwrap();
    assert_eq!(presenter.painter.name, "cpu");
    assert_eq!(first.data(), expected.data());
    assert_eq!(presenter.painter_state()["name"], "cpu");
    assert!(
        !presenter.dirty(),
        "compilation must not cause a repaint loop"
    );
    release.send(()).unwrap();
    ready.recv_timeout(Duration::from_secs(30)).unwrap();
    assert!(presenter
        .surfaces
        .apple_boot
        .as_ref()
        .unwrap()
        .preparing
        .as_ref()
        .unwrap()
        .ready());
    assert!(
        presenter
            .surfaces
            .apple_boot
            .as_ref()
            .unwrap()
            .preparing
            .as_ref()
            .unwrap()
            .ready(),
        "readiness does not consume the result"
    );
    presenter.poll_apple_painter().unwrap();
    assert_eq!(presenter.painter_state()["name"], "cpu");
    assert_eq!(presenter.painter_state()["installed"], "ink");
    let later = presenter
        .apple_frame_for(crate::paint::FrameTarget::Pixels)
        .unwrap()
        .unwrap();
    assert_eq!(presenter.painter.name, "ink");
    assert!(presenter.last_frame_succeeded());
    assert_eq!(presenter.painter_state()["name"], "ink");
    let errors: Vec<_> = first
        .data()
        .chunks_exact(4)
        .zip(later.data().chunks_exact(4))
        .map(|(a, b)| {
            a.iter()
                .zip(b)
                .map(|(a, b)| a.abs_diff(*b) as u64)
                .collect::<Vec<_>>()
        })
        .collect();
    let mean = errors.iter().flatten().sum::<u64>() as f64 / first.data().len() as f64;
    let over = 100. * errors.iter().filter(|p| p.iter().any(|e| *e > 32)).count() as f64
        / errors.len() as f64;
    eprintln!("Metal archive transition: mean {mean:.3}/255, {over:.3}% beyond 32");
    assert!(mean < 5. && over < 6., "D9: mean {mean}, outliers {over}%");
    assert!(
        archive.is_file(),
        "the worker must serialize the populated archive"
    );
    let warm = Startup::with_archive(
        archive,
        Arc::new(|| panic!("a retained archive must not compile")),
    )
    .unwrap();
    let mut retained = make();
    retained.install_apple_painter(warm).unwrap();
    assert_eq!(retained.painter.name, "ink");
    assert!(retained.painter.cached);

    // The other ordering is a real miss whose compile finishes before installation.
    let (finished, ready) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let fast = Startup::with_archive_gated(
        directory.0.join("fast.bin"),
        Arc::new(move || {
            let _ = finished.send(());
        }),
        gate,
    )
    .unwrap();
    release.send(()).unwrap();
    ready.recv_timeout(Duration::from_secs(30)).unwrap();
    let mut fast_presenter = make();
    fast_presenter.install_apple_painter(fast).unwrap();
    assert_eq!(
        fast_presenter.painter.name, "ink",
        "a completed compile needs no CPU frame"
    );
    assert_eq!(
        fast_presenter
            .apple_frame_for(crate::paint::FrameTarget::Pixels)
            .unwrap()
            .unwrap()
            .data(),
        later.data()
    );

    let (finished, ready) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let refused = Startup::with_archive_gated(
        directory.0.join("refused.bin"),
        Arc::new(move || {
            let _ = finished.send(());
        }),
        gate,
    )
    .unwrap();
    let mut failed = make();
    failed.install_apple_painter(refused).unwrap();
    // Refusing submission must keep damage, even when rasterization succeeded.
    failed.apple_window_frame().unwrap();
    assert!(!failed.last_frame_succeeded());
    assert!(failed.dirty());
    drop(release);
    ready.recv_timeout(Duration::from_secs(30)).unwrap();
    failed.poll_apple_painter().unwrap();
    assert!(failed.painter_state()["lastRefusal"]
        .as_str()
        .unwrap()
        .contains("pipeline worker cancelled"));
    assert_eq!(
        failed.painter_state()["device"],
        "CPU startup; Metal pipelines failed"
    );
    assert_eq!(
        failed
            .apple_frame_for(crate::paint::FrameTarget::Pixels)
            .unwrap()
            .unwrap()
            .data(),
        expected.data()
    );
    failed.surfaces.apple_boot.as_mut().unwrap().refused(
        exact_ink::device::Error::DeviceLost("injected failure".into()),
        std::time::Instant::now(),
    );
    failed.poll_apple_painter().unwrap();
    assert!(failed.painter_state()["recovering"].as_bool().unwrap());
    assert!(failed.dirty());
    assert!(failed
        .apple_frame_for(crate::paint::FrameTarget::Pixels)
        .unwrap()
        .is_none());
    assert!(
        failed.dirty(),
        "device backoff must preserve the scene's damage"
    );
}

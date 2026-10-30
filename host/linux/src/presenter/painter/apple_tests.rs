use super::*;
use exact_ink::device::apple::Startup;
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[test]
fn archive_miss_starts_with_cpu_pixels_then_replaces_the_whole_frame_with_ink() {
    let directory =
        std::env::temp_dir().join(format!("exact-metal-startup-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let archive = directory.join("wrong-device/archive.bin");
    let (finished, ready) = mpsc::sync_channel(1);
    let preparing = match Startup::with_archive(
        archive.clone(),
        Arc::new(move || {
            let _ = finished.send(());
        }),
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
    column width=160 height=120 background="#23394f" padding=8
      text "Archive startup" color="#f3e7ca" font-size=18
      box width=75 height=35 background="#e83a5b" border-radius=7
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
    // Even a compiler that finishes before layout must not consume the first frame.
    ready.recv_timeout(Duration::from_secs(30)).unwrap();
    let first = presenter
        .apple_frame_for(crate::paint::FrameTarget::Pixels)
        .unwrap()
        .unwrap();
    assert_eq!(presenter.painter.name, "cpu");
    assert_eq!(first.data(), expected.data());
    let later = presenter
        .apple_frame_for(crate::paint::FrameTarget::Pixels)
        .unwrap()
        .unwrap();
    assert_eq!(presenter.painter.name, "ink");
    assert!(presenter.last_frame_succeeded());
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
    std::fs::remove_dir_all(directory).unwrap();
}

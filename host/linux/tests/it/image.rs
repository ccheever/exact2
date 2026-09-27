//! Natural-size semantics and allocation lifetime across native consumers.
use exact_kernel::ObjectFit;
use exact_linux::paint::object_fit;

#[test]
fn sampled_raster_does_not_change_object_fit_natural_geometry() {
    // A 100x50 sampled bitmap is still a 4000x2000 natural image.
    let natural = (4000, 2000);
    let content = (10., 20., 200., 200.);
    assert_eq!(
        object_fit(natural, ObjectFit::None, content),
        Some((-1890., -880., 4000., 2000.))
    );
    assert_eq!(
        object_fit(natural, ObjectFit::ScaleDown, content),
        Some((10., 70., 200., 100.))
    );
    assert_eq!(
        object_fit(natural, ObjectFit::Contain, content),
        Some((10., 70., 200., 100.))
    );
    assert_eq!(
        object_fit(natural, ObjectFit::Cover, content),
        Some((-90., 20., 400., 200.))
    );
    assert_eq!(object_fit(natural, ObjectFit::Fill, content), Some(content));
    assert_eq!(
        object_fit(natural, ObjectFit::ScaleDown, (0., 0., 8000., 8000.)),
        Some((2000., 3000., 4000., 2000.))
    );
}

#[test]
fn sampled_pixels_keep_natural_auto_axes_and_min_max_layout() {
    use exact_linux::{presenter::PainterChoice, Presenter};
    use exact_runner::{DataError, DataSource, Value};
    use std::io::Write;
    use std::time::Duration;
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }
    let dir = std::env::temp_dir().join(format!("exact-natural-layout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut encoder = png::Encoder::new(
        std::fs::File::create(dir.join("large.png")).unwrap(),
        4000,
        2000,
    );
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header().unwrap();
    {
        let mut stream = writer.stream_writer().unwrap();
        let row: Vec<u8> = (0..4000).flat_map(|_| [21, 80, 130, 255]).collect();
        for _ in 0..2000 {
            stream.write_all(&row).unwrap();
        }
        stream.finish().unwrap();
    }
    writer.finish().unwrap();
    for (attrs, expected) in [
        ("width=100", (100., 50.)),
        ("height=90", (180., 90.)),
        ("max-width=300", (300., 150.)),
        ("width=100 min-width=180 max-width=250", (180., 90.)),
    ] {
        // A non-stretching flex item tests the image's own natural axes;
        // LLP1011 declares bare block auto-width stretching separately.
        let plan = contract::compile(&format!(
            "component App\n  view\n    column align-items=\"flex-start\"\n      image \"large.png\" {attrs} testId=\"photo\"\n"
        ))
        .unwrap();
        let (mut presenter, error) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (800., 600.),
            1.,
            dir.clone(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        presenter.wait_images(Duration::from_secs(3));
        let kernel = presenter.host().kernel();
        let key = kernel.find_by_test_id("photo")[0];
        let node = kernel.node_by_key(key).unwrap();
        assert_eq!((node.frame.width, node.frame.height), expected, "{attrs}");
        let image = &presenter.images().bitmaps[&node.id];
        assert_eq!(image.natural(), (4000, 2000));
        assert!(
            image.width() < 4000,
            "the test must actually use sampled pixels"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn raster_tint_preserves_alpha_background_and_border_in_both_painters() {
    use exact_linux::{presenter::PainterChoice, Presenter};
    use exact_runner::{DataError, DataSource, Value};
    use std::time::Duration;
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }
    let dir = std::env::temp_dir().join(format!("exact-raster-tint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut pixels = tiny_skia::Pixmap::new(12, 4).unwrap();
    for row in pixels.data_mut().chunks_exact_mut(12 * 4) {
        for (i, pixel) in row.chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(match i / 4 {
                0 => &[0, 0, 255, 255],
                1 => &[0, 0, 128, 128],
                _ => &[0, 0, 0, 0],
            });
        }
    }
    pixels.save_png(dir.join("alpha.png")).unwrap();
    let plan = contract::compile(
        r##"component App
  state dark = false
  action flip writes dark
    dark = not dark
    setScheme(dark ? "light" : "dark")
  view
    column align-items="flex-start"
      image "alpha.png" testId="mark" width=12 height=4 padding=4 border-width=2 border-style="solid" border-color="#00ff00" background-color="#ffffff" tint-color="light-dark(#ff0000, #00000080)"
      button "Flip" testId="flip" press=flip
"##,
    ).unwrap();
    let mut choices = vec![PainterChoice::Cpu];
    if exact_linux::gpu::Gpu::new().is_ok() {
        choices.push(PainterChoice::Gpu);
    }
    let mut failures = Vec::new();
    for choice in choices {
        let (mut p, error) =
            Presenter::boot_with(&plan.encode(), NoData, (80., 60.), 1., dir.clone(), choice)
                .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.wait_images(Duration::from_secs(3));
        for (dark, expected) in [
            (false, [[255, 0, 0], [255, 127, 127]]),
            (true, [[127, 127, 127], [191, 191, 191]]),
        ] {
            if dark {
                let k = p.host().kernel();
                let id = k.node_by_key(k.find_by_test_id("flip")[0]).unwrap().id;
                p.tap(id).unwrap();
                p.run_commands(|| NoData);
            }
            let frame = p.frame();
            for ((x, y), expected) in [
                ((0, 7), [0, 255, 0]),
                ((3, 7), [255; 3]),
                ((7, 7), expected[0]),
                ((11, 7), expected[1]),
                ((15, 7), [255; 3]),
            ] {
                let pixel = frame.pixel(x, y).unwrap().demultiply();
                let actual = [pixel.red(), pixel.green(), pixel.blue()];
                if actual
                    .into_iter()
                    .zip(expected)
                    .any(|(a, b)| a.abs_diff(b) > 1)
                {
                    failures.push(format!(
                        "{choice:?} dark={dark} ({x},{y}): {actual:?} != {expected:?}"
                    ));
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

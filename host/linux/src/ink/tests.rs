use super::*;
use crate::raster::Raster;
use exact_kernel::gradient::Geometry;

fn open() -> Option<Ink> {
    match Ink::new() {
        Ok(ink) => Some(ink),
        Err(e) if e.starts_with("no adapter:") => {
            eprintln!("Ink unavailable: {e}");
            None
        }
        Err(e) => panic!("ink device: {e}"),
    }
}

fn compare(a: &Pixmap, b: &Pixmap) {
    let mut sum = 0u64;
    let mut over = 0;
    for (a, b) in a.data().chunks_exact(4).zip(b.data().chunks_exact(4)) {
        let differences: Vec<_> = a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).collect();
        sum += differences.iter().map(|d| *d as u64).sum::<u64>();
        over += usize::from(differences.iter().any(|d| *d > 32));
    }
    let mean = sum as f64 / a.data().len() as f64;
    let percent = over as f64 * 100. / (a.width() * a.height()) as f64;
    assert!(
        mean < 5. && percent < 6.,
        "mean {mean:.3}/255, {percent:.3}% beyond 32"
    );
}

#[test]
fn geometry_survives_nested_layers_transforms_and_resizes() {
    let Some(mut ink) = open() else {
        return;
    };
    let mut cpu = Raster::new();
    for scale in [1., 1.5, 2.] {
        for transform in [
            Transform::identity(),
            Transform::from_row(0.9, 0.2, -0.15, 0.85, 15., 2.),
        ] {
            for backend in [&mut cpu as &mut dyn Backend, &mut ink] {
                backend.page([19, 35, 62, 181]);
                backend.begin(120., 100., scale);
                let outer = Shape::new((3.25, 2.5, 109., 90.), [17., 8., 23., 11.]);
                backend.fill(&outer, [12, 96, 233, 197], transform);
                for delta in [0., 3., 6.] {
                    backend.push_clip(
                        &Shape::new((6. + delta, 7. + delta, 95. - delta, 80. - delta), [15.; 4]),
                        Transform::identity(),
                    );
                }
                backend.push_opacity(0.63);
                backend.push_clip(
                    &Shape::elliptical((11.5, 10.25, 92., 70.), [(25., 17.); 4]),
                    transform,
                );
                let shape = Shape::elliptical((8., 9., 97., 73.), [(27., 18.); 4]);
                backend.fill_gradient(
                    &shape,
                    &GradientPaint {
                        geometry: Geometry::Linear {
                            start: (10., 10.),
                            end: (100., 80.),
                        },
                        repeat: false,
                        stops: vec![
                            (0., exact_kernel::Color::rgba(255, 0, 0, 200)),
                            (1., exact_kernel::Color::rgba(0, 200, 255, 50)),
                        ],
                    },
                    transform,
                );
                backend.fill(
                    &Shape::new((25., 20., 65., 50.), [12.; 4]),
                    [220, 50, 40, 173],
                    transform,
                );
                backend.pop_clip();
                backend.pop_opacity();
                for _ in 0..3 {
                    backend.pop_clip();
                }
                for part in crate::paint::border::border_fills(
                    &outer,
                    [3., 5., 7., 4.],
                    [[240, 65, 90, 191]; 4],
                ) {
                    backend.fill_border(&part, transform);
                }
                backend.pointer(90., 65.);
                backend.push_clip(&Shape::rect((30., 10., -5., 40.)), Transform::identity());
                backend.fill(
                    &Shape::rect((0., 0., 120., 100.)),
                    [255, 0, 255, 255],
                    Transform::identity(),
                );
                backend.pop_clip();
            }
            compare(&cpu.finish().unwrap(), &ink.finish().unwrap());
        }
    }
}

#[test]
fn image_revisions_and_mips_preserve_alpha_and_group_opacity() {
    let Some(mut ink) = open() else {
        return;
    };
    let mut cpu = Raster::new();
    let photo = Pixmap::decode_png(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/fixtures/picker/photo.png"
    )))
    .unwrap();
    let natural = (photo.width(), photo.height());
    let bytes = photo.data().len() as u64;
    let gate = exact_raster::Gate::new();
    let session = gate.session();
    let reservation = session.reserve_allocation(bytes).unwrap();
    let photo = Arc::new(Bitmap::new(photo, natural, reservation.charge()));
    reservation.commit(bytes).unwrap();
    for (width, height) in [(80., 53.), (43., 29.), (17., 11.)] {
        for backend in [&mut cpu as &mut dyn Backend, &mut ink] {
            backend.begin(width, height, 1.);
            backend.image(
                &photo,
                (0., 0., width, height),
                &[],
                Transform::identity(),
                None,
            );
        }
        compare(&cpu.finish().unwrap(), &ink.finish().unwrap());
    }
    for revision in 0..2 {
        let mut pixels = Pixmap::new(256, 128).unwrap();
        for (n, p) in pixels.pixels_mut().iter_mut().enumerate() {
            *p = tiny_skia::Color::from_rgba8(
                (n % 256) as u8,
                (n / 256 * 2) as u8,
                revision * 127,
                173,
            )
            .premultiply()
            .to_color_u8();
        }
        let pixels = Arc::new(pixels);
        for _ in 0..2 {
            for backend in [&mut cpu as &mut dyn Backend, &mut ink] {
                backend.begin(90., 70., 1.);
                backend.push_opacity(0.7);
                backend.canvas(
                    &pixels,
                    (7., 11., 64., 32.),
                    &[Shape::new((7., 11., 64., 32.), [9.; 4])],
                    Transform::identity(),
                );
                backend.pop_opacity();
                backend.island_image(
                    pixels.clone(),
                    (4., 50., 32., 16.),
                    Transform::identity(),
                    0,
                );
            }
            compare(&cpu.finish().unwrap(), &ink.finish().unwrap());
        }
    }
}

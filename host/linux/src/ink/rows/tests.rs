use super::*;
use crate::{gpu::Gpu, raster::Raster};

fn scene(b: &mut dyn Backend, origin: (f32, f32), picture: bool) {
    let ts = Transform::from_translate(origin.0, origin.1);
    b.push_clip(&Shape::new((-6., -5., 58., 49.), [11.; 4]), ts);
    b.push_opacity(0.72);
    b.fill(
        &Shape::elliptical((-4., -3., 54., 45.), [(17., 9.); 4]),
        [40, 170, 230, 255],
        ts,
    );
    b.push_clip(&Shape::new((4., 2., 39., 34.), [9.; 4]), ts);
    b.slot_begin(7);
    if picture {
        slot(b, ts);
    }
    b.slot_end();
    b.fill(&Shape::rect((20., 21., 20., 10.)), [220, 30, 80, 190], ts);
    b.pop_clip();
    b.pop_opacity();
    b.pop_clip();
}

fn slot(b: &mut dyn Backend, ts: Transform) {
    let mut pixels = Pixmap::new(2, 2).unwrap();
    pixels.fill(tiny_skia::Color::from_rgba8(220, 200, 15, 255));
    b.canvas(&Arc::new(pixels), (-10., -10., 80., 80.), &[], ts);
}

fn outer(b: &mut dyn Backend, n: usize) {
    b.push_clip(
        &Shape::rect((12. + n as f32 * 7., 8., 58., 60.)),
        Transform::identity(),
    );
    for offset in [0., 3.] {
        b.push_clip(
            &Shape::new((9. + offset, 7. + offset, 65., 65.), [13.; 4]),
            Transform::identity(),
        );
    }
}

#[test]
fn kept_rows_move_reclip_and_replace_a_slot_without_recording_the_row() {
    let mut backends: Vec<Box<dyn Backend>> =
        vec![Box::new(Ink::new().unwrap()), Box::new(Gpu::new().unwrap())];
    for b in &mut backends {
        for scale in [1., 2.] {
            b.begin(90., 85., scale);
            outer(b.as_mut(), 0);
            b.row_begin(1, (13., 9.), None);
            scene(b.as_mut(), (13., 9.), false);
            assert_eq!(b.row_end((-6., -5., 58., 49.)), 1);
            for _ in 0..3 {
                b.pop_clip();
            }
            for (n, origin) in [(0, (23., 25.)), (1, (9., 11.)), (2, (28., 31.))] {
                b.begin(90., 85., scale);
                outer(b.as_mut(), n);
                if n == 2 {
                    assert!(b.slot_again(1, 7));
                    slot(b.as_mut(), Transform::from_translate(13., 9.));
                    b.slot_end();
                }
                assert!(b.row_culled((200., 0., 20., 20.)));
                assert!(!b.row_culled((20., 20., 20., 20.)));
                b.row_draw(1, origin, (origin.0 - 6., origin.1 - 5., 58., 49.));
                for _ in 0..3 {
                    b.pop_clip();
                }
                let actual = b.finish().unwrap();
                let mut cpu = Raster::new();
                cpu.begin(90., 85., scale);
                outer(&mut cpu, n);
                scene(&mut cpu, origin, n == 2);
                for _ in 0..3 {
                    cpu.pop_clip();
                }
                let expected = cpu.finish().unwrap();
                let mean = actual
                    .data()
                    .iter()
                    .zip(expected.data())
                    .map(|(a, b)| a.abs_diff(*b) as f64)
                    .sum::<f64>()
                    / actual.data().len() as f64;
                assert!(
                    mean < 3.,
                    "{} scale {scale} step {n}: mean {mean}",
                    b.name()
                );
            }
            b.row_free(1);
            assert!(!b.slot_again(1, 7));
        }
    }
}

use exact_game::*;
use placement_fixture_logic::{Options, SmallGame};
#[test]
fn sign_name_button_and_lamp_save_together() {
    let mut sim = Sim::<SmallGame>::new(Options::default()).unwrap();
    sim.run(1000.);
    let w = sim.world();
    assert_eq!(w.query::<&Placed>().iter().count(), 3);
    assert_eq!(w.require::<Placed>("sign").facing, Facing::Fixed);
    assert_eq!(w.require::<Placed>("name").child, 2);
}

#[test]
fn proof_endpoint_pin() {
    let mut sim = Sim::<SmallGame>::new(Options::default()).unwrap();
    sim.bind(&[Value::Number(1.0), Value::Number(0.0)], None)
        .unwrap();
    sim.run(1000.);
    sim.run(4500.);
    assert_eq!(sim.world().tick(), 330);
    sim.assert_pin(include_str!("../../pins.json"));
}

#[test]
fn placed_sign_projection_receipt_at_1280_by_720() {
    for (ms, expected) in [
        (0., [430.91, 299.76, 140.23, 30.55]),
        (983.334, [492.88443, 285.10403, 93.112885, 38.104492]),
        (1000., [494.20934, 284.9197, 92.1972, 38.18921]),
    ] {
        let mut sim = Sim::<SmallGame>::new(Options::default()).unwrap();
        sim.run(ms);
        let w = sim.world();
        let pose = *w.require::<Transform>("camera");
        let view = Mat4::from_rotation_translation(pose.rotation, pose.position).inverse();
        let size = Vec2::new(1280., 720.);
        let p = w.require::<Placed>("sign").project(
            *w.require::<Transform>("sign"),
            Vec2::new(224., 50.),
            view,
            w.require::<Camera>("camera").matrix(size),
            size,
        );
        let h = p.homography;
        let corners = [(0., 0.), (224., 0.), (224., 50.), (0., 50.)].map(|(x, y)| {
            [
                (h[0] * x + h[1] * y + h[2]) / (h[6] * x + h[7] * y + h[8]),
                (h[3] * x + h[4] * y + h[5]) / (h[6] * x + h[7] * y + h[8]),
            ]
        });
        let lo = corners
            .iter()
            .fold([f32::INFINITY; 2], |a, b| [a[0].min(b[0]), a[1].min(b[1])]);
        let hi = corners.iter().fold([f32::NEG_INFINITY; 2], |a, b| {
            [a[0].max(b[0]), a[1].max(b[1])]
        });
        let bounds = [lo[0], lo[1], hi[0] - lo[0], hi[1] - lo[1]];
        println!("projected tick {}: {bounds:?}", w.tick());
        {
            for (a, b) in bounds.into_iter().zip(expected) {
                assert!((a - b).abs() < 0.01);
            }
        }
    }
}

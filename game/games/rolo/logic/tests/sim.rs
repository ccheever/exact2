use exact_game::{Quat, Sim, Transform, Value, Vec3};
use rolo_logic::{Options, Rolo};

#[test]
fn rolo_rolls_belly_up_and_returns_to_his_starting_pose() {
    let mut sim = Sim::<Rolo>::new(Options::default()).unwrap();
    let start = *sim.get::<Transform>("rolo").unwrap();
    sim.run(4500.0);
    let upside_down = *sim.get::<Transform>("rolo").unwrap();
    assert!((upside_down.rotation * Vec3::Y).y < -0.8);
    sim.run(5500.0);
    let end = *sim.get::<Transform>("rolo").unwrap();
    assert!(start.position.distance(end.position) < 0.002);
    assert!((end.rotation * Vec3::Y).distance(Vec3::Y) < 0.002);
    assert!((end.rotation * Vec3::X).distance(Vec3::X) < 0.002);
}

#[test]
fn pause_freezes_the_rig_and_restart_returns_to_the_logo_pose() {
    let mut sim = Sim::<Rolo>::new(Options::default()).unwrap();
    sim.run(4100.0);
    sim.bind(&[Value::Bool(true), Value::Number(0.0)], None)
        .unwrap();
    let frozen = *sim.get::<Transform>("rolo").unwrap();
    sim.run(3000.0);
    assert_eq!(*sim.get::<Transform>("rolo").unwrap(), frozen);
    sim.bind(&[Value::Bool(false), Value::Number(1.0)], None)
        .unwrap();
    let restart = *sim.get::<Transform>("rolo").unwrap();
    assert!(restart.rotation.abs_diff_eq(Quat::IDENTITY, 0.002));
}

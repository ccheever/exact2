use super::*;
use crate::asset::{Node, Skin};

#[test]
fn first_sample_is_current_current() {
    let mut w = world();
    let e = w.spawn((Mesh::asset("rig.model"), Animation::play("slow")));
    w.step_clock();
    assert_eq!(
        w.get::<Pose>(e).unwrap().previous,
        w.get::<Pose>(e).unwrap().local
    );
}
#[test]
fn failed_sample_preserves_history() {
    let mut w = world();
    let e = w.spawn((Mesh::asset("rig.model"), Animation::play("slow")));
    w.step_clock();
    w.step_clock();
    let before = w.get::<Pose>(e).unwrap().clone();
    w.get_mut::<Animation>(e).unwrap().clip = "missing".into();
    w.step_clock();
    let after = w.get::<Pose>(e).unwrap();
    assert_eq!(before.previous, after.previous);
    assert_eq!(before.local, after.local);
}
#[test]
fn extracted_walk_never_doubles_or_snaps() {
    let mut w = world();
    let e = w.spawn((
        Transform::default(),
        Mesh::asset("rig.model"),
        Animation::play("slow").motion_root(""),
    ));
    for _ in 0..125 {
        w.step_clock();
        let delta = w.get::<Animation>(e).unwrap().root_motion();
        assert!((delta.x - 1. / 60.).abs() < 1e-6);
        w.get_mut::<Transform>(e).unwrap().position += delta;
        assert_eq!(w.get::<Pose>(e).unwrap().local[0], 0.);
    }
    assert!((w.get::<Transform>(e).unwrap().position.x - 125. / 60.).abs() < 1e-5);
}
#[test]
fn only_contributing_blend_markers_and_motion() {
    let mut w = world();
    let mut m = w.model("rig.model").unwrap().clone();
    m.clips[0].markers.push((0.01, "inactive".into()));
    m.clips[1].markers.push((0.01, "active".into()));
    w.assets
        .models
        .insert("rig.model".into(), std::sync::Arc::new(m));
    let mut b = Blend::across([(0., "slow"), (1., "fast")]).motion_root("");
    b.axis = 1.;
    let e = w.spawn((Mesh::asset("rig.model"), b));
    w.step_clock();
    let p = w.get::<Pose>(e).unwrap();
    assert_eq!(p.crossed, ["active"]);
    assert!(p.root_motion.x > 0.);
}
#[test]
fn missing_and_removed_sockets_invalidate_and_log_once() {
    let mut w = world();
    let e = w.spawn((
        Transform::default(),
        Mesh::asset("rig.model"),
        Socket("".into()),
    ));
    w.step_clock();
    assert!(w.has::<SocketPose>(e));
    w.remove::<Socket>(e);
    assert!(!w.has::<SocketPose>(e));
    w.insert(e, Socket("missing".into()));
    let f = w.spawn((Transform::at(9., 8., 7.), SocketFollow::new(e)));
    for _ in 0..3 {
        w.step_clock();
    }
    assert!(!w.has::<SocketPose>(e));
    assert_eq!(
        w.get::<Transform>(f).unwrap().position,
        Vec3::new(9., 8., 7.)
    );
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.contains("unknown socket"))
            .count(),
        1
    );
}
#[test]
fn standalone_ik_evaluates_bind_pose() {
    let mut w = world();
    let mut m = w.model("rig.model").unwrap().clone();
    m.nodes = vec![
        Node {
            name: "root".into(),
            ..Default::default()
        },
        Node {
            name: "mid".into(),
            parent: Some(0),
            transform: Mat4::from_translation(Vec3::X).to_cols_array(),
            ..Default::default()
        },
        Node {
            name: "tip".into(),
            parent: Some(1),
            transform: Mat4::from_translation(Vec3::X).to_cols_array(),
            ..Default::default()
        },
    ];
    w.assets
        .models
        .insert("rig.model".into(), std::sync::Arc::new(m));
    let e = w.spawn((
        Mesh::asset("rig.model"),
        Ik {
            chain: ["root", "mid", "tip"].map(String::from),
            target: Vec3::new(1., 1., 0.),
            pole: Vec3::Y,
            weight: 1.,
        },
    ));
    w.step_clock();
    let pose = w
        .get::<Pose>(e)
        .expect("standalone IK creates a sampled pose");
    let tip = joint_matrix(w.model("rig.model").unwrap(), &pose.local, 2)
        .w_axis
        .truncate();
    assert!(tip.distance(Vec3::new(1., 1., 0.)) < 1e-5);
}
#[test]
fn pose_read_returns_all_unique_joints_in_node_order() {
    let mut w = world();
    let mut m = w.model("rig.model").unwrap().clone();
    m.nodes = (0..256)
        .map(|i| Node {
            name: format!("joint{i}"),
            ..Default::default()
        })
        .collect();
    m.skins[0].joints = (0..255).rev().collect();
    m.skins.push(Skin {
        joints: vec![0, 255],
        ..Default::default()
    });
    w.assets
        .models
        .insert("rig.model".into(), std::sync::Arc::new(m));
    let e = w.spawn(Mesh::asset("rig.model"));
    let read = pose_json(&w, e).unwrap();
    assert_eq!(read.matches("\"name\"").count(), 256);
    assert_eq!(read.matches("\"joint0\"").count(), 1);
    assert!(read.contains("\"joint255\""));
    assert!(read.find("\"joint0\"").unwrap() < read.find("\"joint254\"").unwrap());
}
#[test]
fn cubic_uses_left_out_and_right_in_times_span() {
    let mut track = translation("cubic", 2., 2.).tracks.remove(0);
    track.times = vec![0., 2., 5.];
    track.interpolation = Interpolation::CubicSpline;
    track.values = [99., 0., 3., 7., 4., 11., 13., 8., 77.]
        .into_iter()
        .flat_map(|v| [v, 0., 0.])
        .collect();
    assert_eq!(
        value(&track, 0.5)[0],
        0.140625 * 2. * 3. + 0.15625 * 4. - 0.046875 * 2. * 7.
    );
    assert_eq!(
        value(&track, 2.75)[0],
        0.84375 * 4. + 0.140625 * 3. * 11. + 0.15625 * 8. - 0.046875 * 3. * 13.
    );
}
fn translation(name: &str, seconds: f32, distance: f32) -> Clip {
    Clip {
        name: name.into(),
        tracks: vec![Track {
            times: vec![0., seconds],
            values: vec![0., 0., 0., distance, 0., 0.],
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn world() -> World {
    let mut w = World::new(60, 0);
    w.register_scene();
    w.assets.declared.insert("rig.model".into());
    w.assets.models.insert(
        "rig.model".into(),
        std::sync::Arc::new(Model {
            nodes: vec![Node::default()],
            skins: vec![Skin {
                joints: vec![0],
                inverse_binds: Mat4::IDENTITY.to_cols_array().to_vec(),
                ..Default::default()
            }],
            clips: vec![translation("slow", 1., 1.), translation("fast", 0.5, 1.)],
            ..Default::default()
        }),
    );
    w
}
#[test]
fn clock_rounding_markers_loop_and_root_contribution_are_saved() {
    let mut w = world();
    let e = w.spawn_named(
        "actor",
        (
            Transform::default(),
            Mesh::asset("rig.model"),
            Animation::play("slow").motion_root("").marker(0.3, "step"),
        ),
    );
    let mut events = 0;
    for _ in 0..60 {
        w.step_clock();
        events += u32::from(w.get::<Animation>(e).unwrap().crossed("step"));
    }
    assert_eq!(w.get::<Animation>(e).unwrap().time.to_bits(), 0x3f7ffffb);
    assert_eq!(events, 1);
    assert_eq!(w.get::<Transform>(e).unwrap().position, Vec3::ZERO);
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.ends_with(" animation actor slow step"))
            .count(),
        1
    );
    let before = w.save();
    let hash = w.hash();
    w.load(&before).unwrap();
    assert_eq!(hash, w.hash());
    w.step_clock();
    assert!((w.get::<Animation>(e).unwrap().root_motion().x - 1. / 60.).abs() < 1e-6);
    assert!(!w.insert(e, Blend::across([(0., "slow")])));
}
#[test]
fn blend_uses_normalized_phase_and_once_stops() {
    let mut w = world();
    let mut blend = Blend::across([(0., "slow"), (1., "fast")]);
    blend.axis = 0.5;
    let e = w.spawn((Mesh::asset("rig.model"), blend));
    w.step_clock();
    let p = w.get::<Pose>(e).unwrap();
    assert!((p.phase - (1. / 60.) / 0.75).abs() < 1e-7);
    assert!((p.local[0] - p.phase).abs() < 1e-7);
    drop(p);
    let a = w.spawn((
        Mesh::asset("rig.model"),
        Animation::play("fast").once().marker(0.5, "end"),
    ));
    for _ in 0..30 {
        w.step_clock();
    }
    assert!(w.get::<Animation>(a).unwrap().crossed("end"));
    w.step_clock();
    let a = w.get::<Animation>(a).unwrap();
    assert_eq!(a.time, 0.5);
    assert!(!a.crossed("end"));
}
#[test]
fn step_linear_cubic_rotation_and_parent_order() {
    let mut track = translation("test", 2., 2.).tracks.remove(0);
    assert_eq!(value(&track, 1.)[0], 1.);
    track.interpolation = Interpolation::Step;
    assert_eq!(value(&track, 1.)[0], 0.);
    track.interpolation = Interpolation::CubicSpline;
    track.values = vec![
        0., 0., 0., 0., 0., 0., 1., 0., 0., 1., 0., 0., 2., 0., 0., 0., 0., 0.,
    ];
    assert_eq!(value(&track, 1.)[0], 1.);
    let q = Quat::from_rotation_y(1.);
    track.path = TrackPath::Rotation;
    track.interpolation = Interpolation::Linear;
    track.values = [Quat::IDENTITY.to_array(), q.to_array()].concat();
    let got = Quat::from_array(value(&track, 1.));
    assert!(got.dot(Quat::from_rotation_y(0.5)) > 0.999999);
    let m = Model {
        nodes: vec![
            Node {
                parent: Some(1),
                ..Default::default()
            },
            Node::default(),
        ],
        ..Default::default()
    };
    assert_eq!(node_order(&m), [1, 0]);
}

#[test]
fn animator_parameter_fade_markers_and_weighted_root_motion() {
    let mut w = world();
    let mut m = w.model("rig.model").unwrap().clone();
    m.clips[0].markers = vec![(0.01, "step".into())];
    m.clips[1].markers = vec![
        (0.01, "step".into()),
        (0.02, "later".into()),
        (0.05, "audible".into()),
    ];
    w.assets
        .models
        .insert("rig.model".into(), std::sync::Arc::new(m));
    let a = Animator::new([
        State::new("idle", Play::Clip("slow".into()))
            .to("travel", Condition::Arg("go".into(), Cmp::Eq, true.into())),
        State::new(
            "travel",
            Play::Blend(Blend::across([(0., "slow"), (1., "fast")]).parameter("speed")),
        )
        .fade(4. / 60.),
    ])
    .motion_root("");
    let e = w.spawn((Transform::default(), Mesh::asset("rig.model"), a));
    w.step_clock();
    assert!(w.get::<Animator>(e).unwrap().crossed("step"));
    w.get_mut::<Animator>(e).unwrap().set("go", true);
    w.get_mut::<Animator>(e).unwrap().set("speed", 1.);
    for i in 1..=4 {
        w.step_clock();
        let a = w.get::<Animator>(e).unwrap();
        let expected = (1. + i as f32 / 4.) / 60.;
        assert!(
            (a.root_motion().x - expected).abs() < 1e-6,
            "fade {i}: {:?}",
            a.root_motion()
        );
        if i <= 2 {
            assert!(!a.crossed("step") && !a.crossed("later"));
        }
        if i == 3 {
            assert!(a.crossed("audible"));
        }
        assert_eq!(w.get::<Pose>(e).unwrap().local[0], 0.);
    }
    // Named access survives reordered definitions; the parameter is the axis source.
    assert!(w
        .get_mut::<Animator>(e)
        .unwrap()
        .blend_mut("travel")
        .is_some());
    for _ in 0..70 {
        w.step_clock();
    }
    assert!(w.get::<Transform>(e).unwrap().position == Vec3::ZERO);
    assert!((w.get::<Animator>(e).unwrap().root_motion().x - 2. / 60.).abs() < 1e-6);
}
#[test]
fn one_shot_speed_pause_and_end_transition() {
    let mut w = world();
    let mut a = Animator::new([
        State::new("attack", Play::Clip("slow".into()))
            .once()
            .speed(2.)
            .paused(true)
            .to("done", Condition::Arg("done".into(), Cmp::Eq, true.into())),
        State::new("done", Play::Clip("fast".into())),
    ])
    .motion_root("");
    a.set("done", true);
    let e = w.spawn((Mesh::asset("rig.model"), a));
    w.step_clock();
    assert_eq!(w.get::<Animator>(e).unwrap().state(), "attack");
    assert_eq!(w.get::<Animator>(e).unwrap().root_motion(), Vec3::ZERO);
    assert_eq!(w.get::<Pose>(e).unwrap().phase, 0.);
    w.get_mut::<Animator>(e)
        .unwrap()
        .state_mut("attack")
        .unwrap()
        .paused = false;
    for _ in 0..30 {
        w.step_clock();
        assert_eq!(w.get::<Animator>(e).unwrap().state(), "attack");
    }
    // Floating phase reaches the clamped endpoint on the 30th tick at 2x.
    assert_eq!(w.get::<Pose>(e).unwrap().phase, 1.);
    w.step_clock();
    assert_eq!(w.get::<Animator>(e).unwrap().state(), "done");
    assert!(w.get::<Pose>(e).unwrap().phase < 0.1);
}
#[test]
fn explicit_motion_root_is_not_first_skin_joint_and_wraps_backwards() {
    let mut w = world();
    let mut m = w.model("rig.model").unwrap().clone();
    m.nodes.push(Node {
        name: "motion".into(),
        ..Default::default()
    });
    m.clips[0].tracks[0].node = 1;
    w.assets
        .models
        .insert("rig.model".into(), std::sync::Arc::new(m));
    let e = w.spawn((
        Mesh::asset("rig.model"),
        Animation::play("slow").motion_root("motion").speed(-130.),
    ));
    w.step_clock();
    assert!((w.get::<Animation>(e).unwrap().root_motion().x + 130. / 60.).abs() < 1e-6);
    assert_eq!(w.get::<Pose>(e).unwrap().local[10], 0.);
}
#[test]
fn invalidated_socket_restores_authored_follower() {
    let mut w = world();
    let e = w.spawn((
        Transform::at(1., 2., 3.),
        Mesh::asset("rig.model"),
        Socket("".into()),
    ));
    let f = w.spawn((Transform::at(9., 8., 7.), SocketFollow::new(e)));
    w.step_clock();
    assert_eq!(
        w.get::<Transform>(f).unwrap().position,
        Vec3::new(1., 2., 3.)
    );
    w.get_mut::<Socket>(e).unwrap().0 = "missing".into();
    w.step_clock();
    w.step_clock();
    assert!(!w.has::<SocketPose>(e));
    assert_eq!(
        w.get::<Transform>(f).unwrap().position,
        Vec3::new(9., 8., 7.)
    );
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.contains("unknown socket"))
            .count(),
        1
    );
}

#[test]
fn blend_and_animator_walk_smoothly_across_loops_through_common_reads() {
    for animator in [false, true] {
        let mut w = world();
        let e = w.spawn((Transform::default(), Mesh::asset("rig.model")));
        let mut blend = Blend::across([(0., "slow"), (1., "fast")]);
        blend.axis = 0.5;
        if animator {
            w.insert(
                e,
                Animator::new([State::new("walk", Play::Blend(blend))]).motion_root(""),
            );
        } else {
            w.insert(e, blend.motion_root(""));
        }
        for _ in 0..125 {
            w.step_clock();
            let read = |p: &Playback| {
                assert!(!p.crossed("missing"));
                p.root_motion()
            };
            let delta = if animator {
                read(&w.get::<Animator>(e).unwrap())
            } else {
                read(&w.get::<Blend>(e).unwrap())
            };
            assert!((delta.x - 1. / 45.).abs() < 1e-6);
            w.get_mut::<Transform>(e).unwrap().position += delta;
            assert_eq!(w.get::<Pose>(e).unwrap().local[0], 0.);
        }
        assert!((w.get::<Transform>(e).unwrap().position.x - 125. / 45.).abs() < 1e-5);
    }
}
#[test]
fn cubic_quaternions_keep_authored_signs_and_normalize_the_polynomial() {
    let q0 = Quat::IDENTITY.to_array();
    let q1 = (-Quat::from_rotation_z(1.)).to_array();
    let mut track = Track {
        path: TrackPath::Rotation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0., 2.],
        ..Default::default()
    };
    let left_out = [0., 0., 0.2, 0.3];
    let right_in = [0., 0., 0.7, 0.9];
    track.values = [[8.; 4], q0, left_out, right_in, q1, [9.; 4]].concat();
    let expected = Quat::from_array(std::array::from_fn(|i| {
        0.84375 * q0[i] + 0.140625 * 2. * left_out[i] + 0.15625 * q1[i]
            - 0.046875 * 2. * right_in[i]
    }))
    .normalize();
    assert!(Quat::from_array(value(&track, 0.5)).dot(expected) > 0.999999);
}

#[test]
fn reversed_one_shot_starts_at_end_and_does_not_transition_early() {
    let mut w = world();
    let mut a = Animator::new([
        State::new("reverse", Play::Clip("slow".into()))
            .once()
            .speed(-2.)
            .to("done", Condition::Arg("go".into(), Cmp::Eq, true.into())),
        State::new("done", Play::Clip("fast".into())),
    ])
    .motion_root("");
    a.set("go", true);
    let e = w.spawn((Mesh::asset("rig.model"), a));
    w.step_clock();
    assert_eq!(w.get::<Animator>(e).unwrap().state(), "reverse");
    assert!((w.get::<Pose>(e).unwrap().phase - (1. - 2. / 60.)).abs() < 1e-6);
    assert!((w.get::<Animator>(e).unwrap().root_motion().x + 2. / 60.).abs() < 1e-6);
}
#[test]
fn failed_playback_contributes_no_stale_motion_or_events() {
    let mut w = world();
    let e = w.spawn((
        Mesh::asset("rig.model"),
        Animation::play("slow").motion_root("").marker(0.01, "step"),
    ));
    w.step_clock();
    assert!(w.get::<Animation>(e).unwrap().crossed("step"));
    w.get_mut::<Animation>(e).unwrap().clip = "missing".into();
    w.step_clock();
    assert_eq!(w.get::<Animation>(e).unwrap().root_motion(), Vec3::ZERO);
    assert!(!w.get::<Animation>(e).unwrap().crossed("step"));
    assert_eq!(w.get::<Pose>(e).unwrap().root_motion, Vec3::ZERO);
}

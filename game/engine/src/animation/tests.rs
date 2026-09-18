use super::*;
use crate::asset::{Node, Skin};

#[test]
fn first_sample_is_current_current() {
    let mut w = world();
    let e = w.spawn((Mesh::asset("rig.model"), Animation::play("slow")));
    step(&mut w);
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
    step(&mut w);
    w.step_clock();
    step(&mut w);
    w.step_clock();
    let before = w.get::<Pose>(e).unwrap().clone();
    w.get_mut::<Animation>(e).unwrap().clip = "missing".into();
    step(&mut w);
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
        step(&mut w);
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
    step(&mut w);
    w.step_clock();
    let p = w.get::<Pose>(e).unwrap();
    assert_eq!(p.crossed, ["active"]);
    assert!(p.root_motion.x > 0.);
}
#[test]
fn sockets_are_named_queries_and_followers_do_not_write_simulation() {
    let mut w = world();
    let e = w.spawn((Transform::default(), Mesh::asset("rig.model")));
    let f = w.spawn((Transform::at(9., 8., 7.), SocketFollow::new(e, "missing")));
    assert_eq!(socket(&w, e, "").unwrap().position, Vec3::ZERO);
    assert_eq!(
        socket(&w, e, "missing").unwrap_err(),
        "unknown socket `missing`"
    );
    for _ in 0..3 {
        step(&mut w);
        w.step_clock();
    }
    assert_eq!(
        w.get::<Transform>(f).unwrap().position,
        Vec3::new(9., 8., 7.)
    );
    assert!(socket(&w, "absent", "head")
        .unwrap_err()
        .contains("target does not exist"));
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
    step(&mut w);
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
        step(&mut w);
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
    step(&mut w);
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
    step(&mut w);
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
        step(&mut w);
        w.step_clock();
    }
    assert!(w.get::<Animation>(a).unwrap().crossed("end"));
    step(&mut w);
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
    step(&mut w);
    w.step_clock();
    assert!(w.get::<Animator>(e).unwrap().crossed("step"));
    w.get_mut::<Animator>(e).unwrap().set("go", true);
    w.get_mut::<Animator>(e).unwrap().set("speed", 1.);
    for i in 1..=4 {
        step(&mut w);
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
        step(&mut w);
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
    step(&mut w);
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
        step(&mut w);
        w.step_clock();
        assert_eq!(w.get::<Animator>(e).unwrap().state(), "attack");
    }
    // Floating phase reaches the clamped endpoint on the 30th tick at 2x.
    assert_eq!(w.get::<Pose>(e).unwrap().phase, 1.);
    step(&mut w);
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
    step(&mut w);
    w.step_clock();
    assert!((w.get::<Animation>(e).unwrap().root_motion().x + 130. / 60.).abs() < 1e-6);
    assert_eq!(w.get::<Pose>(e).unwrap().local[10], 0.);
}
#[test]
fn independent_attachment_joints_and_query_after_movement() {
    let mut w = world();
    let mut model = w.model("rig.model").unwrap().clone();
    model.nodes[0].name = "head".into();
    model.nodes.push(Node {
        name: "hand".into(),
        transform: Mat4::from_translation(Vec3::X).to_cols_array(),
        ..Default::default()
    });
    w.assets
        .models
        .insert("rig.model".into(), std::sync::Arc::new(model));
    let e = w.spawn((Transform::at(1., 2., 3.), Mesh::asset("rig.model")));
    w.spawn((Transform::default(), SocketFollow::new(e, "head")));
    w.spawn((Transform::default(), SocketFollow::new(e, "hand")));
    assert_eq!(
        socket(&w, e, "head").unwrap().position,
        Vec3::new(1., 2., 3.)
    );
    assert_eq!(
        socket(&w, e, "hand").unwrap().position,
        Vec3::new(2., 2., 3.)
    );
    w.get_mut::<Transform>(e).unwrap().position += Vec3::Z;
    assert_eq!(
        socket(&w, e, "hand").unwrap().position,
        Vec3::new(2., 2., 4.)
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
            step(&mut w);
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
    step(&mut w);
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
    step(&mut w);
    w.step_clock();
    assert!(w.get::<Animation>(e).unwrap().crossed("step"));
    w.get_mut::<Animation>(e).unwrap().clip = "missing".into();
    step(&mut w);
    w.step_clock();
    assert_eq!(w.get::<Animation>(e).unwrap().root_motion(), Vec3::ZERO);
    assert!(!w.get::<Animation>(e).unwrap().crossed("step"));
    assert_eq!(w.get::<Pose>(e).unwrap().root_motion, Vec3::ZERO);
}

#[test]
fn reverse_loop_markers_include_landing_exclude_departure_and_span_loops() {
    assert!(crossed(0.3, 0.4, 0.3, 1., true));
    assert!(!crossed(0.3, 0.3, 0.2, 1., true));
    assert!(crossed(0.3, 0.4, -2.7, 1., true));
    assert!(crossed(0., 0.1, 0., 1., true));
    assert!(!crossed(0., 0., -0.1, 1., true));
}
#[test]
fn standalone_reverse_once_starts_at_end() {
    let mut w = world();
    let e = w.spawn((
        Mesh::asset("rig.model"),
        Animation::play("slow").once().speed(-1.).motion_root(""),
    ));
    step(&mut w);
    w.step_clock();
    let a = w.get::<Animation>(e).unwrap();
    assert!((a.time - (1. - 1. / 60.)).abs() < 1e-6);
    assert!((a.root_motion().x + 1. / 60.).abs() < 1e-6);
    drop(a);
    for _ in 0..65 {
        step(&mut w);
        w.step_clock();
    }
    assert_eq!(w.get::<Animation>(e).unwrap().time, 0.);
    assert_eq!(w.get::<Animation>(e).unwrap().root_motion(), Vec3::ZERO);
}
#[test]
fn zero_length_and_zero_speed_once_finish_but_pause_waits() {
    for (duration, speed) in [(0., 1.), (0., -1.), (1., 0.)] {
        let mut w = world();
        let mut m = w.model("rig.model").unwrap().clone();
        m.clips[0] = Clip {
            name: "still".into(),
            tracks: vec![Track {
                times: vec![duration],
                values: vec![0.; 3],
                ..Default::default()
            }],
            ..Default::default()
        };
        w.assets
            .models
            .insert("rig.model".into(), std::sync::Arc::new(m));
        let mut a = Animator::new([
            State::new("once", Play::Clip("still".into()))
                .once()
                .speed(speed)
                .paused(true)
                .to("done", Condition::Arg("go".into(), Cmp::Eq, true.into())),
            State::new("done", Play::Clip("fast".into())),
        ]);
        a.set("go", true);
        let e = w.spawn((Mesh::asset("rig.model"), a));
        step(&mut w);
        w.step_clock();
        assert_eq!(w.get::<Animator>(e).unwrap().state(), "once");
        w.get_mut::<Animator>(e)
            .unwrap()
            .state_mut("once")
            .unwrap()
            .paused = false;
        step(&mut w);
        w.step_clock();
        assert_eq!(w.get::<Animator>(e).unwrap().state(), "once");
        step(&mut w);
        w.step_clock();
        assert_eq!(
            w.get::<Animator>(e).unwrap().state(),
            "done",
            "duration {duration}, speed {speed}"
        );
    }
}
#[test]
fn explicit_step_returns_owned_motion_without_a_registration_schedule() {
    let mut w = world();
    let e = w.spawn_named(
        "walker",
        (
            Transform::default(),
            Mesh::asset("rig.model"),
            Animation::play("slow").motion_root(""),
        ),
    );
    w.step_clock();
    assert!(!w.has::<Pose>(e));
    let output = step(&mut w);
    assert!(output.root_motion("walker").x > 0.);
    let epoch = w.mutation_epoch();
    let again = step(&mut w);
    assert_eq!(
        w.mutation_epoch(),
        epoch,
        "the early guard takes no write leases"
    );
    assert_eq!(output.root_motion("walker"), again.root_motion(e));
    w.get_mut::<Transform>(e)
        .unwrap()
        .translate_local(output.root_motion(e));
    assert!(socket(&w, e, "missing").is_err());
    assert!(w.get::<Animation>(e).unwrap().root_motion().x > 0.);
}
#[test]
fn first_model_arrival_retries_a_failed_sample_in_the_same_tick() {
    let mut w = world();
    let model = w.assets.models.get("rig.model").unwrap().clone();
    w.assets.models.remove("rig.model");
    let e = w.spawn((Mesh::asset("rig.model"), Animation::play("slow")));
    step(&mut w);
    assert!(!w.has::<Pose>(e));
    assert!(!w.get::<Animation>(e).unwrap().sampled);
    w.assets.models.insert("rig.model".into(), model);
    step(&mut w);
    assert!(w.has::<Pose>(e));
    assert!(w.get::<Animation>(e).unwrap().sampled);
    let time = w.get::<Animation>(e).unwrap().time;
    step(&mut w);
    assert_eq!(w.get::<Animation>(e).unwrap().time, time);
}
#[test]
fn failed_ik_does_not_commit_animator_transition_or_clock() {
    let mut w = world();
    let a = Animator::new([
        State::new("walk", Play::Clip("slow".into()))
            .to("run", Condition::Arg("go".into(), Cmp::Eq, true.into())),
        State::new("run", Play::Clip("fast".into())).fade(0.1),
    ]);
    let e = w.spawn((Mesh::asset("rig.model"), a));
    step(&mut w);
    w.step_clock();
    w.get_mut::<Animator>(e).unwrap().set("go", true);
    let before = crate::bin::to_vec(&*w.get::<Animator>(e).unwrap());
    let pose = crate::bin::to_vec(&*w.get::<Pose>(e).unwrap());
    w.insert(
        e,
        Ik {
            chain: ["missing".into(), "mid".into(), "tip".into()],
            weight: 1.,
            ..Default::default()
        },
    );
    step(&mut w);
    w.step_clock();
    assert_eq!(crate::bin::to_vec(&*w.get::<Animator>(e).unwrap()), before);
    assert_eq!(crate::bin::to_vec(&*w.get::<Pose>(e).unwrap()), pose);
    w.remove::<Ik>(e);
    step(&mut w);
    w.step_clock();
    assert_eq!(w.get::<Animator>(e).unwrap().state(), "run");
}
#[test]
fn redelivered_model_rebuilds_rest_bounds_and_socket_cache() {
    let mut w = world();
    let e = w.spawn((
        Transform::default(),
        Mesh::asset("rig.model"),
        Ik::default(),
    ));
    step(&mut w);
    w.step_clock();
    let mut m = w.model("rig.model").unwrap().clone();
    m.nodes[0].name = "renamed".into();
    m.nodes[0].transform = Mat4::from_translation(Vec3::Y * 3.).to_cols_array();
    let expected = Rig::new(&std::sync::Arc::new(m.clone()));
    w.assets
        .models
        .insert("rig.model".into(), std::sync::Arc::new(m));
    step(&mut w);
    w.step_clock();
    assert!(socket(&w, e, "").is_err());
    assert_eq!(w.get::<Pose>(e).unwrap().local, expected.rest);
    assert_eq!(w.animation.rigs["rig.model"].bounds, expected.bounds);
    step(&mut w);
    w.step_clock();
    assert_eq!(socket(&w, e, "renamed").unwrap().position, Vec3::Y * 3.);
}
#[test]
fn pose_inspection_refuses_model_length_mismatch_by_name() {
    let mut w = world();
    let e = w.spawn((Mesh::asset("rig.model"), Animation::play("slow")));
    step(&mut w);
    w.step_clock();
    w.get_mut::<Pose>(e).unwrap().local.clear();
    assert_eq!(
        pose_json(&w, e).unwrap_err(),
        "saved pose does not match model `rig.model`"
    );
}
#[test]
fn duplicate_motion_roots_choose_first_parent_first_match() {
    let model = Model {
        nodes: vec![
            Node {
                name: "root".into(),
                parent: Some(1),
                ..Default::default()
            },
            Node {
                name: "root".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_eq!(
        Animation::play("walk")
            .motion_root("root")
            .playback
            .root(&model)
            .unwrap(),
        Some(1)
    );
}

#[test]
fn r3_redelivery_primes_every_shared_pose_and_accepts_new_topology() {
    for extra_node in [false, true] {
        let mut w = world();
        let entities: Vec<_> = (0..2)
            .map(|_| {
                w.spawn((
                    Transform::default(),
                    Mesh::asset("rig.model"),
                    Ik::default(),
                ))
            })
            .collect();
        step(&mut w);
        w.step_clock();
        let old = std::sync::Arc::downgrade(w.assets.models.get("rig.model").unwrap());
        let mut m = w.model("rig.model").unwrap().clone();
        m.nodes[0].transform = Mat4::from_translation(Vec3::Y * 3.).to_cols_array();
        if extra_node {
            m.nodes.push(Node::default());
        }
        let expected = bind_pose(&m);
        w.assets
            .models
            .insert("rig.model".into(), std::sync::Arc::new(m));
        assert!(old.upgrade().is_none());
        // Redelivery between samples in the same tick must reach both entities.
        step(&mut w);
        for e in entities {
            let p = w.get::<Pose>(e).unwrap();
            assert_eq!(p.local, expected);
            assert_eq!(p.previous, expected);
            assert_eq!(socket(&w, e, "").unwrap().position, Vec3::Y * 3.);
        }
    }
}
#[test]
fn r3_reverse_controllers_start_on_existing_socket_pose() {
    for animator in [false, true] {
        let mut w = world();
        let e = w.spawn((
            Transform::default(),
            Mesh::asset("rig.model"),
            Ik::default(),
        ));
        step(&mut w);
        w.step_clock();
        if animator {
            let mut a = Animator::new([
                State::new("reverse", Play::Clip("slow".into()))
                    .once()
                    .speed(-1.)
                    .to("done", Condition::Arg("go".into(), Cmp::Eq, true.into())),
                State::new("done", Play::Clip("fast".into())),
            ]);
            a.set("go", true);
            w.insert(e, a);
        } else {
            w.insert(e, Animation::play("slow").once().speed(-1.));
        }
        step(&mut w);
        w.step_clock();
        if animator {
            assert_eq!(w.get::<Animator>(e).unwrap().state(), "reverse");
            assert!((w.get::<Pose>(e).unwrap().phase - (1. - 1. / 60.)).abs() < 1e-6);
        } else {
            assert!((w.get::<Animation>(e).unwrap().time - (1. - 1. / 60.)).abs() < 1e-6);
        }
        // The initialization survives restore; reaching zero must not restart it.
        let bytes = w.save();
        w.load(&bytes).unwrap();
        for _ in 0..65 {
            step(&mut w);
            w.step_clock();
        }
        if animator {
            assert_eq!(w.get::<Animator>(e).unwrap().state(), "done");
        } else {
            assert_eq!(w.get::<Animation>(e).unwrap().time, 0.);
        }
    }
}
#[test]
fn r3_inspection_and_sampling_refuse_either_corrupt_history() {
    for previous in [false, true] {
        let mut w = world();
        let e = w.spawn((Mesh::asset("rig.model"), Animation::play("slow")));
        step(&mut w);
        w.step_clock();
        if previous {
            w.get_mut::<Pose>(e).unwrap().previous.clear();
        } else {
            w.get_mut::<Pose>(e).unwrap().local.clear();
        }
        let expected = "saved pose does not match model `rig.model`";
        assert_eq!(pose_json(&w, e).unwrap_err(), expected);
        step(&mut w);
        w.step_clock();
        assert!(w.journal().iter().any(|line| line.line.contains(expected)));
    }
}

#[test]
fn same_tick_redelivery_resamples_without_advancing_any_controller() {
    for controller in 0..3 {
        let mut w = world();
        let mut model = w.model("rig.model").unwrap().clone();
        model.clips[0].markers = vec![(0.01, "first".into()), (0.02, "second".into())];
        w.assets
            .models
            .insert("rig.model".into(), std::sync::Arc::new(model));
        let entities: Vec<_> = (0..2)
            .map(|_| {
                let e = w.spawn((
                    Transform::default(),
                    Mesh::asset("rig.model"),
                    Ik::default(),
                ));
                match controller {
                    0 => w.insert(e, Animation::play("slow").motion_root("")),
                    1 => w.insert(
                        e,
                        Blend::across([(0., "slow"), (1., "fast")]).motion_root(""),
                    ),
                    _ => w.insert(
                        e,
                        Animator::new([
                            State::new("walk", Play::Clip("slow".into()))
                                .to("run", Condition::Arg("go".into(), Cmp::Eq, true.into())),
                            State::new("run", Play::Clip("fast".into())),
                        ])
                        .motion_root(""),
                    ),
                };
                e
            })
            .collect();
        step(&mut w);
        let before: Vec<_> = entities
            .iter()
            .map(|&e| {
                if let Some(mut a) = w.get_mut::<Animator>(e) {
                    a.set("go", true);
                }
                let bytes = match controller {
                    0 => crate::bin::to_vec(&*w.get::<Animation>(e).unwrap()),
                    1 => crate::bin::to_vec(&*w.get::<Blend>(e).unwrap()),
                    _ => crate::bin::to_vec(&*w.get::<Animator>(e).unwrap()),
                };
                let pose = w.get::<Pose>(e).unwrap().clone();
                assert_eq!(pose.crossed, ["first"]);
                assert!((pose.root_motion.x - 1. / 60.).abs() < 1e-6);
                (bytes, pose)
            })
            .collect();
        let logs = w.journal().len();
        let mut model = w.model("rig.model").unwrap().clone();
        model.nodes.push(Node {
            transform: Mat4::from_translation(Vec3::Y * 3.).to_cols_array(),
            ..Default::default()
        });
        w.assets
            .models
            .insert("rig.model".into(), std::sync::Arc::new(model));
        step(&mut w);
        for (&e, (bytes, before)) in entities.iter().zip(before) {
            let after = match controller {
                0 => crate::bin::to_vec(&*w.get::<Animation>(e).unwrap()),
                1 => crate::bin::to_vec(&*w.get::<Blend>(e).unwrap()),
                _ => crate::bin::to_vec(&*w.get::<Animator>(e).unwrap()),
            };
            assert_eq!(after, bytes, "controller {controller} advanced twice");
            let p = w.get::<Pose>(e).unwrap();
            assert_eq!(p.phase, before.phase);
            assert_eq!(p.crossed, before.crossed);
            assert_eq!(p.root_motion, before.root_motion);
            assert_eq!(p.local.len(), before.local.len() + 10);
            assert_eq!(p.local[before.local.len() + 1], 3.);
            assert_eq!(p.previous, p.local);
        }
        assert_eq!(w.journal().len(), logs, "redelivery re-emitted markers");
    }
}

#[test]
fn topology_change_during_fade_completes_and_allows_later_edge() {
    let mut w = world();
    let e = w.spawn((
        Mesh::asset("rig.model"),
        Animator::new([
            State::new("walk", Play::Clip("slow".into()))
                .to("run", Condition::Arg("go".into(), Cmp::Eq, true.into())),
            State::new("run", Play::Clip("fast".into()))
                .fade(0.1)
                .to("walk", Condition::Arg("go".into(), Cmp::Eq, false.into())),
        ]),
    ));
    step(&mut w);
    w.get_mut::<Animator>(e).unwrap().set("go", true);
    step(&mut w);
    w.step_clock();
    step(&mut w);
    assert_eq!(w.get::<Animator>(e).unwrap().current, 1);
    let mut model = w.model("rig.model").unwrap().clone();
    model.nodes.push(Node::default());
    w.assets
        .models
        .insert("rig.model".into(), std::sync::Arc::new(model));
    step(&mut w);
    for _ in 0..10 {
        step(&mut w);
        w.step_clock();
        step(&mut w);
    }
    let a = w.get::<Animator>(e).unwrap();
    assert_eq!(a.fade_time, a.fade_duration);
    drop(a);
    w.get_mut::<Animator>(e).unwrap().set("go", false);
    step(&mut w);
    w.step_clock();
    step(&mut w);
    assert_eq!(w.get::<Animator>(e).unwrap().current, 0);
}

#[test]
fn tick_end_is_the_boundary_written_by_motion_springs_and_publications() {
    let mut w = world();
    let now = w.now();
    let end = w.tick_end();
    assert_eq!(end.tick, now.tick + 1);
    assert_eq!(end.seconds(), w.dt());
    let mut spring = crate::Spring::default();
    spring.set_target(end, 1.);
    w.publish("time", crate::Value::Number(end.seconds() as f64));
    w.step_clock();
    assert_eq!(w.now(), end);
    assert_eq!(spring.value(w.now()), spring.value(end));
}

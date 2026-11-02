use super::*;

#[test]
fn removing_one_controller_preserves_the_other_controllers_pose_and_tick() {
    for remove_base in [false, true] {
        let mut w = world();
        let e = w.spawn((
            Mesh::asset("rig.model"),
            Animation::play("slow").motion_root(""),
            Layers(vec![Layer::new(Animation::play("fast"))]),
        ));
        step(&mut w);
        let before = crate::bin::to_vec(&*w.get::<Pose>(e).unwrap());
        if remove_base {
            w.remove::<Animation>(e);
        } else {
            w.remove::<Layers>(e);
        }
        assert_eq!(crate::bin::to_vec(&*w.get::<Pose>(e).unwrap()), before);
        step(&mut w);
        assert_eq!(
            crate::bin::to_vec(&*w.get::<Pose>(e).unwrap()),
            before,
            "no second evaluation in the same tick"
        );
        w.step_clock();
        step(&mut w);
        assert!(w.has::<Pose>(e));
        if remove_base {
            w.remove::<Layers>(e);
        } else {
            w.remove::<Animation>(e);
        }
        assert!(!w.has::<Pose>(e));
    }
}

#[test]
fn animator_fades_apply_layers_once_and_resume_the_pre_layer_sample() {
    for additive in [true, false] {
        let mut w = world();
        let mut model = w.model("rig.model").unwrap().clone();
        let mut overlay = translation("flinch", 1., 3.);
        overlay.tracks[0].values[0] = 3.;
        model.clips.push(overlay);
        w.assets.models.insert("rig.model".into(), model.into());
        let animator = || {
            Animator::new([
                State::clip("walk", "slow")
                    .to("run", Condition::Arg("go".into(), Cmp::Eq, true.into())),
                State::clip("run", "fast").fade(0.25),
            ])
        };
        let base = w.spawn_named("plain", (Mesh::asset("rig.model"), animator()));
        let e = w.spawn_named(
            "layered",
            (
                Mesh::asset("rig.model"),
                animator(),
                Layers(vec![Layer {
                    animation: Animation::play("flinch"),
                    weight: 0.5,
                    additive,
                    mask: vec![],
                }]),
            ),
        );
        step(&mut w);
        w.step_clock();
        for entity in [base, e] {
            w.get_mut::<Animator>(entity).unwrap().set("go", true);
        }
        let check = |w: &World| {
            let plain = w.get::<Pose>(base).unwrap().local[0];
            let expected = if additive {
                plain + 1.5
            } else {
                plain * 0.5 + 1.5
            };
            assert!((w.get::<Pose>(e).unwrap().local[0] - expected).abs() < 1e-6);
        };
        step(&mut w);
        check(&w);
        w.step_clock();
        let saved = w.save();
        for _ in 0..10 {
            step(&mut w);
            check(&w);
            w.step_clock();
        }
        let continued = w.save();
        w.load(&saved).unwrap();
        for _ in 0..10 {
            step(&mut w);
            check(&w);
            w.step_clock();
        }
        assert_eq!(continued, w.save());
        let mut fresh = world();
        let mut edited = animator();
        edited.states[1].speed = 0.75;
        fresh.spawn_named("plain", (Mesh::asset("rig.model"), edited.clone()));
        fresh.spawn_named(
            "layered",
            (
                Mesh::asset("rig.model"),
                edited,
                Layers(vec![Layer {
                    animation: Animation::play("flinch"),
                    weight: 0.5,
                    additive,
                    mask: vec![],
                }]),
            ),
        );
        Definitions::capture(&fresh).apply(&mut w);
        step(&mut w);
        check(&w);
    }
}

#[test]
fn layers_cannot_restore_the_base_controllers_extracted_root_translation() {
    for controller in 0..3 {
        let mut w = world();
        let e = w.spawn((
            Mesh::asset("rig.model"),
            Layers(vec![Layer::new(Animation::play("fast"))]),
        ));
        match controller {
            0 => {
                w.insert(e, Animation::play("slow").motion_root(""));
            }
            1 => {
                w.insert(e, Blend::across([(0., "slow")]).motion_root(""));
            }
            _ => {
                w.insert(
                    e,
                    Animator::new([State::clip("walk", "slow")]).motion_root(""),
                );
            }
        }
        for _ in 0..10 {
            let motion = step(&mut w);
            assert!((motion.root_motion(e).x - 1. / 60.).abs() < 1e-6);
            assert_eq!(w.get::<Pose>(e).unwrap().local[0], 0.);
            w.step_clock();
        }
    }
}

#[test]
fn carry_removes_layers_and_matches_reordered_clocks_by_clip_occurrence() {
    let mut w = world();
    let e = w.spawn_named(
        "actor",
        (
            Mesh::asset("rig.model"),
            Animation::play("slow"),
            Layers(vec![
                Layer::new(Animation::play("slow")),
                Layer::new(Animation::play("fast")),
            ]),
        ),
    );
    step(&mut w);
    w.step_clock();
    w.get_mut::<Layers>(e).unwrap().0[0].animation.time = 0.2;
    w.get_mut::<Layers>(e).unwrap().0[1].animation.time = 0.4;
    let mut fresh = world();
    let f = fresh.spawn_named(
        "actor",
        (
            Mesh::asset("rig.model"),
            Animation::play("slow"),
            Layers(vec![
                Layer::new(Animation::play("fast")),
                Layer::new(Animation::play("slow")),
            ]),
        ),
    );
    Definitions::capture(&fresh).apply(&mut w);
    assert_eq!(w.get::<Layers>(e).unwrap().0[0].animation.time, 0.4);
    assert_eq!(w.get::<Layers>(e).unwrap().0[1].animation.time, 0.2);
    fresh.remove::<Layers>(f);
    Definitions::capture(&fresh).apply(&mut w);
    assert!(!w.has::<Layers>(e));
    assert!(w.has::<Pose>(e));
    step(&mut w);
    assert!((w.get::<Pose>(e).unwrap().local[0] - 2. / 60.).abs() < 1e-6);
    fresh.remove::<Animation>(f);
    Definitions::capture(&fresh).apply(&mut w);
    assert!(!w.has::<Animation>(e));
    assert!(!w.has::<Pose>(e));
}

#[test]
fn duplicate_mask_names_select_one_parent_first_subtree_and_rigid_pose_is_inspectable() {
    let mut w = world();
    let mut model = w.model("rig.model").unwrap().clone();
    model.skins.clear();
    model.meshes.push(Default::default());
    model.nodes = vec![
        Node {
            name: "spine".into(),
            parent: Some(2),
            ..Default::default()
        },
        Node {
            name: "spine".into(),
            mesh: Some(0),
            ..Default::default()
        },
        Node {
            name: "parent".into(),
            ..Default::default()
        },
        Node {
            name: "child".into(),
            parent: Some(1),
            ..Default::default()
        },
    ];
    let mut overlay = translation("flinch", 1., 6.);
    let track = overlay.tracks[0].clone();
    overlay.tracks = (0..4)
        .map(|node| {
            let mut t = track.clone();
            t.node = node;
            t
        })
        .collect();
    model.clips.push(overlay);
    w.assets.models.insert("rig.model".into(), model.into());
    let e = w.spawn((
        Mesh::asset("rig.model"),
        Layers(vec![Layer::new(Animation::play("flinch")).mask(["spine"])]),
    ));
    step(&mut w);
    let p = w.get::<Pose>(e).unwrap();
    assert_eq!(p.local[0], 0.);
    assert!((p.local[10] - 0.1).abs() < 1e-6);
    assert_eq!(p.local[20], 0.);
    assert!((p.local[30] - 0.1).abs() < 1e-6);
    drop(p);
    #[derive(Default, Data)]
    struct Row {
        name: String,
        world: Vec<f32>,
    }
    let inspected: Vec<Row> = crate::json::from_str(&pose_json(&w, e).unwrap()).unwrap();
    assert_eq!(inspected.len(), 1);
    assert_eq!(inspected[0].name, "spine");
    assert!((inspected[0].world[12] - 0.1).abs() < 1e-6);
}

use super::*;

#[test]
fn drawn_pose_primes_survive_dirty_feed_and_last_two_observer_ticks() {
    use exact_game::{
        animation, Animation, LodLevel, ModelLod, NodeName, NodeOffsets, Pose, Present,
    };
    struct Posed;
    impl Game for Posed {
        const ID: &'static str = "surface-pose-primes";
        const STREAMED: &'static [&'static str] = &["lod.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            let model = crate::test_model::posable_model();
            let mesh = w.generated_model("actor.model", model.clone()).unwrap();
            let local = animation::bind_pose(w.model("actor.model").unwrap());
            let mut pose = Pose::default();
            pose.previous = local.clone();
            pose.local = local;
            w.spawn_named(
                "actor",
                (Transform::default(), mesh, Animation::play("bend"), pose),
            );
            w.spawn_named(
                "charm",
                (
                    Transform::default(),
                    exact_game::SocketFollow::new("actor", "hand"),
                    Mesh::cube(0.1),
                    exact_game::Placed::child(0),
                ),
            );
            w.spawn((Transform::at(0., 0., 8.), Camera::default()));
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            animation::step(w);
        }
        fn present(p: &mut Present<'_>, _: &()) {
            let e = p.named("actor").unwrap();
            let x = p.tick() as f32 * 0.1;
            p.insert(
                e,
                NodeOffsets::default().with(NodeName::new("arm"), Transform::at(x, 0., 0.)),
            );
        }
    }
    let Some(gpu) = gpu() else { return };
    for event in [
        "initial",
        "restore",
        "replacement",
        "first pose",
        "LOD",
        "teleport",
    ] {
        for ticks in [0, 1, 2, 4] {
            let mut s = WorldSurface::<Posed, crate::ModelExecutor, true>::default();
            s.bind(&[], None).unwrap();
            s.child(0, "", None, [0., 0., 10., 10.]);
            // Establish the clock without consuming the first renderer feed.
            s.sim.as_mut().unwrap().advance(0., Clock::Seekable);
            let mut now = 0.;
            if event != "initial" {
                if event == "first pose" {
                    let w = s.sim.as_mut().unwrap().world_mut();
                    let e = w.named("actor").unwrap();
                    w.remove::<Animation>(e);
                    w.remove::<Pose>(e);
                }
                fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
                now = 50.;
                fixture::render(&gpu, &mut s, &frame(now)).unwrap();
                match event {
                    "restore" => {
                        let saved = s.carry().unwrap().unwrap();
                        s.restore(&saved, Restore::Open).unwrap();
                        s.sim.as_mut().unwrap().advance(now, Clock::Seekable);
                    }
                    "replacement" => {
                        let mut model = crate::test_model::posable_model();
                        model.nodes[0].transform[12] = 2.;
                        s.asset("actor.model", Ok(&exact_game::bin::to_vec(&model)));
                    }
                    "first pose" => {
                        let w = s.sim.as_mut().unwrap().world_mut();
                        let mut pose = Pose::default();
                        pose.local = animation::bind_pose(w.model("actor.model").unwrap());
                        pose.previous = pose.local.clone();
                        pose.local[0] = 2.;
                        let e = w.named("actor").unwrap();
                        w.insert(e, pose);
                        w.insert(e, Animation::play("bend"));
                    }
                    "LOD" => {
                        let w = s.sim.as_mut().unwrap().world_mut();
                        w.insert(
                            w.named("actor").unwrap(),
                            ModelLod {
                                levels: vec![LodLevel {
                                    distance: 1.,
                                    model: "lod.model".into(),
                                }],
                                hide: None,
                            },
                        );
                        assert!(s.assets().requests.contains(&"lod.model".into()));
                        s.asset(
                            "lod.model",
                            Ok(&exact_game::bin::to_vec(&crate::test_model::posable_model())),
                        );
                    }
                    "teleport" => {
                        let w = s.sim.as_mut().unwrap().world_mut();
                        let e = w.named("actor").unwrap();
                        w.teleport(e, Transform::default());
                    }
                    _ => unreachable!(),
                }
            }
            s.dirty = true;
            let sim = s.sim.as_ref().unwrap();
            let before = sim.world().tick();
            now += ticks as f64 * 1000. / 60. + if ticks == 0 { 0. } else { 0.001 };
            assert_eq!(sim.ticks_due(now, Clock::Seekable), ticks);
            fixture::render(&gpu, &mut s, &frame(now)).unwrap();
            assert!(s.error().is_none(), "{event}: {:?}", s.error());
            assert_eq!(s.sim().unwrap().world().tick(), before + ticks as u64);
            let len = crate::test_model::posable_model().nodes.len() * 10;
            let skin = s
                .render
                .as_ref()
                .unwrap()
                .0
                .models
                .skinning
                .as_ref()
                .unwrap();
            let span = skin.pose_words.chunks_exact(2 * len).last().unwrap();
            assert_eq!(&span[..len], &span[len..], "{event}, {ticks} ticks");
            // Socket names and chains belong to the owner's base model.
            let base = &skin.pose_words[..2 * len];
            let expected = [0, 1, 2].into_iter().fold(glam::Mat4::IDENTITY, |m, node| {
                let at = node * 10;
                m * crate::skinning::interpolated_local(
                    [
                        crate::node_poses::read(&base[at..]),
                        crate::node_poses::read(&base[len + at..]),
                    ],
                    0.5,
                )
            });
            let w = s.sim.as_ref().unwrap().world();
            let e = w.named("actor").unwrap();
            let pose = w.get::<Pose>(e).unwrap();
            assert_eq!(
                &span[len..len + 10],
                &pose.local[..10],
                "latest clip endpoint"
            );
            drop(pose);
            let charm = w.named("charm").unwrap();
            let input = s
                .render
                .as_mut()
                .unwrap()
                .1
                .frame_pixels(w, 0.5, (64., 64.));
            let drawn = input
                .attachments
                .iter()
                .find(|a| a.entity == charm)
                .unwrap()
                .matrix;
            s.placed.attachments.frame(0.5);
            let placed = s
                .placed
                .attachments
                .output
                .iter()
                .find(|a| a.entity == charm)
                .unwrap()
                .matrix;
            for actual in [drawn, placed] {
                for (a, b) in actual
                    .to_cols_array()
                    .into_iter()
                    .zip(expected.to_cols_array())
                {
                    assert!(
                        (a - b).abs() < 2e-5,
                        "{event}, {ticks} ticks: socket {a} != {b}"
                    );
                }
            }

            // Once drawn, the next tick resumes ordinary interpolation.
            fixture::render(&gpu, &mut s, &frame(now + 1000. / 60. + 0.001)).unwrap();
            let skin = s
                .render
                .as_ref()
                .unwrap()
                .0
                .models
                .skinning
                .as_ref()
                .unwrap();
            let span = skin.pose_words.chunks_exact(2 * len).last().unwrap();
            assert_ne!(
                &span[..len],
                &span[len..],
                "{event}: prime must end after drawing"
            );
        }
    }
}

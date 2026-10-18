use super::*;
use exact_game::{Args, Component, NodeName, NodeOffsets, Present};
use exact_gpu::wgpu;

#[derive(Default, Component)]
struct Gait(f32);
#[derive(Default, Args)]
struct Population {
    count: u32,
    stride: u32,
}
struct Crowd;
impl Game for Crowd {
    type Args = Population;
    const ID: &'static str = "pose-cycle-allocations";
    fn setup(w: &mut World, a: &Population) {
        for i in 0..a.count * a.stride {
            let e = w.spawn(());
            if i % a.stride == 0 {
                w.insert(e, Transform::default());
                w.insert(e, Gait(i as f32));
                w.insert(e, Mesh::asset("avatar.model"));
            }
        }
    }
    fn tick(_: &mut World, _: &Input, _: &Population) {}
    fn present(p: &mut Present<'_>, _: &Population) {
        p.insert_each::<Gait, NodeOffsets>(|reads, _, g| {
            let pitch = |v| Transform {
                rotation: Quat::from_rotation_x(v),
                ..Default::default()
            };
            let x = exact_game::math::sin(g.0 + reads.tick() as f32 * 0.1) * 0.5;
            Some(
                NodeOffsets::default()
                    .with(NodeName::new("legL"), pitch(x))
                    .with(NodeName::new("legR"), pitch(-x))
                    .with(NodeName::new("armL"), pitch(-x))
                    .with(NodeName::new("armR"), pitch(x))
                    .with(NodeName::new("torso"), Transform::at(0., x * 0.1, 0.)),
            )
        });
    }
}
#[test]
fn clear_present_feed_and_pack_allocate_nothing_clustered_or_sparse() {
    use crate::skinning::Skinning;
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let model = crate::test_model::avatar_pose_model();
    let mut skin = Skinning::new(&gpu.device, None);
    let templates = skin.add(&gpu.device, &gpu.queue, &model);
    let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 80,
        usage: wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    for (count, stride) in [(30, 1), (1000, 1), (30, 512), (1000, 17)] {
        let mut sim = Sim::<Crowd>::new(Population { count, stride }).unwrap();
        let entities: Vec<_> = sim
            .world()
            .query::<&Gait>()
            .iter()
            .map(|(e, _)| e)
            .collect();
        let records: Vec<_> = entities
            .iter()
            .flat_map(|e| {
                templates.iter().take(2).map(|&t| crate::DrawInstance {
                    data: 0,
                    transform: e.index(),
                    geometry: crate::MeshId(0),
                    material: crate::MaterialId(0),
                    local: glam::Mat4::IDENTITY,
                    skin: Some(t),
                    tint: [1.; 4],
                    glow: [0.; 3],
                })
            })
            .collect();
        skin.set(&gpu.device, &gpu.queue, &uniform, &records)
            .unwrap();
        let mut feed = Feed::default();
        let mut sink = Recording {
            models: templates
                .iter()
                .take(2)
                .map(|&t| {
                    (
                        crate::MeshId(0),
                        crate::MaterialId(0),
                        glam::Mat4::IDENTITY,
                        Some(t),
                    )
                })
                .collect(),
            record: false,
            ..Default::default()
        };
        sim.advance(0., Clock::Live);
        for tick in 1..=8 {
            sim.advance_with(tick as f64 * 1000. / 60. + 0.001, Clock::Live, |w, _| {
                feed.feed_to(w, &mut sink).unwrap();
                skin.pack(w, &entities, false);
            });
        }
        sink.record = true;
        sim.advance_with(9. * 1000. / 60. + 0.001, Clock::Live, |w, _| {
            feed.feed_to(w, &mut sink).unwrap();
            skin.pack(w, &entities, false);
        });
        assert!(
            sink.calls.iter().all(|call| !matches!(
                call,
                Call::Begin | Call::Transform(..) | Call::Previous(..)
            )),
            "node offsets must not rewrite entity transforms"
        );
        sink.record = false;
        let start = std::time::Instant::now();
        let allocations = allocations::count(|| {
            for tick in 10..=109 {
                sim.advance_with(tick as f64 * 1000. / 60. + 0.001, Clock::Live, |w, _| {
                    feed.feed_to(w, &mut sink).unwrap();
                    skin.pack(w, &entities, false);
                });
            }
        });
        let micros = start.elapsed().as_secs_f64() * 1e6 / 100.;
        println!("{count} posed avatars stride {stride}: {allocations} allocations / 100 clear-present-feed-pack cycles; {micros:.3} us/cycle ({:.3} us/avatar); {} retained presentation bytes; {} pose upload bytes",micros/count as f64,sim.world().presentation_bytes(),skin.pose_words.len()*4);
        assert_eq!(allocations, 0);
        assert_eq!(skin.records.len(), count as usize, "one span per avatar");
    }
}

//! CPU tick/clear/present and renderer feed costs for 30 and 1,000 Castle avatars.
//! Feed includes pose packing and queue writes; the reported upload bytes are
//! calculated from span sizes, not measured GPU traffic. An empty submission and
//! device wait drain writes outside the timed region. This does not encode or draw
//! a frame, measure GPU passes, shadows or allocations, compare static/far/offscreen
//! populations, or qualify mobile performance under LLP 1046.013 Proof 3.
use exact_game::*;
use exact_game_render::{Feed, Renderer};
use exact_gpu::{fixture, wgpu};
use std::time::Instant;

#[derive(Default, Component)]
struct Gait;
#[derive(Default, Args)]
struct Population {
    count: u32,
}
struct Crowd;
impl Game for Crowd {
    type Args = Population;
    const ID: &'static str = "pose-cost";
    fn setup(w: &mut World, a: &Population) {
        for i in 0..a.count {
            w.spawn((
                Transform::at(i as f32, 0., 0.),
                Mesh::asset("avatar.model"),
                Gait,
            ));
        }
    }
    fn tick(_: &mut World, _: &Input, _: &Population) {}
    fn present(p: &mut Present<'_>, _: &Population) {
        p.insert_each::<Gait, NodeOffsets>(|reads, e, _| {
            let pitch = |x| Transform {
                rotation: Quat::from_rotation_x(x),
                ..Default::default()
            };
            let swing = math::sin(reads.tick() as f32 * 0.1 + e.index() as f32) * 0.8;
            Some(
                NodeOffsets::default()
                    .with(NodeName::new("legL"), pitch(swing))
                    .with(NodeName::new("legR"), pitch(-swing))
                    .with(NodeName::new("armL"), pitch(-swing))
                    .with(NodeName::new("armR"), pitch(swing))
                    .with(NodeName::new("torso"), Transform::at(0., swing * 0.1, 0.)),
            )
        });
    }
}
fn main() {
    let gpu = fixture::device().expect("GPU adapter");
    println!(
        "adapter: {:?}; CPU timing excludes queue submission, encoding and draws",
        gpu.adapter.get_info()
    );
    let path = std::env::args().nth(1).expect("usage: poses AVATAR.glb");
    let mut model = exact_game_bake::model(path).unwrap();
    for node in &mut model.nodes {
        node.posable = matches!(
            node.name.as_str(),
            "legL" | "legR" | "armL" | "armR" | "torso"
        );
    }
    model.validate().unwrap();
    for count in [30, 1000] {
        let mut sim = Sim::<Crowd>::new(Population { count }).unwrap();
        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer.prepare_model("avatar.model", &model).unwrap();
        let mut feed = Feed::default();
        feed.feed(sim.world(), &mut renderer).unwrap();
        let mut present = 0.;
        let mut feeding = 0.;
        for i in 0..240 {
            let start = Instant::now();
            sim.run(1000. / 60.);
            let middle = Instant::now();
            feed.feed(sim.world(), &mut renderer).unwrap();
            let end = Instant::now();
            gpu.queue.submit([]);
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            if i >= 40 {
                present += (middle - start).as_secs_f64();
                feeding += (end - middle).as_secs_f64();
            }
        }
        println!("{count} avatars: tick+clear+present {:.3} us; feed {:.3} us; total {:.3} us/avatar; retained presentation {} bytes; pose upload {} bytes/tick",
            present*1e6/200.,feeding*1e6/200.,(present+feeding)*1e6/200./count as f64,
            sim.world().presentation_bytes(),count as usize*model.nodes.len()*20*4);
    }
}

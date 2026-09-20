#[path = "../src/bin/prepare.rs"]
mod prepare;
#[path = "../src/bin/readback.rs"]
mod readback;
use clod_view::{
    Mode, Renderer, View,
    scene::{Camera, Scene},
    select,
};
use glam::Vec3;
#[test]
fn backdrop_preserves_geometry_near_far_plane() {
    let mut mesh = clod_bake::procedural::octasphere(3).unwrap();
    let baked = clod_bake::bake(&mut mesh, clod_format::Config::default(), [0; 32]).unwrap();
    let reader = clod_format::Reader::new(&baked.bytes).unwrap();
    let baseline = prepare::baseline(&reader);
    let scene = Scene::layout(&reader, "single").unwrap();
    let target = Vec3::new(0.0, 0.0, 1.0);
    let camera = Camera::perspective(
        target + Vec3::new(0.0, -3000.0, 0.0),
        target,
        1.0,
        0.001,
        0.002,
        3030.0,
    );
    let (device, queue, _) = pollster::block_on(clod_view::request_device(false)).unwrap();
    let cut = select::select_culled(&reader, &scene.instances, &camera, 64, 0.0, false);
    let empty = select::Selection {
        pages: vec![vec![]; reader.pages.len()],
        ..Default::default()
    };
    let mut failures = vec![];
    for mode in [Mode::Cluster, Mode::Naive] {
        let mut renderer = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            Some(&baseline),
            &scene,
            [64, 64],
            mode,
        )
        .unwrap();
        renderer.shadows = false;
        renderer.culling = false;
        let f = renderer
            .render(&scene, &camera, View::Lit, &cut, &empty)
            .unwrap();
        let (lit, _) = readback::read(&renderer, &f).unwrap();
        println!(
            "png_bytes={}",
            readback::png_bytes(&lit, 64, 64).unwrap().len()
        );
        let f = renderer
            .render(&scene, &camera, View::Coverage, &cut, &empty)
            .unwrap();
        let (coverage, _) = readback::read(&renderer, &f).unwrap();
        // Empty cluster rendering gives the exact backdrop at the same pixels.
        let mut background = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            None,
            &scene,
            [64, 64],
            Mode::Cluster,
        )
        .unwrap();
        background.shadows = false;
        let f = background
            .render(&scene, &camera, View::Lit, &empty, &empty)
            .unwrap();
        let (bg, _) = readback::read(&background, &f).unwrap();
        let covered = coverage.chunks_exact(4).filter(|p| p[1] == 255).count();
        let visible = lit
            .chunks_exact(4)
            .zip(bg.chunks_exact(4))
            .zip(coverage.chunks_exact(4))
            .filter(|((a, b), c)| c[1] == 255 && a != b)
            .count();
        let diff = readback::difference(&lit, &bg);
        println!(
            "backdrop mode={mode:?} covered={covered} visible={visible} mean={} max={} fraction={}",
            diff.mean, diff.max, diff.fraction
        );
        if covered < 100 || visible < covered / 2 {
            failures.push(format!("{mode:?}: {visible}/{covered}"));
        }
    }
    println!("backdrop_cases=2 failures={failures:?}");
    assert!(failures.is_empty(), "{failures:?}");
}

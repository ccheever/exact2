#[path = "../../../../render/tests/fixture/device.rs"]
mod gpu_test;
use exact_game_render::exact_gpu::{self, fixture, Frame, Surface};
use exact_game_render::{Hooks, ModelExecutor, WorldSurface};
use wind_fixture_logic::WindGame;
use wind_fixture_render::{shader_dir, shaders, Wind};

fn frame(now_ms: f64) -> Frame {
    Frame {
        width: 320.,
        height: 180.,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    }
}

/// Two frames half a second apart, and the world hash after them.
fn film<H: Hooks>(gpu: &exact_gpu::Gpu) -> (fixture::Pixels, fixture::Pixels, u64) {
    let mut surface = WorldSurface::<WindGame, ModelExecutor, true, H>::default();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    surface.bind(&[], None).unwrap();
    // The first frames build and validate the hook pipelines.
    for ms in [0., 16., 33.] {
        fixture::render(gpu, &mut surface, &frame(ms)).unwrap();
    }
    let early = fixture::render(gpu, &mut surface, &frame(1000.)).unwrap().0;
    let late = fixture::render(gpu, &mut surface, &frame(1500.)).unwrap().0;
    assert!(surface.take_error().is_none());
    (early, late, surface.sim().unwrap().world().hash())
}

#[test]
fn reeds_sway_under_a_dusk_sky_without_touching_the_simulation() {
    let Some(gpu) = gpu_test::device_or_skip(fixture::device()) else {
        return;
    };
    let pack = exact_gpu::Registry {
        surfaces: &[],
        shaders: shaders::SHADERS,
    };
    exact_gpu::shaders::load_dir(&shader_dir(), &pack).unwrap();
    let (still, still_later, plain_hash) = film::<()>(&gpu);
    let (windy, windy_later, windy_hash) = film::<Wind>(&gpu);
    windy.save("wind-dusk");
    // The engine draws the field the same at both times; the wind moves it.
    assert_eq!(still, still_later, "nothing moves without the hooks");
    assert_ne!(windy, windy_later, "the reeds sway between frames");
    assert_eq!(plain_hash, windy_hash, "hooks never change the simulation");
    // Top centre is sky: the engine's is cool, the dusk pack's is warm.
    let (engine, dusk) = (still.at(160, 4), windy.at(160, 4));
    assert!(engine[2] > engine[0], "engine sky {engine:?}");
    assert!(dusk[0] > dusk[2] && dusk != engine, "dusk sky {dusk:?}");
}

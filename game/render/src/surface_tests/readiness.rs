use super::*;
use crate::{FrameView, HookGpu, Hooks, Needs, RenderError, RenderWorld};

#[derive(Default, exact_game::Args)]
struct ReadinessArgs {
    #[live]
    paused: bool,
}

struct ReadinessGame;
impl Game for ReadinessGame {
    const ID: &'static str = "submitted-frame-readiness";
    type Args = ReadinessArgs;
    fn setup(world: &mut World, _: &ReadinessArgs) {
        world.spawn((Transform::default(), Mesh::cube(1.0)));
        world.spawn((Transform::at(0., 0., 8.), Camera::default()));
        world.publish("score", 7);
    }
    fn tick(world: &mut World, _: &Input, _: &ReadinessArgs) {
        world.publish("score", world.tick() as f64);
    }
    fn paused(args: &ReadinessArgs) -> bool {
        args.paused
    }
}

#[derive(Default)]
struct Preparing {
    prepares: u32,
    pending: bool,
    hidden: bool,
}
impl Hooks for Preparing {
    fn prepare(
        &mut self,
        _: &HookGpu<'_>,
        _: &FrameView<'_>,
        _: &RenderWorld<'_>,
    ) -> Result<(), RenderError> {
        self.prepares = self.prepares.saturating_add(1);
        Ok(())
    }
    fn needs(&self) -> Needs {
        if self.prepares < 3 || self.pending {
            Needs::PENDING
        } else {
            Needs::NONE
        }
    }
    fn drawable(&self) -> bool {
        self.prepares >= 3 && !self.hidden
    }
    fn device_lost(&mut self) {
        *self = Self::default();
    }
}

type TestSurface = WorldSurface<ReadinessGame, (), false, Preparing>;

fn surface(paused: bool) -> TestSurface {
    let mut surface = TestSurface::default();
    surface.bind(&[Value::Bool(paused)], None).unwrap();
    surface
}

fn published(surface: &mut TestSurface) -> Option<serde_json::Value> {
    surface
        .published()
        .map(|record| serde_json::from_str(&record).unwrap())
}

fn encode(
    gpu: &Gpu,
    surface: &mut TestSurface,
    step: u32,
) -> (wgpu::Texture, wgpu::CommandBuffer, bool) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("submitted-readiness"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let wants = surface.render(
        &frame(f64::from(step) * 17.0),
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        &view,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    (texture, encoder.finish(), wants)
}

fn submit(
    gpu: &Gpu,
    surface: &mut TestSurface,
    texture: &wgpu::Texture,
    commands: wgpu::CommandBuffer,
) {
    gpu.queue.submit([commands]);
    surface.submitted();
    fixture::read(gpu, texture).unwrap();
}

fn complete(gpu: &Gpu, surface: &mut TestSurface, first_step: u32) -> u32 {
    for step in first_step..first_step + 32 {
        let (texture, commands, _) = encode(gpu, surface, step);
        submit(gpu, surface, &texture, commands);
        if surface.publication.ready(surface.sim().unwrap().world()) {
            return step;
        }
        assert!(surface.error().is_none(), "{:?}", surface.error());
    }
    panic!("no complete submitted frame after 32 real fixture frames");
}

#[test]
fn pending_preparation_keeps_ticks_live_and_encoding_waits_for_submission() {
    let Some(gpu) = gpu() else { return };
    let mut surface = surface(false);
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], false);
    let mut pending_tick_observed = false;
    for step in 0..32 {
        let (texture, commands, _) = encode(&gpu, &mut surface, step);
        if surface.hooks.needs().contains(Needs::PENDING) {
            pending_tick_observed |= surface.sim().unwrap().world().tick() > 0;
        }
        let complete = surface.frame_ready() && surface.hooks.drawable();
        assert!(!surface.publication.ready(surface.sim().unwrap().world()));
        if let Some(record) = published(&mut surface) {
            assert_eq!(record["frame_ready"], false);
        }
        let hash = surface.sim().unwrap().world().hash();
        let save = surface.carry().unwrap();
        submit(&gpu, &mut surface, &texture, commands);
        let record = published(&mut surface);
        assert_eq!(surface.sim().unwrap().world().hash(), hash);
        assert_eq!(surface.carry().unwrap(), save);
        if complete {
            assert_eq!(record.unwrap()["frame_ready"], true);
            assert!(pending_tick_observed);
            assert!(surface
                .sim()
                .unwrap()
                .world()
                .published("frame_ready")
                .is_none());
            assert!(published(&mut surface).is_none());
            return;
        }
        assert!(!surface.publication.ready(surface.sim().unwrap().world()));
    }
    panic!("pending hook did not finish its three prepares");
}

#[test]
fn final_paused_frame_publishes_without_another_tick_or_render() {
    let Some(gpu) = gpu() else { return };
    let mut surface = surface(true);
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], false);
    complete(&gpu, &mut surface, 0);
    assert_eq!(surface.sim().unwrap().world().tick(), 0);
    assert_eq!(
        published(&mut surface).unwrap(),
        serde_json::json!({"frame_ready":true,"score":7})
    );
    assert!(published(&mut surface).is_none());
}

#[test]
fn retained_replacement_restore_and_device_loss_publish_the_actual_lifetime() {
    let Some(gpu) = gpu() else { return };
    let mut surface = surface(true);
    published(&mut surface);
    let mut step = complete(&gpu, &mut surface, 0) + 1;
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], true);
    surface.hooks.pending = true;
    let (texture, commands, _) = encode(&gpu, &mut surface, step);
    step += 1;
    submit(&gpu, &mut surface, &texture, commands);
    assert!(surface.publication.ready(surface.sim().unwrap().world()));
    assert!(published(&mut surface).is_none());
    surface.hooks.hidden = true;
    let (texture, commands, _) = encode(&gpu, &mut surface, step);
    step += 1;
    assert!(surface.publication.ready(surface.sim().unwrap().world()));
    submit(&gpu, &mut surface, &texture, commands);
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], false);
    surface.hooks.pending = false;
    surface.hooks.hidden = false;
    step = complete(&gpu, &mut surface, step) + 1;
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], true);

    let save = surface.carry().unwrap().unwrap();
    surface.restore(&save, Restore::Open).unwrap();
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], false);
    step = complete(&gpu, &mut surface, step) + 1;
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], true);
    surface.device_lost();
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], false);
    surface.device_ready(gpu.device.features());
    complete(&gpu, &mut surface, step);
    assert_eq!(published(&mut surface).unwrap()["frame_ready"], true);
    assert_eq!(surface.sim().unwrap().world().tick(), 0);
}

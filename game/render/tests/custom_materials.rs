//! Custom materials on ordinary models: textured MASK and BLEND materials drawn
//! as the engine draws them, culled within their reach, and swapped by `ModelLod`.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d4-custom-materials-inside-the-engines-batches-phase-2
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::hooks::{CustomMaterial, MATERIAL_SHADOWS_WGSL, MATERIAL_WGSL};
use exact_game_render::WorldSurface;
use exact_game_render::{FrameView, HookGpu, Hooks, ModelExecutor, RenderError, RenderWorld};
use exact_gpu::{fixture, wgpu, Frame, Gpu, Surface, Value};

fn gpu() -> Option<Gpu> {
    test_device::device_or_skip(fixture::device())
}
fn frame(now_ms: f64) -> Frame {
    Frame {
        width: 96.,
        height: 96.,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    }
}

/// The forward and shadow shaders of each test material, after the engine's sets.
const SHADE: &str = r#"
@vertex fn vertex(@location(0) p:vec3f,@location(1) n:vec3f,@location(2) uv:vec2f,@location(3) c:vec4f,@builtin(instance_index) i:u32)->ModelVarying {
    return instance_transform(p+SHIFT,n,uv,i,c);
}
@fragment fn shaded(v:ModelVarying,@builtin(front_facing) front:bool)->@location(0) vec4f { return material_shade(v,front); }
@fragment fn green()->@location(0) vec4f { return vec4f(0.0,2.0,0.0,1.0); }
"#;
const SHADOW: &str = r#"
@group(1) @binding(0) var<uniform> light:mat4x4f;
@vertex fn shadow(@location(0) p:vec3f,@location(1) n:vec3f,@location(2) uv:vec2f,@location(3) c:vec4f,@builtin(instance_index) i:u32)->ModelVarying {
    var v=instance_transform(p+SHIFT,n,uv,i,c);
    v.clip=light*vec4f(v.world,1.0);
    return v;
}
@fragment fn cutout(v:ModelVarying) { if model_discarded(v) { discard; } }
"#;

/// A custom material over `model`'s first material. `SHADED`: the engine's own
/// shading (`material_shade`), else flat green; `CUTOUT`: the shadow pass drops
/// what the engine's does; `SHIFT`: the vertex shaders move every vertex this far
/// along -x; `REACH`: the declared reach (`u32::MAX`: unbounded).
#[derive(Default)]
struct Custom<const SHADED: bool, const CUTOUT: bool, const SHIFT: u32, const REACH: u32> {
    materials: Vec<CustomMaterial>,
}
impl<const SHADED: bool, const CUTOUT: bool, const SHIFT: u32, const REACH: u32> Hooks
    for Custom<SHADED, CUTOUT, SHIFT, REACH>
{
    fn prepare(
        &mut self,
        gpu: &HookGpu<'_>,
        _: &FrameView<'_>,
        _: &RenderWorld<'_>,
    ) -> Result<(), RenderError> {
        let materials = gpu.materials.as_ref().unwrap();
        let Some(material) = materials.material("panel.model", 0) else {
            return Ok(());
        };
        if !self.materials.is_empty() {
            return Ok(());
        }
        let shift = format!("const SHIFT=vec3f(-{SHIFT}.0,0.0,0.0);\n");
        let module = |text: &str, lights: bool| {
            let lights = if lights { MATERIAL_SHADOWS_WGSL } else { "" };
            gpu.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("custom material test"),
                    source: wgpu::ShaderSource::Wgsl(
                        format!("{MATERIAL_WGSL}\n{lights}\n{shift}{text}").into(),
                    ),
                })
        };
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[],
            });
        let fragment = if SHADED { "shaded" } else { "green" };
        let forward = materials.pipeline(
            material,
            &module(SHADE, true),
            &layout,
            "vertex",
            Some(fragment),
            false,
        );
        let shadow = materials.pipeline(
            material,
            &module(SHADOW, false),
            &layout,
            "shadow",
            CUTOUT.then_some("cutout"),
            true,
        );
        self.materials.push(CustomMaterial {
            material,
            forward,
            shadow,
            resources: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[],
            }),
            reach: if REACH == u32::MAX {
                f32::INFINITY
            } else {
                REACH as f32
            },
        });
        Ok(())
    }
    fn materials(&self) -> &[CustomMaterial] {
        &self.materials
    }
}

/// Render a few frames (the first builds the hooks' pipelines and draws unmerged).
fn render<G: Game, H: Hooks>(
    gpu: &Gpu,
    args: &[Value],
    texture: Option<asset::TextureData>,
) -> fixture::Pixels {
    let mut surface = WorldSurface::<G, ModelExecutor, true, H>::default();
    surface.device_ready(wgpu::Features::empty());
    surface.bind(args, None).unwrap();
    if let Some(texture) = texture {
        surface.asset("cut.tex", Ok(&bin::to_vec(&texture)));
        surface.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    }
    let mut image = None;
    for i in 0..3 {
        image = Some(
            fixture::render(gpu, &mut surface, &frame(16. * f64::from(i)))
                .unwrap()
                .0,
        );
        assert!(surface.error().is_none(), "{:?}", surface.error());
    }
    image.unwrap()
}

// 8 x 8, nearest: the left half transparent red, the right half half-opaque green.
fn cut() -> asset::TextureData {
    let mips = [8usize, 4, 2, 1]
        .iter()
        .map(|&size| {
            (0..size * size)
                .flat_map(|i| match (size, i % size < size / 2) {
                    (1, _) => [128, 128, 0, 64],
                    (_, true) => [255, 0, 0, 0],
                    _ => [0, 255, 0, 128],
                })
                .collect()
        })
        .collect();
    asset::TextureData {
        width: 8,
        height: 8,
        mips,
        srgb: true,
        filter: [asset::Filter::Nearest; 3],
        ..Default::default()
    }
}

#[derive(Default, Args)]
struct CutoutArgs {
    blend: bool,
}
/// A cut-out panel lying flat over a lit floor, its shadow below it.
struct Cutout;
impl Game for Cutout {
    type Args = CutoutArgs;
    const ID: &'static str = "custom-material-cutout";
    const ASSETS: &'static [&'static str] = &["cut.tex"];
    fn setup(w: &mut World, args: &CutoutArgs) {
        let quad = asset::MeshData {
            positions: vec![-1., -1., 0., 1., -1., 0., 1., 1., 0., -1., 1., 0.],
            normals: [0., 0., 1.].repeat(4),
            uvs: vec![0., 1., 1., 1., 1., 0., 0., 0.],
            colors: [1.; 4].repeat(4),
            indices: vec![0, 1, 2, 0, 2, 3],
            bounds: [-1., -1., 0., 1., 1., 0.],
            ..Default::default()
        };
        let panel = w
            .generated_model(
                "panel.model",
                asset::Model {
                    bounds: quad.bounds,
                    meshes: vec![quad],
                    materials: vec![asset::MaterialData {
                        metallic: 0.,
                        base_color_texture: Some(0),
                        alpha_mode: if args.blend {
                            asset::AlphaMode::Blend
                        } else {
                            asset::AlphaMode::Mask
                        },
                        alpha_cutoff: 0.4,
                        double_sided: true,
                        ..Default::default()
                    }],
                    textures: vec!["cut.tex".into()],
                    nodes: vec![asset::Node {
                        mesh: Some(0),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
        w.spawn((
            Transform {
                rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                ..Transform::at(0., 1., 0.)
            },
            panel,
        ));
        w.spawn((
            Transform::default(),
            Mesh::plane(8., 8.),
            Material::rgb(0.8, 0.8, 0.8),
        ));
        w.spawn((
            Transform::at(0., 6., 3.).looking_at(Vec3::new(0., 0.5, 0.), Vec3::Y),
            Camera::default(),
        ));
        w.spawn((
            Transform::at(0.5, 4., 0.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            fog: None,
            bloom: None,
            ..Default::default()
        });
    }
    fn tick(_: &mut World, _: &Input, _: &CutoutArgs) {}
    fn paused(_: &CutoutArgs) -> bool {
        true
    }
}

/// Pixels that differ by more than `tolerance` in any channel.
fn differing(a: &fixture::Pixels, b: &fixture::Pixels, tolerance: u8) -> usize {
    (0..a.height)
        .flat_map(|y| (0..a.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let (p, q) = (a.at(x, y), b.at(x, y));
            (0..3).any(|c| p[c].abs_diff(q[c]) > tolerance)
        })
        .count()
}

#[test]
fn material_shade_draws_masked_and_blended_textures_as_the_engine_does() {
    let Some(gpu) = gpu() else { return };
    type Shaded = Custom<true, true, 0, 0>;
    for blend in [false, true] {
        let args = [Value::Bool(blend)];
        let stock = render::<Cutout, ()>(&gpu, &args, Some(cut()));
        let custom = render::<Cutout, Shaded>(&gpu, &args, Some(cut()));
        let name = if blend { "custom-blend" } else { "custom-mask" };
        stock.save(&format!("{name}-stock"));
        custom.save(name);
        let off = differing(&stock, &custom, 2);
        eprintln!("{name}: {off} pixels differ from the engine's own drawing");
        assert!(off <= 8, "{name}: {off} pixels differ");
        // The panel's transparent left half shows the floor; its right half is green.
        let (left, right) = (stock.at(36, 48), stock.at(60, 48));
        assert!(right[1] > right[0] + 30, "{name}: green half {right:?}");
        assert!(
            left[0].abs_diff(left[1]) < 12,
            "{name}: floor through the cut {left:?}"
        );
    }
}

#[test]
fn a_shadow_pass_without_the_cutout_shadows_the_cut_half() {
    let Some(gpu) = gpu() else { return };
    let args = [Value::Bool(false)];
    let cutout = render::<Cutout, Custom<true, true, 0, 0>>(&gpu, &args, Some(cut()));
    let solid = render::<Cutout, Custom<true, false, 0, 0>>(&gpu, &args, Some(cut()));
    solid.save("custom-mask-solid-shadow");
    let off = differing(&cutout, &solid, 8);
    eprintln!("an uncut shadow darkens {off} pixels");
    assert!(off > 40, "{off}");
}

#[derive(Default, Args)]
struct PanelArgs {
    x: f64,
    camera: f64,
    lod: bool,
}
/// One plain panel at `x`, seen from `camera` metres down +z, with a red far level.
struct Panel;
impl Game for Panel {
    type Args = PanelArgs;
    const ID: &'static str = "custom-material-panel";
    fn setup(w: &mut World, args: &PanelArgs) {
        let quad = |color: [f32; 4]| asset::MeshData {
            positions: vec![-0.8, -0.8, 0., 0.8, -0.8, 0., 0.8, 0.8, 0., -0.8, 0.8, 0.],
            normals: [0., 0., 1.].repeat(4),
            uvs: vec![0.; 8],
            colors: color.repeat(4),
            indices: vec![0, 1, 2, 0, 2, 3],
            bounds: [-0.8, -0.8, 0., 0.8, 0.8, 0.],
            ..Default::default()
        };
        let panel = w.generated("panel.model", quad([1.; 4])).unwrap();
        w.generated("far.model", quad([1., 0., 0., 1.])).unwrap();
        let at = Transform::at(args.x as f32, 0., 0.);
        if args.lod {
            let lod = ModelLod {
                levels: vec![LodLevel {
                    distance: 10.,
                    model: "far.model".into(),
                }],
                hide: None,
            };
            w.spawn((at, panel, lod));
        } else {
            w.spawn((at, panel));
        }
        w.spawn((Transform::at(0., 0., args.camera as f32), Camera::default()));
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            ambient: 1.,
            zenith: [1.; 3],
            horizon: [1.; 3],
            ground: [1.; 3],
            fog: None,
            bloom: None,
            ..Default::default()
        });
    }
    fn tick(_: &mut World, _: &Input, _: &PanelArgs) {}
    fn paused(_: &PanelArgs) -> bool {
        true
    }
}

#[test]
fn custom_materials_are_culled_outside_their_reach() {
    let Some(gpu) = gpu() else { return };
    // The panel stands 6 m right of the view; its vertex shaders move it 6 m left.
    let args = [Value::Number(6.), Value::Number(5.), Value::Bool(false)];
    let green = |image: &fixture::Pixels| image.at(48, 48)[1] > 150;
    let unbounded = render::<Panel, Custom<false, false, 6, { u32::MAX }>>(&gpu, &args, None);
    let reached = render::<Panel, Custom<false, false, 6, 7>>(&gpu, &args, None);
    let short = render::<Panel, Custom<false, false, 6, 0>>(&gpu, &args, None);
    assert!(
        green(&unbounded) && green(&reached),
        "kept within its reach"
    );
    assert!(!green(&short), "culled where its bounds say it is");
}

#[test]
fn custom_materials_follow_model_lod() {
    let Some(gpu) = gpu() else { return };
    let at = |camera: f64| {
        let args = [Value::Number(0.), Value::Number(camera), Value::Bool(true)];
        render::<Panel, Custom<false, false, 0, 0>>(&gpu, &args, None).at(48, 48)
    };
    let (near, far) = (at(5.), at(20.));
    assert!(
        near[1] > 150 && near[0] < 60,
        "the custom level near: {near:?}"
    );
    assert!(
        far[0] > 100 && far[1] < 60,
        "the stock far level, alone: {far:?}"
    );
}

//! Public, independently authored render-hook proof; no consumer game or ocean code.
//! @ref llp/1046.006.000-render-hooks.rfc.md#5-order-of-work
use exact_game::{Environment, Game, Input, World};
use exact_game_render::{
    FrameView, HookGpu, Hooks, Needs, PostInputs, RenderError, RenderWorld, SceneCopy, WorldSurface,
};
#[cfg(not(target_arch = "wasm32"))]
use exact_gpu::fixture;
use exact_game::Args;
use exact_gpu::{wgpu, Frame, Surface, Value};

pub struct TestGame;
impl Game for TestGame {
    type Args = ();
    const ID: &'static str = "public-render-hooks";
    fn setup(world: &mut World, _: &()) {
        world.insert_resource(Environment {
            background: Some([0.; 3]),
            bloom: None,
            fog: None,
            ..Default::default()
        });
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
    fn paused(_: &()) -> bool {
        true
    }
}

const DRAW: &str = r#"
@group(0) @binding(0) var<storage, read> color: array<vec4f>;
@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4f {
    let x = f32((i<<1u)&2u); let y=f32(i&2u);
    return vec4f(vec2f(x,y)*0.8-0.4,0.6,1.0);
}
@fragment fn fs()->@location(0) vec4f { return color[0]; }
"#;
const COMPUTE: &str = r#"
@group(0) @binding(0) var<storage, read_write> color: array<vec4f>;
@compute @workgroup_size(1) fn cs() { color[0]=vec4f(1.0,0.0,0.0,1.0); }
"#;
const SAMPLE: &str = r#"
@group(0) @binding(0) var color: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4f {
    let p=vec2f(f32((i<<1u)&2u),f32(i&2u));
    return vec4f(p*2.0-1.0,0.3,1.0);
}
@fragment fn surface(@builtin(position) p:vec4f)->@location(0) vec4f {
    let c=textureLoad(color,vec2i(p.xy),0);
    let d=textureLoad(depth,vec2i(p.xy),0).r;
    return vec4f(c.r*0.5,select(0.0,0.1,d>0.0),0.5,1.0);
}
@fragment fn post(@builtin(position) p:vec4f)->@location(0) vec4f {
    let c=textureLoad(color,vec2i(p.xy),0);
    let d=textureLoad(depth,vec2i(p.xy),0).r;
    return vec4f(c.rgb+vec3f(0.0,select(0.0,0.1,d>0.0),0.0),1.0);
}
"#;
fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    fragment: &str,
    samples: u32,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("public hook fixture"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: (samples == 4).then_some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}
struct Gpu {
    device: wgpu::Device,
    compute: wgpu::ComputePipeline,
    compute_group: wgpu::BindGroup,
    opaque: wgpu::RenderPipeline,
    opaque_group: wgpu::BindGroup,
    sampling: wgpu::BindGroupLayout,
    surface: wgpu::RenderPipeline,
    post: wgpu::RenderPipeline,
    fallback: wgpu::TextureView,
}
impl Gpu {
    fn new(gpu: &HookGpu<'_>) -> Self {
        let d = gpu.device;
        let shader = |source: &'static str| {
            d.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("public hook fixture"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        };
        let buffer = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hook compute result"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let storage = |read_only| {
            d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: if read_only {
                        wgpu::ShaderStages::FRAGMENT
                    } else {
                        wgpu::ShaderStages::COMPUTE
                    },
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            })
        };
        let read = storage(true);
        let write = storage(false);
        let group = |layout: &wgpu::BindGroupLayout| {
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };
        let compute_group = group(&write);
        let opaque_group = group(&read);
        let compute = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fixture compute"),
            layout: Some(&d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&write)],
                immediate_size: 0,
            })),
            module: &shader(COMPUTE),
            entry_point: Some("cs"),
            compilation_options: Default::default(),
            cache: None,
        });
        let sampling = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[0, 1].map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }),
        });
        let fallback = d
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 128,
                    height: 128,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Self {
            device: d.clone(),
            compute,
            compute_group,
            opaque: pipeline(d, &shader(DRAW), &read, "fs", 4),
            opaque_group,
            surface: pipeline(d, &shader(SAMPLE), &sampling, "surface", 4),
            post: pipeline(d, &shader(SAMPLE), &sampling, "post", 1),
            sampling,
            fallback,
        }
    }
    fn sample(&self, color: &wgpu::TextureView, depth: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fixture sampled inputs"),
            layout: &self.sampling,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(color),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
            ],
        })
    }
}
#[derive(Default)]
pub struct TestHooks<const SCENE: bool, const POST: bool, const DEPTH: bool> {
    gpu: exact_game_render::hooks::Pipelines<Gpu>,
    stages: Vec<&'static str>,
    resets: u32,
}
impl<const S: bool, const P: bool, const D: bool> Hooks for TestHooks<S, P, D> {
    fn prepare(
        &mut self,
        gpu: &HookGpu<'_>,
        view: &FrameView<'_>,
        world: &RenderWorld<'_>,
    ) -> Result<(), RenderError> {
        assert_eq!(world.tick(), 0);
        assert_eq!(view.sample_count, 4);
        self.stages.clear();
        self.stages.push("prepare");
        self.resets += u32::from(view.time.reset);
        self.gpu
            .update(gpu.device, view.time.shader_generation, || Gpu::new(gpu));
        Ok(())
    }
    fn needs(&self) -> Needs {
        let mut needs = Needs::ANIMATE;
        if self.gpu.pending() {
            needs = needs | Needs::PENDING;
        }
        if S {
            needs = needs | Needs::SCENE_COPY;
        }
        if P {
            needs = needs | Needs::HDR_POST;
        }
        if D {
            needs = needs | Needs::FINAL_DEPTH;
        }
        needs
    }
    fn drawable(&self) -> bool {
        self.gpu.get().is_some()
    }
    fn error(&self) -> Option<&str> {
        self.gpu.error()
    }
    fn compute(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        _: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        self.stages.push("compute");
        let gpu = self.gpu.get().unwrap();
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&gpu.compute);
        pass.set_bind_group(0, &gpu.compute_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
        Ok(())
    }
    fn opaque(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        _: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        self.stages.push("opaque");
        let gpu = self.gpu.get().unwrap();
        pass.set_pipeline(&gpu.opaque);
        pass.set_bind_group(0, &gpu.opaque_group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
    fn background(
        &mut self,
        _: &mut wgpu::RenderPass<'_>,
        _: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        self.stages.push("background");
        Ok(())
    }
    fn surface(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        scene: &SceneCopy<'_>,
        _: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        self.stages.push("surface");
        let gpu = self.gpu.get().unwrap();
        let group = gpu.sample(scene.color, scene.depth);
        pass.set_pipeline(&gpu.surface);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
    fn post(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        inputs: &PostInputs<'_>,
        view: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        self.stages.push("post");
        assert_eq!(inputs.depth.is_some(), D);
        assert_eq!(inputs.opaque_depth.is_some(), S);
        let gpu = self.gpu.get().unwrap();
        let group = gpu.sample(inputs.input, inputs.depth.unwrap_or(&gpu.fallback));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fixture HDR post"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: inputs.output,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(0., 0., view.pixels.0 as f32, view.pixels.1 as f32, 0., 1.);
        pass.set_pipeline(&gpu.post);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn frame() -> Frame {
    Frame {
        width: 64.,
        height: 64.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn check<const S: bool, const P: bool, const D: bool>() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut surface = WorldSurface::<TestGame, (), false, TestHooks<S, P, D>>::default();
    surface.bind(&[], None).unwrap();
    let mut f = frame();
    let hash = surface.sim().unwrap().world().hash();
    let save = surface.sim().unwrap().world().save();
    let (image, wants) = fixture::render(&gpu, &mut surface, &f).unwrap();
    assert!(
        wants,
        "hook requests presentation even when simulation is paused"
    );
    assert_eq!(surface.sim().unwrap().world().hash(), hash);
    assert!(surface.error().is_none(), "{:?}", surface.error());
    let mut stages = vec!["prepare", "compute", "opaque", "background"];
    if S {
        stages.push("surface");
    }
    if P {
        stages.push("post");
    }
    assert_eq!(surface.render_hooks().stages, stages);
    let center = image.at(32, 32);
    let edge = image.at(2, 2);
    assert!(
        center[0] > edge[0] + 20,
        "compute → opaque → sampled scene: {center:?} / {edge:?}"
    );
    if S {
        assert!(
            edge[2] > 100,
            "surface should write blue into HDR: {edge:?}"
        );
    }
    if D && P {
        assert!(
            center[1] > 20,
            "post must see nonzero linear final depth: {center:?}"
        );
    }
    f.now_ms = 20.;
    fixture::render(&gpu, &mut surface, &f).unwrap();
    assert_eq!(surface.render_hooks().resets, 1);
    f.width = 65.;
    f.height = 67.;
    fixture::render(&gpu, &mut surface, &f).unwrap();
    surface.device_lost();
    fixture::render(&gpu, &mut surface, &f).unwrap();
    assert_eq!(surface.render_hooks().resets, 1);
    assert_eq!(surface.sim().unwrap().world().hash(), hash);
    assert_eq!(surface.sim().unwrap().world().save(), save);
    f.now_ms = 3000.;
    fixture::render(&gpu, &mut surface, &f).unwrap();
    assert_eq!(
        surface.render_hooks().resets,
        2,
        "large seek resets history"
    );
    let carry = surface.carry().unwrap().unwrap();
    surface.restore(&carry, exact_gpu::Restore::Carry).unwrap();
    fixture::render(&gpu, &mut surface, &f).unwrap();
    assert_eq!(surface.render_hooks().resets, 3, "restore resets history");
    assert_eq!(surface.sim().unwrap().world().save(), save);
    let tree = surface.agent("{\"op\":\"tree\"}").unwrap();
    assert!(tree.contains("renderHooks"), "{tree}");
    let state = surface.agent("{\"op\":\"state\"}").unwrap();
    assert!(!state.contains("render error"), "{state}");
    if S && P && D {
        surface.agent("{\"op\":\"state\",\"perf\":true}");
        f.seekable = false;
        for _ in 0..8 {
            f.now_ms += 16.;
            fixture::render(&gpu, &mut surface, &f).unwrap();
        }
        let state = surface.agent("{\"op\":\"state\"}").unwrap();
        if gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            assert!(state.contains("\"gpuMs\""), "{state}");
            assert!(state.contains("\"gpuTimingError\":null"), "{state}");
        }
    }
}
#[test]
fn all_stages_and_recovery() {
    check::<true, true, true>();
}
#[test]
fn hdr_post_without_scene_copy() {
    check::<false, true, true>();
}
#[test]
fn opaque_hook_without_extra_attachments() {
    check::<false, false, false>();
}
#[test]
fn scene_copy_without_post() {
    check::<true, false, false>();
}

#[test]
fn rejected_pipeline_update_keeps_last_valid_set() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut cache = exact_game_render::hooks::Pipelines::default();
    cache.update(&gpu.device, 0, || 123u32);
    assert_eq!(cache.get(), Some(&123));
    cache.update(&gpu.device, 1, || {
        let _ = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("deliberate invalid candidate"),
                source: wgpu::ShaderSource::Wgsl("not valid WGSL".into()),
            });
        456
    });
    assert_eq!(cache.get(), Some(&123));
    assert!(cache
        .error()
        .unwrap()
        .contains("deliberate invalid candidate"));
    cache.update(&gpu.device, 2, || 789);
    assert_eq!(cache.get(), Some(&789));
    assert!(cache.error().is_none());
}

struct MaterialGame;
#[derive(Default, Args)]
struct MaterialArgs {
    caster: bool,
}
impl Game for MaterialGame {
    type Args = MaterialArgs;
    const ID: &'static str = "custom-material-public-proof";
    fn setup(w: &mut World, args: &MaterialArgs) {
        use exact_game::*;
        let mesh = w
            .generated(
                "panel.model",
                asset::MeshData {
                    positions: vec![-0.8, -0.8, 0., 0.8, -0.8, 0., 0.8, 0.8, 0., -0.8, 0.8, 0.],
                    normals: [0., 0., 1.].repeat(4),
                    uvs: vec![0.; 8],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    bounds: [-0.8, -0.8, 0., 0.8, 0.8, 0.],
                    ..Default::default()
                },
            )
            .unwrap();
        w.spawn((Transform::default(), mesh));
        w.spawn((Transform::at(0., 0., 5.), Camera::default()));
        if args.caster {
            // Between the sun and the panel's upper right, out of the camera's way.
            w.spawn((
                Transform::at(1.2, 1.8, 2.4),
                Mesh::cube(0.5),
                Material::rgb(1., 1., 1.),
            ));
        }
        w.spawn((
            Transform::at(2., 3., 4.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            fog: None,
            bloom: None,
            ..Default::default()
        });
    }
    fn tick(_: &mut World, _: &Input, _: &MaterialArgs) {}
    fn paused(_: &MaterialArgs) -> bool {
        true
    }
}
// SHADOWS: the forward shader outputs the engine's sun visibility as green.
#[derive(Default)]
struct MaterialHooks<const SHADOWS: bool = false> {
    materials: Vec<exact_game_render::hooks::CustomMaterial>,
}
impl<const SHADOWS: bool> Hooks for MaterialHooks<SHADOWS> {
    fn prepare(
        &mut self,
        gpu: &HookGpu<'_>,
        _: &FrameView<'_>,
        _: &RenderWorld<'_>,
    ) -> Result<(), RenderError> {
        if !self.materials.is_empty() {
            return Ok(());
        }
        let gpu_materials = gpu.materials.as_ref().unwrap();
        let Some(material) = gpu_materials.material("panel.model", 0) else {
            return Ok(());
        };
        let forward = r#"
        @vertex fn vertex(@location(0) p:vec3f,@location(1) n:vec3f,@builtin(instance_index) i:u32)->Varying {
            return instance_transform(p+vec3f(f32(draw_instance(i).data)*0.01,0.0,0.0),n,i,vec4f(1.0));
        }
        @fragment fn fragment()->@location(0) vec4f {return vec4f(0.0,2.0,0.0,1.0);}
        "#;
        let shadowed = r#"
        @vertex fn vertex(@location(0) p:vec3f,@location(1) n:vec3f,@builtin(instance_index) i:u32)->Varying {
            return instance_transform(p,n,i,vec4f(1.0));
        }
        @fragment fn fragment(v:Varying)->@location(0) vec4f {
            return vec4f(0.0,2.0*sun_shadow(v.world,normalize(v.normal)),0.0,1.0);
        }
        "#;
        let forward = if SHADOWS {
            format!("{}\n{shadowed}", exact_game_render::hooks::MATERIAL_SHADOWS_WGSL)
        } else {
            forward.to_owned()
        };
        let shadow = r#"
        @group(1) @binding(0) var<uniform> light:mat4x4f;
        @vertex fn shadow(@location(0) p:vec3f,@location(1) n:vec3f,@builtin(instance_index) i:u32)->@builtin(position) vec4f {
            let v=instance_transform(p+vec3f(f32(draw_instance(i).data)*0.01,0.0,0.0),n,i,vec4f(1.0));
            return light*vec4f(v.world,1.0);
        }
        "#;
        let shader = |text: &str| {
            gpu.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("public custom material"),
                    source: wgpu::ShaderSource::Wgsl(
                        format!("{}\n{text}", exact_game_render::hooks::MATERIAL_WGSL).into(),
                    ),
                })
        };
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[],
            });
        self.materials
            .push(exact_game_render::hooks::CustomMaterial {
                material,
                forward: gpu_materials.pipeline(
                    &shader(&forward),
                    &layout,
                    "vertex",
                    Some("fragment"),
                    false,
                ),
                shadow: gpu_materials.pipeline(&shader(shadow), &layout, "shadow", None, true),
                resources: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &layout,
                    entries: &[],
                }),
            });
        Ok(())
    }
    fn materials(&self) -> &[exact_game_render::hooks::CustomMaterial] {
        &self.materials
    }
    fn instance_data(&self, _: u32) -> u32 {
        10
    }
}
#[test]
fn custom_material_uses_engine_instances_and_paired_shadow() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut surface = WorldSurface::<
        MaterialGame,
        exact_game_render::ModelPresentation,
        true,
        MaterialHooks,
    >::default();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    surface.bind(&[Value::Bool(false)], None).unwrap();
    let hash = surface.sim().unwrap().world().hash();
    let save = surface.sim().unwrap().world().save();
    let (image, _) = fixture::render(&gpu, &mut surface, &frame()).unwrap();
    assert!(surface.error().is_none(), "{:?}", surface.error());
    let center = image.at(32, 32);
    assert!(center[1] > 150 && center[0] < 20, "{center:?}");
    assert_eq!(surface.sim().unwrap().world().hash(), hash);
    assert_eq!(surface.sim().unwrap().world().save(), save);
}

#[test]
fn custom_material_forward_shaders_sample_the_engine_shadow_maps() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let render = |caster: bool| {
        let mut surface = WorldSurface::<
            MaterialGame,
            exact_game_render::ModelPresentation,
            true,
            MaterialHooks<true>,
        >::default();
        surface.device_ready(exact_gpu::wgpu::Features::empty());
        surface.bind(&[Value::Bool(caster)], None).unwrap();
        let (image, _) = fixture::render(&gpu, &mut surface, &frame()).unwrap();
        assert!(surface.error().is_none(), "{:?}", surface.error());
        image.save(if caster { "custom-shadowed" } else { "custom-unshadowed" });
        image
    };
    let (open, shadowed) = (render(false), render(true));
    let lit = |image: &fixture::Pixels| {
        (16..48)
            .flat_map(|y| (16..48).map(move |x| (x, y)))
            .filter(|&(x, y)| image.at(x, y)[1] > 100)
            .count()
    };
    let (open, shadowed) = (lit(&open), lit(&shadowed));
    eprintln!("lit panel pixels without and with the caster: {open} {shadowed}");
    assert!(open > 200, "{open}");
    assert!(shadowed + 40 < open, "{open} {shadowed}");
}

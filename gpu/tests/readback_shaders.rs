//! Headless ownership does not admit unregistered shaders into rendered readback.
use exact_gpu::{
    fixture, shaders, wgpu, Frame, InputEvent, Module, Registry, Surface, SurfaceError, Value,
};

const SOURCE: &str = r#"
@vertex fn vs(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {
  let p=array<vec2<f32>,3>(vec2<f32>(-1.,-1.),vec2<f32>(3.,-1.),vec2<f32>(-1.,3.));
  return vec4<f32>(p[i],0.,1.);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.,0.,0.,1.); }
"#;

#[derive(Default)]
struct Draw(u8);
impl Surface for Draw {
    fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn input(&mut self, _: &InputEvent) {
        self.0 += 1;
    }
    fn carry(&mut self) -> Result<Option<Vec<u8>>, SurfaceError> {
        Ok(Some(vec![self.0]))
    }
    fn render(
        &mut self,
        _: &Frame,
        device: &wgpu::Device,
        _: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        // The same source lookup used by generated reflection's module(). The
        // foreground is a real shader draw, distinguishable from the blue clear.
        let module = device.create_shader_module(shaders::shader_module("readback_probe"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("registered readback probe"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
        false
    }
}

#[test]
fn missing_sources_refuse_readback_but_headless_bind_input_and_save_work() {
    let digest = shaders::interface_digest(SOURCE).unwrap();
    let registry = Box::leak(Box::new(Registry {
        surfaces: &[("probe", 0, || Box::<Draw>::default())],
        shaders: Box::leak(Box::new([("readback_probe", digest)])),
    }));
    let mut module = Module::new(registry);
    let id = module.create_headless("probe").unwrap();
    assert!(module.bind(id, &[], None));
    assert!(module.input_json(
        id,
        r#"{"t":"key","code":"KeyA","key":"a","down":true,"repeat":false,"at":0}"#
    ));
    assert_eq!(module.carry(id).unwrap(), Some(vec![1]));
    let frame = Frame {
        width: 8.,
        height: 8.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    };
    assert!(module.readback(id, &frame).is_none());
    assert!(module
        .take_error()
        .contains("shader `readback_probe` has no source"));
    assert_eq!(module.carry(id).unwrap(), Some(vec![1]));
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    module.set_gpu(gpu);
    assert!(module.readback(id, &frame).is_none());
    assert!(module.take_error().contains("readback_probe"));
    assert!(module.set_shader("readback_probe", SOURCE.into()));
    let (pixels, _) = module
        .readback(id, &frame)
        .expect("registered source produces pixels");
    assert!(pixels.data.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
    assert!(module.set_shader(
        "readback_probe",
        SOURCE.replace("vec4<f32>(1.,0.,0.,1.)", "vec4<f32>(0.,1.,0.,1.)")
    ));
    let (pixels, _) = module.readback(id, &frame).unwrap();
    assert!(pixels.data.chunks_exact(4).all(|p| p == [0, 255, 0, 255]));
    assert_eq!(module.carry(id).unwrap(), Some(vec![1]));
}

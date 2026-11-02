//! A canvas's buffers on the phone's GPU: what replaces a target, and how
//! many frames a ring holds. They run on a device: build with `cargo test
//! -p exact-gpu --lib --target aarch64-linux-android --no-run` (the NDK's
//! clang as the linker, as `scripts/agent-android.mjs` sets it), then run
//! the binary under `adb shell`.
use crate::buffers::IN_FLIGHT;
use crate::{fixture, wgpu, Frame, Module, Registry, Surface, SurfaceError, Value};
use std::ffi::c_void;

/// Clears its target.
struct Clearing;

impl Surface for Clearing {
    fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn render(
        &mut self,
        _: &Frame,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        _: wgpu::TextureFormat,
    ) -> bool {
        pass(encoder, target);
        true
    }
}

fn pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// Covers its target with one triangle whose every fragment loops: a frame
/// the GPU takes far longer to run than the module to record.
#[derive(Default)]
struct Busy {
    pipeline: Option<wgpu::RenderPipeline>,
}

const BUSY: &str = "
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
    var x = fract(p.x * 0.013 + p.y * 0.007);
    for (var i = 0u; i < 20000u; i++) {
        x = fract(x * 1.618 + 0.31);
    }
    return vec4f(x, 0.0, 0.0, 1.0);
}";

impl Surface for Busy {
    fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        Ok(())
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
        let pipeline = self.pipeline.get_or_insert_with(|| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(BUSY.into()),
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
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
            })
        });
        let mut pass = pass(encoder, target);
        pass.set_pipeline(pipeline);
        pass.draw(0..3, 0..1);
        true
    }
}

static REGISTRY: Registry = Registry {
    surfaces: &[
        ("clear", 0, || Box::new(Clearing)),
        ("busy", 0, || Box::<Busy>::default()),
    ],
    shaders: &[],
};

fn frame(size: f32) -> Frame {
    Frame {
        width: size,
        height: size,
        scale: 1.,
        now_ms: 0.,
        seekable: false,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    }
}

/// A module under the agent's clock: no acquisition thread holds a
/// window's surface beside the canvas.
fn module() -> Option<Module> {
    let gpu = fixture::device_or_skip(fixture::device())?;
    let mut m = Module::new(&REGISTRY);
    m.set_gpu(gpu);
    m.set_seekable(true);
    Some(m)
}

/// A bound canvas of surface `name` with no target.
fn canvas(m: &mut Module, name: &str) -> u32 {
    let id = m.create_headless(name).unwrap();
    assert!(m.bind(id, &[], None));
    id
}

#[link(name = "mediandk")]
extern "C" {
    fn AImageReader_newWithUsage(
        width: i32,
        height: i32,
        format: i32,
        usage: u64,
        max_images: i32,
        reader: *mut *mut c_void,
    ) -> i32;
    fn AImageReader_getWindow(reader: *mut c_void, window: *mut *mut c_void) -> i32;
    fn AImageReader_delete(reader: *mut c_void);
}

/// A window no compositor shows: an `AImageReader`'s, which owns it.
struct Window {
    reader: *mut c_void,
    window: *mut c_void,
}

impl Window {
    fn new(size: i32) -> Window {
        /// `AIMAGE_FORMAT_RGBA_8888`.
        const RGBA_8888: i32 = 1;
        /// `AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE | GPU_COLOR_OUTPUT`.
        const USAGE: u64 = (1 << 8) | (1 << 9);
        let (mut reader, mut window) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: the out pointers outlive the calls; the reader is deleted
        // on drop, after every surface made on its window.
        unsafe {
            assert_eq!(
                AImageReader_newWithUsage(size, size, RGBA_8888, USAGE, 4, &mut reader),
                0
            );
            assert_eq!(AImageReader_getWindow(reader, &mut window), 0);
        }
        Window { reader, window }
    }

    fn surface(&self, m: &Module) -> wgpu::Surface<'static> {
        use wgpu::rwh::{
            AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle,
        };
        // SAFETY: the window lives until the reader is deleted, after the module.
        unsafe {
            m.gpu()
                .unwrap()
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(
                        RawDisplayHandle::Android(AndroidDisplayHandle::new()),
                    ),
                    raw_window_handle: RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(
                        std::ptr::NonNull::new(self.window).unwrap(),
                    )),
                })
        }
        .unwrap()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: made by `Window::new`, deleted once.
        unsafe { AImageReader_delete(self.reader) };
    }
}

/// A window replaced by buffers presents the frame rendered into it before
/// its surface goes; the buffers then render.
#[test]
fn a_window_replaced_by_buffers_presents_its_frame_first() {
    let Some(mut m) = module() else {
        return;
    };
    let window = Window::new(8);
    let id = canvas(&mut m, "clear");
    assert!(m.attach(id, window.surface(&m), 8, 8));
    assert_eq!(m.render(id, &frame(8.)), Some(true));
    assert!(m.attach_buffers(id, 8, 8));
    assert!(m.flush());
    assert_eq!(m.render(id, &frame(8.)), Some(true));
    assert!(m.sync());
    assert!(m.buffer(id).is_some(), "the buffers' frame is published");
    drop(m);
    drop(window);
}

/// Buffers replacing buffers: the frame rendered into the first ring is
/// submitted into it, and the second publishes no frame it was not
/// rendered into.
#[test]
fn replaced_buffers_publish_only_frames_rendered_into_them() {
    let Some(mut m) = module() else {
        return;
    };
    let id = canvas(&mut m, "clear");
    assert!(m.attach_buffers(id, 8, 8));
    assert_eq!(m.render(id, &frame(8.)), Some(true));
    assert!(m.attach_buffers(id, 8, 8));
    assert!(m.sync());
    let published = m.buffer(id).map(|(words, _)| words);
    assert_eq!(published, None, "published [generation, slot, serial]");
}

/// Under the agent's clock a canvas's frames wait for the GPU rather than
/// queue past the ring's bound.
#[test]
fn the_agents_frames_keep_the_ring_within_its_bound() {
    let Some(mut m) = module() else {
        return;
    };
    let id = canvas(&mut m, "busy");
    assert!(m.attach_buffers(id, 512, 512));
    for _ in 0..12 {
        assert_eq!(m.render(id, &frame(512.)), Some(true));
        assert!(m.flush());
        let in_flight = m.instances[&id].ring.as_ref().unwrap().in_flight();
        assert!(in_flight <= IN_FLIGHT, "{in_flight} frames on the GPU");
    }
    assert!(m.sync());
    assert!(m.buffer(id).is_some(), "a frame is published");
}

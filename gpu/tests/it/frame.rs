//! One submit per tick (LLP 1009 D7): canvases record into the module's
//! frame, `flush` submits it once and presents; a call about a canvas in the
//! open frame submits it first. Presented targets are `CAMetalLayer`s, so
//! these run on macOS.
#![cfg(target_os = "macos")]

use exact_gpu::{fixture, wgpu, Frame, Module, Registry, Surface, SurfaceError, Value};
use std::cell::RefCell;

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(line: String) {
    LOG.with(|l| l.borrow_mut().push(line));
}

fn take() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// Clears its target and says when it rendered and when that was submitted.
struct Clear(&'static str);

impl Surface for Clear {
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
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(self.0),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        log(format!("render {}", self.0));
        true
    }
    fn submitted(&mut self) {
        log(format!("submitted {}", self.0));
    }
}

static REGISTRY: Registry = Registry {
    surfaces: &[
        ("a", 0, || Box::new(Clear("a"))),
        ("b", 0, || Box::new(Clear("b"))),
    ],
    shaders: &[],
};

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {}

type Layer = objc2::rc::Retained<objc2::runtime::AnyObject>;

/// A module with canvases `a` and `b`, each on its own layer, bound.
fn two() -> Option<(Module, [u32; 2], [Layer; 2])> {
    let gpu = fixture::device_or_skip(fixture::device())?;
    let mut m = Module::new(&REGISTRY);
    m.set_seekable(true);
    m.set_gpu(gpu);
    let mut ids = [0; 2];
    let layers: [Layer; 2] = std::array::from_fn(|_| {
        // SAFETY: a new CAMetalLayer, retained by the test past the module.
        unsafe { objc2::msg_send![objc2::class!(CAMetalLayer), new] }
    });
    for (i, name) in ["a", "b"].into_iter().enumerate() {
        let ptr = objc2::rc::Retained::as_ptr(&layers[i]) as *mut std::ffi::c_void;
        // SAFETY: the layer outlives the module (dropped after it by the caller).
        let target = unsafe {
            m.gpu()
                .unwrap()
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(ptr))
        }
        .unwrap();
        ids[i] = m.create(name, target, 8, 8).expect("create");
        assert!(m.bind(ids[i], &[], None));
    }
    Some((m, ids, layers))
}

fn frame() -> Frame {
    Frame {
        width: 8.,
        height: 8.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    }
}

#[test]
fn canvases_record_into_one_frame_submitted_at_the_flush() {
    let Some((mut m, [a, b], _layers)) = two() else {
        return;
    };
    take();
    assert_eq!(m.render(a, &frame()), Some(true));
    assert_eq!(m.render(b, &frame()), Some(true));
    assert_eq!(take(), ["render a", "render b"], "nothing submitted yet");
    assert!(m.flush());
    assert_eq!(
        take(),
        ["submitted a", "submitted b"],
        "one submit, in order"
    );
    assert!(m.flush(), "nothing open is a no-op");
    assert!(take().is_empty());
    assert!(!m.dirty(a) && !m.dirty(b));
}

#[test]
fn a_call_about_a_canvas_in_the_open_frame_submits_it_first() {
    let Some((mut m, [a, b], _layers)) = two() else {
        return;
    };
    m.render(a, &frame());
    take();
    // A canvas not in the frame leaves it open.
    assert!(m.bind(b, &[], None));
    assert!(take().is_empty());
    // One in it submits it before the call.
    assert!(m.bind(a, &[], None));
    assert_eq!(take(), ["submitted a"]);
    // A second render of a canvas submits its first.
    m.render(b, &frame());
    m.render(b, &frame());
    assert_eq!(take(), ["render b", "submitted b", "render b"]);
    m.destroy(b);
    assert_eq!(take(), ["submitted b"], "destroy presents what it recorded");
    m.render(a, &frame());
    assert!(m.sync());
    assert_eq!(
        take(),
        ["render a", "submitted a"],
        "sync waits for the frame"
    );
}

#[test]
fn a_lost_device_drops_the_open_frame_unsubmitted() {
    let Some((mut m, [a, _], _layers)) = two() else {
        return;
    };
    m.render(a, &frame());
    take();
    let gpu = m.gpu().unwrap();
    gpu.device.destroy();
    let _ = gpu.device.poll(wgpu::PollType::Poll);
    assert!(m.flush());
    assert!(take().is_empty(), "nothing submitted on a lost device");
    assert!(m.gpu().is_none());
    assert!(m.dirty(a) || !m.has_device(a));
}

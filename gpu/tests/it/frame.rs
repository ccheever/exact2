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

/// Records a frame as [`Clear`] does, then reports a failure for it.
struct Fails(Clear, bool);

impl Surface for Fails {
    fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        self.1 = true;
        self.0.render(frame, device, queue, encoder, target, format)
    }
    fn take_error(&mut self) -> Option<SurfaceError> {
        std::mem::take(&mut self.1).then(|| SurfaceError("failed after recording".into()))
    }
}

static REGISTRY: Registry = Registry {
    surfaces: &[
        ("a", 0, || Box::new(Clear("a"))),
        ("b", 0, || Box::new(Clear("b"))),
        ("f", 0, || Box::new(Fails(Clear("f"), false))),
    ],
    shaders: &[],
};

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {}

type Layer = objc2::rc::Retained<objc2::runtime::AnyObject>;

/// A module with canvases `a` and `b`, each on its own layer, bound.
fn two() -> Option<(Module, [u32; 2], [Layer; 2])> {
    on_layers(["a", "b"])
}

/// A module with those two surfaces, each on its own layer, bound.
fn on_layers(names: [&str; 2]) -> Option<(Module, [u32; 2], [Layer; 2])> {
    let gpu = fixture::device_or_skip(fixture::device())?;
    let mut m = Module::new(&REGISTRY);
    m.set_seekable(true);
    m.set_gpu(gpu);
    let mut ids = [0; 2];
    let layers: [Layer; 2] = std::array::from_fn(|_| {
        // SAFETY: a new CAMetalLayer, retained by the test past the module.
        unsafe { objc2::msg_send![objc2::class!(CAMetalLayer), new] }
    });
    for (i, name) in names.into_iter().enumerate() {
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

/// What the device's Metal queue has committed and presented so far (the
/// patched wgpu-hal's counters, vendor/wgpu-hal/EXACT-PATCHES.md).
fn counts(m: &Module) -> wgpu::hal::metal::Counts {
    // SAFETY: the queue's own counters are read; nothing is encoded or freed.
    unsafe { m.gpu().unwrap().queue.as_hal::<wgpu::hal::api::Metal>() }
        .expect("a Metal queue")
        .counts()
}

/// wgpu-core wraps each pass in transition encodings Metal has no use for,
/// and `present` had a command buffer of its own: five or six committed per
/// canvas frame where an `MTKView` draw commits one. With the patched
/// wgpu-hal an encoding that encoded nothing has no command buffer, and the
/// frame's presentations ride the last one that did.
#[test]
fn a_frame_commits_one_command_buffer_a_canvas_and_its_presentations_ride_it() {
    let Some((mut m, [a, b], _layers)) = two() else {
        return;
    };
    // A first frame, so nothing the device does once is counted.
    m.render(a, &frame());
    m.render(b, &frame());
    assert!(m.flush());
    assert!(m.sync());
    for _ in 0..4 {
        let before = counts(&m);
        assert_eq!(m.render(a, &frame()), Some(true));
        assert_eq!(m.render(b, &frame()), Some(true));
        assert!(m.flush());
        let after = counts(&m);
        assert_eq!(
            after.committed - before.committed,
            2,
            "one command buffer a canvas: its pass"
        );
        assert_eq!(
            after.presented_with_submit - before.presented_with_submit,
            2,
            "both drawables presented by the submit"
        );
        assert_eq!(
            after.presented_alone, before.presented_alone,
            "and `present` committed nothing"
        );
    }
    assert!(m.sync());
}

/// A surface that fails after recording has its drawable in the frame's
/// submit and must not be shown (LLP 1009 D7), so that frame's presentations
/// do not ride the submit: the healthy canvas is presented once, by
/// `present`, and the failed one's drawable is let go. More rounds than a
/// layer has drawables: one neither presented nor let go would run it out.
#[test]
fn a_frame_with_a_failed_canvas_presents_the_others_once_and_it_never() {
    let Some((mut m, [a, f], _layers)) = on_layers(["a", "f"]) else {
        return;
    };
    for _ in 0..6 {
        let before = counts(&m);
        assert_eq!(m.render(a, &frame()), Some(true));
        assert_eq!(m.render(f, &frame()), None, "the failure is reported");
        assert_eq!(m.take_error(), "failed after recording");
        assert!(m.flush());
        let after = counts(&m);
        assert_eq!(
            after.presented_with_submit, before.presented_with_submit,
            "nothing rides a submit that holds a failed canvas's drawable"
        );
        assert_eq!(
            after.presented_alone - before.presented_alone,
            1,
            "the healthy canvas, once; the failed one never"
        );
    }
    // The next healthy frame rides its submit again.
    let before = counts(&m);
    assert_eq!(m.render(a, &frame()), Some(true));
    assert!(m.flush());
    let after = counts(&m);
    assert_eq!(
        after.presented_with_submit - before.presented_with_submit,
        1
    );
    assert_eq!(after.presented_alone, before.presented_alone);
    assert!(m.sync());
}

/// A presenter that reuses a layer keeps it hidden until the canvas's first
/// frame is with the compositor (the layer still holds the last picture
/// presented to it): the module says when, from the thread Metal schedules
/// the frame's command buffer on.
#[test]
fn a_canvas_is_seen_once_its_first_frame_is_scheduled() {
    let Some((mut m, [a, b], _layers)) = two() else {
        return;
    };
    assert!(!m.seen(a) && !m.seen(b), "nothing drawn yet");
    assert_eq!(m.render(a, &frame()), Some(true));
    assert!(!m.seen(a), "recorded is not shown");
    assert!(m.flush());
    for _ in 0..400 {
        if m.seen(a) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(m.seen(a), "scheduled within two seconds");
    assert!(!m.seen(b), "b has drawn nothing");
    // A frame with a failed canvas in it is presented the ordinary way, and
    // the healthy canvas's first frame is seen when that returns.
    let Some((mut m, [a, f], _layers)) = on_layers(["a", "f"]) else {
        return;
    };
    m.render(a, &frame());
    assert_eq!(m.render(f, &frame()), None);
    let _ = m.take_error();
    assert!(m.flush());
    assert!(m.seen(a));
    assert!(!m.seen(f), "a frame that failed was never shown");
    assert!(m.sync());
}

//! A canvas's presentation let go of, on a real `CAMetalLayer`.
use crate::{fixture, wgpu, Frame, Module, Registry, Surface, SurfaceError, Value};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {}

/// Renders this many incomplete frames, then complete ones; each render's
/// target is recorded in [`TARGETS`].
const LATE: u32 = 3;
static TARGETS: Mutex<Vec<wgpu::Texture>> = Mutex::new(Vec::new());

/// Clears its target, complete from its `ready`th render on (never at
/// `u32::MAX`).
struct Clearing {
    ready: u32,
    renders: u32,
    record: bool,
}

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
        self.renders += 1;
        if self.record {
            TARGETS.lock().unwrap().push(target.texture().clone());
        }
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
        });
        true
    }
    fn frame_ready(&self) -> bool {
        self.renders > self.ready
    }
}

static REGISTRY: Registry = Registry {
    surfaces: &[
        ("clear", 0, || {
            Box::new(Clearing {
                ready: 0,
                renders: 0,
                record: false,
            })
        }),
        ("pending", 0, || {
            Box::new(Clearing {
                ready: u32::MAX,
                renders: 0,
                record: false,
            })
        }),
        ("late", 0, || {
            Box::new(Clearing {
                ready: LATE,
                renders: 0,
                record: true,
            })
        }),
    ],
    shaders: &[],
};

fn frame() -> Frame {
    Frame {
        width: 8.,
        height: 8.,
        scale: 1.,
        now_ms: 0.,
        seekable: false,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    }
}

fn module() -> Option<Module> {
    let gpu = fixture::device_or_skip(fixture::device())?;
    let mut m = Module::new(&REGISTRY);
    m.set_gpu(gpu);
    Some(m)
}

/// A bound canvas of surface `name` on a new `CAMetalLayer`, which the
/// caller keeps past the module.
fn canvas(m: &mut Module, name: &str) -> (Retained<AnyObject>, u32) {
    // SAFETY: a new layer, returned to the caller to outlive the canvas.
    let layer: Retained<AnyObject> = unsafe { objc2::msg_send![objc2::class!(CAMetalLayer), new] };
    let ptr = Retained::as_ptr(&layer) as *mut std::ffi::c_void;
    // SAFETY: as above.
    let target = unsafe {
        m.gpu()
            .unwrap()
            .instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(ptr))
    }
    .unwrap();
    let id = m.create(name, target, 8, 8).unwrap();
    assert!(m.bind(id, &[], None));
    (layer, id)
}

fn within_two_seconds(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("{what}: not in two seconds")
}

/// A drawable held for an incomplete frame goes before its surface when
/// the canvas is destroyed, and when the module is dropped. Under the
/// agent's clock no acquisition thread holds the surface as well.
#[test]
fn a_held_drawable_goes_before_its_surface() {
    let Some(mut m) = module() else {
        return;
    };
    m.set_seekable(true);
    let (a_layer, a) = canvas(&mut m, "pending");
    let (b_layer, b) = canvas(&mut m, "pending");
    for id in [a, b] {
        assert_eq!(m.render(id, &frame()), Some(true));
    }
    assert!(m.flush());
    assert!(m.instances[&a].held.is_some() && m.instances[&b].held.is_some());
    m.destroy(a);
    // `b` is rendered into the open frame, its drawable held, when the
    // module goes.
    assert_eq!(m.render(b, &frame()), Some(true));
    drop(m);
    drop((a_layer, b_layer));
}

/// Where an acquisition thread waits while the test holds it: 1 entered,
/// 2 let go, 3 gave up after ten seconds (the test was waiting on it).
static GATE: (Mutex<u8>, Condvar) = (Mutex::new(0), Condvar::new());

/// The presenter's callback for a landed drawable, run on the acquisition
/// thread: it stays there until the test lets it go.
extern "C" fn hold() {
    let (state, changed) = &GATE;
    let mut s = state.lock().unwrap();
    *s = 1;
    changed.notify_all();
    let (mut s, waited) = changed
        .wait_timeout_while(s, Duration::from_secs(10), |s| *s != 2)
        .unwrap();
    if waited.timed_out() {
        *s = 3;
    }
}

/// A canvas's window going does not wait for its acquisition thread, which
/// may be waiting for an image: on Android the compositor releases it only
/// after the window's `surfaceDestroyed`, which waits for the detach,
/// returns. The surface is dropped once the thread has ended. (An offscreen
/// layer's `nextDrawable` does not wait for a drawable to be released, so
/// the thread is held in the presenter's callback instead.)
#[test]
fn detach_leaves_the_acquisition_thread_to_end_on_its_own() {
    let _callback = crate::acquire::tests::CALLBACK.lock();
    let Some(mut m) = module() else {
        return;
    };
    let (layer, id) = canvas(&mut m, "clear");
    let (state, changed) = &GATE;
    *state.lock().unwrap() = 0;
    crate::acquire::on_acquire(Some(hold));
    // Starved, or presented and waiting for the next: either way the
    // thread says when a drawable lands.
    assert_eq!(m.render(id, &frame()), Some(true));
    assert!(m.flush());
    let (s, waited) = changed
        .wait_timeout_while(state.lock().unwrap(), Duration::from_secs(2), |s| *s != 1)
        .unwrap();
    assert!(!waited.timed_out(), "a drawable landed");
    drop(s);
    let target = m.instances[&id].presentation.clone().unwrap();
    m.detach(id);
    assert_eq!(*state.lock().unwrap(), 1, "detach waited for the thread");
    m.reap();
    assert!(m.retired.len() == 1 && !m.retired[0].finished());
    assert!(
        Arc::strong_count(&target) > 1,
        "the thread holds the surface"
    );
    *state.lock().unwrap() = 2;
    changed.notify_all();
    within_two_seconds("the acquisition thread ends", || {
        m.reap();
        m.retired.is_empty()
    });
    crate::acquire::on_acquire(None);
    assert_eq!(Arc::strong_count(&target), 1, "the module released it");
    drop((target, m, layer));
}

/// Renders after an incomplete frame go offscreen, and the complete one is
/// copied into the drawable the first was written into: a drawable takes
/// two submissions at most, however long its surface is pending.
#[test]
fn retries_of_an_incomplete_frame_leave_its_drawable_alone() {
    let Some(mut m) = module() else {
        return;
    };
    m.set_seekable(true);
    let (layer, id) = canvas(&mut m, "late");
    for _ in 0..=LATE {
        assert!(m.render(id, &frame()).is_some());
        assert!(m.flush());
    }
    let targets = std::mem::take(&mut *TARGETS.lock().unwrap());
    assert_eq!(targets.len(), LATE as usize + 1);
    assert!(
        targets[1..].iter().all(|t| *t != targets[0]),
        "a retry wrote the held drawable"
    );
    let inst = &m.instances[&id];
    assert!(inst.held.is_none() && inst.scratch.is_none(), "presented");
    within_two_seconds("the complete frame shown", || m.seen(id));
    drop((targets, m, layer));
}

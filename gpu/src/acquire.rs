//! A presented canvas's next drawable, acquired off the presenter's thread.
//!
//! `-[CAMetalLayer nextDrawable]` blocks until the compositor releases one of
//! the layer's three drawables. A canvas that presents every frame asks for
//! its next one about when the compositor lets go of the oldest, so on the
//! iPad the main thread waited a median 4.3 ms (p90 13.5 ms) on nearly every
//! frame of a list of animated canvases, 460 ms a second — the frames the
//! list then missed. Here each presented canvas has a thread that waits
//! instead: the presenter asks for a drawable after each present and takes
//! it on a later frame only when it is ready; until then the canvas keeps
//! its last frame and still wants one (LLP 1009 D4 — the host owns the
//! frame; the instance stays the node's, D2).
//!
//! A drawable presented at the start of frame k comes back about 4 ms into
//! frame k + 3, so a canvas rendered only at the tick would draw two frames
//! in three. A canvas that went without is *starved*: when its drawable
//! arrives the thread tells the presenter (`gpu_on_acquire`), which renders
//! the starved canvases then, within the frame — what the blocking wait did,
//! without holding the main thread. A canvas that asked for its next
//! drawable as its frame was presented is waiting for it in the same sense
//! ([`Acquire::request_awaited`]): the presenter hears when it lands, and
//! until then can see that it has not ([`Acquire::landed`]) without asking
//! for a render that would draw nothing — at 120 Hz, every tick.
//!
//! wgpu holds no device lock while it waits (only the surface's layer), so
//! the other canvases' submits and presents run meanwhile. A seekable clock
//! takes the texture in flight, waiting for it; so does a resize, except on
//! Android, where it configures once the texture has landed
//! ([`Acquire::reconfigure`]). A canvas whose window goes does not wait
//! ([`Acquire::retire`]).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};

/// The texture a surface's acquisition returned.
pub(crate) type Current = wgpu::CurrentSurfaceTexture;

/// One canvas's acquisitions: at most one in flight.
#[derive(Default)]
pub(crate) struct Acquire {
    worker: Option<(
        mpsc::Sender<()>,
        mpsc::Receiver<Current>,
        std::thread::JoinHandle<()>,
    )>,
    in_flight: bool,
    /// A render went without a texture since the last one it drew into, or
    /// the next one was asked for with the presenter waiting on it; shared
    /// with the thread, which notifies the presenter when set.
    starved: Arc<AtomicBool>,
    /// Textures the thread has answered with; shared with it.
    sent: Arc<AtomicUsize>,
    /// Answers taken from the thread (on the presenter's thread only).
    taken: usize,
    /// The surface, held until after `worker` (fields drop in order): a
    /// texture that landed and was never taken is discarded with the
    /// channel, and wgpu discards it through its surface, which must still
    /// exist. The canvas's own handle and the thread's may already be gone.
    surface: Option<Arc<wgpu::Surface<'static>>>,
}

/// An acquisition thread the canvas hung up on ([`Acquire::retire`]), and
/// its surface: the module drops both once the thread has ended
/// (`Module::reap`).
pub(crate) struct Retired {
    thread: std::thread::JoinHandle<()>,
    surface: Option<Arc<wgpu::Surface<'static>>>,
}

impl Retired {
    /// Whether the thread has ended, its own handle on the surface dropped.
    pub(crate) fn finished(&self) -> bool {
        self.thread.is_finished()
    }

    /// Wait for the thread to end, then drop the surface here.
    pub(crate) fn join(self) {
        let _ = self.thread.join();
        drop(self.surface);
    }
}

impl Drop for Acquire {
    fn drop(&mut self) {
        // Every path that lets a canvas's acquisition go retires it; on
        // Android the surface must not be destroyed on the thread (`Retired`).
        #[cfg(target_os = "android")]
        if let Some(retired) = self.retire() {
            retired.join();
        }
    }
}

impl Acquire {
    /// Hang up on the thread without waiting for it. It may be inside an
    /// acquisition that ends only when the compositor releases an image —
    /// on Android not before the window's `surfaceDestroyed` returns, which
    /// waits for this — and it ends when that does. Its surface is kept
    /// with it: on Android, destroying the swapchain waits for the device
    /// to be idle, which must not race the presenter's submits, so the
    /// surface is dropped on the presenter's thread, not the acquiring one.
    pub(crate) fn retire(&mut self) -> Option<Retired> {
        let worker = self.worker.take();
        let surface = self.surface.take();
        // The thread keeps the old flags; a thread spawned later gets new ones.
        self.in_flight = false;
        self.starved = Default::default();
        self.sent = Default::default();
        self.taken = 0;
        let (ask, answers, thread) = worker?;
        drop(ask);
        // A texture that landed and was never taken goes here, through the
        // surface `surface` still holds.
        drop(answers);
        Some(Retired { thread, surface })
    }

    /// Whether the surface may be configured again now: nothing of it is
    /// acquired. A texture that has landed is discarded. One still in flight
    /// is waited for, except on Android, where the compositor may hold the
    /// image until a window callback the presenter is waiting on returns:
    /// there the canvas waits for it to land as a starved one does, and
    /// configures in a later render.
    pub(crate) fn reconfigure(&mut self) -> bool {
        #[cfg(target_os = "android")]
        if !self.landed() {
            self.starved.store(true, Ordering::SeqCst);
            return false;
        }
        drop(self.take(true));
        true
    }

    /// Whether this platform acquires off the presenter's thread: iOS, where
    /// it was measured first, and macOS, where the shader-only Extra Heavy
    /// feed spent 54% of the main thread in `nextDrawable` (2026-09-28). An
    /// occluded window's drawable wait now starves the canvas instead of
    /// holding the main thread; the AppKit presenter's guard stays for the
    /// agent's clock, which acquires on the main thread.
    pub(crate) const ENABLED: bool = cfg!(any(
        target_os = "ios",
        target_os = "tvos",
        target_os = "macos",
        target_os = "android"
    ));

    /// Whether a texture asked for at a present is waited on
    /// ([`Self::request_awaited`]): where the presenter reads
    /// [`Self::landed`] before it renders and draws a canvas once a frame —
    /// the UIKit presenter. The AppKit presenter renders every starved
    /// canvas whenever any texture lands, which with this would draw a
    /// canvas twice in a frame; it keeps asking at the tick until it does
    /// the same. Android also wakes only a render that missed its texture,
    /// through the reader's existing executor descriptor.
    pub(crate) const AWAITED: bool = cfg!(any(target_os = "ios", target_os = "tvos", test));

    /// Ask for `target`'s next texture, unless one is already in flight.
    pub(crate) fn request(&mut self, target: &Arc<wgpu::Surface<'static>>) {
        if self.in_flight {
            return;
        }
        if self.worker.is_none() {
            self.taken = 0;
            self.sent.store(0, Ordering::SeqCst);
            self.worker = spawn(target.clone(), self.starved.clone(), self.sent.clone());
            self.surface = Some(target.clone());
        }
        match &self.worker {
            Some((ask, _, _)) if ask.send(()).is_ok() => self.in_flight = true,
            _ => {
                self.worker = None;
                self.surface = None;
            }
        }
    }

    /// Ask for `target`'s next texture with the presenter waiting on it: the
    /// canvas counts as starved from now, so the texture is announced when
    /// it lands, without a render having to go without one first.
    pub(crate) fn request_awaited(&mut self, target: &Arc<wgpu::Surface<'static>>) {
        if Self::AWAITED {
            self.starved.store(true, Ordering::SeqCst);
        }
        self.request(target);
    }

    /// Whether a render has something to do: the texture in flight has
    /// landed and it would draw, or none is in flight and it would ask for
    /// one. `false` only while one is in flight and not back — a render then
    /// draws nothing. An answer counts once the thread has sent it, so a
    /// landing is never reported ahead of [`Self::take`].
    pub(crate) fn landed(&self) -> bool {
        !self.in_flight || self.sent.load(Ordering::SeqCst) > self.taken
    }

    /// The texture in flight if it is ready; otherwise the canvas is marked
    /// starved first, so a texture that lands after this look is announced.
    pub(crate) fn take_or_starve(&mut self) -> Option<Current> {
        self.starved.store(true, Ordering::SeqCst);
        let current = self.take(false)?;
        self.starved.store(false, Ordering::SeqCst);
        Some(current)
    }

    /// Whether a render went without a texture since the last one it drew.
    pub(crate) fn starved(&self) -> bool {
        self.starved.load(Ordering::SeqCst)
    }

    /// The texture in flight: when it is ready, or (`wait`) once it is.
    pub(crate) fn take(&mut self, wait: bool) -> Option<Current> {
        if !self.in_flight {
            return None;
        }
        let (_, answer, _) = self.worker.as_ref()?;
        let got = if wait {
            answer.recv().map_err(|_| mpsc::TryRecvError::Disconnected)
        } else {
            answer.try_recv()
        };
        match got {
            Ok(current) => {
                self.in_flight = false;
                self.taken += 1;
                Some(current)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.in_flight = false;
                self.worker = None;
                self.surface = None;
                None
            }
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
impl Acquire {
    /// Hang up on the thread (it exits, dropping its handle on the surface)
    /// and hand back the channel it answers on.
    pub(crate) fn hang_up(&mut self) -> Option<mpsc::Receiver<Current>> {
        let (ask, answer, thread) = self.worker.take()?;
        drop(ask);
        // This test takes the answer only after the worker has released its surface.
        let _ = thread.join();
        Some(answer)
    }
}

/// The presenter's callback for a starved canvas's texture (`gpu_on_acquire`),
/// as an address; 0 when none is registered. It runs on the acquiring thread.
static NOTIFY: AtomicUsize = AtomicUsize::new(0);

/// Register (or, with `None`, remove) the presenter's callback.
pub(crate) fn on_acquire(callback: Option<extern "C" fn()>) {
    NOTIFY.store(callback.map_or(0, |f| f as usize), Ordering::SeqCst);
}

fn notify() {
    let address = NOTIFY.load(Ordering::SeqCst);
    if address != 0 {
        // SAFETY: only `on_acquire` stores here, and only an `extern "C" fn()`.
        let callback: extern "C" fn() = unsafe { std::mem::transmute(address) };
        callback();
    }
}

/// A thread that acquires `target`'s textures on request, until the canvas
/// drops its end. It holds the surface (and so the layer) until then.
fn spawn(
    target: Arc<wgpu::Surface<'static>>,
    starved: Arc<AtomicBool>,
    sent: Arc<AtomicUsize>,
) -> Option<(
    mpsc::Sender<()>,
    mpsc::Receiver<Current>,
    std::thread::JoinHandle<()>,
)> {
    let (ask, asked) = mpsc::channel::<()>();
    let (answer, answers) = mpsc::channel::<Current>();
    let thread = std::thread::Builder::new()
        .name("exact-gpu-acquire".into())
        .spawn(move || {
            interactive();
            while asked.recv().is_ok() {
                if answer.send(target.get_current_texture()).is_err() {
                    break;
                }
                sent.fetch_add(1, Ordering::SeqCst);
                if starved.load(Ordering::SeqCst) {
                    notify();
                }
            }
        })
        .ok()?;
    Some((ask, answers, thread))
}

/// The presenter waits on this thread's answer within a frame, so it runs
/// at the presenter's quality of service.
#[cfg(target_vendor = "apple")]
fn interactive() {
    const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;
    extern "C" {
        fn pthread_set_qos_class_self_np(class: u32, relative: i32) -> i32;
    }
    // SAFETY: sets the calling thread's own class; no pointers.
    unsafe {
        pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0);
    }
}

#[cfg(not(target_vendor = "apple"))]
fn interactive() {}

#[cfg(all(test, target_os = "macos"))]
pub(crate) mod tests {
    use crate::{fixture, wgpu, Frame, Module, Registry, Surface, SurfaceError, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Held by a test that registers the presenter's acquisition callback,
    /// which is one for the process.
    pub(crate) static CALLBACK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {}

    /// Clears its target and wants every frame.
    struct Clear;
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
    }

    static REGISTRY: Registry = Registry {
        surfaces: &[("clear", 0, || Box::new(Clear))],
        shaders: &[],
    };

    static HEARD: AtomicUsize = AtomicUsize::new(0);
    extern "C" fn heard() {
        HEARD.fetch_add(1, Ordering::SeqCst);
    }

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

    fn within_two_seconds(what: &str, mut done: impl FnMut() -> bool) {
        for _ in 0..400 {
            if done() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("{what}: not in two seconds")
    }

    /// The acquiring thread and the presenter's, for real: a canvas that
    /// went without a drawable, and then one whose next drawable was asked
    /// for as its frame was presented, each hear when it lands, and
    /// `landed` says so without a render having to ask.
    #[test]
    fn a_waiting_canvas_hears_its_drawable_land_without_asking_again() {
        let _callback = CALLBACK.lock();
        let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
            return;
        };
        let mut m = Module::new(&REGISTRY);
        m.set_gpu(gpu);
        // SAFETY: a new CAMetalLayer, retained by the test past the module.
        let layer: objc2::rc::Retained<objc2::runtime::AnyObject> =
            unsafe { objc2::msg_send![objc2::class!(CAMetalLayer), new] };
        let ptr = objc2::rc::Retained::as_ptr(&layer) as *mut std::ffi::c_void;
        // SAFETY: the layer outlives the module.
        let target = unsafe {
            m.gpu()
                .unwrap()
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(ptr))
        }
        .unwrap();
        let id = m.create("clear", target, 8, 8).unwrap();
        assert!(m.bind(id, &[], None));
        super::on_acquire(Some(heard));

        // The first frame goes without: nothing was asked for before it, so
        // a render has something to do (it asks).
        assert!(m.landed(id));
        assert_eq!(m.render(id, &frame()), Some(true));
        assert!(m.starved(id));
        within_two_seconds("the first drawable", || m.landed(id));
        within_two_seconds("its announcement", || HEARD.load(Ordering::SeqCst) >= 1);
        assert_eq!(m.render(id, &frame()), Some(true));
        assert!(!m.starved(id), "the landed drawable was drawn into");

        // Presented, its next drawable is asked for and waited on: starved
        // from the flush, announced when it lands, with no render between.
        let before = HEARD.load(Ordering::SeqCst);
        assert!(m.flush());
        assert!(m.starved(id), "waiting from the present");
        within_two_seconds("the next drawable", || m.landed(id));
        within_two_seconds("its announcement", || HEARD.load(Ordering::SeqCst) > before);
        assert_eq!(m.render(id, &frame()), Some(true));
        assert!(!m.starved(id));
        assert!(m.flush());
        super::on_acquire(None);
        let target = m.instances[&id].presentation.clone().unwrap();
        m.detach(id);
        within_two_seconds("the acquisition thread ends", || {
            m.reap();
            m.retired.is_empty()
        });
        assert_eq!(
            std::sync::Arc::strong_count(&target),
            1,
            "the module released the swapchain once its acquisition thread ended"
        );
        drop(target);
        drop(m);
        drop(layer);
    }
}

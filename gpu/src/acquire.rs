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
//! without holding the main thread.
//!
//! wgpu holds no device lock while it waits (only the surface's layer), so
//! the other canvases' submits and presents run meanwhile. A resize and a
//! seekable clock take the texture in flight, waiting for it.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};

/// The texture a surface's acquisition returned.
pub(crate) type Current = wgpu::CurrentSurfaceTexture;

/// One canvas's acquisitions: at most one in flight.
#[derive(Default)]
pub(crate) struct Acquire {
    worker: Option<(mpsc::Sender<()>, mpsc::Receiver<Current>)>,
    in_flight: bool,
    /// A render went without a texture since the last one it drew into;
    /// shared with the thread, which notifies the presenter when set.
    starved: Arc<AtomicBool>,
    /// The surface, held until after `worker` (fields drop in order): a
    /// texture that landed and was never taken is discarded with the
    /// channel, and wgpu discards it through its surface, which must still
    /// exist. The canvas's own handle and the thread's may already be gone.
    surface: Option<Arc<wgpu::Surface<'static>>>,
}

impl Acquire {
    /// Whether this platform acquires off the presenter's thread: iOS, where
    /// it was measured. The AppKit presenter keeps its own occlusion guard.
    pub(crate) const ENABLED: bool = cfg!(target_os = "ios");

    /// Ask for `target`'s next texture, unless one is already in flight.
    pub(crate) fn request(&mut self, target: &Arc<wgpu::Surface<'static>>) {
        if self.in_flight {
            return;
        }
        if self.worker.is_none() {
            self.worker = spawn(target.clone(), self.starved.clone());
            self.surface = Some(target.clone());
        }
        match &self.worker {
            Some((ask, _)) if ask.send(()).is_ok() => self.in_flight = true,
            _ => {
                self.worker = None;
                self.surface = None;
            }
        }
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
        let (_, answer) = self.worker.as_ref()?;
        let got = if wait {
            answer.recv().map_err(|_| mpsc::TryRecvError::Disconnected)
        } else {
            answer.try_recv()
        };
        match got {
            Ok(current) => {
                self.in_flight = false;
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

#[cfg(test)]
impl Acquire {
    /// Hang up on the thread (it exits, dropping its handle on the surface)
    /// and hand back the channel it answers on.
    pub(crate) fn hang_up(&mut self) -> Option<mpsc::Receiver<Current>> {
        let (ask, answer) = self.worker.take()?;
        drop(ask);
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
) -> Option<(mpsc::Sender<()>, mpsc::Receiver<Current>)> {
    let (ask, asked) = mpsc::channel::<()>();
    let (answer, answers) = mpsc::channel::<Current>();
    std::thread::Builder::new()
        .name("exact-gpu-acquire".into())
        .spawn(move || {
            interactive();
            while asked.recv().is_ok() {
                if answer.send(target.get_current_texture()).is_err() {
                    break;
                }
                if starved.load(Ordering::SeqCst) {
                    notify();
                }
            }
        })
        .ok()?;
    Some((ask, answers))
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

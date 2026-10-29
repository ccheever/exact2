//! The carrier owns the view; the render thread only owns its Metal layer.
//! @ref LLP 1149 D7, D11, D12.
use super::{Error, Result};
use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSView};
use objc2_core_foundation::CGSize;
use objc2_core_graphics::CGColorSpace;
use objc2_metal::{MTLDrawable, MTLPixelFormat};
use objc2_quartz_core::{CAMetalLayer, CATransaction};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

type Transactions = Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>;

struct Inner {
    view: Retained<NSView>,
    layer: Retained<CAMetalLayer>,
    live_resize: Arc<AtomicBool>,
    retry: Arc<AtomicBool>,
    flight: Arc<AtomicBool>,
    transactions: Transactions,
    wake: Arc<dyn Fn() + Send + Sync>,
    _owner: Rc<dyn HasWindowHandle>,
}

/// An NSView's retained layer. Create, resize and drop it on the main thread.
#[derive(Clone)]
pub struct Layer(Rc<Inner>);

impl Layer {
    pub fn new(
        owner: Rc<dyn HasWindowHandle>,
        size: [u32; 2],
        scale: f64,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        let _main = MainThreadMarker::new()
            .ok_or_else(|| Error::Invalid("attach the Mac view on the main thread".into()))?;
        let RawWindowHandle::AppKit(handle) = owner
            .window_handle()
            .map_err(|e| Error::Invalid(e.to_string()))?
            .as_raw()
        else {
            return Err(Error::Invalid("the carrier needs an AppKit view".into()));
        };
        // SAFETY: the handle borrows this retained window owner, on the main thread.
        let view = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()).unwrap() };
        let layer = CAMetalLayer::new();
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        layer.setMaximumDrawableCount(3);
        layer.setFramebufferOnly(true);
        layer.setOpaque(true);
        layer.setDisplaySyncEnabled(true);
        layer.setAllowsNextDrawableTimeout(true);
        let color =
            CGColorSpace::with_name(Some(unsafe { objc2_core_graphics::kCGColorSpaceSRGB }));
        layer.setColorspace(color.as_deref());
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        let result = Self(Rc::new(Inner {
            view,
            layer,
            live_resize: Arc::new(AtomicBool::new(false)),
            retry: Arc::new(AtomicBool::new(false)),
            flight: Arc::new(AtomicBool::new(false)),
            transactions: Default::default(),
            wake: Arc::new(wake),
            _owner: owner,
        }));
        result.resize(size, scale);
        Ok(result)
    }

    pub fn resize(&self, size: [u32; 2], scale: f64) {
        self.0.layer.setContentsScale(scale);
        self.0
            .layer
            .setDrawableSize(CGSize::new(size[0] as f64, size[1] as f64));
        self.update();
    }

    /// Present resize frames in the view's transaction after scheduling on Metal.
    pub fn update(&self) {
        self.0
            .live_resize
            .store(self.0.view.inLiveResize(), Ordering::Release);
        let pending = std::mem::take(&mut *self.0.transactions.lock().unwrap());
        if !pending.is_empty() {
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            for present in pending {
                present();
            }
            CATransaction::commit();
        }
    }

    pub fn retry_requested(&self) -> bool {
        self.0.retry.swap(false, Ordering::AcqRel)
    }

    /// Damage stays with the presenter until the previous frame has completed.
    pub fn in_flight(&self) -> bool {
        self.0.flight.load(Ordering::Acquire)
    }

    pub(crate) fn begin_frame(&self) -> Result<Flight> {
        Flight::begin(self.0.flight.clone(), self.0.wake.clone())
    }

    /// A posted wheel event carries its own point without moving the cursor.
    pub fn wheel_position(&self) -> Option<[f32; 2]> {
        let event = NSApplication::sharedApplication(MainThreadMarker::new()?).currentEvent()?;
        let point = self
            .0
            .view
            .convertPoint_fromView(event.locationInWindow(), None);
        let y = if self.0.view.isFlipped() {
            point.y
        } else {
            self.0.view.bounds().size.height - point.y
        };
        Some([point.x as f32, y as f32])
    }

    pub(crate) fn render_layer(&self) -> RenderLayer {
        RenderLayer {
            raw: self.0.layer.clone(),
            live_resize: self.0.live_resize.clone(),
            retry: self.0.retry.clone(),
            transactions: self.0.transactions.clone(),
            wake: self.0.wake.clone(),
        }
    }
}

pub(crate) struct Flight {
    pending: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl Flight {
    fn begin(pending: Arc<AtomicBool>, wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        if pending.swap(true, Ordering::AcqRel) {
            return Err(Error::Invalid("a window frame is already in flight".into()));
        }
        Ok(Self { pending, wake })
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        self.pending.store(false, Ordering::Release);
        (self.wake)();
    }
}

pub(crate) struct RenderLayer {
    pub raw: Retained<CAMetalLayer>,
    pub live_resize: Arc<AtomicBool>,
    retry: Arc<AtomicBool>,
    transactions: Transactions,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl RenderLayer {
    pub fn track(&self, drawable: &objc2::runtime::ProtocolObject<dyn objc2_metal::MTLDrawable>) {
        let retry = self.retry.clone();
        let wake = self.wake.clone();
        let block = block2::RcBlock::new(
            move |shown: std::ptr::NonNull<
                objc2::runtime::ProtocolObject<dyn objc2_metal::MTLDrawable>,
            >| {
                // A drawable can complete on the GPU without reaching the screen,
                // including while the window's first layer transaction is pending.
                // https://developer.apple.com/documentation/metal/mtldrawable/presentedtime
                if unsafe { shown.as_ref().presentedTime() } == 0. {
                    retry.store(true, Ordering::Release);
                    wake();
                }
            },
        );
        // SAFETY: Metal copies the block and supplies a live drawable to it.
        unsafe {
            drawable.addPresentedHandler(block2::RcBlock::as_ptr(&block));
        }
    }
    pub fn retry(&self) {
        self.retry.store(true, Ordering::Release);
        (self.wake)();
    }
    pub fn transaction(&self, present: impl FnOnce() + Send + 'static) {
        self.transactions.lock().unwrap().push(Box::new(present));
        (self.wake)();
    }
}
// SAFETY: Core Animation layer properties are thread safe. Only the main-thread
// owner touches NSView; this handle acquires drawables on one render thread.
unsafe impl Send for RenderLayer {}

/// The unmerged AppKit phases: winit merges touch and momentum into TouchPhase.
pub fn wheel_phases() -> [u64; 2] {
    let Some(main) = MainThreadMarker::new() else {
        return [0; 2];
    };
    NSApplication::sharedApplication(main)
        .currentEvent()
        .map_or([0; 2], |e| [e.phase().0 as u64, e.momentumPhase().0 as u64])
}

type Callback = Box<dyn Fn() + Send + Sync>;
#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVDisplayLinkCreateWithActiveCGDisplays(out: *mut *mut std::ffi::c_void) -> i32;
    fn CVDisplayLinkSetOutputCallback(
        link: *mut std::ffi::c_void,
        callback: unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *const std::ffi::c_void,
            *const std::ffi::c_void,
            u64,
            *mut u64,
            *mut std::ffi::c_void,
        ) -> i32,
        context: *mut std::ffi::c_void,
    ) -> i32;
    fn CVDisplayLinkStart(link: *mut std::ffi::c_void) -> i32;
    fn CVDisplayLinkStop(link: *mut std::ffi::c_void) -> i32;
    fn CVDisplayLinkSetCurrentCGDisplay(link: *mut std::ffi::c_void, display: u32) -> i32;
    fn CVDisplayLinkRelease(link: *mut std::ffi::c_void);
}

/// Display-paced wakeups; the callback never reads the presenter or the view.
pub struct DisplayLink {
    raw: *mut std::ffi::c_void,
    callback: Box<Callback>,
    running: bool,
}
impl DisplayLink {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        unsafe extern "C" fn tick(
            _: *mut std::ffi::c_void,
            _: *const std::ffi::c_void,
            _: *const std::ffi::c_void,
            _: u64,
            _: *mut u64,
            context: *mut std::ffi::c_void,
        ) -> i32 {
            let callback = unsafe { &*context.cast::<Callback>() };
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
            0
        }
        let mut result = Self {
            raw: std::ptr::null_mut(),
            callback: Box::new(Box::new(wake)),
            running: false,
        };
        // SAFETY: CoreVideo retains the context until the stopped link is released.
        // The double box keeps its address stable when this owner moves.
        unsafe {
            let status = CVDisplayLinkCreateWithActiveCGDisplays(&mut result.raw);
            if status != 0 {
                return Err(Error::Device(format!("CVDisplayLinkCreate: {status}")));
            }
            let status = CVDisplayLinkSetOutputCallback(
                result.raw,
                tick,
                (&mut *result.callback as *mut Callback).cast(),
            );
            if status != 0 {
                return Err(Error::Device(format!(
                    "CVDisplayLinkSetOutputCallback: {status}"
                )));
            }
        }
        Ok(result)
    }
    pub fn set_display(&self, display: u32) -> Result<()> {
        let status = unsafe { CVDisplayLinkSetCurrentCGDisplay(self.raw, display) };
        if status == 0 {
            Ok(())
        } else {
            Err(Error::Device(format!(
                "CVDisplayLinkSetCurrentCGDisplay: {status}"
            )))
        }
    }
    pub fn set_running(&mut self, running: bool) -> Result<()> {
        if running == self.running {
            return Ok(());
        }
        let status = unsafe {
            if running {
                CVDisplayLinkStart(self.raw)
            } else {
                CVDisplayLinkStop(self.raw)
            }
        };
        if status != 0 {
            return Err(Error::Device(format!("CVDisplayLink start/stop: {status}")));
        }
        self.running = running;
        Ok(())
    }
}
impl Drop for DisplayLink {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                if self.running {
                    CVDisplayLinkStop(self.raw);
                }
                CVDisplayLinkRelease(self.raw);
            }
        }
    }
}

/// Publish readiness to the optional, linked G1 benchmark probe.
/// The offset is in points; the probe records it separately from GPUI's item index.
pub fn benchmark_frame(complete: bool, offset: f32) {
    type Ready = unsafe extern "C" fn(i32, usize, f32);
    static READY: std::sync::OnceLock<Option<Ready>> = std::sync::OnceLock::new();
    let ready = READY.get_or_init(|| unsafe {
        let symbol = libc::dlsym(libc::RTLD_DEFAULT, c"g1_ready".as_ptr());
        if symbol.is_null() {
            None
        } else {
            Some(std::mem::transmute::<*mut std::ffi::c_void, Ready>(symbol))
        }
    });
    if let Some(ready) = ready {
        unsafe {
            ready(i32::from(complete), 0, offset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_releases_the_frame_and_wakes_pending_damage() {
        let pending = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || tx.send(()).unwrap());
        let frame = Flight::begin(pending.clone(), wake.clone()).unwrap();
        assert!(pending.load(Ordering::Acquire));
        assert!(Flight::begin(pending.clone(), wake.clone()).is_err());
        assert!(rx.try_recv().is_err());
        std::thread::spawn(move || drop(frame)).join().unwrap();
        rx.recv().unwrap();
        assert!(!pending.load(Ordering::Acquire));
        drop(Flight::begin(pending.clone(), wake).unwrap());
        rx.recv().unwrap();
        assert!(!pending.load(Ordering::Acquire));
        assert!(rx.try_recv().is_err());
    }
}

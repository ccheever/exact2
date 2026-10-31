//! The carrier owns the view; the render thread only owns its Metal layer.
//! @ref LLP 1149 D7, D11, D12.
#[cfg(exact_ink_metal)]
pub use super::metal::startup::Startup;
#[cfg(not(exact_ink_metal))]
#[path = "apple/startup_unavailable.rs"]
mod startup_unavailable;
use super::{Error, Result};
use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSView};
use objc2_core_foundation::CGSize;
use objc2_core_graphics::CGColorSpace;
use objc2_metal::{MTLDrawable, MTLPixelFormat};
use objc2_quartz_core::CAMetalLayer;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(not(exact_ink_metal))]
pub use startup_unavailable::Startup;
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
};

struct Inner {
    view: Retained<NSView>,
    layer: Retained<CAMetalLayer>,
    retry: Arc<Retry>,
    presentation: Arc<Presentation>,
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
            retry: Arc::default(),
            presentation: Arc::default(),
            wake: Arc::new(wake),
            _owner: owner,
        }));
        result.register_boot_expose();
        result.resize(size, scale);
        Ok(result)
    }

    pub fn resize(&self, size: [u32; 2], scale: f64) {
        self.resume_retries();
        self.0.layer.setContentsScale(scale);
        self.0
            .layer
            .setDrawableSize(CGSize::new(size[0] as f64, size[1] as f64));
    }

    fn register_boot_expose(&self) {
        if std::env::var_os("G1_BOOT_ONLY").is_none() {
            return;
        }
        type Register = unsafe extern "C" fn(*const std::ffi::c_void);
        // SAFETY: the optional probe copies a void(void) Objective-C block.
        let register = unsafe {
            let symbol = libc::dlsym(libc::RTLD_DEFAULT, c"g1_boot_expose".as_ptr());
            if symbol.is_null() {
                return;
            }
            std::mem::transmute::<*mut std::ffi::c_void, Register>(symbol)
        };
        let retry = self.0.retry.clone();
        let presentation = self.0.presentation.clone();
        let wake = self.0.wake.clone();
        let callback = block2::RcBlock::new(move || {
            if presentation.painter().is_none() {
                retry.exposed();
                wake();
            }
        });
        unsafe {
            register(block2::RcBlock::as_ptr(&callback).cast());
        }
    }

    pub fn retry_requested(&self) -> bool {
        let requested = self.0.retry.requested.swap(false, Ordering::AcqRel);
        if requested {
            trace_mark(c"presentation retry consumed", || 1.);
        }
        requested
    }

    /// An input or surface transition permits another recovery attempt.
    pub fn resume_retries(&self) {
        self.0.retry.reset();
    }

    /// Discard callbacks from the failed device generation.
    pub fn invalidate(&self) {
        self.0.presentation.invalidate();
        self.resume_retries();
    }

    /// The painter of the last drawable with a nonzero displayed timestamp.
    pub fn presented_painter(&self) -> Option<&'static str> {
        self.0.presentation.painter()
    }

    pub fn live_resize(&self) -> bool {
        self.0.view.inLiveResize()
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
            retry: self.0.retry.clone(),
            wake: self.0.wake.clone(),
            presentation: self.0.presentation.clone(),
            generation: self.0.presentation.0.lock().unwrap().generation,
        }
    }
}

pub(crate) struct RenderLayer {
    pub raw: Retained<CAMetalLayer>,
    retry: Arc<Retry>,
    wake: Arc<dyn Fn() + Send + Sync>,
    presentation: Arc<Presentation>,
    generation: u64,
}
impl RenderLayer {
    #[cfg(exact_ink_metal)]
    pub(crate) fn framebuffer_only(self) {
        self.raw.setFramebufferOnly(true);
    }
    pub fn track(
        &self,
        drawable: &objc2::runtime::ProtocolObject<dyn objc2_metal::MTLDrawable>,
        ink: bool,
    ) {
        trace_mark(c"drawable framebuffer only", || {
            f64::from(self.raw.framebufferOnly())
        });
        let retry = self.retry.clone();
        let wake = self.wake.clone();
        let presentation = self.presentation.clone();
        let generation = self.generation;
        let block = block2::RcBlock::new(
            move |shown: std::ptr::NonNull<
                objc2::runtime::ProtocolObject<dyn objc2_metal::MTLDrawable>,
            >| {
                // A drawable can complete on the GPU without reaching the screen,
                // including while the window's first layer transaction is pending.
                // https://developer.apple.com/documentation/metal/mtldrawable/presentedtime
                let time = unsafe { shown.as_ref().presentedTime() };
                trace_mark(c"presented handler", || time);
                trace_interval(c"presented", || (time, time, 0.));
                if !presentation.record(generation, time, ink) {
                    return;
                }
                if time == 0. && retry.failed() {
                    wake();
                } else if time != 0. {
                    crate::counters::displayed();
                    retry.reset();
                }
            },
        );
        // SAFETY: Metal copies the block and supplies a live drawable to it.
        unsafe {
            drawable.addPresentedHandler(block2::RcBlock::as_ptr(&block));
        }
    }
    pub fn retry(&self) {
        if self.retry.failed() {
            (self.wake)();
        }
    }
}

#[derive(Default)]
struct Presentation(Mutex<Shown>);
#[derive(Default)]
struct Shown {
    generation: u64,
    time: f64,
    painter: Option<&'static str>,
}
impl Presentation {
    fn painter(&self) -> Option<&'static str> {
        self.0.lock().unwrap().painter
    }
    fn invalidate(&self) {
        let mut shown = self.0.lock().unwrap();
        *shown = Shown {
            generation: shown.generation + 1,
            ..Shown::default()
        };
    }
    fn record(&self, generation: u64, time: f64, ink: bool) -> bool {
        let mut shown = self.0.lock().unwrap();
        if shown.generation != generation {
            return false;
        }
        // Completion callbacks can arrive in a different order from display.
        if time > shown.time {
            shown.time = time;
            shown.painter = Some(if ink { "ink" } else { "cpu" });
        }
        true
    }
}

#[derive(Default)]
struct Retry {
    failures: AtomicU8,
    requested: AtomicBool,
}
impl Retry {
    fn exposed(&self) {
        trace_mark(c"presentation exposed", || 0.);
        self.failures.store(0, Ordering::Release);
        self.requested.store(true, Ordering::Release);
    }
    fn failed(&self) -> bool {
        let previous = self
            .failures
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                Some(n.saturating_add(1).min(2))
            })
            .unwrap();
        trace_mark(c"presentation retry failures", || f64::from(previous) + 1.);
        if previous == 0 {
            self.requested.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }
    fn reset(&self) {
        self.failures.store(0, Ordering::Release);
        self.requested.store(false, Ordering::Release);
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

struct CallbackSlot(std::sync::Mutex<Option<Callback>>);
impl CallbackSlot {
    fn call(&self) {
        let callback = self.0.lock().unwrap();
        if let Some(callback) = callback.as_ref() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
        }
    }
    fn clear(&self) {
        self.0.lock().unwrap().take();
    }
}

/// Optional benchmark intervals on Core Animation's clock, shared with presentedTime.
/// The linked probe owns storage; ordinary carriers do not collect anything.
pub struct Trace {
    name: &'static core::ffi::CStr,
    start: f64,
    hook: TraceHook,
    value: f64,
}
impl Trace {
    pub fn new(name: &'static core::ffi::CStr) -> Option<Self> {
        trace_hook().map(|hook| Self {
            name,
            start: unsafe { CACurrentMediaTime() },
            hook,
            value: 0.,
        })
    }
    pub fn with_value(name: &'static core::ffi::CStr, value: f64) -> Option<Self> {
        Self::new(name).map(|mut span| {
            span.value = value;
            span
        })
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        unsafe {
            (self.hook)(
                self.name.as_ptr(),
                self.start,
                CACurrentMediaTime(),
                self.value,
            )
        };
    }
}
type TraceHook = unsafe extern "C" fn(*const core::ffi::c_char, f64, f64, f64);
fn trace_hook() -> Option<TraceHook> {
    static HOOK: std::sync::OnceLock<Option<TraceHook>> = std::sync::OnceLock::new();
    *HOOK.get_or_init(|| {
        std::env::var_os("G1_TRACE")?;
        // SAFETY: the optional benchmark probe exports this exact C signature.
        unsafe {
            let p = libc::dlsym(libc::RTLD_DEFAULT, c"g1_interval".as_ptr());
            (!p.is_null()).then(|| std::mem::transmute::<*mut core::ffi::c_void, TraceHook>(p))
        }
    })
}
#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {
    fn CACurrentMediaTime() -> f64;
}
pub fn trace_time() -> f64 {
    trace_hook().map_or(0., |_| unsafe { CACurrentMediaTime() })
}
#[inline]
pub fn trace_interval(name: &'static core::ffi::CStr, values: impl FnOnce() -> (f64, f64, f64)) {
    if let Some(hook) = trace_hook() {
        let (start, end, value) = values();
        unsafe { hook(name.as_ptr(), start, end, value) };
    }
}
#[inline]
pub fn trace_mark(name: &'static core::ffi::CStr, value: impl FnOnce() -> f64) {
    if let Some(hook) = trace_hook() {
        let now = unsafe { CACurrentMediaTime() };
        unsafe { hook(name.as_ptr(), now, now, value()) };
    }
}

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
    callback: &'static CallbackSlot,
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
            trace_mark(c"display callback", || 0.);
            let _trace = Trace::new(c"display link wake");
            let callback = unsafe { &*context.cast::<CallbackSlot>() };
            callback.call();
            0
        }
        let mut result = Self {
            raw: std::ptr::null_mut(),
            callback: Box::leak(Box::new(CallbackSlot(std::sync::Mutex::new(Some(
                Box::new(wake),
            ))))),
            running: false,
        };
        // SAFETY: the context has a stable address and outlives every callback.
        unsafe {
            let status = CVDisplayLinkCreateWithActiveCGDisplays(&mut result.raw);
            if status != 0 {
                return Err(Error::Device(format!("CVDisplayLinkCreate: {status}")));
            }
            let status = CVDisplayLinkSetOutputCallback(
                result.raw,
                tick,
                (result.callback as *const CallbackSlot).cast_mut().cast(),
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
        trace_mark(c"display running", || f64::from(running));
        Ok(())
    }
}
impl Drop for DisplayLink {
    fn drop(&mut self) {
        // CVDisplayLinkStop documents no callback join. Keep the empty context alive;
        // clearing it waits for an in-flight wake and releases that wake's captures.
        // https://developer.apple.com/documentation/corevideo/cvdisplaylinkstop(_:)
        self.callback.clear();
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

thread_local! {
    static BENCHMARK: std::cell::Cell<(bool, f32)> = const { std::cell::Cell::new((false, 0.)) };
}

/// Readiness and scroll offset of the frame being encoded, for the optional probe.
pub fn benchmark_frame(complete: bool, offset: f32) {
    BENCHMARK.set((complete, offset));
}

pub(crate) fn benchmark_scene() -> (bool, f32) {
    BENCHMARK.get()
}

/// Record which painter owns the next whole drawable, on the present worker.
pub(crate) fn benchmark_painter(ink: bool) {
    type Mark = unsafe extern "C" fn(i32);
    static MARK: std::sync::OnceLock<Option<Mark>> = std::sync::OnceLock::new();
    let mark = MARK.get_or_init(|| unsafe {
        let symbol = libc::dlsym(libc::RTLD_DEFAULT, c"g1_painter".as_ptr());
        if symbol.is_null() {
            None
        } else {
            Some(std::mem::transmute::<*mut std::ffi::c_void, Mark>(symbol))
        }
    });
    if let Some(mark) = mark {
        unsafe {
            mark(i32::from(ink));
        }
    }
}

/// Let the linked probe read the carrier thread's counters at its CPU endpoints.
pub fn benchmark_counters() {
    if !crate::counters::enabled() {
        return;
    }
    type Register = unsafe extern "C" fn(
        extern "C" fn(usize) -> *const core::ffi::c_char,
        extern "C" fn(usize, bool) -> u64,
    );
    // SAFETY: the optional probe exports this signature and calls back on main.
    unsafe {
        let p = libc::dlsym(libc::RTLD_DEFAULT, c"g1_set_counter_reader".as_ptr());
        if !p.is_null() {
            std::mem::transmute::<*mut core::ffi::c_void, Register>(p)(
                crate::counters::probe_name,
                crate::counters::probe_value,
            );
        }
    }
}

/// Only a linked benchmark probe can replace the final sample with a clear.
pub fn benchmark_present_clear() -> bool {
    type Clear = unsafe extern "C" fn() -> i32;
    static CLEAR: std::sync::OnceLock<Option<Clear>> = std::sync::OnceLock::new();
    let clear = CLEAR.get_or_init(|| unsafe {
        let symbol = libc::dlsym(libc::RTLD_DEFAULT, c"g1_present_clear".as_ptr());
        (!symbol.is_null()).then(|| std::mem::transmute::<*mut std::ffi::c_void, Clear>(symbol))
    });
    clear.is_some_and(|clear| unsafe { clear() != 0 })
}

/// Set the probe's metadata on the worker immediately before drawable acquisition.
pub(crate) fn benchmark_present((complete, offset): (bool, f32)) {
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
    fn painter_identity_follows_display_and_rejects_a_lost_generations_callbacks() {
        let presented = Presentation::default();
        assert_eq!(presented.painter(), None);
        presented.record(0, 0., true);
        assert_eq!(presented.painter(), None);
        presented.record(0, 1., false);
        assert_eq!(presented.painter(), Some("cpu"));
        presented.record(0, 0., true);
        assert_eq!(presented.painter(), Some("cpu"));
        presented.record(0, 2., true);
        assert_eq!(presented.painter(), Some("ink"));
        presented.record(0, 1.5, false);
        assert_eq!(presented.painter(), Some("ink"));
        presented.invalidate();
        assert!(!presented.record(0, 3., true));
        assert_eq!(presented.painter(), None);
        presented.record(1, 4., false);
        assert_eq!(presented.painter(), Some("cpu"));
    }

    #[test]
    fn clearing_the_display_callback_waits_for_a_running_wake_and_releases_it() {
        let (entered, started) = std::sync::mpsc::channel();
        let (release, finish) = std::sync::mpsc::channel();
        let finish = std::sync::Mutex::new(finish);
        let capture = Arc::new(());
        let weak = Arc::downgrade(&capture);
        let slot = CallbackSlot(std::sync::Mutex::new(Some(Box::new(move || {
            let _keep = &capture;
            entered.send(()).unwrap();
            finish.lock().unwrap().recv().unwrap();
        }))));
        std::thread::scope(|scope| {
            scope.spawn(|| slot.call());
            started.recv().unwrap();
            let (cleared, done) = std::sync::mpsc::channel();
            let slot = &slot;
            scope.spawn(move || {
                slot.clear();
                cleared.send(()).unwrap();
            });
            assert!(matches!(
                done.recv_timeout(std::time::Duration::from_millis(100)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            release.send(()).unwrap();
            done.recv().unwrap();
        });
        assert!(weak.upgrade().is_none());
        slot.call();
    }

    #[test]
    fn callback_retry_allowance_resets_on_activity_or_a_shown_frame() {
        let retry = Retry::default();
        for _ in 0..3 {
            assert!(retry.failed());
            assert!(retry.requested.swap(false, Ordering::AcqRel));
            for _ in 0..8 {
                assert!(!retry.failed());
                assert!(!retry.requested.load(Ordering::Acquire));
            }
            retry.reset();
        }
        assert!(retry.failed());
        assert!(!retry.failed());
        retry.exposed();
        assert!(retry.requested.swap(false, Ordering::AcqRel));
        assert!(
            retry.failed(),
            "the first expose restores the retry allowance"
        );
    }

    #[test]
    fn disabled_trace_never_evaluates_its_arguments_or_constructs_a_span() {
        assert!(
            trace_hook().is_none(),
            "run this test without a linked trace probe"
        );
        assert!(Trace::new(c"unused span").is_none());
        trace_mark(c"unused value", || panic!("disabled value evaluated"));
        trace_interval(c"unused interval", || panic!("disabled interval evaluated"));
        assert_eq!(trace_time(), 0.);
    }
}

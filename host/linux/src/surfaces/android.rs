//! GPU canvases that present, on Android (LLP 1009 on the Canvas host,
//! LLP 1076): the module loads with a device, a canvas is made as on Linux
//! (headless, so its binding and assets run unchanged) and is given the
//! window its reader's view makes for it (`gpu_attach`); the host renders and
//! flushes every attached canvas each frame. Shader text (LLP 1030 D8) is the
//! app's `shaders/*.wgsl` under the asset root.
#![allow(unsafe_code)]
use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The module's native library directory: `current_exe` is the zygote's.
pub(super) fn library_dir() -> Option<PathBuf> {
    std::env::var_os("EXACT_NATIVE_LIBS").map(|d| PathBuf::from(d).join("app"))
}

/// Load `abi` with a device and register the app's shaders.
pub(super) fn load(abi: &Abi) -> Result<(), String> {
    if unsafe { abi.symbol::<unsafe extern "C" fn() -> u32>(b"gpu_load")() } != 0 {
        return Err(abi.error().unwrap_or_else(|| "gpu_load failed".into()));
    }
    let root = std::env::var_os("EXACT_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_default();
    let shader = unsafe {
        abi.symbol::<unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> u32>(b"gpu_shader")
    };
    if let Ok(dir) = std::fs::read_dir(root.join("shaders")) {
        for entry in dir.flatten() {
            let path = entry.path();
            let (Some(name), Some("wgsl")) = (
                path.file_stem().and_then(|s| s.to_str()),
                path.extension().and_then(|s| s.to_str()),
            ) else {
                continue;
            };
            let text = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if unsafe { shader(name.as_ptr(), name.len(), text.as_ptr(), text.len()) } != 0 {
                return Err(abi
                    .error()
                    .unwrap_or_else(|| format!("shader {name} refused")));
            }
        }
    }
    Ok(())
}

/// Start making the app's primary GPU module's device, on threads of
/// their own, while the host boots (LLP 1076): the canvas's first frame on
/// the host's thread loads the module (`gpu_load`, which takes the device)
/// without making one. Nothing when the app has no GPU module.
pub(crate) fn prepare(compat: &'static str) {
    // Set before the thread runs: no frame may open the module in between.
    PREPARING.store(true, Ordering::Release);
    let made = std::thread::Builder::new()
        .name("exact-gpu-open".into())
        .spawn(move || {
            let done = || PREPARING.store(false, Ordering::Release);
            let Ok(compat) = serde_json::from_str::<Value>(compat) else {
                return done();
            };
            if !gpu_card(&compat, "").is_object() {
                return done();
            }
            let Ok(abi) = Abi::open_as(&compat, "", false) else {
                return done();
            };
            // A module without it was built before it: it loads as it did.
            if unsafe { abi.library.get::<unsafe extern "C" fn()>(b"gpu_prepare") }.is_ok() {
                unsafe { abi.symbol::<unsafe extern "C" fn()>(b"gpu_prepare")() };
            }
            match unsafe {
                abi.library
                    .get::<unsafe extern "C" fn() -> u32>(b"gpu_prepared")
            } {
                Ok(ready) => PREPARED_FN.store(*ready as usize, Ordering::Release),
                Err(_) => done(),
            }
            // The library stays loaded (the host's own open finds it, and
            // PREPARED_FN stays callable), and nothing it made belongs to this
            // thread.
            std::mem::forget(abi);
        });
    if made.is_err() {
        PREPARING.store(false, Ordering::Release);
    }
}

/// [`prepare`] is still opening the module or making its device.
static PREPARING: AtomicBool = AtomicBool::new(false);
/// The module's `gpu_prepared`, once [`prepare`] opened it.
static PREPARED_FN: AtomicUsize = AtomicUsize::new(0);

/// Whether the device [`prepare`] makes is not ready yet: opening the module
/// now would wait for it. A host skips its canvases for the frame instead.
pub(crate) fn device_pending() -> bool {
    if !PREPARING.load(Ordering::Acquire) {
        return false;
    }
    let ready = PREPARED_FN.load(Ordering::Acquire);
    if ready == 0 {
        return true;
    }
    // SAFETY: the module's own `gpu_prepared`, from a library that is never
    // unloaded (prepare forgot its handle).
    let ready: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(ready) };
    if unsafe { ready() } == 1 {
        PREPARING.store(false, Ordering::Release);
        return false;
    }
    true
}

/// A window a reader gave a canvas's view: the `ANativeWindow`, its pixel
/// size, and whether the canvas has it. No window (0) is a canvas that
/// renders into buffers the reader draws itself ([`frames`]).
pub(crate) struct Window {
    window: usize,
    size: (u32, u32),
    attached: bool,
}

/// A canvas's newest finished frame, for a reader on another thread: the
/// module's `[generation, slot, serial]` and a reference to the buffer.
struct BufferFrame {
    words: [u32; 3],
    buffer: usize,
}

impl Drop for BufferFrame {
    fn drop(&mut self) {
        // SAFETY: the reference `publish` acquired.
        unsafe { AHardwareBuffer_release(self.buffer as *mut _) };
    }
}

#[link(name = "android")]
extern "C" {
    fn AHardwareBuffer_acquire(buffer: *mut std::ffi::c_void);
    fn AHardwareBuffer_release(buffer: *mut std::ffi::c_void);
}

/// By canvas view: `None` once the module refused buffers (the reader gives
/// the canvas a window instead).
static FRAMES: std::sync::Mutex<BTreeMap<u32, Option<BufferFrame>>> =
    std::sync::Mutex::new(BTreeMap::new());

fn frames() -> std::sync::MutexGuard<'static, BTreeMap<u32, Option<BufferFrame>>> {
    FRAMES.lock().unwrap_or_else(|e| e.into_inner())
}

/// What canvas `view` has to draw, from any thread: `Ok` its newest frame's
/// words and its buffer (an `AHardwareBuffer*` the caller now holds a
/// reference to when `take`, else null), `Err(false)` nothing yet,
/// `Err(true)` no buffers on this device.
pub fn buffer_frame(view: u32, take: bool) -> Result<([u32; 3], usize), bool> {
    match frames().get(&view) {
        Some(Some(f)) => {
            if take {
                // SAFETY: the map's own reference keeps it alive here.
                unsafe { AHardwareBuffer_acquire(f.buffer as *mut _) };
            }
            Ok((f.words, if take { f.buffer } else { 0 }))
        }
        Some(None) => Err(true),
        None => Err(false),
    }
}

impl Surfaces {
    /// Attach every window whose canvas exists and has not got it.
    pub(super) fn attach_windows(&mut self) {
        for (view, w) in self.windows.iter_mut() {
            let Some(c) = self.canvases.get(view).filter(|_| !w.attached) else {
                continue;
            };
            let abi = &self.abis[&c.artifact];
            if w.window == 0 {
                // A module built before buffers, or a device without them:
                // the reader hears, and gives the canvas a window.
                let made = unsafe {
                    abi.library
                        .get::<unsafe extern "C" fn(u32, u32, u32) -> u32>(b"gpu_attach_buffers")
                }
                .is_ok_and(|attach| unsafe { attach(c.id, w.size.0, w.size.1) } == 0);
                if made {
                    w.attached = true;
                } else {
                    let _ = abi.error();
                    frames().insert(*view, None);
                }
                continue;
            }
            let attach = unsafe {
                abi.symbol::<unsafe extern "C" fn(u32, *mut std::ffi::c_void, u32, u32) -> u32>(
                    b"gpu_attach",
                )
            };
            if unsafe { attach(c.id, w.window as *mut _, w.size.0, w.size.1) } == 0 {
                w.attached = true;
            } else {
                self.error = abi.error();
            }
        }
    }

    /// Each buffered canvas's newest finished frame, where a reader on
    /// another thread finds it ([`buffer_frame`]).
    fn publish_buffers(&mut self) {
        for (view, w) in &self.windows {
            if w.window != 0 || !w.attached {
                continue;
            }
            let Some(c) = self.canvases.get(view) else {
                continue;
            };
            let abi = &self.abis[&c.artifact];
            let mut words = [0u32; 3];
            let buffer = unsafe {
                abi.symbol::<unsafe extern "C" fn(u32, *mut u32) -> *mut std::ffi::c_void>(
                    b"gpu_buffer",
                )(c.id, words.as_mut_ptr())
            };
            if buffer.is_null() {
                continue;
            }
            let mut frames = frames();
            if frames
                .get(view)
                .is_some_and(|f| f.as_ref().is_some_and(|f| f.words == words))
            {
                continue;
            }
            // SAFETY: the module's ring keeps the buffer alive on this
            // thread; the map takes a reference of its own.
            unsafe { AHardwareBuffer_acquire(buffer) };
            frames.insert(
                *view,
                Some(BufferFrame {
                    words,
                    buffer: buffer as usize,
                }),
            );
        }
    }

    fn detach(&mut self, view: u32) {
        frames().remove(&view);
        if let (Some(w), Some(c)) = (self.windows.remove(&view), self.canvases.get(&view)) {
            if w.attached {
                let abi = &self.abis[&c.artifact];
                unsafe { abi.symbol::<unsafe extern "C" fn(u32)>(b"gpu_detach")(c.id) };
            }
        }
    }
}

impl<D: DataSource> Presenter<D> {
    /// A reader's window for canvas `view` (an `ANativeWindow` of `size`
    /// pixels, alive until [`Presenter::detach_window`]). Whether the view is
    /// a GPU canvas (a 2D canvas's window is never used).
    pub(crate) fn attach_window(&mut self, view: u32, window: usize, size: (u32, u32)) -> bool {
        self.surfaces.detach(view);
        self.surfaces.windows.insert(
            view,
            Window {
                window,
                size,
                attached: false,
            },
        );
        self.surfaces.attach_windows();
        self.surfaces.canvases.contains_key(&view)
    }

    /// The window of `view` is going: the canvas presents nowhere until another.
    pub(crate) fn detach_window(&mut self, view: u32) {
        self.surfaces.detach(view);
    }

    /// Render every attached canvas at `now` and present them; whether any
    /// wants another frame.
    pub(crate) fn render_surfaces(&mut self, now: f64) -> bool {
        let scale = self.brush.scale;
        let mut wants = false;
        let mut flush = BTreeSet::new();
        let views: Vec<(u32, bool)> = self
            .surfaces
            .windows
            .iter()
            .filter(|(_, w)| w.attached)
            .map(|(v, w)| (*v, w.window == 0))
            .collect();
        let (vw, vh) = self.viewport();
        for (view, buffered) in views {
            let Some((x, y, w, h)) = self.rect_of(view) else {
                continue;
            };
            // A canvas with a window stops when its view is not drawn (the
            // compositor takes no frame, so none can be acquired). One with
            // buffers has no such word: it draws while it is in the viewport,
            // or just outside (its box is the last paint's, which a scroll
            // has moved since).
            if buffered
                && (y + h < -0.05 * vh || y > 1.05 * vh || x + w < -0.05 * vw || x > 1.05 * vw)
            {
                continue;
            }
            let Some(c) = self.surfaces.canvases.get(&view) else {
                continue;
            };
            let abi = &self.surfaces.abis[&c.artifact];
            let render = unsafe {
                abi.symbol::<unsafe extern "C" fn(u32, f32, f32, f32, f64) -> u32>(b"gpu_render")
            };
            match unsafe { render(c.id, w, h, scale, now) } {
                1 => wants = true,
                2 => self.surfaces.error = abi.error(),
                _ => {}
            }
            flush.insert(c.artifact.clone());
        }
        for artifact in flush {
            let abi = &self.surfaces.abis[&artifact];
            if unsafe { abi.symbol::<unsafe extern "C" fn() -> u32>(b"gpu_flush")() } != 0 {
                self.surfaces.error = abi.error();
            }
        }
        self.surfaces.publish_buffers();
        wants
    }
}

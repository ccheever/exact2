//! The Android host: this crate's presenter and painter on a render thread
//! of an Android app, presenting into a `Surface` (a `SurfaceView`'s window)
//! through the GPU painter's swapchain path (`gpu/present.rs`). The thread
//! is an `ALooper` thread: `AChoreographer` paces paint to the display's
//! vsync, the presenter's executor and image wakes are looper fds, and the
//! app's Kotlin side reaches it through [`Handle`] (an eventfd-woken queue).
//!
//! Same loop as the DRM display (`display.rs`): paint when something changed
//! and the display can take a frame, timers then frame tasks once per
//! frame, input as it arrives. The app crate owns JNI and hands this module
//! an `ANativeWindow` it has acquired.
//!
//! @ref LLP 1076 §3.4 (reuse Exact's Rust painter on Android)

#![allow(unsafe_code)]

use exact_runner::DataSource;
use std::cell::Cell;
use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

#[link(name = "android")]
extern "C" {
    fn ANativeWindow_release(window: *mut c_void);
    fn ALooper_prepare(opts: c_int) -> *mut c_void;
    fn ALooper_pollOnce(
        timeout: c_int,
        fd: *mut c_int,
        events: *mut c_int,
        data: *mut *mut c_void,
    ) -> c_int;
    fn ALooper_addFd(
        looper: *mut c_void,
        fd: c_int,
        ident: c_int,
        events: c_int,
        callback: *const c_void,
        data: *mut c_void,
    ) -> c_int;
    fn AChoreographer_getInstance() -> *mut c_void;
    fn AChoreographer_postFrameCallback64(
        choreographer: *mut c_void,
        callback: extern "C" fn(i64, *mut c_void),
        data: *mut c_void,
    );
    fn ATrace_beginSection(name: *const c_char);
    fn ATrace_endSection();
}

#[link(name = "log")]
extern "C" {
    fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

/// One line to logcat under `exact`.
pub fn log(text: &str) {
    let tag = c"exact";
    if let Ok(text) = CString::new(text.replace('\0', " ")) {
        // SAFETY: both strings are NUL-terminated and live across the call.
        unsafe { __android_log_write(4, tag.as_ptr(), text.as_ptr()) };
    }
}

/// Send this process's stdout and stderr to logcat, line by line: the host
/// reports through `eprintln!`, which Android otherwise discards.
pub fn redirect_stdio() {
    let mut fds = [0 as c_int; 2];
    // SAFETY: a fresh pipe; its write end replaces fds 1 and 2.
    unsafe {
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return;
        }
        libc::dup2(fds[1], 1);
        libc::dup2(fds[1], 2);
    }
    let read = fds[0];
    std::thread::Builder::new()
        .name("exact-log".into())
        .spawn(move || {
            use std::io::BufRead;
            use std::os::fd::FromRawFd;
            // SAFETY: the read end is ours alone from here on.
            let file = unsafe { std::fs::File::from_raw_fd(read) };
            for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
                log(&line);
            }
        })
        .ok();
}

/// Let go of the read-only pages of the library holding `at` (its code,
/// constants and the bytes it carries, such as the plan): each is dropped from this process's
/// page tables and comes back, from the page cache, the next time it is
/// touched. What boot alone touched (decoding the plan, the app's baked
/// data, one-time setup) then stops counting toward the process. The
/// writable segments (relocated data, `.data`, `.bss`) are left alone.
/// Returns the bytes let go of.
fn release_pages_of(at: *const c_void) -> usize {
    /// The loader's view of an ELF64 file header, up to the program headers.
    #[repr(C)]
    struct Header {
        ident: [u8; 16],
        _kind: u16,
        _machine: u16,
        _version: u32,
        _entry: u64,
        phoff: u64,
        _shoff: u64,
        _flags: u32,
        _ehsize: u16,
        phentsize: u16,
        phnum: u16,
    }
    /// A program header's loadable type and writable flag (ELF's `PT_LOAD`, `PF_W`).
    const PT_LOAD: u32 = 1;
    const PF_W: u32 = 2;
    // Only this library's own headers are read (`dl_iterate_phdr` would touch
    // the first page of every library in the process, ~11 MB of mappings).
    // SAFETY: `Dl_info` is plain pointers, for which zero is a valid value.
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    // SAFETY: `dladdr` fills `info` for an address inside a loaded object
    // (`at` is in one: this library, or a module it opened and keeps).
    if unsafe { libc::dladdr(at, &mut info) } == 0 || info.dli_fbase.is_null() {
        return 0;
    }
    let base = info.dli_fbase as usize;
    // SAFETY: `dli_fbase` is where the loader mapped this library's first
    // segment, which starts with its ELF header; the program headers it
    // names are in that segment (the loader read them from there).
    let headers = unsafe {
        let header = &*(base as *const Header);
        if header.ident[..4] != *b"\x7fELF"
            || header.phentsize as usize != std::mem::size_of::<libc::Elf64_Phdr>()
        {
            return 0;
        }
        std::slice::from_raw_parts(
            (base + header.phoff as usize) as *const libc::Elf64_Phdr,
            header.phnum as usize,
        )
        .to_vec()
    };
    // SAFETY: sysconf has no preconditions.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as usize;
    let mut released = 0;
    for h in headers {
        if h.p_type != PT_LOAD || h.p_flags & PF_W != 0 {
            continue;
        }
        let start = (base + h.p_vaddr as usize).next_multiple_of(page);
        let end = (base + (h.p_vaddr + h.p_filesz) as usize) / page * page;
        // SAFETY: a read-only, file-backed private mapping of this library:
        // dropping its pages loses nothing (they read back from the file
        // unchanged) and no Rust reference observes it.
        if end > start
            && unsafe { libc::madvise(start as *mut c_void, end - start, libc::MADV_DONTNEED) } == 0
        {
            released += end - start;
        }
    }
    released
}

/// The libraries this app loads besides its own (the GPU module, LLP 1009),
/// each by an address inside it: their read-only pages go too.
static MODULES: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());

/// `at`'s library is the app's and stays loaded: [`release_library_pages`]
/// lets go of its pages too.
pub fn release_module_pages_too(at: *const c_void) {
    MODULES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(at as usize);
}

/// [`release_pages_of`] this library and the modules registered with
/// [`release_module_pages_too`]; the bytes let go of.
pub fn release_library_pages() -> usize {
    let mut released = release_pages_of(release_library_pages as *const c_void);
    for at in MODULES.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        released += release_pages_of(*at as *const c_void);
    }
    released
}

/// [`release_library_pages`] once, `after` this call, on a thread of its
/// own: boot's one-time work (the plan's decode, the app's baked data, setup)
/// is over by then, and what the app goes on to use comes back as it runs.
/// Later calls do nothing.
pub fn release_library_pages_after(after: std::time::Duration) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("exact-release".into())
            .stack_size(64 * 1024)
            .spawn(move || {
                std::thread::sleep(after);
                trace(c"exact release pages", release_library_pages);
            });
    });
}

/// What the app's Kotlin side asks of the render thread.
pub enum Command {
    /// Scroll whatever is under the viewport's center by this many pixels
    /// (the benchmark driver's per-frame step).
    Scroll(f32),
    /// A touch: 0 down, 1 up, 2 move, 3 cancel; pixels.
    Touch(i32, f32, f32),
    /// Paint again (the app attached a surface the painter presents to).
    Repaint,
    /// The surface is going away: stop painting and drop the painter.
    Stop,
}

/// The app's end of a running host.
pub struct Handle {
    tx: Sender<Command>,
    wake: c_int,
    /// Set once the first frame has been presented.
    pub first_frame: Arc<AtomicBool>,
    /// Frames presented so far.
    pub frames: Arc<AtomicU64>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Handle {
    /// Queue a command and wake the render thread.
    pub fn send(&self, command: Command) {
        if self.tx.send(command).is_ok() {
            let one: u64 = 1;
            // SAFETY: an eventfd write of one 8-byte counter increment.
            unsafe { libc::write(self.wake, (&one as *const u64).cast(), 8) };
        }
    }

    /// Stop the render thread and wait for it.
    pub fn stop(mut self) {
        self.send(Command::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        // SAFETY: the render thread is gone; the eventfd is ours to close.
        unsafe { libc::close(self.wake) };
    }
}

/// The surface and how to boot on it.
pub struct Launch {
    /// The plan the binary carries.
    pub plan: &'static [u8],
    /// Its compatibility record.
    pub compat: &'static str,
    /// An `ANativeWindow` whose reference passes to the host (released when
    /// the render thread ends), or null: the app boots before its surface
    /// exists and its painter attaches one later (then sends
    /// [`Command::Repaint`]).
    pub window: *mut c_void,
    /// The window's size, pixels.
    pub size: (u32, u32),
    /// Device pixels per point (CSS px = dp).
    pub scale: f32,
    /// The window's safe-area insets (top, right, bottom, left), pixels:
    /// a surface under the status bar reports it so `env()` reserves it.
    pub insets: [f32; 4],
}

struct Window(*mut c_void);
// SAFETY: an ANativeWindow reference may be used and released from any thread.
unsafe impl Send for Window {}

/// Boot `D`'s app on its own render thread over the window. The environment
/// (`EXACT_ASSETS`, `EXACT_CACHE`, `EXACT_FONTS`, …) is read as on Linux, so
/// set it before calling.
pub fn start<D: DataSource + Default + 'static>(launch: Launch) -> Handle {
    let (tx, rx) = channel();
    // SAFETY: a fresh non-blocking eventfd.
    let wake = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
    let first_frame = Arc::new(AtomicBool::new(false));
    let frames = Arc::new(AtomicU64::new(0));
    let (ff, fr) = (first_frame.clone(), frames.clone());
    let window = Window(launch.window);
    let (plan, compat, size, scale) = (launch.plan, launch.compat, launch.size, launch.scale);
    let insets = launch.insets;
    let thread = std::thread::Builder::new()
        .name("exact-render".into())
        .spawn(move || {
            let window = window;
            // A display thread (Android's THREAD_PRIORITY_DISPLAY and below):
            // the first the system allows, before the boot it runs.
            for nice in [-10, -8, -4] {
                // SAFETY: PRIO_PROCESS of this thread.
                if unsafe {
                    libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, nice)
                } == 0
                {
                    break;
                }
            }
            run::<D>(
                plan, compat, window.0, size, scale, insets, rx, wake, ff, fr,
            );
            // SAFETY: the painter (and its surface) dropped inside `run`.
            if !window.0.is_null() {
                unsafe { ANativeWindow_release(window.0) };
            }
        })
        .expect("render thread");
    Handle {
        tx,
        wake,
        first_frame,
        frames,
        thread: Some(thread),
    }
}

/// Whether a vsync arrived since the last paint (written by the
/// choreographer's callback on this same thread).
struct Vsync {
    requested: Cell<bool>,
    arrived: Cell<bool>,
}

extern "C" fn on_vsync(_frame_time_nanos: i64, data: *mut c_void) {
    // SAFETY: `data` is the loop's `Vsync`, alive for the loop's duration,
    // and the callback runs on the loop's own thread inside `pollOnce`.
    let v = unsafe { &*data.cast::<Vsync>() };
    v.requested.set(false);
    v.arrived.set(true);
}

/// Run the calling thread at background priority (nice 10, Android's
/// `THREAD_PRIORITY_BACKGROUND`), as an image loader's decoders run: below
/// the main thread and RenderThread, whose frames come first. A thread a
/// reader's own exact2 thread spawns would otherwise inherit its priority.
pub fn background_priority() {
    // SAFETY: setpriority on this thread's own id; no memory is passed.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, 10);
    }
}

/// Run the calling thread only on the cores outside the slowest cluster
/// (those whose top clock is above the lowest top clock). A thread that does
/// a frame's work in bursts and sleeps between them looks idle to the
/// scheduler, which wakes it on a little core: a tap's commit there took two
/// to three times as long. Whether the mask was set.
pub fn fast_cores() -> bool {
    let fast = fast_set();
    if fast.is_empty() {
        return false;
    }
    // SAFETY: a zeroed cpu_set_t is the empty set; CPU_SET writes within it
    // (every index is below CPU_SETSIZE); the call reads it for its duration.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for &cpu in fast {
            libc::CPU_SET(cpu, &mut set);
        }
        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0
    }
}

/// The cores outside the slowest cluster, read once (a thread widens its
/// mask to them again after every tap it was colocated for,
/// `crate::android_hint::colocate`).
pub fn fast_set() -> &'static [usize] {
    static FAST: std::sync::OnceLock<Vec<usize>> = std::sync::OnceLock::new();
    FAST.get_or_init(|| {
        let mut tops: Vec<(usize, u64)> = Vec::new();
        for cpu in 0..libc::CPU_SETSIZE {
            let dir = format!("/sys/devices/system/cpu/cpu{cpu}");
            if !std::path::Path::new(&dir).exists() {
                break;
            }
            let top = std::fs::read_to_string(format!("{dir}/cpufreq/cpuinfo_max_freq"));
            if let Some(top) = top.ok().and_then(|t| t.trim().parse().ok()) {
                tops.push((cpu, top));
            }
        }
        let slowest = tops.iter().map(|t| t.1).min().unwrap_or(0);
        tops.iter().filter(|t| t.1 > slowest).map(|t| t.0).collect()
    })
}

/// Open an atrace section on this thread (close it with [`section_end`]).
pub fn section_begin(name: &core::ffi::CStr) {
    // SAFETY: a NUL-terminated name.
    unsafe { ATrace_beginSection(name.as_ptr()) };
}

/// Close the innermost atrace section this thread opened.
pub fn section_end() {
    // SAFETY: pairs with `section_begin` on this thread.
    unsafe { ATrace_endSection() };
}

/// Run `f` inside an atrace section (Perfetto shows it on this thread).
pub fn trace<T>(name: &core::ffi::CStr, f: impl FnOnce() -> T) -> T {
    // SAFETY: a NUL-terminated name; begin and end pair on this thread.
    unsafe { ATrace_beginSection(name.as_ptr()) };
    let r = f();
    // SAFETY: closes the section begun above.
    unsafe { ATrace_endSection() };
    r
}

#[allow(clippy::too_many_arguments)]
fn run<D: DataSource + Default>(
    plan: &'static [u8],
    compat: &'static str,
    window: *mut c_void,
    (pw, ph): (u32, u32),
    scale: f32,
    insets: [f32; 4],
    rx: Receiver<Command>,
    wake: c_int,
    first_frame: Arc<AtomicBool>,
    frames: Arc<AtomicU64>,
) {
    let started = Instant::now();
    let wall = || started.elapsed().as_secs_f64() * 1000.0;
    // SAFETY: prepares this thread's looper (allowing callbacks) and adds
    // fds the loop owns or the presenter keeps open while it lives.
    let looper = unsafe { ALooper_prepare(1) }; // ALOOPER_PREPARE_ALLOW_NON_CALLBACKS
    let choreographer = unsafe { AChoreographer_getInstance() };
    unsafe { ALooper_addFd(looper, wake, 1, 1, std::ptr::null(), std::ptr::null_mut()) };
    crate::gpu::set_window(window, pw, ph);
    let viewport = (pw as f32 / scale, ph as f32 / scale);
    std::env::set_var("EXACT_SCALE", scale.to_string());
    let mut config = crate::app::Config::from_env_static(plan, compat);
    config.scale = scale;
    let booted = trace(c"exact boot", || {
        crate::app::boot_presenter::<D>(&mut config, viewport)
    });
    crate::gpu::set_window(std::ptr::null_mut(), 0, 0);
    let (mut p, error) = match booted {
        Ok(v) => v,
        Err(e) => {
            log(&format!("exact: boot: {e}"));
            return;
        }
    };
    if let Some(e) = error {
        log(&format!("exact: {e}"));
    }
    if insets.iter().any(|i| *i != 0.0) {
        if let Some(e) = p.set_safe_area(insets.map(|i| i / scale)) {
            log(&format!("exact: {e}"));
        }
    }
    log(&format!(
        "exact: {pw}x{ph} px, scale {scale}, viewport {:.1}x{:.1}, painter {} {:?}, boot {:.1} ms (fonts {:.1} ms)",
        viewport.0,
        viewport.1,
        p.painter.name,
        p.painter.adapter,
        wall(),
        p.fonts_ms
    ));
    // SAFETY: fds the presenter owns for its whole life.
    unsafe {
        ALooper_addFd(
            looper,
            p.executor_fd(),
            2,
            1,
            std::ptr::null(),
            std::ptr::null_mut(),
        );
        ALooper_addFd(
            looper,
            p.image_fd(),
            3,
            1,
            std::ptr::null(),
            std::ptr::null_mut(),
        );
    }
    // Shared with the callback through a pointer, so every field is a Cell.
    let vsync = Box::new(Vsync {
        requested: Cell::new(false),
        arrived: Cell::new(true),
    });
    let vsync_ptr: *const Vsync = &*vsync;
    let center = (viewport.0 / 2.0, viewport.1 / 2.0);
    // `EXACT_DEFER_COLLECTIONS=0` runs a list's turn before its frame.
    p.set_deferred_collections(std::env::var("EXACT_DEFER_COLLECTIONS").as_deref() != Ok("0"));
    let mut last_tick = 0.0f64;
    let mut first_pixel = false;
    let frame_ms = 1000.0 / 120.0;
    let mut images_woke = true;
    loop {
        let mut drain = [0u8; 8];
        // SAFETY: a non-blocking read of the eventfd's counter.
        unsafe { libc::read(wake, drain.as_mut_ptr().cast(), 8) };
        if let Some(e) = trace(c"exact pump", || p.pump(wall())) {
            log(&format!("exact: {e}"));
        }
        trace(c"exact poll update", || p.poll_update());
        trace(c"exact commands", || p.run_commands(D::default));
        // Input after the presenter's own work (as the DRM loop orders it),
        // so a scroll step's dirt reaches this vsync's frame.
        let mut stop = false;
        while let Ok(c) = rx.try_recv() {
            let now = wall();
            let r = match c {
                Command::Scroll(dy) => {
                    trace(c"exact scroll", || {
                        p.wheel_at(center.0, center.1, 0.0, dy / scale)
                    });
                    Ok(())
                }
                Command::Touch(action, x, y) => {
                    let (x, y) = (x / scale, y / scale);
                    match action {
                        0 => p.pointer_down(x, y, now).map(|_| ()),
                        1 => p.pointer_up(x, y, now).map(|_| ()),
                        2 => p.pointer_move(x, y, now).map(|_| ()),
                        _ => p.pointer_cancel(now),
                    }
                }
                Command::Repaint => {
                    p.repaint();
                    Ok(())
                }
                Command::Stop => {
                    stop = true;
                    Ok(())
                }
            };
            if let Err(e) = r {
                log(&format!("exact: {e}"));
            }
        }
        if stop {
            break;
        }
        let now = wall();
        if vsync.arrived.get() && p.host().wants_frames() {
            last_tick = now;
            if let Some(e) = trace(c"exact animation frame", || p.animation_frame(now)) {
                log(&format!("exact: {e}"));
            }
        } else if p
            .host()
            .timer_due_ms()
            .is_some_and(|due| due <= now && now - last_tick >= frame_ms * 0.5)
        {
            last_tick = now;
            if let Some(e) = trace(c"exact advance", || p.advance(now)) {
                log(&format!("exact: {e}"));
            }
        }
        if vsync.arrived.get() && trace(c"exact needs frame", || p.needs_animation_frame()) {
            trace(c"exact tick", || p.tick(now));
        }
        // Decodes that finished since: only when the image fd woke the loop
        // (a frame's picture sync polls them too); every iteration polled
        // every picture's state twice a frame.
        if images_woke {
            trace(c"exact poll images", || p.poll_images());
        }
        let mut painted = false;
        if p.dirty() && vsync.arrived.get() {
            vsync.arrived.set(false);
            painted = true;
            let frame = trace(c"exact frame", || p.display_frame());
            if let Some(frame) = frame {
                trace(c"exact complete", || p.display_complete(&frame));
                frames.fetch_add(1, Ordering::Relaxed);
                if !first_pixel {
                    first_pixel = true;
                    first_frame.store(true, Ordering::Release);
                    log(&format!("exact: first frame at {:.1} ms", wall()));
                }
            }
        }
        // Rows the scroll this frame showed will need, mounted after it
        // (once this iteration's frame is out, or nothing waits to paint).
        // The next vsync is asked for first: work after a frame must not
        // make a request that misses the vsync the frame is waiting on.
        if painted || !p.dirty() {
            if painted && !vsync.requested.get() {
                vsync.requested.set(true);
                // SAFETY: as below — this thread's callback, `vsync` outlives it.
                unsafe {
                    AChoreographer_postFrameCallback64(
                        choreographer,
                        on_vsync,
                        vsync_ptr as *mut c_void,
                    )
                };
            }
            if trace(c"exact deferred collections", || {
                p.run_deferred_collections()
            }) {
                painted = true;
            }
        }
        if p.module_pending() {
            p.first_pixel();
        }
        // After a paint the next vsync is asked for whether or not anything
        // is dirty yet (as Choreographer keeps ticking through an
        // animation): input that lands just after it still makes it.
        let wants = painted || p.dirty() || p.needs_animation_frame() || p.host().wants_frames();
        if wants && !vsync.requested.get() {
            vsync.requested.set(true);
            // SAFETY: the callback runs on this thread inside pollOnce;
            // `vsync` outlives every callback posted (the loop drains before
            // returning, and the box lives to the end of `run`).
            unsafe {
                AChoreographer_postFrameCallback64(
                    choreographer,
                    on_vsync,
                    vsync_ptr as *mut c_void,
                )
            };
        }
        let timeout = match p.host().timer_due_ms() {
            Some(due) => ((due - wall()).max(0.0).ceil() as c_int).min(1000),
            None => 1000,
        };
        let timeout = if wants && vsync.arrived.get() {
            0
        } else {
            timeout
        };
        // SAFETY: polls this thread's looper; fds were added above.
        let ident = unsafe {
            ALooper_pollOnce(
                timeout,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        images_woke = ident == 3;
    }
    // Let a posted vsync callback run before `vsync` is dropped.
    if vsync.requested.get() {
        for _ in 0..4 {
            // SAFETY: as above.
            unsafe {
                ALooper_pollOnce(
                    20,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if !vsync.requested.get() {
                break;
            }
        }
    }
    drop(p);
}

//! A picture decoded straight into a GPU buffer (Android's `AHardwareBuffer`),
//! as the platform's image loaders decode to a hardware bitmap: the reader
//! wraps the same buffer, so a picture has one copy of its pixels, made once
//! on a decode thread, however often it leaves the screen and comes back.
//! Whoever reads the pixels in Rust (nothing on the Canvas host's paths)
//! gets a copy made on first use.
//!
//! @ref LLP 1076 §3.4 (the Rust painter on Android)
#![allow(unsafe_code)]
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tiny_skia::Pixmap;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether platform-decoded pictures go straight into GPU buffers: set by a
/// host whose reader draws them from there (the Canvas host).
pub fn hardware_pictures(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub(super) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

#[repr(C)]
struct Desc {
    width: u32,
    height: u32,
    layers: u32,
    format: u32,
    usage: u64,
    stride: u32,
    rfu0: u32,
    rfu1: u64,
}

#[link(name = "android")]
extern "C" {
    fn AHardwareBuffer_allocate(desc: *const Desc, out: *mut *mut c_void) -> i32;
    fn AHardwareBuffer_release(buffer: *mut c_void);
    fn AHardwareBuffer_describe(buffer: *const c_void, desc: *mut Desc);
    fn AHardwareBuffer_lock(
        buffer: *mut c_void,
        usage: u64,
        fence: i32,
        rect: *const c_void,
        out: *mut *mut c_void,
    ) -> i32;
    fn AHardwareBuffer_unlock(buffer: *mut c_void, fence: *mut i32) -> i32;
}

const CPU_READ_RARELY: u64 = 2;
const CPU_WRITE_RARELY: u64 = 2 << 4;
const GPU_SAMPLED_IMAGE: u64 = 1 << 8;
const R8G8B8A8_UNORM: u32 = 1;

/// One RGBA8 GPU buffer, written once.
pub struct Hardware {
    buffer: *mut c_void,
    size: (u32, u32),
    /// Bytes from one row to the next.
    stride: usize,
    read: OnceLock<Pixmap>,
}
// SAFETY: an AHardwareBuffer is reference counted and usable from any
// thread; after `fill` nothing writes it.
unsafe impl Send for Hardware {}
unsafe impl Sync for Hardware {}

impl Hardware {
    /// A buffer `w`×`h` filled by `fill(pixels, stride, bytes)`, which
    /// returns whether it wrote every row; `None` when the buffer could not
    /// be made or `fill` refused.
    pub(super) fn filled(
        w: u32,
        h: u32,
        fill: impl FnOnce(*mut c_void, usize, usize) -> bool,
    ) -> Option<Self> {
        let desc = Desc {
            width: w,
            height: h,
            layers: 1,
            format: R8G8B8A8_UNORM,
            usage: CPU_READ_RARELY | CPU_WRITE_RARELY | GPU_SAMPLED_IMAGE,
            stride: 0,
            rfu0: 0,
            rfu1: 0,
        };
        let mut buffer = std::ptr::null_mut();
        // SAFETY: `desc` and `buffer` outlive the calls; the buffer is
        // released by `Drop` on every path once `this` exists.
        unsafe {
            if AHardwareBuffer_allocate(&desc, &mut buffer) != 0 || buffer.is_null() {
                return None;
            }
            let mut got = Desc { stride: 0, ..desc };
            AHardwareBuffer_describe(buffer, &mut got);
            let this = Self {
                buffer,
                size: (w, h),
                stride: got.stride as usize * 4,
                read: OnceLock::new(),
            };
            let mut pixels = std::ptr::null_mut();
            if this.stride < w as usize * 4
                || AHardwareBuffer_lock(buffer, CPU_WRITE_RARELY, -1, std::ptr::null(), &mut pixels)
                    != 0
            {
                return None;
            }
            let ok = fill(pixels, this.stride, this.stride * h as usize);
            AHardwareBuffer_unlock(buffer, std::ptr::null_mut());
            ok.then_some(this)
        }
    }

    pub(super) fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The `AHardwareBuffer*`, alive while this is.
    pub(super) fn buffer(&self) -> *mut c_void {
        self.buffer
    }

    /// The pixels, copied out on first use (rows packed).
    pub(super) fn pixels(&self) -> &Pixmap {
        self.read.get_or_init(|| {
            let (w, h) = self.size;
            let mut out = Pixmap::new(w, h).expect("a decoded picture has a size");
            let row = w as usize * 4;
            let mut src = std::ptr::null_mut();
            // SAFETY: locked for reading, the buffer holds `h` rows `stride`
            // bytes apart, each at least `row` long.
            unsafe {
                if AHardwareBuffer_lock(
                    self.buffer,
                    CPU_READ_RARELY,
                    -1,
                    std::ptr::null(),
                    &mut src,
                ) == 0
                {
                    let src = src.cast::<u8>();
                    for (y, line) in out.data_mut().chunks_exact_mut(row).enumerate() {
                        std::ptr::copy_nonoverlapping(
                            src.add(y * self.stride),
                            line.as_mut_ptr(),
                            row,
                        );
                    }
                    AHardwareBuffer_unlock(self.buffer, std::ptr::null_mut());
                }
            }
            out
        })
    }
}

impl Drop for Hardware {
    fn drop(&mut self) {
        // SAFETY: the reference `filled` took.
        unsafe { AHardwareBuffer_release(self.buffer) };
    }
}

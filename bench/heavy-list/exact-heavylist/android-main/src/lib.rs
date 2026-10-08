//! Heavy List over main's public Android native-window painter.
#![deny(missing_docs)]
/// The same ahead-of-time Heavy List Contract used by the retained host.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
/// Its Android compatibility record.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

// Match the allocator installed by main's shipped Android Canvas JNI adapter.
// This reuses its allocator only; layout/painting still use the public Handle.
#[cfg(target_os = "android")]
#[global_allocator]
static EXACT_ALLOCATOR: exact_linux::canvas::jni::Allocator = exact_linux::canvas::jni::Allocator;

#[cfg(target_os = "android")]
mod android {
    use exact_linux::android::{self, Command, Handle, Launch};
    use std::{ffi::c_void, sync::atomic::Ordering};
    // C++ owns the opaque Handle pointer on its UI thread. All painter access
    // goes through the public command queue/counters; no private scene reader.
    #[unsafe(no_mangle)]
    pub extern "C" fn heavy_main_start(
        window: *mut c_void,
        w: u32,
        h: u32,
        scale: f32,
    ) -> *mut c_void {
        Box::into_raw(Box::new(android::start::<exact_heavylist_data::Heavy>(
            Launch {
                plan: super::PLAN,
                compat: super::COMPAT,
                window,
                size: (w, h),
                scale,
            },
        )))
        .cast()
    }
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn heavy_main_stop(raw: *mut c_void) {
        // SAFETY: JNI closes exactly once, after removing its live pointer.
        let handle = unsafe { Box::from_raw(raw.cast::<Handle>()) };
        handle.stop();
    }
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn heavy_main_ready(raw: *const c_void) -> bool {
        // SAFETY: JNI keeps the Handle live for this owner-thread query.
        unsafe { &*raw.cast::<Handle>() }
            .first_frame
            .load(Ordering::Acquire)
    }
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn heavy_main_frames(raw: *const c_void) -> u64 {
        // SAFETY: JNI keeps the Handle live for this owner-thread query.
        unsafe { &*raw.cast::<Handle>() }
            .frames
            .load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn heavy_main_scroll(raw: *const c_void, dy: f32) {
        // SAFETY: JNI keeps the Handle live; public Scroll takes physical pixels.
        unsafe { &*raw.cast::<Handle>() }.send(Command::Scroll(dy));
    }
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn heavy_main_touch(raw: *const c_void, action: i32, x: f32, y: f32) {
        // SAFETY: JNI keeps the Handle live; the render thread owns all painting.
        unsafe { &*raw.cast::<Handle>() }.send(Command::Touch(action, x, y));
    }
}

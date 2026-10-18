//! The plan and compatibility document the link carries beside the archive.
//!
//! A TypeScript app's Apple build bakes its plan outside Cargo and links it,
//! with its `compat.json`, into the executable's `__TEXT` segment
//! (`ld -sectcreate`, `host/apple/build.mjs`), so an edit to the Contract
//! alone links again and compiles no Rust. Both stay in the signed executable,
//! as they were in the archive; reading them is a pointer into the mapped
//! image (`getsectiondata(3)`), well under a microsecond.

/// The linked plan. Panics when the link carried none: an archive whose
/// entry reads its plan here is linked by `build.mjs`, which always adds it.
pub fn plan() -> &'static [u8] {
    section(c"__exact_plan")
        .expect("no __TEXT,__exact_plan: host/apple/build.mjs links this archive's plan")
}

/// The linked compatibility document (`compat.json`); panics as [`plan`] does.
pub fn compat() -> &'static str {
    static TEXT: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    TEXT.get_or_init(|| {
        let bytes = section(c"__exact_compat").expect(
            "no __TEXT,__exact_compat: host/apple/build.mjs links this archive's compat.json",
        );
        std::str::from_utf8(bytes).expect("the linked compat.json is UTF-8")
    })
}

/// The bytes of `__TEXT,<name>` in the image this code is linked into (the
/// app's executable, or the framework that embeds it).
#[cfg(target_vendor = "apple")]
#[allow(unsafe_code)]
fn section(name: &std::ffi::CStr) -> Option<&'static [u8]> {
    use std::ffi::{c_char, c_void};
    extern "C" {
        /// The linker's symbol for the Mach-O header of the image defining it.
        static __dso_handle: u8;
        fn getsectiondata(
            header: *const c_void,
            segment: *const c_char,
            section: *const c_char,
            size: *mut usize,
        ) -> *mut u8;
    }
    let mut size = 0usize;
    // SAFETY: `__dso_handle` is this image's mapped header, which lives as long
    // as the process; `getsectiondata` returns a pointer into that mapping and
    // its length, or null.
    unsafe {
        let header = std::ptr::addr_of!(__dso_handle).cast::<c_void>();
        let data = getsectiondata(header, c"__TEXT".as_ptr(), name.as_ptr(), &mut size);
        (!data.is_null()).then(|| std::slice::from_raw_parts(data, size))
    }
}

#[cfg(not(target_vendor = "apple"))]
fn section(_: &std::ffi::CStr) -> Option<&'static [u8]> {
    None
}

//! `backgroundMaterial`'s table for the Swift presenters (LLP 1053.000 D4):
//! the schema's row for a name, as the platform's own name for it.
// The C seam reads a caller's name and writes one static pointer.
#![allow(unsafe_code)]

/// This platform's name for material `name` (`platform` 0 iOS, 1 macOS):
/// its length, the bytes at `*out` (static), `~` first when the platform
/// draws the named one in its place. 0 when the schema has no such
/// material.
///
/// # Safety
/// `name` is valid for `len` bytes and `out` for one write.
pub unsafe fn platform(name: *const u8, len: usize, platform: u8, out: *mut *const u8) -> usize {
    if name.is_null() || out.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees `name` is valid for `len` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(name, len) };
    let Some(m) = std::str::from_utf8(bytes)
        .ok()
        .and_then(exact_kernel::generated::material)
    else {
        return 0;
    };
    let value = if platform == 0 { m.ios } else { m.macos };
    // SAFETY: the caller guarantees `out` is valid for one write.
    unsafe { *out = value.as_ptr() };
    value.len()
}

/// Export the material table from the application's static archive.
#[macro_export]
macro_rules! material_exports {
    () => {
        /// This platform's name for a `backgroundMaterial` (LLP 1053.000 D4).
        ///
        /// # Safety
        /// As [`$crate::material::platform`].
        #[no_mangle]
        pub unsafe extern "C" fn exact_material_platform(
            name: *const u8,
            len: usize,
            platform: u8,
            out: *mut *const u8,
        ) -> usize {
            // SAFETY: forwarded from this function's own contract.
            unsafe { $crate::material::platform(name, len, platform, out) }
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_name_is_its_platforms_and_an_unknown_one_is_none() {
        let mut out = std::ptr::null();
        let get = |name: &str, platform: u8, out: &mut *const u8| {
            // SAFETY: a live str and a live out pointer.
            let n = unsafe { super::platform(name.as_ptr(), name.len(), platform, out) };
            // SAFETY: `platform` wrote a static str of `n` bytes.
            (n > 0).then(|| {
                unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(*out, n)) }
                    .to_string()
            })
        };
        assert_eq!(
            get("thin", 0, &mut out).as_deref(),
            Some("systemThinMaterial")
        );
        assert_eq!(get("sidebar", 1, &mut out).as_deref(), Some("sidebar"));
        assert_eq!(
            get("sidebar", 0, &mut out).as_deref(),
            Some("~systemThinMaterial")
        );
        assert_eq!(get("sidebars", 0, &mut out), None);
    }
}

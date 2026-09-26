//! The local zone's offset, for the date (LLP 1027.000.000, LLP 1054 R12).
#![allow(unsafe_code)]

/// The local zone's current UTC offset in minutes east, from the C
/// library's zone database (`TZ`, else `/etc/localtime`); zero without one.
pub fn local_offset_minutes() -> f64 {
    // SAFETY: `time(NULL)` reads the clock; `localtime_r` writes only the
    // `tm` it is given and returns null on failure.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return 0.0;
        }
        tm.tm_gmtoff as f64 / 60.0
    }
}

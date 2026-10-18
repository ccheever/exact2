//! The presenter's Foundation formatter, called synchronously on the owner.
#![allow(unsafe_code)]
use exact_kernel::time::TimeRequest;
use std::ffi::c_void;

/// Borrowed UTF-8 fields, valid only for the synchronous formatter call.
#[repr(C)]
pub struct CTimeRequest {
    /// Instant in Unix milliseconds.
    pub datetime: f64,
    /// Runner date in Unix milliseconds.
    pub now: f64,
    /// Minutes east of UTC.
    pub utc_offset: f64,
    /// Presentation name, UTF-8.
    pub format: *const u8,
    /// Presentation byte count.
    pub format_len: usize,
    /// BCP 47 locale, UTF-8.
    pub locale: *const u8,
    /// Locale byte count.
    pub locale_len: usize,
    /// IANA zone, UTF-8.
    pub time_zone: *const u8,
    /// Zone byte count.
    pub time_zone_len: usize,
}

/// Returns the UTF-8 size, writing at most the provided capacity.
pub type TimeFormatFn = extern "C" fn(*mut c_void, *const CTimeRequest, *mut u8, usize) -> usize;

/// Copy the synchronous platform answer into owned, validated UTF-8.
pub fn format(f: TimeFormatFn, ctx: *mut c_void, r: &TimeRequest<'_>) -> Option<String> {
    let q = CTimeRequest {
        datetime: r.datetime,
        now: r.now,
        utc_offset: r.utc_offset,
        format: r.format.as_ptr(),
        format_len: r.format.len(),
        locale: r.locale.as_ptr(),
        locale_len: r.locale.len(),
        time_zone: r.time_zone.as_ptr(),
        time_zone_len: r.time_zone.len(),
    };
    let mut bytes = vec![0; 256];
    let mut n = f(ctx, &q, bytes.as_mut_ptr(), bytes.len());
    if n > bytes.len() && n <= 16_384 {
        bytes.resize(n, 0);
        n = f(ctx, &q, bytes.as_mut_ptr(), bytes.len());
    }
    if n == 0 || n > bytes.len() {
        return None;
    }
    bytes.truncate(n);
    String::from_utf8(bytes).ok()
}

pub(crate) fn padding_props(
    node: &exact_kernel::NodeRef<'_>,
    out: &mut exact_kernel::SortedMap<String, String>,
) {
    if let Some(pad) = node.time_padding() {
        out.insert("timePaddingText".into(), pad.text.into_owned());
        out.insert(
            "timePaddingSpacing".into(),
            pad.style.letter_spacing.to_string(),
        );
    }
}

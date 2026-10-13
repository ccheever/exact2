//! A measurement log of each exchange's connection (exact2 LLP 1120 §5):
//! when `IBEX2_HTTP_METRICS` names a file, every transport that reports
//! metrics appends one JSON line per exchange to it, the old blocking
//! sessions and the asynchronous lanes alike, so the arms compare like for
//! like. Off (no environment variable) it costs one lookup per exchange.

use std::io::Write;

/// Append one exchange's connection to the log, if one is named.
pub fn note(
    transport: &str,
    url: &str,
    protocol: Option<&str>,
    reused: Option<bool>,
    local: Option<&str>,
    remote: Option<&str>,
    queued_ms: Option<f64>,
) {
    let Some(path) = std::env::var_os("IBEX2_HTTP_METRICS") else {
        return;
    };
    let text = |v: Option<&str>| v.map_or("null".to_string(), |v| format!("{v:?}"));
    let line = format!(
        "{{\"t\":{},\"transport\":{transport:?},\"url\":{url:?},\"protocol\":{},\"reused\":{},\"local\":{},\"remote\":{},\"queued_ms\":{}}}\n",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis()),
        text(protocol),
        reused.map_or("null".to_string(), |r| r.to_string()),
        text(local),
        text(remote),
        queued_ms.map_or("null".to_string(), |q| format!("{q:.1}")),
    );
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// The blocking session's report (`darwin_http.mm`).
#[cfg(target_vendor = "apple")]
#[no_mangle]
extern "C" fn ibex2_metrics_note(
    url: *const std::ffi::c_char,
    protocol: *const std::ffi::c_char,
    reused: std::ffi::c_int,
    local: *const std::ffi::c_char,
    remote: *const std::ffi::c_char,
) {
    // SAFETY: NUL-terminated strings or null, owned by the caller for this call.
    let text = |p: *const std::ffi::c_char| {
        (!p.is_null()).then(|| {
            unsafe { std::ffi::CStr::from_ptr(p) }
                .to_string_lossy()
                .into_owned()
        })
    };
    note(
        "session",
        &text(url).unwrap_or_default(),
        text(protocol).as_deref(),
        (reused >= 0).then_some(reused == 1),
        text(local).as_deref(),
        text(remote).as_deref(),
        None,
    );
}

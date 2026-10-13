//! A measurement log of each exchange's connection (exact2 LLP 1120 §5):
//! when `IBEX2_HTTP_METRICS` names a file, every transport that reports
//! metrics appends one JSON line per exchange to it, the old blocking
//! sessions and the asynchronous lanes alike, so the arms compare like for
//! like. Off (no environment variable) it costs one cached check per
//! exchange; the variable is read once, at the first.

use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// The log's file, as the process's environment named it at the first
/// exchange; `None` when it names none. Records are written whole under it.
fn file() -> Option<&'static Mutex<std::fs::File>> {
    static FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = std::env::var_os("IBEX2_HTTP_METRICS")?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        Some(Mutex::new(file))
    })
    .as_ref()
}

/// Whether a log is named: a transport checks before it prepares a record.
pub fn enabled() -> bool {
    file().is_some()
}

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
    let Some(file) = file() else {
        return;
    };
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let record = serde_json::json!({
        "t": t,
        "transport": transport,
        "url": url,
        "protocol": protocol,
        "reused": reused,
        "local": local,
        "remote": remote,
        "queued_ms": queued_ms,
    });
    let line = format!("{record}\n");
    if let Ok(mut file) = file.lock() {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Whether the blocking session should prepare a record (`darwin_http.mm`).
#[cfg(target_vendor = "apple")]
#[no_mangle]
extern "C" fn ibex2_metrics_enabled() -> std::ffi::c_int {
    std::ffi::c_int::from(enabled())
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

//! Exact's optional asynchronous HTTP connection log (LLP 1120 §5).
//! `IBEX2_HTTP_METRICS` selects the file once, at the first exchange.
//! Blocking/native comparison is an explicit upstream diagnostic; Ibex
//! itself installs no process-wide logging policy.

use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// The log's file, as the process's environment named it at the first
/// exchange; `None` when it names none. Records are written whole under it.
fn file() -> Option<&'static Mutex<std::fs::File>> {
    static FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = std::env::var_os("IBEX2_HTTP_METRICS")?;
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        // The log holds URLs: the owner's to read only.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        match options.open(&path) {
            Ok(file) => Some(Mutex::new(file)),
            Err(e) => {
                eprintln!(
                    "IBEX2_HTTP_METRICS {}: {e}; not logging",
                    path.to_string_lossy()
                );
                None
            }
        }
    })
    .as_ref()
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

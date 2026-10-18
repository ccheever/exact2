//! The terminal uses the machine's wall clock and zone, as other running hosts do.
#![allow(unsafe_code)]
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn launch() -> (f64, f64, String, String) {
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;
    let zone = iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into());
    let locale = ["LC_ALL", "LC_TIME", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "en-US".into())
        .split('.')
        .next()
        .unwrap_or("en-US")
        .replace('_', "-");
    let locale = if matches!(locale.as_str(), "C" | "POSIX") {
        "en-US".into()
    } else {
        locale
    };
    (epoch, offset(epoch), locale, zone)
}

#[cfg(unix)]
fn offset(epoch: f64) -> f64 {
    // SAFETY: localtime_r writes only its output; the timestamp is in time_t's range.
    unsafe {
        let now = (epoch / 1000.) as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            0.
        } else {
            tm.tm_gmtoff as f64 / 60.
        }
    }
}
#[cfg(not(unix))]
fn offset(_: f64) -> f64 {
    0.
}

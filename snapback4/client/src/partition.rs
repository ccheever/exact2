//! A partition belongs to one app, origin and viewer, on every host.

use serde_json::{json, Value as Json};
use snapback4_device::{device::Device, store::DeviceStore};

pub const PARTITION: &str = "exact2:partition";
/// The device's durable random identity: write ids derive from it.
pub const DEVICE: &str = "exact:device";

/// The identity a partition is bound to.
pub fn identity(app_id: &str, origin: &str, viewer: &str) -> String {
    json!({"appId": app_id, "origin": origin, "viewer": viewer}).to_string()
}

/// Bind a just-opened device to `identity`, or refuse one bound to another;
/// give it a durable random identity (`exact:device`) from `entropy` once.
pub fn bind<S: DeviceStore>(
    device: &mut Device<S>,
    identity: &str,
    existed: bool,
    entropy: impl FnOnce() -> Result<String, String>,
) -> Result<(), String> {
    match device.meta(PARTITION).map_err(|e| e.to_string())? {
        Some(kept) if kept != identity => {
            return Err("Snapback4 partition belongs to another app, origin, or viewer".into());
        }
        None if existed => return Err("Snapback4 database has no app partition identity".into()),
        None => device
            .set_meta(PARTITION, identity)
            .map_err(|e| e.to_string())?,
        Some(_) => {}
    }
    if device
        .meta("exact:device")
        .map_err(|e| e.to_string())?
        .is_none()
    {
        device
            .set_meta("exact:device", &entropy()?)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// What a host checks before a device call: queries, predictions and every
/// outbox entry name the opened viewer, and the partition's identity and the
/// device's are immutable.
pub fn guard(request: &Json, viewer: &str) -> Result<(), String> {
    let op = request["op"].as_str().unwrap_or_default();
    if matches!(op, "query" | "predict") && request["viewer"].as_str() != Some(viewer) {
        return Err("Snapback4 viewer differs from the opened partition".into());
    }
    if op == "set_meta" && matches!(request["key"].as_str(), Some(PARTITION | DEVICE)) {
        return Err("Snapback4 partition and device identities are immutable".into());
    }
    if matches!(op, "admit" | "enqueue") && request["entry"]["viewer"].as_str() != Some(viewer) {
        return Err("Snapback4 viewer differs from the opened partition".into());
    }
    Ok(())
}

/// A filename an app may use: `app:/data/<name>.sqlite`, the name ASCII
/// letters, digits, `.`, `_`, `-` and a literal `%` (never decoded).
pub fn valid_path(path: &str) -> bool {
    path.strip_prefix("app:/data/").is_some_and(|leaf| {
        leaf.ends_with(".sqlite")
            && leaf.len() > ".sqlite".len()
            && !leaf.starts_with('.')
            && leaf
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-%".contains(&b))
    })
}

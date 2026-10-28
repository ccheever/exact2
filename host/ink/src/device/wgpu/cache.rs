use std::path::{Path, PathBuf};

pub(super) fn path(info: &wgpu::AdapterInfo, shader: &str) -> Option<PathBuf> {
    let key = wgpu::util::pipeline_cache_key(info)?;
    let dir = std::env::var_os("EXACT_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|d| PathBuf::from(d).join("exact")))
        .or_else(|| std::env::var_os("HOME").map(|d| PathBuf::from(d).join(".cache/exact")))?;
    // FNV-1a identifies the source and device; wgpu's cache header validates
    // the payload. A layout change increments the prefix alongside the shader.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in shader
        .bytes()
        .chain(info.driver.bytes())
        .chain(info.driver_info.bytes())
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    let safe: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    Some(dir.join(format!("pipelines-ink-v1-{safe}-{hash:016x}.bin")))
}

pub(super) fn save(path: &Path, bytes: &[u8], old: Option<&[u8]>) {
    if old == Some(bytes) {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Concurrent opens in one process must not rename each other's temporary.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = path.with_extension(format!("{}-{serial}.tmp", std::process::id()));
    if std::fs::write(&temp, bytes).is_ok() && std::fs::rename(&temp, path).is_err() {
        let _ = std::fs::remove_file(temp);
    }
}

use std::path::PathBuf;

/// The raw or signature-normalized digest of `path`, cached separately while the
/// file stays the same one (inode, length, modification time): the host
/// opens a module that its boot already verified on another thread
/// (`prepare_gpu`), and a module is megabytes.
pub(super) fn file(path: &std::path::Path, code: bool) -> Result<String, String> {
    use std::sync::Mutex;
    type Stamp = (u64, u64, Option<std::time::SystemTime>);
    type Entry = (PathBuf, Stamp, bool, String);
    static DIGESTS: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
    let stamp = |m: &std::fs::Metadata| -> Stamp {
        #[cfg(unix)]
        let inode = std::os::unix::fs::MetadataExt::ino(m);
        #[cfg(not(unix))]
        let inode = 0;
        (inode, m.len(), m.modified().ok())
    };
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let before = stamp(&file.metadata().map_err(|e| e.to_string())?);
    let known = DIGESTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((.., digest)) = known
        .iter()
        .find(|(p, s, c, _)| p == path && *s == before && *c == code)
    {
        return Ok(digest.clone());
    }
    drop(known);
    #[cfg(target_os = "android")]
    let stored = digest_store(path, &before, code);
    #[cfg(target_os = "android")]
    if let Some(digest) = stored.as_ref().and_then(|s| s.found.clone()) {
        let mut known = DIGESTS.lock().unwrap_or_else(|e| e.into_inner());
        known.retain(|(p, _, c, _)| p != path || *c != code);
        known.push((path.to_path_buf(), before, code, digest.clone()));
        return Ok(digest);
    }
    let mut bytes = Vec::with_capacity(before.1 as usize);
    std::io::Read::read_to_end(&mut &file, &mut bytes).map_err(|e| e.to_string())?;
    let digest = bytes_digest(&bytes, code);
    // Remembered only when nothing changed the file while it was read.
    if stamp(&file.metadata().map_err(|e| e.to_string())?) == before && before.2.is_some() {
        let mut known = DIGESTS.lock().unwrap_or_else(|e| e.into_inner());
        known.retain(|(p, _, c, _)| p != path || *c != code);
        known.push((path.to_path_buf(), before, code, digest.clone()));
        #[cfg(target_os = "android")]
        if let Some(store) = stored {
            store.save(&digest);
        }
    }
    Ok(digest)
}

/// On Android, a digest [`file`] computed is kept across launches
/// beside the font cache (`exact/digests-<path hash>` under
/// `$XDG_CACHE_HOME` or `$HOME/.cache`), for the same file (path, inode,
/// length, modification time): a module ships in the APK's library
/// directory, which only an install replaces, and hashing its megabytes was
/// ~35 ms of every cold start before the GPU device could be made.
#[cfg(target_os = "android")]
struct DigestStore {
    file: PathBuf,
    key: String,
    found: Option<String>,
}

#[cfg(target_os = "android")]
fn digest_store(
    path: &std::path::Path,
    stamp: &(u64, u64, Option<std::time::SystemTime>),
    code: bool,
) -> Option<DigestStore> {
    use std::hash::{Hash, Hasher};
    let modified = stamp
        .2?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    code.hash(&mut h);
    let file = base
        .join("exact")
        .join(format!("digests-{:016x}", h.finish()));
    let key = format!(
        "{} {} {} {modified} {code}",
        path.display(),
        stamp.0,
        stamp.1
    );
    let found = std::fs::read_to_string(&file).ok().and_then(|text| {
        let (k, digest) = text.trim_end().rsplit_once(' ')?;
        (k == key && digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| digest.to_owned())
    });
    Some(DigestStore { file, key, found })
}

#[cfg(target_os = "android")]
impl DigestStore {
    fn save(&self, digest: &str) {
        if let Some(parent) = self.file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let partial = self.file.with_extension("partial");
        if std::fs::write(&partial, format!("{} {digest}\n", self.key)).is_ok() {
            let _ = std::fs::rename(&partial, &self.file);
        }
    }
}

// Completed development receipts bind every byte. A Mac can run either
// receipt policy through this headless host.
pub(super) fn bytes_digest(bytes: &[u8], code: bool) -> String {
    if code {
        exact_update::gpu_code_digest(bytes)
    } else {
        exact_update::sha256_hex(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn signed(signature: &[u8], code: u8) -> Vec<u8> {
        let mut bytes = vec![0; 160];
        bytes[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
        bytes[16..20].copy_from_slice(&2u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&0x19u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&72u32.to_le_bytes());
        bytes[40..50].copy_from_slice(b"__LINKEDIT");
        bytes[64..72].copy_from_slice(&(signature.len() as u64).to_le_bytes());
        bytes[80..88].copy_from_slice(&(signature.len() as u64 + 8).to_le_bytes());
        bytes[104..108].copy_from_slice(&0x1du32.to_le_bytes());
        bytes[108..112].copy_from_slice(&16u32.to_le_bytes());
        bytes[112..116].copy_from_slice(&160u32.to_le_bytes());
        bytes[116..120].copy_from_slice(&(signature.len() as u32).to_le_bytes());
        bytes[150] = code;
        bytes.extend_from_slice(signature);
        bytes
    }

    #[test]
    fn baked_code_and_completed_receipts_verify_their_own_digest() {
        let original = signed(b"ad-hoc", 1);
        let mut expected = original[..160].to_vec();
        for (at, len) in [(64, 8), (80, 8), (116, 4)] {
            expected[at..at + len].fill(0);
        }
        let dir = std::env::temp_dir().join(format!("gpu-code-digest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("module");
        let mut compat = json!({"id":"cohort", "inputs":{"app":"app"},
            "embedded":{"gpu":{"app":"app", "cohort":"cohort",
            "trust":"production", "sha256":bytes_digest(&expected, false)}}});
        std::fs::write(&path, &original).unwrap();
        super::super::verify_module(&path, &compat, "").unwrap();
        std::fs::write(&path, signed(b"longer recipient signature", 1)).unwrap();
        super::super::verify_module(&path, &compat, "").unwrap();
        std::fs::write(&path, signed(b"ad-hoc", 2)).unwrap();
        assert!(super::super::verify_module(&path, &compat, "")
            .unwrap_err()
            .contains("digest mismatch"));

        std::fs::write(&path, &original).unwrap();
        compat["embedded"]["gpu"]["trust"] = json!("development");
        compat["embedded"]["gpu"]["receipt"] = json!(true);
        std::fs::write(
            path.with_extension("proof.json"),
            json!({"inputs":"0".repeat(64), "artifact":bytes_digest(&original, false)}).to_string(),
        )
        .unwrap();
        super::super::verify_module(&path, &compat, "").unwrap();
        std::fs::write(&path, signed(b"longer recipient signature", 1)).unwrap();
        assert!(super::super::verify_module(&path, &compat, "")
            .unwrap_err()
            .contains("digest mismatch"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

use std::path::PathBuf;

/// Proof receipts hash the whole file; baked cards hash its signed code.
/// Cache each identity while inode, length and modification time stay unchanged:
/// boot and canvas creation can verify the same megabytes on different threads.
pub(super) fn file_digest(path: &std::path::Path, raw: bool) -> Result<String, String> {
    use std::sync::Mutex;
    type Stamp = (u64, u64, Option<std::time::SystemTime>);
    type Entry = ((PathBuf, bool), Stamp, String);
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
        .find(|(p, s, _)| p.0 == path && p.1 == raw && *s == before)
    {
        return Ok(digest.clone());
    }
    drop(known);
    #[cfg(target_os = "android")]
    let stored = digest_store(path, &before, raw);
    #[cfg(target_os = "android")]
    if let Some(digest) = stored.as_ref().and_then(|s| s.found.clone()) {
        let mut known = DIGESTS.lock().unwrap_or_else(|e| e.into_inner());
        known.retain(|(p, ..)| p.0 != path || p.1 != raw);
        known.push(((path.to_path_buf(), raw), before, digest.clone()));
        return Ok(digest);
    }
    let mut bytes = Vec::with_capacity(before.1 as usize);
    std::io::Read::read_to_end(&mut &file, &mut bytes).map_err(|e| e.to_string())?;
    let digest = if raw {
        hash(&bytes)
    } else {
        code_digest(&bytes)
    };
    // Remembered only when nothing changed the file while it was read.
    if stamp(&file.metadata().map_err(|e| e.to_string())?) == before && before.2.is_some() {
        let mut known = DIGESTS.lock().unwrap_or_else(|e| e.into_inner());
        known.retain(|(p, ..)| p.0 != path || p.1 != raw);
        known.push(((path.to_path_buf(), raw), before, digest.clone()));
        #[cfg(target_os = "android")]
        if let Some(store) = stored {
            store.save(&digest);
        }
    }
    Ok(digest)
}

/// On Android, a digest [`file_digest`] computed is kept across launches
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
    raw: bool,
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
    let file = base
        .join("exact")
        .join(format!("digests-{:016x}", h.finish()));
    let key = format!(
        "{} {} {} {modified} {raw}",
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

/// A GPU product's digest: its exact bytes, except that a signed 64-bit
/// Mach-O's code signature is left out, with the three sizes that follow it
/// (the signature command's `datasize`, `__LINKEDIT`'s `vmsize` and
/// `filesize`) read as zero. A re-signer (AppDrop signs every .ipa again
/// for each recipient) keeps the digest; any change to the code it signs
/// does not. Anything else is hashed whole. This matches bake/src/receipt.rs and GpuModule.swift.
fn code_digest(bytes: &[u8]) -> String {
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
    };
    let macho = || -> Option<Vec<u8>> {
        if word(0)? != 0xfeed_facf {
            return None;
        }
        let (mut at, mut signature, mut zero) = (32, None, Vec::new());
        for _ in 0..word(16)? {
            let (cmd, size) = (word(at)?, word(at + 4)?);
            if cmd == 0x1d {
                signature = Some(word(at + 8)?);
                zero.push((at + 12, 4));
            } else if cmd == 0x19 && bytes.get(at + 8..at + 24)? == b"__LINKEDIT\0\0\0\0\0\0" {
                zero.extend([(at + 32, 8), (at + 48, 8)]);
            }
            at += size.max(8);
        }
        let mut code = bytes.get(..signature?)?.to_vec();
        for (at, len) in zero {
            code.get_mut(at..at + len)?.fill(0);
        }
        Some(code)
    };
    hash(&macho().unwrap_or_else(|| bytes.to_vec()))
}

fn hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn baked_code_and_completed_receipts_verify_their_own_digest() {
        let signed = |signature: &[u8], code: u8| {
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
        };
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
            "trust":"production", "sha256":hash(&expected)}}});
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
            json!({"inputs":"0".repeat(64), "artifact":hash(&original)}).to_string(),
        )
        .unwrap();
        super::super::verify_module(&path, &compat, "").unwrap();
        std::fs::write(&path, signed(b"longer recipient signature", 1)).unwrap();
        assert!(super::super::verify_module(&path, &compat, "")
            .unwrap_err()
            .contains("digest mismatch"));
        assert_eq!(code_digest(b"ELF or PE"), hash(b"ELF or PE"));
        assert_eq!(code_digest(&original[..100]), hash(&original[..100]));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

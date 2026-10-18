use std::path::PathBuf;

/// The SHA-256 of the file at `path`, hashed once per process while the
/// file stays the same one (inode, length, modification time): the host
/// opens a module that its boot already verified on another thread
/// (`prepare_gpu`), and a module is megabytes.
pub(super) fn file(path: &std::path::Path, code: bool) -> Result<String, String> {
    use std::sync::Mutex;
    type Stamp = (u64, u64, Option<std::time::SystemTime>);
    static DIGESTS: Mutex<Vec<(PathBuf, Stamp, bool, String)>> = Mutex::new(Vec::new());
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
        known.retain(|(p, ..)| p != path);
        known.push((path.to_path_buf(), before, code, digest.clone()));
        return Ok(digest);
    }
    let mut bytes = Vec::with_capacity(before.1 as usize);
    std::io::Read::read_to_end(&mut &file, &mut bytes).map_err(|e| e.to_string())?;
    let digest = bytes_digest(&bytes, code);
    // Remembered only when nothing changed the file while it was read.
    if stamp(&file.metadata().map_err(|e| e.to_string())?) == before && before.2.is_some() {
        let mut known = DIGESTS.lock().unwrap_or_else(|e| e.into_inner());
        known.retain(|(p, ..)| p != path);
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

// Baked cards use bake/src/receipt.rs `code_digest`; completed development
// receipts bind every byte. A Mac can run either through this headless host.
pub(super) fn bytes_digest(bytes: &[u8], code: bool) -> String {
    use sha2::Digest;
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
    };
    let macho = || -> Option<Vec<u8>> {
        if !code || word(0)? != 0xfeed_facf {
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
    format!(
        "{:x}",
        sha2::Sha256::digest(macho().as_deref().unwrap_or(bytes))
    )
}

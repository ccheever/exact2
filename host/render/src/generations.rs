//! The builds of `app.wasm` this server has served, for the browsers that
//! still hold one (Compression Dictionary Transport; LLP 1047.000 §9).
//!
//! The web build names `app.wasm` by its content (`/app.wasm?v=<digest>`), so
//! a browser caches it for good and may keep it as a dictionary. With
//! `--generations <dir>` the server keeps each build under its SHA-256, the
//! newest [`KEEP`], and a browser that asks for this build naming an earlier
//! one it holds gets this build as `dcb` against it: a returning reader
//! downloads what a deploy changed, not the artifact. Each delta is made once,
//! off every request's path, at brotli's best; until it is made, the build
//! goes as it otherwise would.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

/// Builds kept, the one served included.
pub(crate) const KEEP: usize = 4;

pub(crate) struct Generations {
    /// The build served now, by its ETag: a delta is only ever this build.
    pub(crate) tag: String,
    /// Each earlier build's delta to this one, by that build's SHA-256 in
    /// base64 (the name `Available-Dictionary` gives it): none when it didn't
    /// shrink or doesn't fit brotli's window.
    deltas: HashMap<String, Arc<OnceLock<Option<Vec<u8>>>>>,
}

impl Generations {
    /// Keep `wasm`, the build served now, in `dir` beside the newest earlier
    /// ones, drop the rest, and start making the deltas.
    pub(crate) fn open(dir: &Path, wasm: &[u8]) -> std::io::Result<Generations> {
        use sha2::{Digest, Sha256};
        std::fs::create_dir_all(dir)?;
        let current = dir.join(format!("{:x}.wasm", Sha256::digest(wasm)));
        // Written again, so it is the newest.
        std::fs::write(&current, wasm)?;
        let mut kept: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let hex = path.file_name()?.to_str()?.strip_suffix(".wasm")?;
                if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return None;
                }
                Some((entry.metadata().ok()?.modified().ok()?, path))
            })
            .filter(|(_, path)| *path != current)
            .collect();
        kept.sort_by_key(|k| std::cmp::Reverse(k.0));
        let mut deltas = HashMap::new();
        for (i, (_, path)) in kept.into_iter().enumerate() {
            if i + 1 >= KEEP {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            let Ok(earlier) = std::fs::read(&path) else {
                continue;
            };
            let hash: [u8; 32] = Sha256::digest(&earlier).into();
            let made = Arc::new(OnceLock::new());
            let (slot, build) = (Arc::clone(&made), wasm.to_vec());
            std::thread::Builder::new()
                .name("exact-render-generation".into())
                .spawn(move || slot.set(crate::encode::against_at(&build, &earlier, &hash, 11)))?;
            deltas.insert(exact_data::envelope::base64(&hash), made);
        }
        Ok(Generations {
            tag: crate::encode::tag(wasm),
            deltas,
        })
    }

    /// This build against the earlier one `hash` names, once it is made.
    pub(crate) fn delta(&self, hash: &str) -> Option<&[u8]> {
        self.deltas.get(hash)?.get()?.as_deref()
    }

    /// Whether every delta is made (a test's wait).
    #[cfg(test)]
    pub(crate) fn made(&self) -> bool {
        self.deltas.values().all(|made| made.get().is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::time::Duration;

    #[test]
    fn the_newest_builds_are_kept_and_the_rest_dropped() {
        let dir = std::env::temp_dir().join(format!("exact-generations-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let build = |n: u8| vec![n; 4096];
        let file = |n: u8| dir.join(format!("{:x}.wasm", Sha256::digest(build(n))));
        // Five earlier builds, the first oldest, and a file that isn't one.
        let now = SystemTime::now();
        for n in 1..=5u8 {
            std::fs::write(file(n), build(n)).unwrap();
            let at = now - Duration::from_secs(60 * u64::from(6 - n));
            std::fs::File::options()
                .write(true)
                .open(file(n))
                .unwrap()
                .set_modified(at)
                .unwrap();
        }
        std::fs::write(dir.join("notes.wasm"), b"not a build").unwrap();
        let generations = Generations::open(&dir, &build(6)).unwrap();
        let kept: Vec<bool> = (1..=6).map(|n| file(n).is_file()).collect();
        assert_eq!(kept, [false, false, true, true, true, true]);
        assert!(dir.join("notes.wasm").is_file());
        while !generations.made() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let name = |n: u8| exact_data::envelope::base64(&Sha256::digest(build(n)));
        assert!(generations.delta(&name(5)).is_some());
        assert!(generations.delta(&name(3)).is_some());
        assert!(generations.delta(&name(2)).is_none());
        assert_eq!(generations.tag, crate::encode::tag(&build(6)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

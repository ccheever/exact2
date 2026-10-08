//! `fs.compressImage` (Exact patch 9; exact2 LLP 1069.002 Amendment A1):
//! read an app file, hand its bytes to the embedder's image codec, and write
//! the JPEG it returns, atomically, under the same grants and app-directory
//! handles every other `fs` operation uses. Ibex owns the paths; the codec
//! owns the pixels and never sees a path.
use super::app_fs::AppDirectories;
use super::fs::{FsOp, FsResult};
use crate::boundary::HostError;
use crate::grant::{GrantSet, Operation};
use std::time::{Duration, Instant};

/// What a codec made: JPEG bytes and their pixel size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The embedder's codec: `(source bytes, maxDimension, maxBytes, deadline)`.
/// It starts no decode or trial at or after `deadline`. Its error is the
/// guest's message as it is (`compressImage: <code>: <detail>`).
pub type ImageCodec =
    dyn Fn(&[u8], u32, u64, Instant) -> Result<CompressedImage, String> + Send + Sync;

/// How long after the guest issued the call the codec may start work.
pub const TRIAL_BUDGET: Duration = Duration::from_secs(20);
/// How long after the guest issued the call the result may still be
/// written. An embedder that waits on the call gives up no sooner than
/// [`EMBEDDER_WAIT`] after issue, so nothing is written after the guest
/// could have been told the call failed (exact2 `js/src/storage.rs`).
pub const COMMIT_BUDGET: Duration = Duration::from_secs(24);
/// The shortest wait an embedder may put on a storage call.
pub const EMBEDDER_WAIT: Duration = Duration::from_secs(30);

/// What was written at the destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFile {
    pub width: u32,
    pub height: u32,
    pub size: u64,
}

/// The largest source read (exact2 `exact_raster::MAX_ENCODED_BYTES`).
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

fn failed(code: &str, detail: impl std::fmt::Display) -> HostError {
    HostError::Failed(format!("compressImage: {code}: {detail}"))
}

/// Admit `fs.read` on `from` and `fs.write` on `to` before anything is read,
/// check the source's size, run `codec`, and replace `to` with its JPEG. On
/// any failure `to` is as it was. `issued` is when the guest made the call:
/// the codec starts nothing after [`TRIAL_BUDGET`] from it, and nothing is
/// written after [`COMMIT_BUDGET`] (`timeout`).
#[allow(clippy::too_many_arguments)]
pub fn compress_image(
    grants: &GrantSet,
    directories: Option<&AppDirectories>,
    codec: Option<&ImageCodec>,
    from: &str,
    to: &str,
    max_dimension: u32,
    max_bytes: u64,
    issued: Instant,
) -> Result<ImageFile, HostError> {
    if !from.starts_with("app:/") || !to.starts_with("app:/") {
        return Err(HostError::Failed("compressImage: needs app:/ paths".into()));
    }
    let directories = directories
        .ok_or_else(|| HostError::Failed("app directories are not configured".into()))?;
    // The write is admitted lexically now, so a denial is reported before
    // seconds of work; the write below admits it again, after parsing.
    crate::boundary::admit(grants, &Operation::FsWrite { path: to.into() })?;
    // `stat` admits the read.
    match directories.run(grants, FsOp::Stat, from, None, None)? {
        FsResult::Stat(stat) if stat.is_file && stat.size > MAX_SOURCE_BYTES => {
            return Err(failed(
                "too-large",
                format!("{} bytes is over {MAX_SOURCE_BYTES}", stat.size),
            ));
        }
        _ => {}
    }
    let codec = codec.ok_or_else(|| failed("unsupported", "no JPEG encoder on this host"))?;
    let FsResult::Bytes(source) = directories.run(grants, FsOp::ReadFile, from, None, None)? else {
        return Err(HostError::Failed(
            "compressImage: the read returned no bytes".into(),
        ));
    };
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err(failed(
            "too-large",
            format!("{} bytes is over {MAX_SOURCE_BYTES}", source.len()),
        ));
    }
    let image = codec(&source, max_dimension, max_bytes, issued + TRIAL_BUDGET)
        .map_err(HostError::Failed)?;
    drop(source);
    if image.bytes.len() as u64 > max_bytes {
        return Err(failed(
            "unfit",
            format!(
                "the codec returned {} bytes for {max_bytes}",
                image.bytes.len()
            ),
        ));
    }
    // The last moment a write may begin: after it, the guest may already
    // have been told the call failed, and later storage may have run.
    if issued.elapsed() >= COMMIT_BUDGET {
        return Err(failed(
            "timeout",
            format!("the result was ready after {} s", COMMIT_BUDGET.as_secs()),
        ));
    }
    directories.run(grants, FsOp::AtomicWriteFile, to, None, Some(&image.bytes))?;
    Ok(ImageFile {
        width: image.width,
        height: image.height,
        size: image.bytes.len() as u64,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct Fixture {
        path: PathBuf,
        dirs: AppDirectories,
    }
    impl Fixture {
        fn new() -> Self {
            let id = crate::stdlib::crypto::random_uuid().unwrap();
            let path = std::env::temp_dir().join(format!("ibex-compress-{id}"));
            for root in ["data", "cache", "tmp"] {
                std::fs::create_dir_all(path.join(root)).unwrap();
            }
            let dirs = AppDirectories::new(path.join("data"), path.join("cache"), path.join("tmp"))
                .unwrap();
            Self { path, dirs }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn grants(text: &str) -> GrantSet {
        GrantSet::parse(text).unwrap()
    }

    /// Reverses the bytes and reports a 3 × 2 image; counts its calls.
    fn reverse(
        calls: &Arc<AtomicUsize>,
    ) -> impl Fn(&[u8], u32, u64, Instant) -> Result<CompressedImage, String> {
        let calls = calls.clone();
        move |bytes, _, _, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(CompressedImage {
                bytes: bytes.iter().rev().copied().collect(),
                width: 3,
                height: 2,
            })
        }
    }

    #[test]
    fn the_codecs_bytes_replace_the_destination() {
        let f = Fixture::new();
        std::fs::write(f.path.join("tmp/in.png"), b"abc").unwrap();
        std::fs::write(f.path.join("data/out.jpg"), b"old").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let codec = reverse(&calls);
        let all = grants("fs.read app:/tmp\nfs.write app:/data");
        let file = compress_image(
            &all,
            Some(&f.dirs),
            Some(&codec),
            "app:/tmp/in.png",
            "app:/data/out.jpg",
            4000,
            10,
            Instant::now(),
        )
        .unwrap();
        assert_eq!(
            file,
            ImageFile {
                width: 3,
                height: 2,
                size: 3
            }
        );
        assert_eq!(std::fs::read(f.path.join("data/out.jpg")).unwrap(), b"cba");
        // In place: the source is read in full before the write.
        let same = grants("fs.read app:/tmp\nfs.write app:/tmp");
        compress_image(
            &same,
            Some(&f.dirs),
            Some(&codec),
            "app:/tmp/in.png",
            "app:/tmp/in.png",
            1,
            10,
            Instant::now(),
        )
        .unwrap();
        assert_eq!(std::fs::read(f.path.join("tmp/in.png")).unwrap(), b"cba");
    }

    #[test]
    fn both_grants_are_checked_before_the_codec_runs() {
        let f = Fixture::new();
        std::fs::write(f.path.join("tmp/in.png"), b"abc").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let codec = reverse(&calls);
        for (text, denied) in [
            ("fs.read app:/tmp", "denied: fs.write"),
            ("fs.write app:/tmp", "denied: fs.read"),
        ] {
            let err = compress_image(
                &grants(text),
                Some(&f.dirs),
                Some(&codec),
                "app:/tmp/in.png",
                "app:/tmp/out.jpg",
                10,
                10,
                Instant::now(),
            )
            .unwrap_err();
            assert!(err.to_string().starts_with(denied), "{err}");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!f.path.join("tmp/out.jpg").exists());
    }

    #[test]
    fn refusals_leave_the_destination_as_it_was() {
        let f = Fixture::new();
        let all = grants("fs.read app:/\nfs.write app:/");
        std::fs::write(f.path.join("tmp/in.png"), b"abc").unwrap();
        std::fs::write(f.path.join("tmp/out.jpg"), b"old").unwrap();
        let failing = |_: &[u8], _: u32, _: u64, _: Instant| -> Result<CompressedImage, String> {
            Err("compressImage: undecodable: not an image".into())
        };
        let err = compress_image(
            &all,
            Some(&f.dirs),
            Some(&failing),
            "app:/tmp/in.png",
            "app:/tmp/out.jpg",
            10,
            10,
            Instant::now(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "compressImage: undecodable: not an image");
        // A codec that overruns the budget is refused too.
        let calls = Arc::new(AtomicUsize::new(0));
        let codec = reverse(&calls);
        let err = compress_image(
            &all,
            Some(&f.dirs),
            Some(&codec),
            "app:/tmp/in.png",
            "app:/tmp/out.jpg",
            10,
            2,
            Instant::now(),
        )
        .unwrap_err();
        assert!(
            err.to_string().starts_with("compressImage: unfit: "),
            "{err}"
        );
        // No codec installed.
        let err = compress_image(
            &all,
            Some(&f.dirs),
            None,
            "app:/tmp/in.png",
            "app:/tmp/out.jpg",
            10,
            10,
            Instant::now(),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "compressImage: unsupported: no JPEG encoder on this host"
        );
        assert_eq!(std::fs::read(f.path.join("tmp/out.jpg")).unwrap(), b"old");
    }

    /// A result ready after the commit budget is never written: by then the
    /// embedder may have told the guest the call failed and run later storage.
    #[test]
    fn a_late_result_is_a_timeout_and_writes_nothing() {
        let f = Fixture::new();
        let all = grants("fs.read app:/\nfs.write app:/");
        std::fs::write(f.path.join("tmp/in.png"), b"abc").unwrap();
        std::fs::write(f.path.join("tmp/out.jpg"), b"old").unwrap();
        let issued = Instant::now() - COMMIT_BUDGET;
        let saw = Arc::new(std::sync::Mutex::new(None));
        let seen = saw.clone();
        let codec = move |bytes: &[u8], _: u32, _: u64, deadline: Instant| {
            *seen.lock().unwrap() = Some(deadline);
            Ok(CompressedImage {
                bytes: bytes.to_vec(),
                width: 1,
                height: 1,
            })
        };
        let err = compress_image(
            &all,
            Some(&f.dirs),
            Some(&codec),
            "app:/tmp/in.png",
            "app:/tmp/out.jpg",
            10,
            10,
            issued,
        )
        .unwrap_err();
        assert!(
            err.to_string().starts_with("compressImage: timeout: "),
            "{err}"
        );
        assert_eq!(saw.lock().unwrap().unwrap(), issued + TRIAL_BUDGET);
        assert_eq!(std::fs::read(f.path.join("tmp/out.jpg")).unwrap(), b"old");
        assert!(COMMIT_BUDGET < EMBEDDER_WAIT && TRIAL_BUDGET < COMMIT_BUDGET);
    }

    #[test]
    fn in_place_a_failure_leaves_the_source() {
        let f = Fixture::new();
        let all = grants("fs.read app:/\nfs.write app:/");
        std::fs::write(f.path.join("tmp/in.png"), b"abc").unwrap();
        let failing = |_: &[u8], _: u32, _: u64, _: Instant| -> Result<CompressedImage, String> {
            Err("compressImage: unfit: no".into())
        };
        assert!(compress_image(
            &all,
            Some(&f.dirs),
            Some(&failing),
            "app:/tmp/in.png",
            "app:/tmp/in.png",
            10,
            10,
            Instant::now()
        )
        .is_err());
        assert_eq!(std::fs::read(f.path.join("tmp/in.png")).unwrap(), b"abc");
    }

    #[test]
    fn sources_that_are_missing_large_or_not_app_paths_are_refused() {
        let f = Fixture::new();
        let all = grants("fs.read app:/\nfs.write app:/\nfs.read /\nfs.write /");
        let calls = Arc::new(AtomicUsize::new(0));
        let codec = reverse(&calls);
        let run = |from: &str, to: &str| {
            compress_image(
                &all,
                Some(&f.dirs),
                Some(&codec),
                from,
                to,
                10,
                10,
                Instant::now(),
            )
            .unwrap_err()
            .to_string()
        };
        assert!(run("app:/tmp/absent", "app:/tmp/out.jpg").contains("os error 2"));
        assert_eq!(
            run("/etc/hosts", "app:/tmp/out.jpg"),
            "compressImage: needs app:/ paths"
        );
        assert_eq!(
            run("app:/tmp/in", "doc:/1/out.jpg"),
            "compressImage: needs app:/ paths"
        );
        assert!(run("app:/tmp/../data/x", "app:/tmp/out.jpg").contains("traversal"));
        std::fs::create_dir(f.path.join("tmp/dir")).unwrap();
        assert!(run("app:/tmp/dir", "app:/tmp/out.jpg").contains("regular file"));
        let big = std::fs::File::create(f.path.join("tmp/big")).unwrap();
        big.set_len(MAX_SOURCE_BYTES + 1).unwrap();
        assert!(run("app:/tmp/big", "app:/tmp/out.jpg").starts_with("compressImage: too-large: "));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let err = compress_image(
            &all,
            None,
            Some(&codec),
            "app:/tmp/in",
            "app:/tmp/out",
            10,
            10,
            Instant::now(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "app directories are not configured");
    }
}

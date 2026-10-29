//! The file picker on Linux (LLP 1069.002 D8): there is no desktop host
//! yet, so `showPicker` is refused with `cancel` (ruled, Q4) and only the
//! agent's substitute answers (D9). What both need is here: the app's
//! directories behind `app:/` (D4, D7), the copy into `app:/tmp/picked/`,
//! and a picked image's pixel size read from its header (D3).

use exact_runner::DataError;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[cfg(windows)]
#[path = "picker_windows.rs"]
mod windows;

fn identity(name: &str) -> bool {
    !matches!(name, "" | "." | "..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
        && leaf(name)
}

fn leaf(name: &str) -> bool {
    if name.is_empty() || matches!(name, "." | "..") || name.contains('\0') {
        return false;
    }
    #[cfg(windows)]
    {
        let mut parts = Path::new(name).components();
        if exact_grants::WindowsPath::validate_component(name).is_err()
            || !matches!(parts.next(), Some(std::path::Component::Normal(_)))
            || parts.next().is_some()
        {
            return false;
        }
    }
    true
}

/// Whether a `showPicker` is held for the agent: `EXACT_AGENT=1`, which a
/// production bake has already dropped (LLP 1069.007 D2).
pub fn agent() -> bool {
    std::env::var("EXACT_AGENT").as_deref() == Ok("1") || cfg!(test)
}

/// `app:/data`, `app:/cache`, `app:/tmp`, once storage is configured.
static ROOTS: Mutex<Option<[PathBuf; 3]>> = Mutex::new(None);

/// Record the app's directories, and empty `app:/tmp/picked/` and
/// `app:/tmp/shared/`: what the last launch picked or was shared is gone (D4).
pub fn set_roots(data: PathBuf, cache: PathBuf, temporary: PathBuf) {
    let _ = std::fs::remove_dir_all(temporary.join("picked"));
    let _ = std::fs::remove_dir_all(temporary.join("shared"));
    *ROOTS.lock().unwrap_or_else(|e| e.into_inner()) = Some([data, cache, temporary]);
}

/// The app's `app:/data`, `app:/cache` and `app:/tmp` (LLP 1027.001): under
/// the XDG bases (Windows: the user's LocalAppData), or a scripted drive's scratch tree (`--storage`), apart
/// from the app's real files, with that tree when an authored test starts it
/// empty (`EXACT_AGENT_STORAGE_FRESH`). `None`: an app with no id, or a
/// drive with no scratch store, which has no storage.
#[allow(clippy::type_complexity)]
pub(crate) fn app_dirs(app_id: &str) -> Result<Option<([PathBuf; 3], Option<PathBuf>)>, DataError> {
    // Scripted drives must not read or write the developer's app files;
    // one that names a scratch tree gets storage there instead.
    let scratch = match std::env::var_os("EXACT_AGENT") {
        Some(_) => match agent_scratch()? {
            Some(name) => Some(name),
            None => return Ok(None),
        },
        None => None,
    };
    if app_id.is_empty() {
        return Ok(None);
    }
    if !identity(app_id) {
        return Err(DataError::Unavailable("unsafe app storage identity".into()));
    }
    #[cfg(windows)]
    let (mut data, mut cache) = windows::bases(&windows::local_app_data()?, app_id)?;
    #[cfg(not(windows))]
    let (mut data, mut cache) = {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| DataError::Unavailable("app storage needs an absolute HOME".into()))?;
        let base = |variable: &str, fallback: &str| {
            std::env::var_os(variable)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
                .join("exact")
                .join(app_id)
        };
        (
            base("XDG_DATA_HOME", ".local/share").join("data"),
            base("XDG_CACHE_HOME", ".cache"),
        )
    };
    let mut fresh = None;
    if let Some(name) = scratch {
        cache = cache.join("agent").join(name);
        if std::env::var_os("EXACT_AGENT_STORAGE_FRESH").is_some() {
            fresh = Some(cache.clone());
        }
        data = cache.join("data");
    }
    // Sibling roots keep app:/cache grants from implicitly reaching tmp.
    // The user's cache base avoids a predictable shared /tmp directory.
    Ok(Some((
        [data, cache.join("cache"), cache.join("temporary")],
        fresh,
    )))
}

/// Root for kept answers and persisted states (`<root>/kv`), including
/// Windows outside the agent. A nameless drive has no persistent storage.
pub(crate) fn store_root(app_id: &str) -> Result<Option<PathBuf>, DataError> {
    Ok(app_dirs(app_id)?.and_then(|(roots, _)| roots[0].parent().map(|path| path.to_path_buf())))
}

/// Where `secret.keep` names live (`<root>/secrets`). A named agent drive
/// keeps them in its scratch tree beside its `app:/data`; outside the agent
/// the app keeps them in its own data root,
/// `$XDG_DATA_HOME/exact/<app id>` (`~/.local/share` when unset), as Ibex's
/// file backends: a file per secret, `0600` in a `0700` directory, written
/// whole and renamed into place, not encrypted (LLP 1027.007 D13, ibex LLP
/// 0069 §3). `None` keeps them in memory: a drive that names no scratch
/// store, an app with no id, and Windows outside the agent (until its
/// credential vault).
pub(crate) fn secret_root(app_id: &str) -> Result<Option<PathBuf>, DataError> {
    if app_id.is_empty() || (cfg!(windows) && std::env::var_os("EXACT_AGENT").is_none()) {
        return Ok(None);
    }
    // `roots[0]` is `<root>/data`. In a drive, secrets live in the scratch
    // tree, so a fresh drive's emptying of that tree ([`empty_fresh_tree`])
    // takes them too.
    store_root(app_id)
}

/// The scratch trees a fresh drive has emptied in this process.
static EMPTIED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Empty a fresh drive's scratch tree (`EXACT_AGENT_STORAGE_FRESH`) once a
/// process: at the first boot that reads its store, before the snapshot and
/// before any commit can write a secret or a kept answer into it. The
/// post-pixel activation and any later boot here leave it as it is; emptying
/// it again there removed what a commit in between had written while memory
/// still held it, so a relaunch read nothing back.
pub(crate) fn empty_fresh_tree(app_id: &str) -> Result<(), DataError> {
    let Some((_, Some(tree))) = app_dirs(app_id)? else {
        return Ok(());
    };
    let mut emptied = EMPTIED.lock().unwrap_or_else(|e| e.into_inner());
    if emptied.contains(&tree) {
        return Ok(());
    }
    unlinked(&tree)?;
    // `remove_dir_all` follows no link inside the tree; the directories
    // above it were just checked.
    match std::fs::remove_dir_all(&tree) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(DataError::Unavailable(format!(
                "EXACT_AGENT_STORAGE_FRESH: could not empty {}: {e}",
                tree.display()
            )));
        }
        _ => {}
    }
    emptied.push(tree);
    Ok(())
}

/// Refuses to empty a scratch tree (`<cache base>/exact/<app id>/agent/<name>`)
/// when it, or a directory between it and the cache base, is a symbolic link:
/// emptying it through one would delete whatever the link points at, such as
/// the app's real files.
fn unlinked(tree: &std::path::Path) -> Result<(), DataError> {
    let Some(base) = tree.ancestors().nth(4) else {
        return Ok(());
    };
    let mut at = base.to_path_buf();
    for part in tree.strip_prefix(base).unwrap_or(tree).components() {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(DataError::Unavailable(format!(
                    "EXACT_AGENT_STORAGE_FRESH: {} is a symbolic link, so the scratch store under it is not emptied: remove the link, or name another store",
                    at.display()
                )));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(DataError::Unavailable(format!(
                    "EXACT_AGENT_STORAGE_FRESH: could not inspect {}: {e}",
                    at.display()
                )));
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

/// A scripted drive's scratch storage (`EXACT_AGENT_STORAGE=<name>`): a tree
/// of its own under the cache base, so a drive can exercise storage without
/// touching the app's real files. Absent, a drive has no storage.
fn agent_scratch() -> Result<Option<String>, DataError> {
    let Some(name) = std::env::var_os("EXACT_AGENT_STORAGE") else {
        return Ok(None);
    };
    match name.to_str() {
        Some(name) if identity(name) => Ok(Some(name.to_owned())),
        _ => Err(DataError::Unavailable(
            "EXACT_AGENT_STORAGE: one name of letters, digits, '.', '-' or '_'".into(),
        )),
    }
}

/// The app's directories at boot, before storage is configured after first
/// pixel: an `image "app:/data/…"` resolves from the first frame, whether or
/// not anything was picked (D7; recipes F18). Nothing is emptied until
/// storage is configured ([`set_roots`]).
pub fn know_roots(app_id: &str) {
    if let Ok(Some((roots, _))) = app_dirs(app_id) {
        *ROOTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(roots);
    }
}

/// A scripted drive without a scratch store has no app directories; its
/// picks land in a directory of this process's own.
fn roots() -> [PathBuf; 3] {
    if let Some(r) = ROOTS.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return r;
    }
    let base = std::env::temp_dir().join(format!("exact-agent-{}", std::process::id()));
    let r = [
        base.join("data"),
        base.join("cache"),
        base.join("temporary"),
    ];
    *ROOTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(r.clone());
    r
}

/// The file an `app:/data|cache|tmp/…` path names (D7): `None` for any
/// other scheme, root, or a `..`.
pub fn resolve(path: &str) -> Option<PathBuf> {
    resolve_from(path, roots)
}

fn resolve_from(path: &str, roots: impl FnOnce() -> [PathBuf; 3]) -> Option<PathBuf> {
    let rest = path.strip_prefix("app:/")?;
    let (dir, rest) = rest.split_once('/')?;
    let index = match dir {
        "data" => 0,
        "cache" => 1,
        "tmp" => 2,
        _ => return None,
    };
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.iter().any(|p| !leaf(p)) {
        return None;
    }
    let root = roots().into_iter().nth(index)?;
    Some(parts.iter().fold(root, |p, part| p.join(part)))
}

/// A picked image's pixel size from its header, without decoding: PNG,
/// JPEG (the frame's, with EXIF orientation 5–8 swapping it), GIF, WebP.
pub fn dimensions(bytes: &[u8]) -> Option<(f64, f64)> {
    let be16 = |b: &[u8], i: usize| Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as f64);
    let le16 = |b: &[u8], i: usize| Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]) as f64);
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let w = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?) as f64;
        let h = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?) as f64;
        return Some((w, h));
    }
    if bytes.starts_with(b"GIF8") {
        return Some((le16(bytes, 6)?, le16(bytes, 8)?));
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        let le24 = |i: usize| -> Option<f64> {
            Some(
                (u32::from(*bytes.get(i)?)
                    | u32::from(*bytes.get(i + 1)?) << 8
                    | u32::from(*bytes.get(i + 2)?) << 16) as f64,
            )
        };
        return match bytes.get(12..16)? {
            b"VP8 " => Some((
                (le16(bytes, 26)? as u32 & 0x3fff) as f64,
                (le16(bytes, 28)? as u32 & 0x3fff) as f64,
            )),
            b"VP8L" => {
                let b = u32::from_le_bytes(bytes.get(21..25)?.try_into().ok()?);
                Some(((b & 0x3fff) as f64 + 1.0, ((b >> 14) & 0x3fff) as f64 + 1.0))
            }
            b"VP8X" => Some((le24(24)? + 1.0, le24(27)? + 1.0)),
            _ => None,
        };
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        let mut i = 2;
        let mut swap = false;
        while i + 4 <= bytes.len() {
            if bytes[i] != 0xff {
                return None;
            }
            let marker = bytes[i + 1];
            let len = be16(bytes, i + 2)? as usize;
            if marker == 0xe1 && bytes.get(i + 4..i + 10) == Some(b"Exif\0\0") {
                swap = exif_rotates(bytes.get(i + 10..i + 2 + len)?).unwrap_or(false);
            }
            if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
                let (h, w) = (be16(bytes, i + 5)?, be16(bytes, i + 7)?);
                return Some(if swap { (h, w) } else { (w, h) });
            }
            i += 2 + len;
        }
    }
    None
}

/// Whether a TIFF-in-EXIF block's orientation is 5–8 (a quarter turn).
fn exif_rotates(tiff: &[u8]) -> Option<bool> {
    let le = tiff.get(0..2)? == b"II";
    let u16_at = |i: usize| -> Option<u16> {
        let b = [*tiff.get(i)?, *tiff.get(i + 1)?];
        Some(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let u32_at = |i: usize| -> Option<u32> {
        let b: [u8; 4] = tiff.get(i..i + 4)?.try_into().ok()?;
        Some(if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let ifd = u32_at(4)? as usize;
    for k in 0..u16_at(ifd)? as usize {
        let entry = ifd + 2 + k * 12;
        if u16_at(entry)? == 0x0112 {
            return Some((5..=8).contains(&u16_at(entry + 8)?));
        }
    }
    Some(false)
}

/// Copy the driver's file at `source` to `app_path` (under
/// `app:/tmp/picked/`), and describe it as `change` delivers it. The type
/// is what the name says; a video's duration is not read here.
pub fn copy_in(source: &Path, app_path: &str, mime: &str) -> Result<exact_runner::Picked, String> {
    let target = resolve(app_path).ok_or_else(|| format!("{app_path}: not an app path"))?;
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::copy(source, &target).map_err(|e| format!("{}: {e}", source.display()))?;
    let size = std::fs::metadata(&target).map_err(|e| e.to_string())?.len() as f64;
    let (width, height) = if mime.starts_with("image/") {
        std::fs::read(&target)
            .ok()
            .and_then(|b| dimensions(&b))
            .map_or((None, None), |(w, h)| (Some(w), Some(h)))
    } else {
        (None, None)
    };
    Ok(exact_runner::Picked {
        path: app_path.into(),
        name: source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        mime: mime.into(),
        size,
        width,
        height,
        duration: None,
    })
}

/// A cold share's claim (LLP 1133.024 D9), whose copies the presenter makes
/// once storage is configured after first pixel, with the descriptors its
/// files arrived as ([`Held`]).
pub(crate) static COLD_SHARE: Mutex<Option<(exact_runner::share_in::Claimed, Held)>> =
    Mutex::new(None);

/// The files a share from Android's share sheet arrived as: each stream the
/// intent's grant opened, named to the claim as `/proc/self/fd/<n>` and read
/// from the descriptor itself, since opening that path again goes through
/// the sender's storage, which the grant does not open. Dropping them, once
/// the claim has copied them, closes them. Empty on every other host.
pub(crate) type Held = Vec<(PathBuf, std::fs::File)>;

/// The descriptors `share`'s files name, taken over: the reader detached
/// each for this library to close (`Shares.kt`). Only a share the reader
/// made is passed here, never the driver's.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
pub(crate) fn hold(share: &exact_runner::share_in::Pending) -> Held {
    use std::os::fd::FromRawFd;
    share
        .files
        .iter()
        .filter_map(|f| {
            let fd = f
                .source
                .strip_prefix("/proc/self/fd/")
                .ok()?
                .to_str()?
                .parse::<i32>()
                .ok()
                .filter(|fd| *fd > 2)?;
            // SAFETY: the reader detached each descriptor from its
            // ParcelFileDescriptor and gave it to this library alone.
            Some((f.source.clone(), unsafe { std::fs::File::from_raw_fd(fd) }))
        })
        .collect()
}

/// A cold share (LLP 1133.024 D5, D9): on Android the reader's, from the
/// launching intent (`EXACT_SHARE`), else under the agent the driver's
/// (`EXACT_AGENT_SHARE`); each `{"target", "share"}`, as the `share` op
/// carries them. Claimed before the boot, which takes its location and
/// tokens with the launch's other facts. A host takes a share from the
/// environment nowhere else.
pub fn cold_share(seed: f64) -> Option<(String, exact_runner::time::Launch)> {
    use exact_runner::share_in::{Claims, Pending, ShareTarget};
    let (variable, value) = match std::env::var("EXACT_SHARE") {
        Ok(value) if cfg!(target_os = "android") => ("EXACT_SHARE", value),
        _ if agent() => (
            "EXACT_AGENT_SHARE",
            std::env::var("EXACT_AGENT_SHARE").ok()?,
        ),
        _ => return None,
    };
    let request: serde_json::Value = serde_json::from_str(&value).ok()?;
    let (Some(target), Some(share)) = (
        ShareTarget::from_json(&request["target"]),
        Pending::from_json(&request["share"], None),
    ) else {
        eprintln!("exact: {variable} refused: it needs the manifest's share_target and a share");
        return None;
    };
    #[cfg(target_os = "android")]
    let held = if variable == "EXACT_SHARE" {
        hold(&share)
    } else {
        Held::new()
    };
    #[cfg(not(target_os = "android"))]
    let held = Held::new();
    let mut claims = Claims::new(seed);
    let claimed = claims.claim(&target, share);
    let launch = exact_runner::time::Launch {
        shared: claimed.tokens.clone(),
        claims: Some(claims),
        ..exact_runner::time::Launch::NONE
    };
    let location = claimed.location.clone();
    *COLD_SHARE.lock().unwrap_or_else(|e| e.into_inner()) = Some((claimed, held));
    Some((location, launch))
}

/// `app:/tmp/shared/` on this machine, once storage is configured.
pub(crate) fn shared_dir() -> Option<PathBuf> {
    resolve(&format!("{}x", exact_runner::share_in::SHARED))
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reset refuses a scratch tree reached through a symbolic link above it
    /// (an `agent` directory linked to the app's real data, say), leaving
    /// what the link points at as it was; an ordinary tree is let through.
    #[cfg(unix)]
    #[test]
    fn a_reset_refuses_a_scratch_tree_reached_through_a_link() {
        let base =
            std::env::temp_dir().join(format!("exact-linux-unlinked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let real = base.join("real-data");
        std::fs::create_dir_all(real.join("s1")).unwrap();
        std::fs::write(real.join("s1").join("library.json"), "kept").unwrap();
        std::fs::create_dir_all(base.join("cache/exact/app.id")).unwrap();
        std::os::unix::fs::symlink(&real, base.join("cache/exact/app.id/agent")).unwrap();
        let tree = base.join("cache/exact/app.id/agent/s1");
        let refused = unlinked(&tree).unwrap_err();
        assert!(
            format!("{refused:?}").contains("is a symbolic link"),
            "{refused:?}"
        );
        assert_eq!(
            std::fs::read_to_string(real.join("s1").join("library.json")).unwrap(),
            "kept"
        );
        std::fs::remove_file(base.join("cache/exact/app.id/agent")).unwrap();
        std::fs::create_dir_all(&tree).unwrap();
        assert!(unlinked(&tree).is_ok());
        assert!(unlinked(&base.join("cache/exact/app.id/agent/absent")).is_ok());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn headers_give_the_pixel_size() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(640u32.to_be_bytes());
        png.extend(480u32.to_be_bytes());
        assert_eq!(dimensions(&png), Some((640.0, 480.0)));
        let gif = b"GIF89a\x20\x00\x10\x00";
        assert_eq!(dimensions(gif), Some((32.0, 16.0)));
        // SOI, a baseline frame of 300 × 200.
        let jpeg = [
            0xff, 0xd8, 0xff, 0xc0, 0, 11, 8, 0, 200, 1, 44, 3, 0, 0, 0, 0,
        ];
        assert_eq!(dimensions(&jpeg), Some((300.0, 200.0)));
        assert_eq!(dimensions(b"nothing"), None);
    }

    #[test]
    fn app_paths_resolve_under_their_roots_and_refuse_traversal() {
        assert!(resolve("app:/tmp/picked/1-1.jpg")
            .is_some_and(|p| p.ends_with("temporary/picked/1-1.jpg")));
        assert_eq!(resolve("app:/tmp/../x"), None);
        assert_eq!(resolve("app:/etc/passwd"), None);
        assert_eq!(resolve("/etc/passwd"), None);
    }

    /// An `app:/data` image resolves before storage is configured, with
    /// nothing picked (D7; recipes F18): in a child, whose roots are its own.
    #[test]
    fn app_roots_are_known_at_boot_before_storage() {
        const CHILD: &str = "EXACT_LINUX_ROOTS_TEST";
        if std::env::var_os(CHILD).is_none() {
            let home = std::env::temp_dir().join(format!("exact-roots-{}", std::process::id()));
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "picker::tests::app_roots_are_known_at_boot_before_storage",
                ])
                .env(CHILD, "1")
                .env("HOME", &home)
                .env("EXACT_AGENT", "1")
                .env("EXACT_AGENT_STORAGE", "s1")
                .env_remove("XDG_DATA_HOME")
                .env_remove("XDG_CACHE_HOME")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(!home.exists(), "knowing the roots creates nothing");
            return;
        }
        know_roots("test.exact.roots");
        let file = resolve("app:/data/photos/a.jpg").unwrap();
        #[cfg(not(windows))]
        assert!(
            file.ends_with(".cache/exact/test.exact.roots/agent/s1/data/photos/a.jpg"),
            "{}",
            file.display()
        );
        #[cfg(windows)]
        assert_eq!(
            file,
            windows::local_app_data()
                .unwrap()
                .join("exact/test.exact.roots/agent/s1/data/photos/a.jpg")
        );
    }
}

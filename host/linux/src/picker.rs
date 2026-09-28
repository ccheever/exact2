//! The file picker on Linux (LLP 1069.002 D8): there is no desktop host
//! yet, so `showPicker` is refused with `cancel` (ruled, Q4) and only the
//! agent's substitute answers (D9). What both need is here: the app's
//! directories behind `app:/` (D4, D7), the copy into `app:/tmp/picked/`,
//! and a picked image's pixel size read from its header (D3).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Whether a `showPicker` is held for the agent: `EXACT_AGENT=1`, which a
/// production bake has already dropped (LLP 1069.007 D2).
pub fn agent() -> bool {
    std::env::var("EXACT_AGENT").as_deref() == Ok("1") || cfg!(test)
}

/// `app:/data`, `app:/cache`, `app:/tmp`, once storage is configured.
static ROOTS: Mutex<Option<[PathBuf; 3]>> = Mutex::new(None);

/// Record the app's directories, and empty `app:/tmp/picked/`: what the
/// last launch picked is gone (D4).
pub fn set_roots(data: PathBuf, cache: PathBuf, temporary: PathBuf) {
    let _ = std::fs::remove_dir_all(temporary.join("picked"));
    *ROOTS.lock().unwrap_or_else(|e| e.into_inner()) = Some([data, cache, temporary]);
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
    let rest = path.strip_prefix("app:/")?;
    let (dir, rest) = rest.split_once('/')?;
    let [data, cache, temporary] = roots();
    let root = match dir {
        "data" => data,
        "cache" => cache,
        "tmp" => temporary,
        _ => return None,
    };
    let parts: Vec<&str> = rest.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains('\0'))
    {
        return None;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

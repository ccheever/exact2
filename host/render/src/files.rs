//! The dist's files as the server names them (LLP 1048.000 D10): which
//! paths are files, their types, and the build a URL names.

use std::path::{Path, PathBuf};

/// Whether a path names a file (its last segment has an extension the
/// server knows as a type) rather than a page.
pub(crate) fn asset_shaped(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or("");
    last.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && extension != "html"
            && content_type(&extension.to_ascii_lowercase()) != "application/octet-stream"
    })
}

/// Whether `target` names the build whose ETag is `tag`: the web build asks
/// for `app.wasm?v=` and the first 16 hex digits of its SHA-256, and a URL
/// that names its content may be cached for good.
pub(crate) fn named_build(target: &str, tag: &str) -> bool {
    target.split_once('?').is_some_and(|(_, query)| {
        query.split('&').any(|pair| {
            pair.strip_prefix("v=")
                .is_some_and(|v| v.len() == 16 && tag.starts_with(v))
        })
    })
}

/// The web's auth callback page (LLP 1069.006 D4), served with no referrer
/// and never stored, as its script is.
pub(crate) const AUTH_CALLBACK: &str = "/.exact/auth/callback";

/// A file `dist` serves as it is: anything under `/.exact/`, and any other
/// file but a page (`.html`), which is rendered. Never outside `dist`.
pub(crate) fn static_file(dist: &Path, path: &str) -> Option<(PathBuf, &'static str)> {
    let decoded = percent_decode(path, false)?;
    if decoded.split('/').any(|part| part == ".." || part == ".") || decoded.contains('\\') {
        return None;
    }
    let mut file = dist.join(decoded.trim_start_matches('/'));
    let exact = decoded.starts_with("/.exact/");
    if exact && file.is_dir() {
        file = file.join("index.html");
    }
    let root = dist.canonicalize().ok()?;
    let file = file.canonicalize().ok()?;
    if !file.starts_with(&root) || !file.is_file() {
        return None;
    }
    let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("");
    if extension == "html" && !exact {
        return None;
    }
    // The auth callback page (LLP 1069.006 D4): one page at exactly this path.
    if decoded == AUTH_CALLBACK {
        return Some((file.clone(), content_type("html")));
    }
    Some((file.clone(), content_type(extension)))
}

pub(crate) fn percent_decode(path: &str, allow_slash: bool) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            let byte = u8::from_str_radix(hex, 16).ok()?;
            if (!allow_slash && byte == b'/') || byte == 0 {
                return None;
            }
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

pub(crate) fn content_type(extension: &str) -> &'static str {
    match extension {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" | "wgsl" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

//! Compression (LLP 1048.000 D11): brotli, else gzip, as `Accept-Encoding`
//! allows, for text, scripts, the wasm and SVG, with `Vary:
//! Accept-Encoding` and the encoding in the ETag. A page is compressed as it
//! is sent (brotli at quality 5: a 57 KB document in about a millisecond); a
//! file of the dist once, at bind and off the request path, at brotli's best
//! — until its variant is made it goes out as it is.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Smaller than this, a body goes as it is.
const MIN: usize = 1024;
/// What the dist's variants may hold, in all.
const HELD: usize = 256 << 20;

/// The encodings a request accepts (RFC 9110 §12.5.3: a `q` of 0 refuses).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Accepts {
    br: bool,
    gzip: bool,
}

impl Accepts {
    /// Read an `Accept-Encoding` value.
    pub(crate) fn parse(value: &str) -> Accepts {
        let mut accepts = Accepts::default();
        for part in value.split(',') {
            let mut params = part.split(';');
            let name = params.next().unwrap_or("").trim().to_ascii_lowercase();
            let refused = params.any(|param| {
                param
                    .trim()
                    .strip_prefix("q=")
                    .is_some_and(|q| q.trim().parse::<f32>().map_or(true, |q| q <= 0.0))
            });
            match (name.as_str(), refused) {
                (_, true) => {}
                ("br", _) => accepts.br = true,
                ("gzip", _) => accepts.gzip = true,
                ("*", _) => {
                    accepts.br = true;
                    accepts.gzip = true;
                }
                _ => {}
            }
        }
        accepts
    }

    /// Brotli, else gzip, else none.
    fn pick(self) -> Option<Encoding> {
        if self.br {
            Some(Encoding::Br)
        } else if self.gzip {
            Some(Encoding::Gzip)
        } else {
            None
        }
    }
}

/// A content coding the server makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Encoding {
    Br,
    Gzip,
}

impl Encoding {
    /// Its `Content-Encoding` token.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Encoding::Br => "br",
            Encoding::Gzip => "gzip",
        }
    }
}

/// Whether a body of this `Content-Type` is worth compressing.
pub(crate) fn compressible(content_type: &str) -> bool {
    matches!(
        content_type.split(';').next().unwrap_or("").trim(),
        "text/html"
            | "text/javascript"
            | "text/css"
            | "text/plain"
            | "application/json"
            | "application/manifest+json"
            | "application/wasm"
            | "application/xml"
            | "image/svg+xml"
    )
}

/// `body` as the client accepts it, now: the encoding and the bytes, or
/// none when it accepts neither, the body is small, or compressing didn't
/// make it smaller.
pub(crate) fn now(body: &[u8], accepts: Accepts) -> Option<(Encoding, Vec<u8>)> {
    let encoding = accepts.pick()?;
    if body.len() < MIN {
        return None;
    }
    compress(body, encoding, false).map(|out| (encoding, out))
}

fn compress(body: &[u8], encoding: Encoding, best: bool) -> Option<Vec<u8>> {
    let out = match encoding {
        Encoding::Br => {
            let params = brotli::enc::BrotliEncoderParams {
                quality: if best { 11 } else { 5 },
                lgwin: 22,
                size_hint: body.len(),
                ..Default::default()
            };
            let mut out = Vec::with_capacity(body.len() / 3);
            brotli::BrotliCompress(&mut &body[..], &mut out, &params).ok()?;
            out
        }
        Encoding::Gzip => {
            let level = flate2::Compression::new(if best { 9 } else { 6 });
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), level);
            encoder.write_all(body).ok()?;
            encoder.finish().ok()?
        }
    };
    (out.len() < body.len()).then_some(out)
}

/// The dist's files, each compressed once, by one thread from bind: a
/// variant is served while the file is the one it was made from (its length
/// and modification time).
#[derive(Clone, Default)]
pub(crate) struct Variants {
    made: Arc<Mutex<HashMap<(PathBuf, Encoding), Variant>>>,
}

struct Variant {
    stamp: (u64, Option<SystemTime>),
    body: Arc<Vec<u8>>,
}

fn stamp(file: &Path) -> Option<(u64, Option<SystemTime>)> {
    let meta = std::fs::metadata(file).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

impl Variants {
    /// Start making `files`' variants, brotli's and gzip's, off the request
    /// path; a file whose variants would pass what the server holds is left
    /// as it is.
    pub(crate) fn warm(&self, files: Vec<PathBuf>) {
        let made = self.made.clone();
        let _ = std::thread::Builder::new()
            .name("exact-render-compress".into())
            .spawn(move || {
                let mut held = 0;
                for file in files {
                    let (Some(stamp), Ok(body)) = (stamp(&file), std::fs::read(&file)) else {
                        continue;
                    };
                    for encoding in [Encoding::Br, Encoding::Gzip] {
                        let Some(out) = compress(&body, encoding, true) else {
                            continue;
                        };
                        if held + out.len() > HELD {
                            return;
                        }
                        held += out.len();
                        let variant = Variant {
                            stamp,
                            body: Arc::new(out),
                        };
                        made.lock()
                            .unwrap()
                            .insert((file.clone(), encoding), variant);
                    }
                }
            });
    }

    /// `file` as the client accepts it, if its variant is made.
    pub(crate) fn get(&self, file: &Path, accepts: Accepts) -> Option<(Encoding, Arc<Vec<u8>>)> {
        let stamp = stamp(file)?;
        let made = self.made.lock().unwrap();
        [Encoding::Br, Encoding::Gzip]
            .into_iter()
            .filter(|encoding| match encoding {
                Encoding::Br => accepts.br,
                Encoding::Gzip => accepts.gzip,
            })
            .find_map(|encoding| {
                made.get(&(file.to_path_buf(), encoding))
                    .filter(|variant| variant.stamp == stamp)
                    .map(|variant| (encoding, variant.body.clone()))
            })
    }
}

/// Every file under `dist` worth compressing, the pages aside, by the
/// canonical path the server finds a request's file at, smallest first: the
/// glue a first load waits on is ready in moments, the wasm (seconds at
/// brotli's best) after it.
pub(crate) fn files(dist: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(root) = dist.canonicalize() else {
        return found;
    };
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if extension != "html" && compressible(crate::serve::content_type(extension)) {
                found.push(path);
            }
        }
    }
    found.sort_by_key(|path| {
        (
            std::fs::metadata(path).map_or(0, |meta| meta.len()),
            path.clone(),
        )
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_encoding_reads_as_rfc_9110_says() {
        let both = Accepts {
            br: true,
            gzip: true,
        };
        assert_eq!(Accepts::parse("gzip, deflate, br, zstd"), both);
        assert_eq!(Accepts::parse("*"), both);
        assert_eq!(
            Accepts::parse("br;q=0, gzip;q=0.5").pick(),
            Some(Encoding::Gzip)
        );
        assert_eq!(Accepts::parse("BR;Q=1").pick(), Some(Encoding::Br));
        assert_eq!(Accepts::parse("identity").pick(), None);
        assert_eq!(Accepts::parse("").pick(), None);
    }

    #[test]
    fn a_small_body_or_one_that_does_not_shrink_goes_as_it_is() {
        let accepts = Accepts::parse("br, gzip");
        assert!(now(b"<p>short</p>", accepts).is_none());
        // xorshift: bytes nothing compresses.
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let noise: Vec<u8> = (0..4096)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 56) as u8
            })
            .collect();
        assert!(now(&noise, accepts).is_none());
        let page = "<div>the same words again</div>".repeat(100);
        let (encoding, out) = now(page.as_bytes(), accepts).unwrap();
        assert_eq!(encoding, Encoding::Br);
        assert!(out.len() < page.len() / 4);
    }
}

//! Compression (LLP 1048.000 D11): brotli, else gzip, as `Accept-Encoding`
//! allows, for text, scripts, the wasm and SVG, with `Vary:
//! Accept-Encoding` and the encoding in the ETag. A page is compressed as it
//! is sent (brotli at quality 5: a 57 KB document in about a millisecond),
//! and a page the origin keeps once more at the best, off the request path; a
//! file of the dist once, at bind and off the request path, at brotli's best
//! — until its variant is made it goes out as it is.

use std::collections::HashMap;
use std::io::{Read as _, Write as _};
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
    /// Dictionary-Compressed Brotli, named outright (`*` doesn't offer it).
    dcb: bool,
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
                ("dcb", _) => accepts.dcb = true,
                ("*", _) => {
                    accepts.br = true;
                    accepts.gzip = true;
                }
                _ => {}
            }
        }
        accepts
    }

    /// Whether it takes a body compressed against a dictionary it holds.
    pub(crate) fn dcb(self) -> bool {
        self.dcb
    }

    /// Brotli, else gzip, else none.
    pub(crate) fn pick(self) -> Option<Encoding> {
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

/// `body` as Dictionary-Compressed Brotli (`dcb`) against `dictionary`, whose
/// SHA-256 is `hash`: four magic bytes, the hash, then a brotli stream that may
/// copy from the dictionary as if it came first (a raw LZ77 prefix). Quality 5,
/// as a page is compressed as it is sent; none when it doesn't shrink.
pub(crate) fn against(body: &[u8], dictionary: &[u8], hash: &[u8; 32]) -> Option<Vec<u8>> {
    against_at(body, dictionary, hash, 5)
}

/// [`against`] at `quality`: a build's delta to an earlier one is made once,
/// off every request's path, at brotli's best ([`crate::generations`]).
pub(crate) fn against_at(
    body: &[u8],
    dictionary: &[u8],
    hash: &[u8; 32],
    quality: i32,
) -> Option<Vec<u8>> {
    let params = brotli::enc::BrotliEncoderParams {
        quality,
        lgwin: 22,
        size_hint: body.len(),
        ..Default::default()
    };
    // The window holds the dictionary and the body, or the stream can't reach it.
    if dictionary.len() + body.len() > (1usize << params.lgwin) - 16 {
        return None;
    }
    let mut out = vec![0xff, 0x44, 0x43, 0x42];
    out.extend_from_slice(hash);
    let (mut input, mut output) = (vec![0u8; 4096], vec![0u8; 4096]);
    brotli::BrotliCompressCustomIoCustomDict(
        &mut brotli::IoReaderWrapper(&mut &body[..]),
        &mut brotli::IoWriterWrapper(&mut out),
        &mut input,
        &mut output,
        &params,
        brotli::enc::StandardAlloc::default(),
        &mut |_, _, _, _| (),
        dictionary,
        std::io::Error::from(std::io::ErrorKind::UnexpectedEof),
    )
    .ok()?;
    (out.len() < body.len()).then_some(out)
}

/// A cached page's bodies at brotli's and gzip's best, each only when it's
/// smaller, made once off the request path (D11).
#[derive(Default)]
pub(crate) struct Best {
    br: Option<Vec<u8>>,
    gzip: Option<Vec<u8>>,
}

impl Best {
    pub(crate) fn of(body: &[u8]) -> Best {
        if body.len() < MIN {
            return Best::default();
        }
        Best {
            br: compress(body, Encoding::Br, true),
            gzip: compress(body, Encoding::Gzip, true),
        }
    }

    /// The variant `accepts` picks, when it was made.
    pub(crate) fn pick(&self, accepts: Accepts) -> Option<(Encoding, &[u8])> {
        let encoding = accepts.pick()?;
        let body = match encoding {
            Encoding::Br => self.br.as_deref(),
            Encoding::Gzip => self.gzip.as_deref(),
        }?;
        Some((encoding, body))
    }

    /// The bytes it holds.
    pub(crate) fn bytes(&self) -> usize {
        self.br.as_ref().map_or(0, Vec::len) + self.gzip.as_ref().map_or(0, Vec::len)
    }
}

/// The dist's files as the server sends them: each file's validator (a hash
/// of its bytes), and its variants, compressed once by one thread from bind.
/// What is known of a file holds while it is the file it was read from (its
/// length and modification time).
#[derive(Clone, Default)]
pub(crate) struct Variants {
    known: Arc<Mutex<HashMap<PathBuf, Known>>>,
}

#[derive(Clone)]
struct Known {
    stamp: (u64, Option<SystemTime>),
    /// The first 128 bits of the bytes' SHA-256, in hex.
    tag: String,
    br: Option<Arc<Vec<u8>>>,
    gzip: Option<Arc<Vec<u8>>>,
}

/// A dist file as one request gets it.
pub(crate) struct Served {
    /// The ETag, quoted: the file's tag, and the encoding when there is one.
    pub(crate) etag: String,
    pub(crate) encoding: Option<Encoding>,
    pub(crate) body: Arc<Vec<u8>>,
}

// Bound both request reads and background compression. The extra byte catches
// files that grow after metadata was checked without retaining their remainder.
fn read_file(path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > crate::serve::MAX_PAGE as u64 {
        return None;
    }
    let mut body = Vec::new();
    file.take(crate::serve::MAX_PAGE as u64 + 1)
        .read_to_end(&mut body)
        .ok()?;
    (body.len() <= crate::serve::MAX_PAGE).then_some(body)
}

fn stamp(file: &Path) -> Option<(u64, Option<SystemTime>)> {
    let meta = std::fs::metadata(file).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

/// A file's ETag: the first half of its SHA-256, in hex.
pub(crate) fn tag(body: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(body)[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Variants {
    /// Start learning `files`: every file's tag first, then its brotli and
    /// gzip variants, off the request path. The variants stop at what the
    /// server holds; the tags don't.
    pub(crate) fn warm(&self, files: Vec<PathBuf>) {
        let known = self.known.clone();
        let _ = std::thread::Builder::new()
            .name("exact-render-compress".into())
            .spawn(move || {
                let read = |file: &PathBuf| Some((stamp(file)?, read_file(file)?));
                for file in &files {
                    let Some((stamp, body)) = read(file) else {
                        continue;
                    };
                    let mut known = known.lock().unwrap();
                    if !known.get(file).is_some_and(|k| k.stamp == stamp) {
                        let entry = Known {
                            stamp,
                            tag: tag(&body),
                            br: None,
                            gzip: None,
                        };
                        known.insert(file.clone(), entry);
                    }
                }
                let mut held = 0;
                for file in &files {
                    let Some((stamp, body)) = read(file) else {
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
                        let mut known = known.lock().unwrap();
                        let Some(entry) = known.get_mut(file).filter(|k| k.stamp == stamp) else {
                            continue;
                        };
                        let out = Some(Arc::new(out));
                        match encoding {
                            Encoding::Br => entry.br = out,
                            Encoding::Gzip => entry.gzip = out,
                        }
                    }
                }
            });
    }

    /// `file` as the client accepts it: a variant once it is made (only for
    /// a type worth compressing), else the file as it is; either way with
    /// its ETag. None when the file can't be read.
    pub(crate) fn serve(
        &self,
        file: &Path,
        accepts: Accepts,
        compressible: bool,
    ) -> Option<Served> {
        let stamp = stamp(file)?;
        if stamp.0 > crate::serve::MAX_PAGE as u64 {
            return None;
        }
        let entry = {
            let known = self.known.lock().unwrap();
            known.get(file).filter(|k| k.stamp == stamp).cloned()
        };
        if let Some(entry) = &entry {
            let variants = [
                (Encoding::Br, accepts.br, &entry.br),
                (Encoding::Gzip, accepts.gzip, &entry.gzip),
            ];
            let made = variants.into_iter().find_map(|(encoding, accepted, body)| {
                Some((encoding, body.clone().filter(|_| accepted)?))
            });
            if let (true, Some((encoding, body))) = (compressible, made) {
                return Some(Served {
                    etag: format!("\"{}-{}\"", entry.tag, encoding.name()),
                    encoding: Some(encoding),
                    body,
                });
            }
        }
        let body = read_file(file)?;
        let tag = match entry {
            Some(entry) => entry.tag,
            None => {
                let tag = tag(&body);
                let entry = Known {
                    stamp,
                    tag: tag.clone(),
                    br: None,
                    gzip: None,
                };
                self.known.lock().unwrap().insert(file.to_path_buf(), entry);
                // A file new or changed since bind (a dist rebuilt under a
                // running server) gets its variants made as at bind; until
                // then it goes as it is.
                if compressible {
                    self.warm(vec![file.to_path_buf()]);
                }
                tag
            }
        };
        Some(Served {
            etag: format!("\"{tag}\""),
            encoding: None,
            body: Arc::new(body),
        })
    }
}

/// Whether an `If-None-Match` value names `etag` (RFC 9110 §13.1.2: a list,
/// or `*`, compared weakly).
pub(crate) fn none_match(header: Option<&str>, etag: &str) -> bool {
    let bare = |tag: &str| tag.trim().trim_start_matches("W/").to_string();
    header.is_some_and(|header| {
        header.trim() == "*" || header.split(',').any(|tag| bare(tag) == bare(etag))
    })
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
            if extension != "html" && compressible(crate::files::content_type(extension)) {
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
    fn a_body_against_a_dictionary_decodes_to_the_same_bytes() {
        use sha2::{Digest, Sha256};
        // Two pages that share their chrome, as a site's pages do.
        let chrome: String = (0..400)
            .map(|i| format!("<a href=\"/tag/t{}\">t{i}</a>", i * 7919 % 1000))
            .collect();
        let dictionary = format!("<!doctype html>{chrome}<h1>Global Feed</h1>").into_bytes();
        let body = format!("<!doctype html>{chrome}<h1>python</h1>").into_bytes();
        let hash: [u8; 32] = Sha256::digest(&dictionary).into();
        let made = against(&body, &dictionary, &hash).unwrap();
        assert_eq!(made[..4], [0xff, 0x44, 0x43, 0x42]);
        assert_eq!(made[4..36], hash);
        assert!(made.len() < compress(&body, Encoding::Br, true).unwrap().len());
        let mut decoded = Vec::new();
        brotli::Decompressor::new_with_custom_dict(&made[36..], 4096, dictionary.clone().into())
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, body);
        // A dictionary the window can't hold makes no variant.
        assert_eq!(against(&body, &vec![b'x'; 4 << 20], &hash), None);
    }

    #[test]
    fn accept_encoding_reads_as_rfc_9110_says() {
        let both = Accepts {
            br: true,
            gzip: true,
            dcb: false,
        };
        assert_eq!(Accepts::parse("gzip, deflate, br, zstd"), both);
        assert_eq!(Accepts::parse("*"), both);
        // A dictionary's coding is only ever named, never implied by `*`.
        assert!(Accepts::parse("gzip, deflate, br, zstd, dcb, dcz").dcb());
        assert!(!Accepts::parse("*").dcb() && !Accepts::parse("dcb;q=0").dcb());
        assert_eq!(
            Accepts::parse("br;q=0, gzip;q=0.5").pick(),
            Some(Encoding::Gzip)
        );
        assert_eq!(Accepts::parse("BR;Q=1").pick(), Some(Encoding::Br));
        assert_eq!(Accepts::parse("identity").pick(), None);
        assert_eq!(Accepts::parse("").pick(), None);
    }

    #[test]
    fn if_none_match_is_a_list_compared_weakly() {
        let etag = "\"abc-br\"";
        assert!(none_match(Some("\"abc-br\""), etag));
        assert!(none_match(Some("\"x\", W/\"abc-br\""), etag));
        assert!(none_match(Some("*"), etag));
        assert!(!none_match(Some("\"abc\""), etag));
        assert!(!none_match(None, etag));
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

    #[test]
    fn a_file_changed_after_bind_gets_its_variants_made() {
        let dir =
            std::env::temp_dir().join(format!("exact-render-variants-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app.js");
        std::fs::write(&file, "export const a = 'the same words';\n".repeat(200)).unwrap();
        let variants = Variants::default();
        let accepts = Accepts::parse("br");
        // Never warmed: the first answer goes as it is, and makes the variant.
        assert!(variants
            .serve(&file, accepts, true)
            .unwrap()
            .encoding
            .is_none());
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while variants
            .serve(&file, accepts, true)
            .unwrap()
            .encoding
            .is_none()
        {
            assert!(std::time::Instant::now() < until, "no variant made");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

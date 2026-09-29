//! A page sent in two parts (LLP 1071 D6, the early flush): the head of a
//! JavaScript page as the request arrives, before any of its data is asked
//! ([`crate::page::head_js`]), then the rest when the render is done
//! ([`crate::page::body_js`]), as one chunked, compressed response.
//!
//! Only a browser's navigation (`Sec-Fetch-Dest: document`) to a rendered
//! route that isn't the not-found one is sent this way, and never a CDN's
//! request, a conditional one, or a `HEAD`: those wait for the render and
//! get its status, validators and caching as before. Once the head has gone
//! the status has too, so a flushed page is `200` and `private, no-cache`
//! (`no-store` for a `request` route), with no ETag. What the render then
//! decides goes in the page instead:
//! - a head `status` (404, 410, 503) or a render at its deadline: the
//!   document the buffered answer would carry (at the deadline, with its
//!   placeholders and the checkpoint listing what is pending, which the
//!   runtime asks again), and the head's `robots` meta — never kept at the
//!   origin;
//! - a render that fails or panics: the same "couldn't be rendered" text a
//!   500 carries, after the head;
//! - a 200: kept at the origin as a buffered render is, so the next request
//!   is a hit with its ETag.
//!
//! A crawler, `curl` or a CDN sends no `Sec-Fetch-Dest`, so every client
//! that reads a status still gets the right one; the server's log line has
//! it for flushed pages too.
//!
//! 103 Early Hints: before a flushed page, and before a CDN's buffered
//! render, the entry's `modulepreload`s go as `Link` headers. Chrome acts
//! on a 103 only over HTTP/2 or later, so on this HTTP/1.1 server's own
//! connections it changes nothing (the flushed head names the same
//! preloads at the same moment); a CDN in front, speaking HTTP/2 to the
//! browser, can forward it while this server renders.

use crate::encode::Accepts;
use std::io::Write;
use std::net::TcpStream;

/// The entry's `modulepreload`s in `shell`, as absolute paths: the shell's
/// `<base href="/">` resolves `./app.js` to `/app.js`, where a `Link`
/// header would resolve it against the request's path.
pub(crate) fn preload_paths(shell: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = shell;
    while let Some(at) = rest.find("<link rel=\"modulepreload\" href=\"") {
        rest = &rest[at + "<link rel=\"modulepreload\" href=\"".len()..];
        let Some(end) = rest.find('"') else { break };
        let href = &rest[..end];
        if let Some(path) = href.strip_prefix("./") {
            if !path.contains(['<', '>', ',', ';', '\r', '\n', ' ']) {
                found.push(format!("/{path}"));
            }
        }
        rest = &rest[end..];
    }
    found
}

/// Send a 103 naming `paths` as `modulepreload`s. Best effort: a client
/// that closed is found by the response after it.
pub(crate) fn hint(out: &mut TcpStream, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    let mut head = String::from("HTTP/1.1 103 Early Hints\r\n");
    for path in paths {
        head.push_str("Link: <");
        head.push_str(path);
        head.push_str(">; rel=modulepreload\r\n");
    }
    head.push_str("\r\n");
    let _ = out.write_all(head.as_bytes());
}

/// Each write, one HTTP/1.1 chunk.
struct Chunked<'a>(&'a mut TcpStream);

impl Write for Chunked<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let mut frame = format!("{:x}\r\n", bytes.len()).into_bytes();
        frame.extend_from_slice(bytes);
        frame.extend_from_slice(b"\r\n");
        self.0.write_all(&frame)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

enum Encoder<'a> {
    Br(Box<brotli::CompressorWriter<Chunked<'a>>>),
    Gzip(flate2::write::GzEncoder<Chunked<'a>>),
    Plain(Chunked<'a>),
}

/// A page on its way: the headers and the head have gone.
pub(crate) struct Flush<'a> {
    encoder: Encoder<'a>,
    failed: bool,
}

impl<'a> Flush<'a> {
    /// Send a 200's headers — `cache` its `Cache-Control`, compressed as
    /// `accepts` allows (brotli at the quality a page is compressed at as
    /// it is sent, else gzip) — and `head`, flushed through the encoder.
    pub(crate) fn open(
        out: &'a mut TcpStream,
        head: &str,
        cache: &str,
        accepts: Accepts,
        csp: &str,
        permissions: &str,
        keep: bool,
    ) -> Flush<'a> {
        let encoding = accepts.pick();
        let mut headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: {cache}\r\nVary: Accept-Encoding\r\n"
        );
        if let Some(encoding) = encoding {
            headers.push_str("Content-Encoding: ");
            headers.push_str(encoding.name());
            headers.push_str("\r\n");
        }
        headers.push_str("Content-Security-Policy: ");
        headers.push_str(csp);
        headers.push_str("\r\n");
        if !permissions.is_empty() {
            headers.push_str("Permissions-Policy: ");
            headers.push_str(permissions);
            headers.push_str("\r\n");
        }
        headers.push_str("X-Content-Type-Options: nosniff\r\nReferrer-Policy: strict-origin-when-cross-origin\r\nTransfer-Encoding: chunked\r\n");
        headers.push_str(if keep {
            "Connection: keep-alive\r\n\r\n"
        } else {
            "Connection: close\r\n\r\n"
        });
        let failed = out.write_all(headers.as_bytes()).is_err();
        let chunked = Chunked(out);
        let encoder = match encoding {
            Some(crate::encode::Encoding::Br) => Encoder::Br(Box::new(
                brotli::CompressorWriter::new(chunked, 4096, 5, 22),
            )),
            Some(crate::encode::Encoding::Gzip) => Encoder::Gzip(flate2::write::GzEncoder::new(
                chunked,
                flate2::Compression::new(6),
            )),
            None => Encoder::Plain(chunked),
        };
        let mut flush = Flush { encoder, failed };
        flush.send(head.as_bytes());
        flush
    }

    /// Send `bytes` now: written and flushed through the encoder.
    pub(crate) fn send(&mut self, bytes: &[u8]) {
        if self.failed {
            return;
        }
        let sent = match &mut self.encoder {
            Encoder::Br(w) => w.write_all(bytes).and_then(|()| w.flush()),
            Encoder::Gzip(w) => w.write_all(bytes).and_then(|()| w.flush()),
            Encoder::Plain(w) => w.write_all(bytes).and_then(|()| w.flush()),
        };
        self.failed = sent.is_err();
    }

    /// End the encoding and the chunked body.
    pub(crate) fn end(self) {
        if self.failed {
            return;
        }
        let chunked = match self.encoder {
            Encoder::Br(w) => Some(w.into_inner()),
            Encoder::Gzip(w) => w.finish().ok(),
            Encoder::Plain(w) => Some(w),
        };
        if let Some(Chunked(out)) = chunked {
            let _ = out.write_all(b"0\r\n\r\n");
            let _ = out.flush();
        }
    }
}

/// How long a kept connection may wait for its next request.
const IDLE: std::time::Duration = std::time::Duration::from_secs(2);

/// Wait on a kept connection (HTTP/1.1 keep-alive) for the client's next
/// request: true once its first byte is here; false when the client
/// closes, [`IDLE`] passes, the server drains, or another connection waits
/// for a worker (an idle connection never holds a worker a new one needs).
pub(crate) fn idle(
    stream: &TcpStream,
    waiting: &std::sync::Mutex<(std::collections::VecDeque<TcpStream>, usize)>,
    stop: &std::sync::atomic::AtomicBool,
) -> bool {
    use std::io::ErrorKind::{TimedOut, WouldBlock};
    let until = std::time::Instant::now() + IDLE;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(10)));
    let mut byte = [0u8];
    while std::time::Instant::now() < until {
        match stream.peek(&mut byte) {
            Ok(0) => return false,
            Ok(_) => return true,
            Err(e) if matches!(e.kind(), WouldBlock | TimedOut) => {
                if stop.load(std::sync::atomic::Ordering::SeqCst)
                    || !waiting.lock().unwrap().0.is_empty()
                {
                    return false;
                }
            }
            Err(_) => return false,
        }
    }
    false
}

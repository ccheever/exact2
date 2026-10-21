//! A development transport, and a placeholder for the platform's.
//!
//! LLP 0057 §3 gives the platform sockets, TLS, proxy configuration, HTTP/2
//! and /3, connection pooling, and the system certificate store. This is none
//! of that: it is plaintext HTTP/1.1 over a raw socket, one connection per
//! request, so the semantics above it can be exercised end to end before any
//! platform binding exists.
//!
//! **It does not speak TLS, and that is deliberate rather than unfinished.** A
//! hand-rolled TLS client is exactly the kind of thing LLP 0059.000 §3.4 warns
//! about for URL parsing — a partial implementation of a security-critical
//! protocol is a vulnerability, not a missing feature. `https` is refused here
//! and belongs to `NSURLSession` on Apple platforms and the equivalent
//! elsewhere.
//!
//! @ref LLP 0057#3-the-boundary — the platform owns transport; this stands in for it

use crate::boundary::HostError;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::fetch::{
    over_limit, Body, BodySource, Headers, Request, StreamingResponse, Transport,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct DevTcpTransport {
    pub timeout: Option<Duration>,
}
impl DevTcpTransport {
    pub fn new() -> Self {
        Self {
            timeout: Some(Duration::from_secs(10)),
        }
    }
}
const MAX_HEAD: usize = 64 * 1024;
fn failed(error: impl std::fmt::Display) -> HostError {
    HostError::Failed(format!("TypeError: Failed to fetch — {error}"))
}
fn line(reader: &mut BufReader<TcpStream>, budget: &mut usize) -> Result<Vec<u8>, HostError> {
    let mut line = Vec::new();
    reader
        .take(budget.saturating_add(1) as u64)
        .read_until(b'\n', &mut line)
        .map_err(failed)?;
    if line.len() > *budget {
        return Err(failed("response head exceeded the limit"));
    }
    *budget -= line.len();
    if !line.ends_with(b"\r\n") {
        return Err(failed("truncated HTTP framing"));
    }
    line.truncate(line.len() - 2);
    Ok(line)
}
enum Framing {
    Length(u64),
    Chunked { remaining: u64, separator: bool },
    Eof,
    Done,
}
struct Source {
    reader: BufReader<TcpStream>,
    framing: Framing,
    _registration: AbortRegistration,
}
impl Drop for Source {
    fn drop(&mut self) {
        let _ = self.reader.get_ref().shutdown(Shutdown::Both);
    }
}
impl BodySource for Source {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, HostError> {
        if out.is_empty() {
            return Ok(0);
        }
        let maximum = match &mut self.framing {
            Framing::Done | Framing::Length(0) => return Ok(0),
            Framing::Length(n) => (*n).min(out.len() as u64) as usize,
            Framing::Eof => out.len(),
            Framing::Chunked {
                remaining,
                separator,
            } => {
                if *remaining == 0 {
                    let mut budget = MAX_HEAD;
                    if *separator && !line(&mut self.reader, &mut budget)?.is_empty() {
                        return Err(failed("malformed chunk separator"));
                    }
                    let size = line(&mut self.reader, &mut budget)?;
                    let size = std::str::from_utf8(&size)
                        .map_err(failed)?
                        .split(';')
                        .next()
                        .unwrap();
                    if size.is_empty() || !size.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(failed("malformed chunk size"));
                    }
                    *remaining = u64::from_str_radix(size, 16).map_err(failed)?;
                    *separator = true;
                    if *remaining == 0 {
                        while !line(&mut self.reader, &mut budget)?.is_empty() {}
                        self.framing = Framing::Done;
                        return Ok(0);
                    }
                }
                (*remaining).min(out.len() as u64) as usize
            }
        };
        let n = self.reader.read(&mut out[..maximum]).map_err(failed)?;
        match &mut self.framing {
            Framing::Length(left)
            | Framing::Chunked {
                remaining: left, ..
            } => {
                if n == 0 {
                    return Err(failed("truncated response body"));
                }
                *left -= n as u64;
            }
            _ => {}
        }
        Ok(n)
    }
}
impl Transport for DevTcpTransport {
    fn open(
        &self,
        request: &Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        signal.check()?;
        let url = url::Url::parse(&request.url).map_err(failed)?;
        if url.scheme() != "http" {
            return Err(failed(
                "the development transport speaks plaintext http only",
            ));
        }
        let host = url.host_str().ok_or_else(|| failed("URL has no host"))?;
        let port = url.port_or_known_default().unwrap_or(80);
        let mut stream = TcpStream::connect((host, port)).map_err(failed)?;
        stream
            .set_read_timeout(self.timeout)
            .and_then(|_| stream.set_write_timeout(self.timeout))
            .map_err(failed)?;
        let socket = stream.try_clone().map_err(failed)?;
        let registration = signal.register(move || {
            let _ = socket.shutdown(Shutdown::Both);
        });
        signal.check()?;
        let path = match url.query() {
            Some(q) => format!("{}?{q}", url.path()),
            None => url.path().to_owned(),
        };
        let authority = &url[url::Position::BeforeHost..url::Position::AfterPort];
        let mut head = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            request.method, path, authority
        );
        for (name, value) in request.headers.entries() {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        if let Some(body) = &request.body {
            head.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        head.push_str("\r\n");
        stream
            .write_all(head.as_bytes())
            .and_then(|_| match &request.body {
                Some(body) => stream.write_all(body),
                None => Ok(()),
            })
            .map_err(failed)?;
        let mut reader = BufReader::with_capacity(16 * 1024, stream);
        let mut budget = MAX_HEAD;
        let (status, status_text, headers, length, chunked) = loop {
            let status_line = line(&mut reader, &mut budget)?;
            let status_line = std::str::from_utf8(&status_line).map_err(failed)?;
            let mut parts = status_line.splitn(3, ' ');
            if !matches!(parts.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
                return Err(failed("bad HTTP version"));
            }
            let status: u16 = parts
                .next()
                .and_then(|s| s.parse().ok())
                .filter(|n| (100..600).contains(n))
                .ok_or_else(|| failed("bad status"))?;
            let status_text = parts.next().unwrap_or("").to_owned();
            let mut headers = Headers::new();
            let mut length = None;
            let mut chunked = false;
            loop {
                let bytes = line(&mut reader, &mut budget)?;
                if bytes.is_empty() {
                    break;
                }
                let text = std::str::from_utf8(&bytes).map_err(failed)?;
                let (name, value) = text
                    .split_once(':')
                    .ok_or_else(|| failed("malformed header"))?;
                if name.eq_ignore_ascii_case("content-length") {
                    let n: u64 = value.trim().parse().map_err(failed)?;
                    if length.is_some_and(|old| old != n) {
                        return Err(failed("conflicting Content-Length"));
                    }
                    length = Some(n);
                }
                if name.eq_ignore_ascii_case("transfer-encoding") {
                    if chunked || !value.trim().eq_ignore_ascii_case("chunked") {
                        return Err(failed("unsupported transfer encoding"));
                    }
                    chunked = true;
                }
                headers.set_response(name, value);
            }
            if status == 101 {
                return Err(failed("protocol upgrades are unsupported"));
            }
            if status >= 200 {
                break (status, status_text, headers, length, chunked);
            }
        };
        signal.check()?;
        let no_body = request.method == "HEAD" || matches!(status, 204 | 304);
        let limit = request.body_limit();
        if !no_body && !chunked && length.is_some_and(|n| n > limit as u64) {
            return Err(over_limit(limit));
        }
        if chunked && length.is_some() {
            return Err(failed("ambiguous HTTP framing"));
        }
        let framing = if no_body {
            Framing::Done
        } else if chunked {
            Framing::Chunked {
                remaining: 0,
                separator: false,
            }
        } else {
            length.map(Framing::Length).unwrap_or(Framing::Eof)
        };
        Ok(StreamingResponse {
            status,
            status_text,
            headers,
            body: Body::new(
                Box::new(Source {
                    reader,
                    framing,
                    _registration: registration,
                }),
                limit,
                signal.clone(),
            ),
            url: request.url.clone(),
            redirected: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn https_is_refused_rather_than_downgraded() {
        let error = DevTcpTransport::new()
            .send(&Request::get("https://example.com/"))
            .unwrap_err();
        assert!(error.to_string().contains("plaintext http only"));
    }
}

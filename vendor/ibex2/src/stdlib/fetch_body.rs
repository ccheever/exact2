//! Demand-driven response bodies. No executor or buffering task is installed.
use super::abort::AbortSignal;
use super::fetch::{over_limit, Headers, Response};
use crate::boundary::HostError;

pub trait BodySource: Send {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, HostError>;
}
impl BodySource for std::io::Cursor<Vec<u8>> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, HostError> {
        std::io::Read::read(self, output).map_err(|e| HostError::Failed(e.to_string()))
    }
}

pub struct Body {
    source: Option<Box<dyn BodySource>>,
    signal: AbortSignal,
    remaining: usize,
    limit: usize,
    error: Option<HostError>,
}
impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body")
            .field("remaining", &self.remaining)
            .field("closed", &self.source.is_none())
            .finish()
    }
}
impl Body {
    pub fn new(source: Box<dyn BodySource>, limit: usize, signal: AbortSignal) -> Self {
        Self {
            source: Some(source),
            signal,
            remaining: limit,
            limit,
            error: None,
        }
    }
    pub fn read(&mut self, output: &mut [u8]) -> Result<usize, HostError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self.source.is_none() || output.is_empty() {
            return Ok(0);
        }
        let result = (|| {
            self.signal.check()?;
            let length = output.len().min(self.remaining.saturating_add(1));
            let result = self.source.as_mut().unwrap().read(&mut output[..length]);
            self.signal.check()?;
            let n = result?;
            if n > self.remaining || n > length {
                return Err(over_limit(self.limit));
            }
            self.remaining -= n;
            Ok(n)
        })();
        match result {
            Ok(0) => {
                self.source.take();
            }
            Err(ref error) => {
                self.error = Some(error.clone());
                self.source.take();
            }
            _ => {}
        }
        result
    }
    pub fn cancel(&mut self) {
        self.source.take();
    }
    pub fn collect(mut self) -> Result<Vec<u8>, HostError> {
        let mut result = Vec::new();
        let mut chunk = [0; 16 * 1024];
        loop {
            let n = self.read(&mut chunk)?;
            if n == 0 {
                return Ok(result);
            }
            result.extend_from_slice(&chunk[..n]);
        }
    }
}

#[derive(Debug)]
pub struct StreamingResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Headers,
    pub body: Body,
    pub url: String,
    pub redirected: bool,
}
impl StreamingResponse {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    pub fn collect(self) -> Result<Response, HostError> {
        Ok(Response {
            status: self.status,
            status_text: self.status_text,
            headers: self.headers,
            body: self.body.collect()?,
            url: self.url,
            redirected: self.redirected,
        })
    }
}
impl Response {
    /// An already-owned body, useful for in-memory transports.
    pub fn into_stream(self, limit: usize, signal: AbortSignal) -> StreamingResponse {
        StreamingResponse {
            status: self.status,
            status_text: self.status_text,
            headers: self.headers,
            body: Body::new(Box::new(std::io::Cursor::new(self.body)), limit, signal),
            url: self.url,
            redirected: self.redirected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::abort::AbortController;
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Probe {
        read: Arc<AtomicUsize>,
        dropped: Arc<AtomicUsize>,
    }
    impl BodySource for Probe {
        fn read(&mut self, out: &mut [u8]) -> Result<usize, HostError> {
            self.read.fetch_add(out.len(), Ordering::SeqCst);
            out.fill(42);
            Ok(out.len())
        }
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn quota_stops_at_one_byte_past_limit_and_releases_source() {
        let read = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut body = Body::new(
            Box::new(Probe {
                read: read.clone(),
                dropped: dropped.clone(),
            }),
            3,
            AbortSignal::default(),
        );
        assert_eq!(read.load(Ordering::SeqCst), 0);
        assert_eq!(body.read(&mut [0; 2]).unwrap(), 2);
        let error = body.read(&mut [0; 100]).unwrap_err();
        assert_eq!(error, over_limit(3));
        assert_eq!(body.read(&mut [0; 1]).unwrap_err(), error);
        assert_eq!(read.load(Ordering::SeqCst), 4);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn empty_read_is_not_eof_and_exact_limit_can_close() {
        let mut body = Body::new(
            Box::new(std::io::Cursor::new(vec![1, 2])),
            2,
            AbortSignal::default(),
        );
        assert_eq!(body.read(&mut []).unwrap(), 0);
        assert_eq!(body.collect().unwrap(), vec![1, 2]);
    }
    #[test]
    fn abort_before_read_and_cancel_release_unread_body() {
        let controller = AbortController::new();
        let read = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut body = Body::new(
            Box::new(Probe {
                read: read.clone(),
                dropped: dropped.clone(),
            }),
            3,
            controller.signal(),
        );
        controller.abort();
        assert!(body
            .read(&mut [0; 1])
            .unwrap_err()
            .to_string()
            .starts_with("AbortError"));
        assert_eq!(read.load(Ordering::SeqCst), 0);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        let mut body = Body::new(
            Box::new(Probe {
                read,
                dropped: dropped.clone(),
            }),
            3,
            AbortSignal::default(),
        );
        body.cancel();
        assert_eq!(dropped.load(Ordering::SeqCst), 2);
        assert_eq!(body.read(&mut [0; 1]).unwrap(), 0);
    }
    #[test]
    fn abort_after_eof_does_not_reopen_closed_body() {
        let controller = AbortController::new();
        let mut body = Body::new(
            Box::new(std::io::Cursor::new(vec![])),
            0,
            controller.signal(),
        );
        assert_eq!(body.read(&mut [0; 1]).unwrap(), 0);
        controller.abort();
        assert_eq!(body.read(&mut [0; 1]).unwrap(), 0);
    }
}

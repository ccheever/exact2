//! Network transfers have their own bounded workers; a slow server cannot hold
//! either raster decoder. Encoded data is spooled, never collected in RAM.
use super::{Backend, Prepared, SourceOwner};
use exact_raster::Refusal;
use ibex2::stdlib::{
    abort::AbortSignal,
    fetch::{Request, Transport},
};
use std::{
    collections::VecDeque,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex, OnceLock, Weak,
    },
    time::{Duration, Instant},
};

const QUEUED: usize = 64;
const DISK_BYTES: u64 = 64 * 1024 * 1024;
static USED: AtomicU64 = AtomicU64::new(0);

pub(super) struct File {
    pub path: PathBuf,
    bytes: u64,
}
impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        USED.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}
struct Job {
    source: Weak<SourceOwner>,
    backend: Weak<Backend>,
    url: String,
    signal: AbortSignal,
}
struct Downloads {
    queue: Mutex<VecDeque<Job>>,
    ready: Condvar,
}
impl Downloads {
    fn shared() -> &'static Arc<Self> {
        static INSTANCE: OnceLock<Arc<Downloads>> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let pool = Arc::new(Self {
                queue: Mutex::new(VecDeque::new()),
                ready: Condvar::new(),
            });
            for n in 0..2 {
                let pool = pool.clone();
                std::thread::Builder::new()
                    .name(format!("exact-image-http-{n}"))
                    .spawn(move || pool.run())
                    .expect("image download worker");
            }
            pool
        })
    }
    fn run(&self) {
        let transport = ibex2::transport::default_transport();
        loop {
            let job = {
                let mut queue = self.queue.lock().unwrap();
                loop {
                    if let Some(job) = queue.pop_front() {
                        break job;
                    }
                    queue = self.ready.wait(queue).unwrap();
                }
            };
            if job.source.strong_count() == 0 || job.signal.aborted() {
                continue;
            }
            let result = download(&*transport, &job.url, &job.signal);
            let (Some(source), Some(backend)) = (job.source.upgrade(), job.backend.upgrade())
            else {
                continue;
            };
            *source.prepared.lock().unwrap() = match result {
                Ok(file) => {
                    *source.remote.lock().unwrap() = Some(file);
                    Prepared::Queued
                }
                Err(reason) => Prepared::Failed(reason),
            };
            backend.session.wake();
            backend.notify();
        }
    }
}

pub(super) fn enqueue(source: &Arc<SourceOwner>, backend: &Arc<Backend>) -> Result<(), Refusal> {
    let pool = Downloads::shared();
    let mut queue = pool.queue.lock().unwrap();
    queue.retain(|job| job.source.strong_count() > 0 && !job.signal.aborted());
    if queue.len() == QUEUED {
        return Err(Refusal::QueueFull);
    }
    queue.push_back(Job {
        source: Arc::downgrade(source),
        backend: Arc::downgrade(backend),
        url: source.name.clone(),
        signal: source.cancel.signal(),
    });
    pool.ready.notify_one();
    Ok(())
}

fn download(transport: &dyn Transport, url: &str, signal: &AbortSignal) -> Result<File, Refusal> {
    let start = Instant::now();
    let mut url = url.to_owned();
    for redirect in 0..=20 {
        signal.check().map_err(|_| Refusal::Stale)?;
        let parsed = ibex2::stdlib::url::parse(&url, None).map_err(|_| Refusal::DecodeFailed)?;
        if !matches!(parsed.protocol.as_str(), "http:" | "https:")
            || !parsed.username.is_empty()
            || !parsed.password.is_empty()
        {
            return Err(Refusal::DecodeFailed);
        }
        let remaining = Duration::from_secs(30)
            .checked_sub(start.elapsed())
            .ok_or(Refusal::DecodeFailed)?;
        let mut request = Request::get(&parsed.href);
        request.max_body = Some(exact_raster::MAX_ENCODED_BYTES as usize);
        request.timeout = Some(remaining.min(Duration::from_secs(15)));
        let mut response = transport
            .open(&request, signal)
            .map_err(|_| Refusal::DecodeFailed)?;
        if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
            if redirect == 20 {
                return Err(Refusal::DecodeFailed);
            }
            let location = response
                .headers
                .get("location")
                .ok_or(Refusal::DecodeFailed)?;
            url = ibex2::stdlib::url::parse(location, Some(&parsed.href))
                .map_err(|_| Refusal::DecodeFailed)?
                .href;
            continue;
        }
        if !response.ok() {
            return Err(Refusal::DecodeFailed);
        }
        if response
            .headers
            .get("content-length")
            .and_then(|n| n.parse::<u64>().ok())
            .is_some_and(|n| n > exact_raster::MAX_ENCODED_BYTES)
        {
            return Err(Refusal::EncodedLimit);
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| Refusal::DecodeFailed)?;
        let path =
            std::env::temp_dir().join(format!("exact-image-{:032x}", u128::from_le_bytes(random)));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(&path).map_err(|_| Refusal::DecodeFailed)?;
        let mut file = File { path, bytes: 0 };
        let result = spool(&mut response.body, &mut output, &mut file);
        // Close before unlinking an incomplete transfer on Windows.
        drop(output);
        result?;
        return Ok(file);
    }
    Err(Refusal::DecodeFailed)
}

fn spool(
    body: &mut ibex2::stdlib::fetch::Body,
    output: &mut std::fs::File,
    file: &mut File,
) -> Result<(), Refusal> {
    let mut chunk = [0; 16 * 1024];
    loop {
        let n = body.read(&mut chunk).map_err(|_| Refusal::DecodeFailed)?;
        if n == 0 {
            return Ok(());
        }
        if file.bytes + n as u64 > exact_raster::MAX_ENCODED_BYTES {
            return Err(Refusal::EncodedLimit);
        }
        USED.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
            (used + n as u64 <= DISK_BYTES).then_some(used + n as u64)
        })
        .map_err(|_| Refusal::Budget)?;
        file.bytes += n as u64;
        output
            .write_all(&chunk[..n])
            .map_err(|_| Refusal::DecodeFailed)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibex2::{
        boundary::HostError,
        stdlib::fetch::{Body, Headers, StreamingResponse},
    };

    struct Server {
        status: u16,
        headers: Headers,
        requests: Mutex<Vec<Request>>,
    }
    impl Transport for Server {
        fn open(
            &self,
            request: &Request,
            signal: &AbortSignal,
        ) -> Result<StreamingResponse, HostError> {
            self.requests.lock().unwrap().push(request.clone());
            Ok(StreamingResponse {
                status: self.status,
                status_text: String::new(),
                headers: self.headers.clone(),
                body: Body::new(
                    Box::new(std::io::Cursor::new(vec![1, 2, 3])),
                    request.body_limit(),
                    signal.clone(),
                ),
                url: request.url.clone(),
                redirected: false,
            })
        }
    }
    #[test]
    fn remote_failures_are_bounded_and_never_forward_credentials() {
        let mut server = Server {
            status: 200,
            headers: Headers::new(),
            requests: Mutex::new(Vec::new()),
        };
        let signal = AbortSignal::default();
        for url in ["file:///photo", "https://user:secret@example.test/photo"] {
            assert!(matches!(
                download(&server, url, &signal),
                Err(Refusal::DecodeFailed)
            ));
        }
        assert!(server.requests.lock().unwrap().is_empty());
        server.headers.set(
            "content-length",
            &(exact_raster::MAX_ENCODED_BYTES + 1).to_string(),
        );
        assert!(matches!(
            download(&server, "https://example.test/photo", &signal),
            Err(Refusal::EncodedLimit)
        ));
        server.headers = Headers::new();
        server.status = 404;
        assert!(matches!(
            download(&server, "https://example.test/photo", &signal),
            Err(Refusal::DecodeFailed)
        ));
        server.status = 302;
        server.headers.set("location", "/loop");
        assert!(matches!(
            download(&server, "https://example.test/photo", &signal),
            Err(Refusal::DecodeFailed)
        ));
        assert_eq!(server.requests.lock().unwrap().len(), 23);
        for request in server.requests.lock().unwrap().iter() {
            assert_eq!(
                request.max_body,
                Some(exact_raster::MAX_ENCODED_BYTES as usize)
            );
            assert!(request.timeout.unwrap() <= Duration::from_secs(15));
            for header in ["authorization", "cookie", "referer"] {
                assert!(request.headers.get(header).is_none());
            }
        }
        let cancel = ibex2::stdlib::abort::AbortController::new();
        cancel.abort();
        assert!(matches!(
            download(&server, "https://example.test/photo", &cancel.signal()),
            Err(Refusal::Stale)
        ));
        assert_eq!(server.requests.lock().unwrap().len(), 23);
    }
}

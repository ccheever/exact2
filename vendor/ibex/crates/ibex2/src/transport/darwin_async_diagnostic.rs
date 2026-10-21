//! Optional local transport comparison. Each mode runs in a fresh process;
//! process RSS/threads include the identical local server fixture.
use super::DarwinLane;
use crate::boundary::HostError;
use crate::grant::{Grant, GrantSet};
use crate::stdlib::abort::AbortSignal;
use crate::stdlib::fetch::{origin_of, Request, Transport};
use crate::stdlib::fetch_async::{Collected, FetchEvents, FetchTask, HeaderCaps};
use crate::transport::DarwinTransport;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

const REQUESTS: usize = 24;
const CONCURRENCY: usize = 4;
const PAYLOAD: usize = 65536;
const TEST_NAME: &str =
    "transport::darwin_async::diagnostic::compare_native_and_blocking_transport";

fn resources() -> (u64, i32) {
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::uninit();
    let size = std::mem::size_of::<libc::proc_taskinfo>();
    // SAFETY: proc_pidinfo writes exactly the described sized output buffer.
    let written = unsafe {
        libc::proc_pidinfo(
            std::process::id() as i32,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size as i32,
        )
    };
    assert_eq!(written as usize, size);
    // SAFETY: the successful call initialized the complete structure.
    let info = unsafe { info.assume_init() };
    (info.pti_resident_size, info.pti_threadnum)
}

struct Completed(mpsc::Sender<f64>, Instant);
impl FetchEvents for Completed {
    fn outcome(&self, result: Result<Collected, HostError>) {
        assert_eq!(result.unwrap().body.len(), PAYLOAD);
        self.0
            .send(self.1.elapsed().as_secs_f64() * 1000.0)
            .unwrap();
    }
}

#[test]
#[ignore = "local diagnostic: no timing or resource regression threshold"]
fn compare_native_and_blocking_transport() {
    let Ok(mode) = std::env::var("IBEX_HTTP_DIAGNOSTIC_MODE") else {
        for mode in ["blocking", "async"] {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    TEST_NAME,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("IBEX_HTTP_DIAGNOSTIC_MODE", mode)
                .output()
                .unwrap();
            println!("{}", String::from_utf8_lossy(&result.stdout));
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        return;
    };
    assert!(matches!(mode.as_str(), "blocking" | "async"));
    let baseline = resources();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let mut peers = Vec::new();
        for _ in 0..REQUESTS {
            let (mut socket, _) = listener.accept().unwrap();
            peers.push(std::thread::spawn(move || {
                socket.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                std::thread::sleep(Duration::from_millis(30));
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {PAYLOAD}\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
                socket.write_all(&vec![b'x'; PAYLOAD]).unwrap();
            }));
        }
        for peer in peers {
            peer.join().unwrap();
        }
    });
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    let mut workers = Vec::new();
    let mut tasks = Vec::new();
    if mode == "blocking" {
        let transport = Arc::new(DarwinTransport::new());
        let next = Arc::new(AtomicUsize::new(0));
        for _ in 0..CONCURRENCY {
            let transport = transport.clone();
            let next = next.clone();
            let url = url.clone();
            let tx = tx.clone();
            workers.push(std::thread::spawn(move || {
                while next.fetch_add(1, Ordering::Relaxed) < REQUESTS {
                    let mut request = Request::get(&url);
                    request.max_body = Some(PAYLOAD);
                    assert_eq!(transport.send(&request).unwrap().body.len(), PAYLOAD);
                    tx.send(started.elapsed().as_secs_f64() * 1000.0).unwrap();
                }
            }));
        }
    } else {
        let transport = Arc::new(DarwinLane::new(CONCURRENCY));
        let grants = Arc::new(GrantSet::none().with(Grant::Fetch(origin_of(&url).unwrap())));
        for _ in 0..REQUESTS {
            let mut request = Request::get(&url);
            request.max_body = Some(PAYLOAD);
            tasks.push(FetchTask::start(
                transport.clone(),
                grants.clone(),
                request,
                None,
                AbortSignal::default(),
                HeaderCaps {
                    bytes: 65536,
                    entries: 128,
                },
                Arc::new(Completed(tx.clone(), started)),
            ));
        }
    }
    drop(tx);
    let mut completed = Vec::new();
    let mut peak = baseline;
    while completed.len() < REQUESTS {
        let sample = resources();
        peak.0 = peak.0.max(sample.0);
        peak.1 = peak.1.max(sample.1);
        match rx.recv_timeout(Duration::from_millis(1)) {
            Ok(elapsed) => completed.push(elapsed),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                assert!(started.elapsed() < Duration::from_secs(20))
            }
            Err(error) => panic!("missing outcome: {error}"),
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    completed.sort_by(f64::total_cmp);
    println!("http-diagnostic mode={mode} requests={REQUESTS} concurrency={CONCURRENCY} body={PAYLOAD} elapsed_ms={:.2} throughput_rps={:.2} batch_completion_p50_ms={:.2} batch_completion_p95_ms={:.2} baseline_rss={} sampled_peak_rss={} baseline_threads={} sampled_peak_threads={}", seconds * 1000.0, REQUESTS as f64 / seconds, completed[REQUESTS / 2], completed[(REQUESTS * 95 / 100).min(REQUESTS - 1)], baseline.0, peak.0, baseline.1, peak.1);
    for worker in workers {
        worker.join().unwrap();
    }
    server.join().unwrap();
    drop(tasks);
}

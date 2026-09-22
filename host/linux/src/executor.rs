//! The executor (LLP 1016 D2, Linux): `ibex2::host` on a worker thread.
//!
//! The Apple host's executor (`host/apple/src/executor.rs`) with the wake
//! the display loop can poll: one byte on a socketpair per reply, the
//! reading end in the loop's `poll` set beside the input devices and the
//! VNC server's, so an idle process stays parked and a reply wakes it. The
//! headless agent path has no loop; it pumps while it waits (`agent.rs`).
//! Requests run one at a time, in order, on `ibex2`'s default transport —
//! rustls off Apple, `NSURLSession` when this host runs on a Mac.

use exact_runner::{FailureKind, Outcome, Request, RequestOut, Response};
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Receiver, Sender};

struct Job {
    ticket: u64,
    request: Request,
    forced: bool,
    work: Option<Box<dyn FnOnce() -> Outcome + Send>>,
}

/// One worker thread, its two queues, and the wake the loop polls.
pub struct Executor {
    jobs: Sender<Job>,
    outcomes: Receiver<(u64, Outcome)>,
    wake: UnixStream,
    note: Option<String>,
}

/// The platform transport, and what to say about it: off Apple, rustls with
/// the machine's trust store (or the compiled-in roots where it has none —
/// worth a journal line); on a Mac, `NSURLSession`, which says nothing.
fn transport() -> (ibex2::host::Host, Option<String>) {
    #[cfg(not(target_vendor = "apple"))]
    {
        let t = ibex2::transport::RustlsHttpTransport::new();
        let note = format!("trust roots: {}", t.roots());
        (ibex2::host::Host::with_transport(Box::new(t)), Some(note))
    }
    #[cfg(target_vendor = "apple")]
    {
        (ibex2::host::Host::new(), None)
    }
}

impl Executor {
    /// Start the worker with the app's grants (LLP 1016 D6).
    pub fn start(grants: &str) -> Executor {
        let (jobs, job_rx) = channel::<Job>();
        let (outcome_tx, outcomes) = channel();
        let (wake, mut signal) = UnixStream::pair().expect("a socketpair for the executor's wake");
        wake.set_nonblocking(true)
            .expect("the wake's reading end non-blocking");
        let grants = grants.to_string();
        let (host, note) = transport();
        std::thread::Builder::new()
            .name("exact-executor".into())
            .spawn(move || {
                let bindings = ibex2::grant::GrantSet::parse(&exact_runner::io_grants(&grants))
                    .ok()
                    .map(|g| host.endow(g));
                for job in job_rx {
                    let scoped = job.request.grants.as_deref().map(|scope| {
                        exact_data::storage::scope(&grants, Some(scope))
                            .and_then(|s| {
                                ibex2::grant::GrantSet::parse(&exact_runner::io_grants(s))
                                    .map_err(|e| e.to_string())
                            })
                            .map(|g| ibex2::host::Host::new().endow(g))
                    });
                    let outcome = match scoped {
                        Some(Err(message)) => Outcome::Failed {
                            kind: FailureKind::Refused,
                            message,
                        },
                        Some(Ok(ref b)) => run(Some(b), job.request, job.forced, job.work),
                        None => run(bindings.as_ref(), job.request, job.forced, job.work),
                    };
                    if outcome_tx.send((job.ticket, outcome)).is_err() {
                        break;
                    }
                    let _ = signal.write_all(&[1]);
                }
            })
            .expect("the executor thread");
        Executor {
            jobs,
            outcomes,
            wake,
            note,
        }
    }

    /// What the executor has to say about its transport at start (the
    /// trust store it found, off Apple), for the journal.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// The wake's reading end: readable when a reply is queued.
    pub fn fd(&self) -> RawFd {
        self.wake.as_raw_fd()
    }

    /// Hand a request to the worker.
    pub fn run(&self, r: RequestOut, work: Option<Box<dyn FnOnce() -> Outcome + Send>>) {
        let _ = self.jobs.send(Job {
            ticket: r.ticket,
            request: r.request,
            forced: r.forced,
            work,
        });
    }

    /// Every outcome queued since the last drain, oldest first; the wake
    /// bytes go with them.
    pub fn drain(&self) -> Vec<(u64, Outcome)> {
        let mut buf = [0u8; 64];
        while let Ok(n) = (&self.wake).read(&mut buf) {
            if n == 0 {
                break;
            }
        }
        self.outcomes.try_iter().collect()
    }
}

fn run(
    bindings: Option<&ibex2::host::Bindings>,
    request: Request,
    forced: bool,
    work: Option<Box<dyn FnOnce() -> Outcome + Send>>,
) -> Outcome {
    if request.storage.is_some() {
        return Outcome::Failed {
            kind: FailureKind::Unsupported,
            message: "storage requires an app storage adapter".into(),
        };
    }
    if request.continuation.is_some() {
        return work.map_or_else(
            || Outcome::Failed {
                kind: FailureKind::Unsupported,
                message: "missing or consumed native continuation".into(),
            },
            |work| work(),
        );
    }
    let Some(b) = bindings else {
        return Outcome::Failed {
            kind: FailureKind::Refused,
            message: "the app declares no grants".into(),
        };
    };
    let mut req = ibex2::stdlib::fetch::Request::get(&request.url);
    req.method = request.method;
    for (k, v) in &request.headers {
        req.headers.append(k, v);
    }
    if forced {
        req.headers.set("cache-control", "no-cache");
    }
    if !request.body.is_empty() {
        req.body = Some(request.body);
    }
    match b.fetch.send(req) {
        Ok(r) => Outcome::Response(Response {
            status: r.status,
            headers: r.headers.entries().to_vec(),
            body: r.body,
        }),
        Err(ibex2::boundary::HostError::Denied { capability }) => Outcome::Failed {
            kind: FailureKind::Refused,
            message: format!("outside the app's grants ({capability})"),
        },
        Err(e) => Outcome::Failed {
            kind: FailureKind::Network,
            message: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn continuations_run_on_worker_and_missing_tokens_refuse_without_network() {
        let executor = Executor::start("");
        let renderer = std::thread::current().id();
        let (entered, on_worker) = channel();
        let (release, wait) = channel();
        executor.run(
            RequestOut {
                ticket: 7,
                target: "storage".into(),
                request: Request::continuation(12),
                forced: false,
            },
            Some(Box::new(move || {
                entered.send(std::thread::current().id()).unwrap();
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: b"done".to_vec(),
                })
            })),
        );
        let worker = on_worker.recv_timeout(Duration::from_secs(5));
        // Always unblock the worker before assertions, including failed checks.
        release.send(()).unwrap();
        assert_ne!(renderer, worker.unwrap());
        executor.run(
            RequestOut {
                ticket: 8,
                target: "missing".into(),
                request: Request::continuation(12),
                forced: false,
            },
            None,
        );
        let first = executor
            .outcomes
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(first.0, 7);
        assert!(matches!(
            first.1,
            Outcome::Response(Response { status: 200, .. })
        ));
        let missing = executor
            .outcomes
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(missing.0, 8);
        assert!(matches!(
            missing.1,
            Outcome::Failed {
                kind: FailureKind::Unsupported,
                ..
            }
        ));
        // An HTTP request still follows grant/transport handling, even if
        // handed a stray continuation closure.
        assert!(matches!(
            run(
                None,
                Request::get("https://example.com"),
                false,
                Some(Box::new(|| panic!(
                    "HTTP must not execute continuation work"
                )))
            ),
            Outcome::Failed {
                kind: FailureKind::Refused,
                ..
            }
        ));
    }
}

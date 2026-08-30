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
}

/// One worker thread, its two queues, and the wake the loop polls.
pub struct Executor {
    jobs: Sender<Job>,
    outcomes: Receiver<(u64, Outcome)>,
    wake: UnixStream,
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
        std::thread::Builder::new()
            .name("exact-executor".into())
            .spawn(move || {
                let host = ibex2::host::Host::new();
                let bindings = ibex2::grant::GrantSet::parse(&grants)
                    .ok()
                    .map(|g| host.endow(g));
                for job in job_rx {
                    let outcome = run(bindings.as_ref(), job.request, job.forced);
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
        }
    }

    /// The wake's reading end: readable when a reply is queued.
    pub fn fd(&self) -> RawFd {
        self.wake.as_raw_fd()
    }

    /// Hand a request to the worker.
    pub fn run(&self, r: RequestOut) {
        let _ = self.jobs.send(Job {
            ticket: r.ticket,
            request: r.request,
            forced: r.forced,
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

fn run(bindings: Option<&ibex2::host::Bindings>, request: Request, forced: bool) -> Outcome {
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

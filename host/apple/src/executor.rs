//! The executor (LLP 1016 D2, Apple): `ibex2::host` on a worker thread.
//!
//! The runner never does I/O; a request it hands out (`Runner::take_requests`)
//! goes to this thread, which holds the app's `Bindings` — endowed at boot
//! from the grants the data crate declares (D6; ibex LLP 0067/0068) over one
//! `ibex2::host::Host`, the platform transport (`NSURLSession` here) and its
//! secret store (`crate::store`, LLP 1018 D6). Each outcome
//! is queued for the main thread and the host's wake callback is called
//! from here, carrying nothing; the presenter hops to its main thread and
//! calls `exact_pump`, which delivers every queued outcome to the runner as
//! one batch. Requests run one at a time, in order; parallelism is later.

use exact_runner::{FailureKind, Outcome, Request, RequestOut, Response};
use std::ffi::c_void;
use std::sync::mpsc::{channel, Receiver, Sender};

/// The host's wake callback: "an outcome is queued; pump on your thread".
/// Called on the executor's thread.
pub type WakeFn = extern "C" fn(ctx: *mut c_void);

struct Job {
    ticket: u64,
    request: Request,
    forced: bool,
    work: Option<Box<dyn FnOnce() -> Outcome + Send>>,
}

/// One worker thread and its two queues.
pub struct Executor {
    jobs: Sender<Job>,
    outcomes: Receiver<(u64, Outcome)>,
}

impl Executor {
    /// Start the worker with the app's bindings (`None`: it declares no
    /// grants that parse, and every request is refused) and the host's wake.
    pub fn start(
        bindings: Option<ibex2::host::Bindings>,
        grants: &str,
        wake: Option<(WakeFn, *mut c_void)>,
    ) -> Executor {
        let grants = grants.to_string();
        let (jobs, job_rx) = channel::<Job>();
        let (outcome_tx, outcomes) = channel();
        // The context pointer crosses to the worker as an integer: it is the
        // presenter's, opaque here, and handed back untouched.
        let wake = wake.map(|(f, ctx)| (f, ctx as usize));
        std::thread::Builder::new()
            .name("exact-executor".into())
            .spawn(move || {
                for job in job_rx {
                    let scoped = job.request.grants.as_deref().map(|scope| {
                        exact_data::storage::scope(&grants, Some(scope))
                            .and_then(|s| {
                                ibex2::grant::GrantSet::parse(s).map_err(|e| e.to_string())
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
                    if let Some((f, ctx)) = wake {
                        f(ctx as *mut c_void);
                    }
                }
            })
            .expect("the executor thread");
        Executor { jobs, outcomes }
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

    /// Every outcome queued since the last drain, oldest first.
    pub fn drain(&self) -> Vec<(u64, Outcome)> {
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
        let executor = Executor::start(None, "", None);
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

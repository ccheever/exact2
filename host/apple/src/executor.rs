//! The executor (LLP 1016 D2, Apple): `ibex2::host` on a worker thread.
//!
//! The runner never does I/O; a request it hands out (`Runner::take_requests`)
//! goes to this thread, which holds the app's `Bindings` — endowed at boot
//! from the grants the data crate declares (D6; ibex LLP 0067/0068) over one
//! `ibex2::host::Host`, the platform transport (`NSURLSession` here) and its
//! secret store (`crate::store`, LLP 1018 D6). Each outcome
//! is queued for the runtime owner and the host's wake callback is called
//! from here, carrying nothing; the Apple glue hops through main to capture
//! its clock, then calls `exact_pump` on the dedicated runtime thread. That
//! delivers every queued outcome to the runner as one batch. Requests run
//! one at a time, in order; parallelism is later.

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
        wake: Option<(WakeFn, *mut c_void)>,
    ) -> Executor {
        let (jobs, job_rx) = channel::<Job>();
        let (outcome_tx, outcomes) = channel();
        // The context pointer crosses to the worker as an integer: it is the
        // presenter's, opaque here, and handed back untouched.
        let wake = wake.map(|(f, ctx)| (f, ctx as usize));
        std::thread::Builder::new()
            .name("exact-executor".into())
            .spawn(move || {
                for job in job_rx {
                    let outcome = run(bindings.as_ref(), job.request, job.forced);
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
    pub fn run(&self, r: RequestOut) {
        let _ = self.jobs.send(Job {
            ticket: r.ticket,
            request: r.request,
            forced: r.forced,
        });
    }

    /// Every outcome queued since the last drain, oldest first.
    pub fn drain(&self) -> Vec<(u64, Outcome)> {
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

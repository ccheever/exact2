//! What an outcome keeps until the UI takes it, and how a fetch's failure
//! is worded: shared by the workers and the asynchronous core.
use super::failed;
use exact_runner::{FailureKind, Outcome};
use ibex2::stdlib::abort::AbortController;

pub(super) fn bounded_outcome(outcome: Outcome, limit: usize) -> Outcome {
    if retained(&outcome) > limit {
        failed(
            FailureKind::Refused,
            "native outcome exceeds retention limit",
        )
    } else {
        outcome
    }
}

/// The bytes an outcome keeps until the UI takes it.
pub(super) fn retained(outcome: &Outcome) -> usize {
    match outcome {
        Outcome::Response(r) => r
            .body
            .capacity()
            .saturating_add(
                r.headers
                    .capacity()
                    .saturating_mul(std::mem::size_of::<(String, String)>()),
            )
            .saturating_add(
                r.headers
                    .iter()
                    .map(|(k, v)| k.capacity().saturating_add(v.capacity()))
                    .sum::<usize>(),
            ),
        Outcome::Storage(b) => b.capacity(),
        Outcome::Message(m) => m
            .data
            .capacity()
            .saturating_add(m.event.capacity())
            .saturating_add(m.id.capacity())
            .saturating_add(m.binary.as_ref().map_or(0, Vec::capacity)),
        Outcome::Closed { reason, .. } => reason.capacity(),
        Outcome::Surface(exact_runner::SurfaceOutcome::Captured(b)) => b.capacity(),
        Outcome::Surface(exact_runner::SurfaceOutcome::Restored) => 0,
        Outcome::Failed { message, .. } => message.capacity(),
    }
}

pub(super) fn fetch_failure(e: ibex2::boundary::HostError, abort: &AbortController) -> Outcome {
    match e {
        _ if abort.signal().aborted() => failed(FailureKind::Aborted, "native request aborted"),
        ibex2::boundary::HostError::Denied { capability } => failed(
            FailureKind::Refused,
            format!("outside the app's grants ({capability})"),
        ),
        // The origin a redirect led to, which the grants lack (podcast F5).
        ibex2::boundary::HostError::DeniedRedirect { capability, origin } => failed(
            FailureKind::Refused,
            format!("outside the app's grants ({capability}): redirected to {origin}"),
        ),
        e => failed(
            FailureKind::Network,
            e.to_string().chars().take(2048).collect::<String>(),
        ),
    }
}

/// A count a measurement build sets (LLP 1120 §5's arms: `EXACT_HTTP_WORKERS`,
/// `EXACT_HTTP_IN_FLIGHT`), or `default`.
pub(super) const fn build_number(set: Option<&str>, default: usize) -> usize {
    let Some(text) = set else {
        return default;
    };
    let bytes = text.as_bytes();
    let (mut n, mut i): (usize, usize) = (0, 0);
    while i < bytes.len() {
        assert!(bytes[i].is_ascii_digit(), "a build count is digits");
        n = match n.checked_mul(10) {
            Some(tens) => match tens.checked_add((bytes[i] - b'0') as usize) {
                Some(next) => next,
                None => panic!("a build count past usize"),
            },
            None => panic!("a build count past usize"),
        };
        i += 1;
    }
    assert!(n > 0, "a build count is at least 1");
    n
}

/// A request's deadline: a thread that aborts the request when it passes,
/// and ends at once when the request finishes first (the sender drops).
pub(super) struct Deadline {
    passed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _done: std::sync::mpsc::Sender<()>,
}

impl Deadline {
    pub(super) fn arm(ms: u32, abort: &AbortController) -> Result<Deadline, String> {
        let (done, wait) = std::sync::mpsc::channel::<()>();
        let passed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (flag, abort) = (passed.clone(), abort.clone());
        std::thread::Builder::new()
            .name("exact-fetch-deadline".into())
            .spawn(move || {
                // A request the caller already aborted (a newer answer, an
                // unload) stays Aborted: the deadline did not end it.
                if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) =
                    wait.recv_timeout(std::time::Duration::from_millis(ms.into()))
                {
                    if !abort.signal().aborted() {
                        flag.store(true, std::sync::atomic::Ordering::Release);
                        abort.abort();
                    }
                }
            })
            .map_err(|e| format!("the request's deadline could not be armed: {e}"))?;
        Ok(Deadline {
            passed,
            _done: done,
        })
    }

    pub(super) fn passed(&self) -> bool {
        self.passed.load(std::sync::atomic::Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_build_count_is_its_digits_or_the_default() {
        assert_eq!(super::build_number(None, 6), 6);
        assert_eq!(super::build_number(Some("7"), 6), 7);
        assert_eq!(super::build_number(Some("32"), 6), 32);
        for bad in ["", "0", "7a", "18446744073709551623"] {
            assert!(
                std::panic::catch_unwind(|| super::build_number(Some(bad), 6)).is_err(),
                "{bad}"
            );
        }
    }
}

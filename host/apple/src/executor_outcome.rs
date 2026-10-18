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

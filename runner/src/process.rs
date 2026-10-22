//! Desktop processes (LLP 1133.007): the request an `@exact/child-process`
//! call becomes, the operations staged on a running child, the origin each
//! carries, and the clock a host's process work keeps under the agent.
//!
//! A process request is a [`Request`] to [`PROCESS_URL`] whose body is the
//! call as JSON, as a long native call is one to `exact-native:`: every
//! seam that carries a request (the module ABI, a worker's turn, the door)
//! carries it unchanged. `spawn` is a stream whose resource owns the child
//! (`socket`), and its stdin writes, ends and kills ride a socket's staged
//! path as [`crate::SocketOp::Process`].

use crate::request::{HttpScheduling, Redirect, Request};

/// The URL of a process request: not HTTP.
pub const PROCESS_URL: &str = "exact-process:";

/// The header a runner stamps on a process request made by a turn that is
/// not a person's (D13); absent, the turn was a person's or the app's own
/// foreground code.
pub const ORIGIN_HEADER: &str = "exact-origin";

/// Where a turn came from (LLP 1133.002 D12): what a host checks before a
/// child is started, written to, ended or signalled (LLP 1133.007 D13, Q8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TurnOrigin {
    /// A person's input, or the app's own code in a foreground session.
    #[default]
    Person,
    /// A tool invocation (LLP 1111).
    Tool,
    /// A background wake (LLP 1114.001).
    Wake,
    /// A headless session.
    Headless,
}

impl TurnOrigin {
    /// Its name in a request, the journal and `state`.
    pub fn name(self) -> &'static str {
        match self {
            TurnOrigin::Person => "person",
            TurnOrigin::Tool => "tool",
            TurnOrigin::Wake => "wake",
            TurnOrigin::Headless => "headless",
        }
    }

    /// The origin `name` spells.
    pub fn parse(name: &str) -> Option<TurnOrigin> {
        [
            TurnOrigin::Person,
            TurnOrigin::Tool,
            TurnOrigin::Wake,
            TurnOrigin::Headless,
        ]
        .into_iter()
        .find(|o| o.name() == name)
    }

    /// Whether a turn of this origin may start a program or write to, end
    /// or signal a running one: only a person's (Q8). A tool, a wake or a
    /// headless session fails `person-required`.
    pub fn directs_children(self) -> bool {
        self == TurnOrigin::Person
    }
}

/// One operation on a running child (D2), staged with the commit that made
/// it and sent only if that commit stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessOp {
    /// Bytes for its stdin.
    Write(Vec<u8>),
    /// Close its stdin.
    End,
    /// One signal to its process group, by name (`SIGTERM`).
    Kill(String),
}

impl ProcessOp {
    /// The bytes it writes.
    pub fn size(&self) -> usize {
        match self {
            ProcessOp::Write(bytes) => bytes.len(),
            ProcessOp::End | ProcessOp::Kill(_) => 0,
        }
    }

    /// Its name in the journal.
    pub fn name(&self) -> &'static str {
        match self {
            ProcessOp::Write(_) => "write",
            ProcessOp::End => "end",
            ProcessOp::Kill(_) => "kill",
        }
    }
}

/// The clock a host's process work keeps under the agent (LLP 1133.007 §7):
/// restart delays, readiness intervals and timeouts, the drain and the kill
/// escalation, and `execFile`'s timeout move when the agent's clock moves,
/// not on the wall. A host gives the runner one only under the agent
/// ([`crate::Runner::set_work_clock`]).
pub trait WorkClock: Send + Sync {
    /// The agent's clock reads `now_ms`: run the work due by then, and
    /// return once it has, its outcomes queued for the runner.
    fn advance(&self, now_ms: f64);
    /// When the next work on this clock is due.
    fn due(&self) -> Option<f64>;
    /// Whether work this clock ran has outcomes the runner has not taken.
    fn owed(&self) -> bool;
}

impl Request {
    /// A process request with this JSON body (`@exact/child-process`'s
    /// call, LLP 1133.007 D12): a `spawn` is a stream whose resource owns
    /// the child; anything else answers once, within `max_response_bytes`.
    pub fn process(body: Vec<u8>, stream: bool, max_response_bytes: u32) -> Request {
        Request {
            http: HttpScheduling::Independent { max_response_bytes },
            continuation: None,
            storage: None,
            surface: None,
            grants: None,
            method: "POST".into(),
            url: PROCESS_URL.into(),
            headers: Vec::new(),
            body,
            stream,
            socket: stream,
            bytes: false,
            timeout_ms: None,
            body_from: None,
            redirect: Redirect::Follow,
        }
    }

    /// A process request, for the host's process work, not the network.
    pub fn is_process(&self) -> bool {
        self.url == PROCESS_URL
            && self.continuation.is_none()
            && self.storage.is_none()
            && self.surface.is_none()
    }

    /// The origin a runner stamped on this process request.
    pub fn process_origin(&self) -> TurnOrigin {
        (self.headers.iter())
            .find(|(name, _)| name == ORIGIN_HEADER)
            .and_then(|(_, value)| TurnOrigin::parse(value))
            .unwrap_or_default()
    }
}

/// What a process request asks, for the journal and `state.streams`: its
/// kind (`spawn`, `execFile`, …) and its program as its grant line names
/// it. Never its arguments or environment, which may carry tokens.
pub fn summary(body: &[u8]) -> (String, Option<String>) {
    let json: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    let kind = json["kind"].as_str().unwrap_or("process").to_string();
    let program = json["program"].as_str().map(str::to_string);
    (kind, program)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_process_request_is_its_own_kind_and_carries_its_origin() {
        let mut r = Request::process(
            br#"{"kind":"spawn","program":"path:ssh"}"#.to_vec(),
            true,
            1 << 20,
        );
        assert!(r.is_process() && r.stream && r.socket);
        assert!(!r.is_native() && !r.is_auth());
        assert_eq!(r.process_origin(), TurnOrigin::Person);
        r.headers.push((ORIGIN_HEADER.into(), "tool".into()));
        assert_eq!(r.process_origin(), TurnOrigin::Tool);
        assert!(!r.process_origin().directs_children());
        assert_eq!(summary(&r.body), ("spawn".into(), Some("path:ssh".into())));
        let once = Request::process(br#"{"kind":"execPath"}"#.to_vec(), false, 4096);
        assert!(!once.stream && !once.socket);
        assert_eq!(summary(&once.body).1, None);
    }
}

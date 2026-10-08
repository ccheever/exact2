//! The reply as it is written. A transcript gains an answer only once the
//! agent has finished it; a session behind Fleet's queue (Codex's app
//! server, Claude's gateway) keeps the running turn's text as it streams.
//! While such a session works and is open, the phone attaches to it and
//! polls that text (`poll`, every STREAM_MS), and shows it after the
//! transcript until the transcript has the answer.

use super::*;

/// How often the running turn's text is read.
const STREAM_MS: f64 = 700.0;

/// What the phone streams from.
#[derive(Clone, Debug, Default)]
pub struct Stream {
    /// The session attached to, and its thread.
    attached: Option<(Key, String)>,
    /// The running turn's text so far, for the open session.
    pub text: String,
    /// What the last request was: attach, poll or detach.
    step: &'static str,
}

impl Model {
    /// The open session, when it streams and is working: its key, thread
    /// and queue route.
    fn streamable(&self) -> Option<(Key, String, &'static str)> {
        let key = self.open.clone()?;
        let s = self.live_session(&key)?;
        if s.send_route() != SendRoute::Queue || !s.working() {
            return None;
        }
        Some((key, s.native_id.clone(), s.queue_leaf()))
    }

    /// Whether a stream request is due: one while the open session works,
    /// and a detach once it doesn't (or another is open).
    pub(super) fn stream_due(&self, now: f64) -> bool {
        if self.stream.inflight || now < self.stream.next_at {
            return false;
        }
        let want = self.streamable().map(|(k, _, _)| k);
        let attached = self.streaming.attached.as_ref().map(|(k, _)| k.clone());
        want.is_some() || attached.is_some()
    }

    /// The next stream request: URL, bearer, body.
    pub fn stream_request(&mut self) -> Option<(String, String, String)> {
        let conn = self.conn.as_ref()?;
        let want = self.streamable();
        let attached = self.streaming.attached.clone();
        let (key, thread, leaf, operation) = match (attached, want) {
            // Streaming this one: read on.
            (Some((attached, _)), Some((key, thread, leaf))) if attached == key => {
                (key, thread, leaf, "poll")
            }
            // Another session is open, or this one stopped working: let go.
            (Some((attached, thread)), _) => {
                let leaf = self
                    .live_session(&attached)
                    .map(|s| s.queue_leaf())
                    .unwrap_or("codex-client");
                (attached, thread, leaf, "detach")
            }
            (None, Some((key, thread, leaf))) => (key, thread, leaf, "attach"),
            (None, None) => return None,
        };
        let body = serde_json::json!({
            "operation": operation,
            "client_id": self.client_id(),
            "thread_id": thread,
        });
        let url = conn.session_url(&self.route_of(&self.via), &key.0, &key.1, leaf);
        let bearer = conn.bearer();
        if operation == "attach" {
            self.streaming.attached = Some((key, thread));
        }
        self.streaming.step = operation;
        self.stream.start(self.now);
        Some((url, bearer, body.to_string()))
    }

    /// A stream request answered with the thread's turns (or failed).
    pub fn stream_done(&mut self, result: Result<serde_json::Value, String>) {
        self.stream.inflight = false;
        self.stream.next_at = self.now + STREAM_MS;
        let step = std::mem::take(&mut self.streaming.step);
        if step == "detach" {
            self.streaming.attached = None;
            self.set_stream_text(String::new());
            return;
        }
        let Ok(reply) = result else {
            // Not attached after all (Fleet restarted, a peer answered):
            // attach again next time.
            self.streaming.attached = None;
            return;
        };
        // The newest turn still running, and what it has written.
        let text = reply
            .get("turns")
            .and_then(|t| t.as_array())
            .and_then(|turns| {
                turns
                    .iter()
                    .rev()
                    .find(|t| t.get("complete") != Some(&true.into()))
            })
            .and_then(|t| t.get("message").and_then(|m| m.as_str()))
            .unwrap_or("")
            .to_string();
        self.set_stream_text(text);
    }

    fn set_stream_text(&mut self, text: String) {
        if self.streaming.text != text {
            self.streaming.text = text;
            self.version += 1;
        }
    }

    /// The text being written in the open session, unless the transcript
    /// already ends with it.
    pub fn streaming_text(&self) -> Option<&str> {
        let text = self.streaming.text.trim();
        let key = self.open.as_ref()?;
        if text.is_empty() || self.streaming.attached.as_ref().map(|(k, _)| k) != Some(key) {
            return None;
        }
        let shown = self
            .entries(key)
            .iter()
            .rev()
            .find(|e| e.kind == "assistant")
            .is_some_and(|e| e.text.trim() == text);
        (!shown).then_some(text)
    }
}

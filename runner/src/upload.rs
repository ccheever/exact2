//! An upload's progress (LLP 1016.000 D10): a stream's request whose host
//! delivers the body's bytes sent as they go, then the reply, read whole,
//! as the one answer that ends it. Its terms, and the message that carries
//! the bytes sent.

use crate::request::{Message, Redirect, Request};

/// The event of an upload's progress message: its data is
/// `{"loaded":…,"total":…}`, the body's bytes sent so far and its length.
/// Only an upload sends it.
pub const UPLOAD_PROGRESS: &str = "exact:progress";

impl Request {
    /// An upload whose bytes sent are delivered as it goes (the field of
    /// this name), on a stream's request: `fetch(url, {exactStream,
    /// exactUploadProgress: true})`.
    pub fn upload_progress(mut self) -> Self {
        self.upload_progress = true;
        self
    }

    /// Why this request's upload progress is refused, if it is: not a
    /// stream, a WebSocket, a long poll, a `GET` or `HEAD` (which send no
    /// body), or a redirect other than followed, which a browser's
    /// `XMLHttpRequest` cannot do.
    pub fn upload_progress_refusal(&self) -> Option<&'static str> {
        let scheme = |s: &str| {
            self.url
                .get(..s.len())
                .is_some_and(|u| u.eq_ignore_ascii_case(s))
        };
        let method = |m: &str| self.method.eq_ignore_ascii_case(m);
        if !self.upload_progress {
            None
        } else if !self.stream || self.listen.is_some() {
            Some("exactUploadProgress reports an upload's progress to its stream: give exactStream too")
        } else if scheme("ws:") || scheme("wss:") {
            Some("exactUploadProgress: a WebSocket sends no body")
        } else if self.long_poll.is_some() {
            Some("exactUploadProgress: a long poll sends no body")
        } else if method("GET") || method("HEAD") {
            Some("exactUploadProgress: a GET or HEAD request sends no body")
        } else if self.redirect != Redirect::Follow {
            Some("exactUploadProgress: an upload follows its redirects")
        } else {
            None
        }
    }
}

impl Message {
    /// An upload's progress: `loaded` of the body's `total` bytes sent.
    pub fn upload_progress(loaded: u64, total: u64) -> Message {
        Message {
            event: UPLOAD_PROGRESS.into(),
            data: format!("{{\"loaded\":{loaded},\"total\":{total}}}"),
            ..Message::default()
        }
    }

    /// The bytes sent and the body's length, when this is an upload's
    /// progress ([`Message::upload_progress`]).
    pub fn sent(&self) -> Option<(u64, u64)> {
        let field = |key: &str| -> Option<u64> {
            let at = self.data.find(&format!("\"{key}\":"))? + key.len() + 3;
            let digits = self.data[at..]
                .split(|c: char| !c.is_ascii_digit())
                .next()?;
            digits.parse().ok()
        };
        (self.event == UPLOAD_PROGRESS).then(|| Some((field("loaded")?, field("total")?)))?
    }
}

#[cfg(test)]
mod tests {
    use crate::{Answer, Message, Redirect, Request};

    fn upload(url: &str) -> Request {
        let mut request = Request::get(url).upload_progress();
        request.method = "POST".into();
        match Answer::stream(request) {
            Answer::Later(request) => request,
            Answer::Now(_) => unreachable!("a stream is a request"),
        }
    }

    #[test]
    fn an_upload_is_a_stream_with_a_body_that_follows_its_redirects() {
        assert_eq!(
            upload("https://example.test/up").upload_progress_refusal(),
            None
        );
        // Not asked, nothing to refuse.
        assert_eq!(
            Request::get("https://example.test/").upload_progress_refusal(),
            None
        );
        let plain = Request::get("https://example.test/").upload_progress();
        assert!(plain
            .upload_progress_refusal()
            .unwrap()
            .contains("give exactStream too"));
        let mut get = upload("https://example.test/up");
        get.method = "get".into();
        assert!(get
            .upload_progress_refusal()
            .unwrap()
            .contains("GET or HEAD"));
        let socket = upload("wss://example.test/up");
        assert!(socket
            .upload_progress_refusal()
            .unwrap()
            .contains("WebSocket"));
        let poll = upload("https://example.test/up").long_poll("since", "seq");
        assert!(poll
            .upload_progress_refusal()
            .unwrap()
            .contains("long poll"));
        let mut manual = upload("https://example.test/up");
        manual.redirect = Redirect::Manual;
        assert!(manual
            .upload_progress_refusal()
            .unwrap()
            .contains("follows its redirects"));
    }

    #[test]
    fn an_upload_may_take_a_deadline_where_another_stream_may_not() {
        let mut timed = upload("https://example.test/up");
        timed.timeout_ms = Some(300_000);
        assert_eq!(timed.timeout_refusal(), None);
        timed.upload_progress = false;
        assert_eq!(timed.timeout_refusal(), Some("a stream has no timeout"));
    }

    #[test]
    fn a_progress_message_carries_the_bytes_sent_and_the_length() {
        let m = Message::upload_progress(65_536, 4_194_304);
        assert_eq!(m.event, "exact:progress");
        assert_eq!(m.data, r#"{"loaded":65536,"total":4194304}"#);
        assert_eq!(m.sent(), Some((65_536, 4_194_304)));
        // A server's event of another name is no upload's progress.
        let other = Message {
            event: "progress".into(),
            ..m.clone()
        };
        assert_eq!(other.sent(), None);
    }
}
